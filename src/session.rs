use crate::{
    devices::{get_device, Device},
    StateRef,
};
use axum::{
    body::Body,
    extract::{ws, Path, Query, State, WebSocketUpgrade},
    response::Response,
    Json,
};
use chromiumoxide::{
    browser::BrowserConfigBuilder,
    cdp::{browser_protocol::target::EventTargetCreated, de},
    handler::viewport::{self, Viewport},
};
use chromiumoxide::{Browser, Handler};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{cell::RefCell, time::SystemTime};
use tokio::{select, sync::oneshot};
use tokio_tungstenite::tungstenite;

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

#[derive(Debug, Clone)]
pub(crate) struct SessionOption {
    pub landscape: bool,
    pub cleanup: bool,
    pub enable_cache: bool,
}

impl Default for SessionOption {
    fn default() -> Self {
        Self {
            landscape: false,
            cleanup: true,
            enable_cache: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) id: String,
    data_dir: String,
    device: Option<Device>,
    // cleanup data_dir when session closed
    cleanup: bool,
    enable_cache: bool,
    created_at: SystemTime,
    ws_url: String,
    shutdown_tx: RefCell<Option<oneshot::Sender<()>>>,
    pub(crate) browser: RefCell<Option<Browser>>,
    pub(crate) handler: RefCell<Option<Handler>>,
}

#[derive(Deserialize)]
pub struct CreateSessionParams {
    #[serde(rename = "device")]
    emulating_device: Option<String>,
}

impl Drop for Session {
    fn drop(&mut self) {
        match self.cleanup {
            true => match std::fs::remove_dir_all(&self.data_dir) {
                Ok(_) => {
                    log::info!("remove_dir_all id: {} dir: {}", self.id, self.data_dir);
                }
                Err(e) => {
                    log::error!(
                        "remove_dir_all id: {} dir: {} error: {}",
                        self.id,
                        self.data_dir,
                        e
                    );
                }
            },
            false => {}
        }
        log::info!("session drop id: {}", self.id);
    }
}

pub(crate) struct SessionGuard {
    state: StateRef,
    id: String,
}

impl SessionGuard {
    pub(crate) fn new(state: StateRef, session: Session) -> Self {
        let id = session.id.clone();
        state.sessions.lock().unwrap().push(session);
        Self { state, id }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.state
            .sessions
            .lock()
            .unwrap()
            .retain(|s| s.id != self.id);
    }
}

pub(crate) async fn create_browser_session(
    opt: SessionOption,
    device: Option<Device>,
    state: StateRef,
    shutdown_tx: Option<oneshot::Sender<()>>,
) -> Result<Session, String> {
    if state.is_full() {
        return Err("too many sessions".into());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let data_dir = format!("{}/{}", state.data_root.trim_end_matches("/"), id);

    let config = BrowserConfigBuilder::default().user_data_dir(&data_dir);

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
    .map_err(|e| e.to_string())?;

    let (browser, handler) = Browser::launch(config).await.map_err(|e| e.to_string())?;

    let ws_url = browser.websocket_address().to_string();
    log::info!("create session, id: {} dir: {} -> {}", id, data_dir, ws_url);

    // let mut page_from_target = browser
    //     .event_listener::<EventTargetCreated>()
    //     .await
    //     .map_err(|e| e.to_string())?;

    // let id_ref = id.clone();
    // tokio::task::spawn(async move {
    //     while let Some(event) = page_from_target.next().await {
    //         if event.target_info.r#type == "page" {
    //             let target_id = event.target_info.target_id.clone();
    //             let page = match browser.get_page(target_id.clone()).await {
    //                 Ok(page) => page,
    //                 Err(e) => {
    //                     log::error!(
    //                         "get_page error, session:{} page: {} error: {}",
    //                         id_ref,
    //                         target_id.as_ref(),
    //                         e
    //                     );
    //                     continue;
    //                 }
    //             };
    //         }
    //     }
    // });

    Ok(Session {
        id,
        data_dir,
        ws_url,
        device,
        cleanup: opt.cleanup,
        enable_cache: opt.enable_cache,
        shutdown_tx: RefCell::new(shutdown_tx),
        browser: RefCell::new(Some(browser)),
        handler: RefCell::new(Some(handler)),
        created_at: SystemTime::now(),
    })
}

/// Workflow
/// 1. render index.html when GET / and request not websocket
/// 2. create an chrome session when websocket request    
///     2.1 bridge websocket to chrome session
pub(crate) async fn create_session(
    ws: Option<WebSocketUpgrade>,
    Query(params): Query<CreateSessionParams>,
    State(state): State<StateRef>,
) -> Response {
    let ws = match ws {
        Some(ws) => ws,
        None => {
            return Response::builder()
                .status(200)
                .body(Body::from("show the sessions"))
                .unwrap()
        }
    };

    match handle_session(ws, Query(params), State(state)).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("handle_session error: {}", e);
            Response::builder().status(500).body(Body::from(e)).unwrap()
        }
    }
}

pub(crate) async fn handle_session(
    ws: WebSocketUpgrade,
    Query(params): Query<CreateSessionParams>,
    State(state): State<StateRef>,
) -> Result<Response, String> {
    let opt = SessionOption::default();
    let device = get_device(&params.emulating_device.clone().unwrap_or_default());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let session = create_browser_session(opt, device, state.clone(), Some(shutdown_tx)).await?;
    let mut browser: Browser = session.browser.take().ok_or_else(|| "browser is None")?;
    let mut handler = session.handler.take().ok_or_else(|| "handler is None")?;

    let (upstream, _) = tokio_tungstenite::connect_async(&session.ws_url)
        .await
        .map_err(|e| e.to_string())?;

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
                }
            }
        };

        let client_to_server = async {
            while let Some(Ok(msg)) = client_ws_rx.next().await {
                if let Err(e) = server_ws_tx.send(to_ts_message(msg)).await {
                    log::error!("server_ws_tx.send id: {} error: {}", id, e);
                    break;
                }
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

pub(crate) async fn list_session(State(state): State<StateRef>) -> Json<Value> {
    let sessions = state.sessions.lock().unwrap();
    let data = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "data_dir": s.data_dir,
                "device": s.device,
                "cleanup": s.cleanup,
                "enable_cache": s.enable_cache,
                "created_at": s.created_at
            })
        })
        .collect::<Vec<_>>();
    Json(json!(data))
}

pub(crate) async fn kill_session(Path(session_id): Path<String>, State(state): State<StateRef>) {
    state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| s.shutdown_tx.take());
}

pub(crate) async fn killall_session(State(state): State<StateRef>) {
    state.sessions.lock().unwrap().iter().for_each(|s| {
        s.shutdown_tx.take();
    });
}
