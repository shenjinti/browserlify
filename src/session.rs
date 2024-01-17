use crate::StateRef;
use axum::{
    body::Body,
    extract::{Path, State, WebSocketUpgrade},
    response::Response,
    Json,
};
use chromiumoxide::browser::BrowserConfigBuilder;
use chromiumoxide::Browser;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{sync::Mutex, time::SystemTime};
use tokio::select;
use tokio_tungstenite::WebSocketStream;

fn from_ts_message(
    msg: tokio_tungstenite::tungstenite::Message,
) -> Option<axum::extract::ws::Message> {
    match msg {
        tokio_tungstenite::tungstenite::Message::Text(s) => {
            Some(axum::extract::ws::Message::Text(s))
        }
        tokio_tungstenite::tungstenite::Message::Binary(b) => {
            Some(axum::extract::ws::Message::Binary(b))
        }
        tokio_tungstenite::tungstenite::Message::Ping(b) => {
            Some(axum::extract::ws::Message::Ping(b))
        }
        tokio_tungstenite::tungstenite::Message::Pong(b) => {
            Some(axum::extract::ws::Message::Pong(b))
        }
        tokio_tungstenite::tungstenite::Message::Close(Some(close)) => Some(
            axum::extract::ws::Message::Close(Some(axum::extract::ws::CloseFrame {
                code: close.code.into(),
                reason: close.reason,
            })),
        ),
        tokio_tungstenite::tungstenite::Message::Close(None) => {
            Some(axum::extract::ws::Message::Close(None))
        }
        tokio_tungstenite::tungstenite::Message::Frame(_) => None,
    }
}

fn to_ts_message(msg: axum::extract::ws::Message) -> tokio_tungstenite::tungstenite::Message {
    match msg {
        axum::extract::ws::Message::Text(s) => tokio_tungstenite::tungstenite::Message::Text(s),
        axum::extract::ws::Message::Binary(b) => tokio_tungstenite::tungstenite::Message::Binary(b),
        axum::extract::ws::Message::Ping(b) => tokio_tungstenite::tungstenite::Message::Ping(b),
        axum::extract::ws::Message::Pong(b) => tokio_tungstenite::tungstenite::Message::Pong(b),
        axum::extract::ws::Message::Close(Some(close)) => {
            tokio_tungstenite::tungstenite::Message::Close(Some(
                tokio_tungstenite::tungstenite::protocol::CloseFrame {
                    code: close.code.into(),
                    reason: close.reason,
                },
            ))
        }
        axum::extract::ws::Message::Close(None) => {
            tokio_tungstenite::tungstenite::Message::Close(None)
        }
    }
}

#[derive(Debug)]
pub(crate) struct Session {
    created_at: SystemTime,
    id: String,
    addr: String,
    data_dir: String,
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl Session {
    pub fn new(
        data_root: &str,
        ws_url: &str,
        shutdown_tx: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        let data_dir = format!("{}/{}", data_root, uuid::Uuid::new_v4());
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            data_dir,
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
            addr: ws_url.to_string(),
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
        Err(e) => Response::builder()
            .status(500)
            .body(Body::from(e.to_string()))
            .unwrap(),
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

    let addr = browser.websocket_address().to_string();

    let (mut server_ws_tx, mut server_ws_rx) = WebSocketStream::from_raw_socket(
        tokio::net::TcpStream::connect(&addr)
            .await
            .map_err(|e| axum::Error::new(e))?,
        tokio_tungstenite::tungstenite::protocol::Role::Client,
        None,
    )
    .await
    .split();

    let r = ws.on_upgrade(|client_stream| async move {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let session = Session::new(&state.data_root, &addr, shutdown_tx);
        state.sessions.lock().unwrap().push(session);

        let (mut client_ws_tx, mut client_ws_rx) = client_stream.split();

        let server_to_client = async {
            while let Some(Ok(msg)) = server_ws_rx.next().await {
                if let Some(msg) = from_ts_message(msg) {
                    match client_ws_tx.send(msg).await {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("client_ws_tx.send error: {}", e);
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
                        log::error!("server_ws_tx.send error: {}", e);
                        break;
                    }
                }
            }
        };

        select! {
            _ = server_to_client => {
                log::error!("server_to_client shutdown: {}", addr);
            }
            _ = client_to_server => {
                log::error!("client_to_server shutdown: {}", addr);
            }
            _ = async {
                while let Some(_) = handler.next().await {
                    continue;
                }
            } => {
                log::error!("browser handler shutdown {}", addr);
            }
            _  = shutdown_rx => {
                log::error!("shutdown_rx shutdown: {}", addr);
            }
        }
        state.sessions.lock().unwrap().retain(|s| s.addr != addr);
        browser.close().await.ok();
        browser.wait().await.ok();
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
                "addr": s.addr,
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
        .find(|s| s.addr == session_id)
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
