use self::x11vnc::{create_x11_session, X11SessionOption};
use crate::{
    session::{kill_session, SessionGuard},
    StateRef,
};
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    select,
    sync::oneshot,
};

#[cfg(test)]
mod tests;
mod x11vnc;
const REMOTE_SUFFIX: &str = ".remote.json";

#[derive(Debug)]
pub struct RemoteHandler {
    pub(super) child_x11vnc: Option<tokio::process::Child>,
    pub(super) child_xvfb: Option<tokio::process::Child>,
    pub(super) shutdown_tx: Option<oneshot::Sender<()>>,
}

impl Drop for RemoteHandler {
    fn drop(&mut self) {
        self.child_x11vnc.take();
        self.child_xvfb.take();
        self.shutdown_tx.take();
    }
}

#[derive(Deserialize)]
pub struct CreateRemoteParams {
    pub id: Option<String>,
    pub name: Option<String>,
    pub homepage: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct RemoteInfo {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub screen: Option<String>,
    pub binary: Option<String>,
    pub homepage: Option<String>,
}

pub(crate) async fn list_remote(
    State(state): State<StateRef>,
) -> Result<Json<Value>, crate::Error> {
    let root = state.data_root.clone();
    let mut remotes = Vec::new();
    let mut entries = fs::read_dir(root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let remote_file = path.join(REMOTE_SUFFIX);
            match fs::read_to_string(&remote_file).await {
                Ok(data) => {
                    let remote_info: RemoteInfo = serde_json::from_str(&data)?;
                    remotes.push(remote_info);
                }
                Err(e) => {
                    log::error!(
                        "read remote file error remote: {:?} error: {}",
                        remote_file,
                        e
                    );
                    continue;
                }
            };
        }
    }
    Ok(Json(json!(remotes)))
}

pub(crate) async fn create_remote(
    State(state): State<StateRef>,
    Json(params): Json<CreateRemoteParams>,
) -> Result<Json<Value>, crate::Error> {
    let id = params.id.unwrap_or(uuid::Uuid::new_v4().to_string());
    let data_root = std::path::Path::new(&state.data_root);
    let remote_dir = data_root.join(&id);

    if remote_dir.exists() {
        return Err(crate::Error::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "remote dir exists",
        ));
    }

    fs::create_dir_all(&remote_dir).await?;

    let remote_file = remote_dir.join(REMOTE_SUFFIX);
    let remote_info = RemoteInfo {
        id: id.clone(),
        name: params.name,
        created_at: chrono::Local::now().to_rfc3339(),
        screen: None,
        binary: None,
        homepage: params.homepage,
    };
    let data = serde_json::to_string_pretty(&remote_info)?;
    fs::write(&remote_file, data).await?;
    Ok(Json(json!(remote_info)))
}

pub(crate) async fn connect_remote(
    ws: WebSocketUpgrade,
    Path(remote_id): Path<String>,
    State(state): State<StateRef>,
) -> Result<Response, crate::Error> {
    let data_root = std::path::Path::new(&state.data_root);
    let remote_dir = data_root.join(&remote_id);
    if !remote_dir.exists() {
        return Err(crate::Error::new(
            StatusCode::BAD_GATEWAY,
            "remote not exists",
        ));
    }

    let endpoint = match state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == remote_id)
    {
        Some(s) => s.endpoint.clone(),
        None => {
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "session is shutdown",
            ));
        }
    };
    // parse endponit
    let addr = url::Url::parse(&endpoint)?
        .socket_addrs(|| None)?
        .first()
        .ok_or_else(|| crate::Error::new(StatusCode::BAD_GATEWAY, "target invalid"))?
        .clone();

    // connect to remote
    let mut target = tokio::net::TcpStream::connect(addr).await?;
    let r = ws.on_upgrade(|client_stream| async move {
        let (mut client_ws_tx, mut client_ws_rx) = client_stream.split();
        let (mut server_rx, mut server_tx) = target.split();
        let id = remote_id.clone();

        let server_to_client = async {
            let mut buf = [0u8; 8192];
            while let Ok(n) = server_rx.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let data = &buf[..n];
                if let Err(e) = client_ws_tx.send(data.into()).await {
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
        };

        let client_to_server = async {
            while let Some(Ok(msg)) = client_ws_rx.next().await {
                let buf = msg.into_data();
                if let Err(e) = server_tx.write(&buf).await {
                    log::error!("server_tx.write id: {} error: {}", id, e);
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
            _ = server_to_client => {
                log::info!("remote server_to_client id: {} exit", remote_id);
            }
            _ = client_to_server => {
                log::info!("remote client_to_server id: {} exit", remote_id);
            }
        }
        log::info!(
            "connect_remote id: {} exit endponit: {}",
            remote_id,
            endpoint
        );
    });
    Ok(r)
}

pub(crate) async fn stop_remote(
    Path(remote_id): Path<String>,
    State(state): State<StateRef>,
) -> Result<Response, crate::Error> {
    let data_root = std::path::Path::new(&state.data_root);
    let remote_dir = data_root.join(&remote_id);
    if !remote_dir.exists() {
        return Err(crate::Error::new(
            StatusCode::BAD_GATEWAY,
            "remote not exists",
        ));
    }
    // stop session first
    kill_session(Path(remote_id.clone()), State(state.clone())).await;
    Ok(Response::new("ok".into()))
}

pub(crate) async fn start_remote(
    Path(remote_id): Path<String>,
    State(state): State<StateRef>,
) -> Result<Response, crate::Error> {
    let data_root = std::path::Path::new(&state.data_root);
    let remote_dir = data_root.join(&remote_id);
    if !remote_dir.exists() {
        return Err(crate::Error::new(
            StatusCode::BAD_GATEWAY,
            "remote not exists",
        ));
    }

    match state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == remote_id)
    {
        Some(_) => {
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "session is shutdown",
            ))
        }
        None => {}
    };

    // load remote info
    let remote_file = remote_dir.join(REMOTE_SUFFIX);
    let data = match fs::read_to_string(&remote_file).await {
        Ok(data) => data,
        Err(e) => {
            log::error!(
                "read remote file error remote: {:?} error: {}",
                remote_file,
                e
            );
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "remote file error",
            ));
        }
    };

    let remote_info: RemoteInfo = serde_json::from_str(&data)?;
    let opt = X11SessionOption {
        id: remote_id.clone(),
        name: remote_info.name,
        data_dir: remote_dir.to_str().unwrap().to_string(),
        screen: remote_info.screen,
        binary: remote_info.binary,
        lc_ctype: std::env::var("LC_CTYPE").ok(),
        timezone: std::env::var("TZ").ok(),
        homepage: remote_info.homepage,
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let session = create_x11_session(opt, shutdown_tx).await?;

    tokio::spawn(async move {
        let guard = SessionGuard::new(state, session);
        select! {
            _ = shutdown_rx => {}
        }
        drop(guard);
        log::info!("remote sesson id: {} exit", remote_id);
    });
    Ok(Response::new("ok".into()))
}

pub(crate) async fn remove_remote(
    Path(remote_id): Path<String>,
    State(state): State<StateRef>,
) -> Result<Response, crate::Error> {
    let data_root = std::path::Path::new(&state.data_root);
    let remote_dir = data_root.join(&remote_id);
    if !remote_dir.exists() {
        return Err(crate::Error::new(
            StatusCode::BAD_GATEWAY,
            "remote not exists",
        ));
    }
    // stop session first
    kill_session(Path(remote_id.clone()), State(state.clone())).await;
    fs::remove_dir_all(&remote_dir).await?;
    Ok(Response::new("ok".into()))
}
