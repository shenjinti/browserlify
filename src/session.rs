use crate::StateRef;
use axum::{
    body::Body,
    extract::{ws, Path, State, WebSocketUpgrade},
    response::Response,
    Json,
};
use chromiumoxide::browser::BrowserConfigBuilder;
use chromiumoxide::Browser;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{sync::Mutex, time::SystemTime};
use tokio::select;
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

#[derive(Debug)]
pub(crate) struct Session {
    created_at: SystemTime,
    id: String,
    ws_url: String,
    data_dir: String,
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl Session {
    pub fn new(
        data_root: &str,
        ws_url: &str,
        shutdown_tx: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        let id = ws_url
            .split("/")
            .last()
            .unwrap_or(uuid::Uuid::new_v4().to_string().as_str())
            .to_string();
        let data_dir = format!("{}/{}", data_root.trim_end_matches("/"), id);
        Self {
            id,
            data_dir,
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
            ws_url: ws_url.to_string(),
            created_at: SystemTime::now(),
        }
    }
}

/// Workflow
/// 1. render index.html when GET / and request not websocket
/// 2. create an chrome session when websocket request    
///     2.1 bridge websocket to chrome session
pub(crate) async fn create_session(
    ws: Option<WebSocketUpgrade>,
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

    if state.is_full() {
        return Response::builder()
            .status(503)
            .body(Body::from("too many sessions"))
            .unwrap();
    }
    match handle_session(ws, State(state)).await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("create session error: {}", e);
            Response::builder()
                .status(500)
                .body(Body::from(e.to_string()))
                .unwrap()
        }
    }
}

pub(crate) async fn handle_session(
    ws: WebSocketUpgrade,
    State(state): State<StateRef>,
) -> std::result::Result<Response, axum::Error> {
    let config = BrowserConfigBuilder::default()
        .build()
        .map_err(|e| axum::Error::new(e))?;

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| axum::Error::new(e))?;

    let ws_url = browser.websocket_address().to_string();
    let (upstream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| axum::Error::new(e))?;

    let (mut server_ws_tx, mut server_ws_rx) = upstream.split();

    let r = ws.on_upgrade(|client_stream| async move {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let session = Session::new(&state.data_root, &ws_url, shutdown_tx);
        let id = session.id.clone();
        log::info!("new session, {}", ws_url);

        state.sessions.lock().unwrap().push(session);

        let (mut client_ws_tx, mut client_ws_rx) = client_stream.split();

        let server_to_client = async {
            while let Some(Ok(msg)) = server_ws_rx.next().await {
                if let Some(msg) = from_ts_message(msg) {
                    match client_ws_tx.send(msg).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("client_ws_tx.send id: {} error: {}", id, e);
                            break;
                        }
                    }
                }
            }
        };

        let client_to_server = async {
            while let Some(Ok(msg)) = client_ws_rx.next().await {
                match server_ws_tx.send(to_ts_message(msg)).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("server_ws_tx.send id: {} error: {}", id, e);
                        break;
                    }
                }
            }
        };

        select! {
            _ = server_to_client => {
            }
            _ = client_to_server => {
            }
            _ = async {
                while let Some(_) = handler.next().await {
                    continue;
                }
            } => {
            }
            _  = shutdown_rx => {
                log::warn!("shutdown_rx shutdown id: {}", id);
            }
        }
        state
            .sessions
            .lock()
            .unwrap()
            .retain(|s| s.ws_url != ws_url);
        browser.kill().await;
        log::warn!("session closed id: {}", id);
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
                "created_at": s.created_at
            })
        })
        .collect::<Vec<_>>();
    Json(json!(data))
}

pub(crate) async fn kill_session(
    Path(session_id): Path<String>,
    State(state): State<StateRef>,
) -> Response {
    state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.ws_url == session_id)
        .and_then(|s| s.shutdown_tx.lock().unwrap().take());

    Response::builder()
        .status(200)
        .body(Body::from("ok"))
        .unwrap()
}

pub(crate) async fn killall_session(State(state): State<StateRef>) {
    state.sessions.lock().unwrap().iter().for_each(|s| {
        s.shutdown_tx.lock().unwrap().take();
    });
}
