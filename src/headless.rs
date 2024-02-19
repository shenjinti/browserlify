use std::{cell::RefCell, time::SystemTime};

use axum::{
    body::Body,
    extract::{ws, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chromiumoxide::{
    browser::BrowserConfigBuilder, cdp::browser_protocol::page::CaptureScreenshotFormat,
    page::ScreenshotParams, Browser,
};
use futures::{SinkExt, StreamExt};
use tokio::{select, sync::oneshot};
use tokio_tungstenite::tungstenite;

use crate::{
    devices::{get_device, Device},
    session::{
        handle_index_page, CreateSessionParams, Session, SessionGuard, SessionOption, SessionType,
    },
    Error, StateRef,
};

fn from_ts_message(msg: tungstenite::Message) -> Option<ws::Message> {
    match msg {
        tungstenite::Message::Text(text) => Some(ws::Message::Text(text)),
        tungstenite::Message::Binary(data) => Some(ws::Message::Binary(data)),
        tungstenite::Message::Ping(data) => Some(ws::Message::Ping(data)),
        tungstenite::Message::Pong(data) => Some(ws::Message::Pong(data)),
        tungstenite::Message::Close(Some(close)) => {
            Some(ws::Message::Close(Some(ws::CloseFrame {
                code: close.code.into(),
                reason: close.reason,
            })))
        }
        tungstenite::Message::Close(None) => Some(ws::Message::Close(None)),
        tungstenite::Message::Frame(_) => None,
    }
}

fn to_ts_message(msg: ws::Message) -> tungstenite::Message {
    match msg {
        ws::Message::Text(text) => tungstenite::Message::Text(text),
        ws::Message::Binary(data) => tungstenite::Message::Binary(data),
        ws::Message::Ping(data) => tungstenite::Message::Ping(data),
        ws::Message::Pong(data) => tungstenite::Message::Pong(data),
        ws::Message::Close(Some(close)) => {
            tungstenite::Message::Close(Some(tungstenite::protocol::CloseFrame {
                code: close.code.into(),
                reason: close.reason,
            }))
        }
        ws::Message::Close(None) => tungstenite::Message::Close(None),
    }
}
pub(crate) async fn create_headless_browser_session(
    opt: SessionOption,
    device: Option<Device>,
    state: StateRef,
    shutdown_tx: Option<oneshot::Sender<()>>,
) -> Result<Session, Error> {
    if state.is_full() {
        return Err(Error::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "too many sessions",
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let data_dir = format!("{}/{}", state.data_root.trim_end_matches("/"), id);

    let config = BrowserConfigBuilder::default()
        .disable_default_args()
        .args(vec![
            "--enable-features=NetworkService,NetworkServiceInProcess",
            "--disable-background-timer-throttling",
            "--disable-backgrounding-occluded-windows",
            "--disable-breakpad",
            "--disable-client-side-phishing-detection",
            "--disable-component-extensions-with-background-pages",
            "--disable-default-apps",
            "--disable-dev-shm-usage",
            "--disable-extensions",
            "--disable-features=TranslateUI",
            "--disable-hang-monitor",
            "--disable-ipc-flooding-protection",
            "--disable-popup-blocking",
            "--disable-prompt-on-repost",
            "--disable-renderer-backgrounding",
            "--disable-sync",
            "--force-color-profile=srgb",
            "--metrics-recording-only",
            "--no-first-run",
            "--enable-automation",
            "--password-store=basic",
            "--use-mock-keychain",
            "--enable-blink-features=IdleDetection",
            "--lang=en_US",
        ])
        .user_data_dir(&data_dir);

    let config = match opt.enable_cache {
        true => config,
        false => config.disable_cache(),
    };

    let config = match device.as_ref() {
        Some(d) => {
            let viewport = d.get_viewport(opt.landscape);
            config.viewport(viewport)
        }
        None => config,
    }
    .build()
    .map_err(|e| Error::new(StatusCode::SERVICE_UNAVAILABLE, &e))?;

    let (browser, handler) = Browser::launch(config).await?;

    let ws_url = browser.websocket_address().to_string();
    log::info!("create session, id: {} dir: {} -> {}", id, data_dir, ws_url);

    Ok(Session {
        id,
        r#type: SessionType::Headless,
        data_dir,
        endpoint: ws_url,
        device,
        cleanup: opt.cleanup,
        enable_cache: opt.enable_cache,
        shutdown_tx: RefCell::new(shutdown_tx),
        browser: RefCell::new(Some(browser)),
        headless_handler: RefCell::new(Some(handler)),
        created_at: SystemTime::now(),
        updated_at: RefCell::new(SystemTime::now()),
        #[cfg(feature = "remote")]
        remote_handler: None,
    })
}

/// Workflow
/// 1. render index.html when GET / and request not websocket
/// 2. create an chrome session when websocket request    
///     2.1 bridge websocket to chrome session
pub(crate) async fn create_headless_session(
    ws: Option<WebSocketUpgrade>,
    Query(params): Query<CreateSessionParams>,
    State(state): State<StateRef>,
) -> Response {
    let ws = match ws {
        Some(ws) => ws,
        None => return handle_index_page().await,
    };

    match handle_headless_session(ws, Query(params), State(state)).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("handle_session error: {}", e);
            e.into_response()
        }
    }
}

pub(crate) async fn handle_headless_session(
    ws: WebSocketUpgrade,
    Query(params): Query<CreateSessionParams>,
    State(state): State<StateRef>,
) -> Result<Response, Error> {
    let opt = SessionOption::default();
    let device = get_device(&params.emulating_device.clone().unwrap_or_default());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session =
        create_headless_browser_session(opt, device, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "browser is None")?;
    let mut handler = session
        .headless_handler
        .take()
        .ok_or_else(|| "handler is None")?;

    let (upstream, _) = tokio_tungstenite::connect_async(&session.endpoint)
        .await
        .map_err(|e| Error::new(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    let (mut server_ws_tx, mut server_ws_rx) = upstream.split();

    let r = ws.on_upgrade(|client_stream| async move {
        let id = session.id.clone();
        let _guard = SessionGuard::new(state.clone(), session);

        let (mut client_ws_tx, mut client_ws_rx) = client_stream.split();

        let server_to_client = async {
            while let Some(Ok(msg)) = server_ws_rx.next().await {
                if let Some(msg) = from_ts_message(msg) {
                    if let Err(e) = client_ws_tx.send(msg).await {
                        log::error!("client_ws_tx.send id: {} error: {}", id, e);
                        break;
                    }
                    state
                        .sessions
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .find(|s| s.id == id)
                        .map(|s| s.touch_updatedat());
                }
            }
        };

        let client_to_server = async {
            while let Some(Ok(msg)) = client_ws_rx.next().await {
                if let Err(e) = server_ws_tx.send(to_ts_message(msg)).await {
                    log::error!("server_ws_tx.send id: {} error: {}", id, e);
                    break;
                }
                state
                    .sessions
                    .lock()
                    .unwrap()
                    .iter_mut()
                    .find(|s| s.id == id)
                    .map(|s| s.touch_updatedat());
            }
        };

        select! {
            _ = server_to_client => {}
            _ = client_to_server => {}
            _ = async {
                while let Some(_) = handler.next().await {}
            } => { }
            _  = shutdown_rx => {
                log::info!("shutdown_rx shutdown id: {}", id);
            }
        }
        browser.kill().await;
    });
    Ok(r)
}

pub(crate) async fn screen_headless_screen(
    session_id: String,
    state: StateRef,
) -> Result<Response, crate::Error> {
    let endpoint_url = state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == session_id)
        .ok_or_else(|| crate::Error::new(StatusCode::BAD_GATEWAY, "session not found"))?
        .endpoint
        .clone();

    let (mut browser, mut handler) = Browser::connect(endpoint_url).await?;
    let handler_job = tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    let params = ScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .build();

    let screenshot = browser
        .pages()
        .await
        .map(|pages| pages[0].clone())?
        .screenshot(params)
        .await?;

    let mut response = Response::new(Body::from(screenshot));
    response
        .headers_mut()
        .insert("Content-Type", "image/png".parse().unwrap());

    browser.close().await.ok();
    browser.wait().await.ok();
    handler_job.abort();

    Ok(response.into_response())
}
