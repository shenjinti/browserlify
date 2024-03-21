use crate::{devices::Device, StateRef};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use chromiumoxide::{Browser, Handler};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    io,
    time::{Duration, SystemTime},
};
use tokio::{fs::read_to_string, sync::oneshot};

#[derive(Debug, Clone)]
pub(crate) struct SessionOption {
    pub uuid: Option<String>,
    pub landscape: bool,
    pub cleanup: bool,
    pub enable_cache: bool,
    pub userdatadir_expire: Option<u64>,
}

impl Default for SessionOption {
    fn default() -> Self {
        Self {
            uuid: None,
            landscape: false,
            cleanup: true,
            enable_cache: false,
            userdatadir_expire: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SessionType {
    #[cfg(feature = "headless")]
    Headless,
    #[cfg(feature = "remote")]
    RemoteChrome,
}
#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) r#type: SessionType,
    pub(crate) data_dir: String,
    pub(crate) device: Option<Device>,
    // cleanup data_dir when session closed
    pub(crate) cleanup: bool,
    pub(crate) enable_cache: bool,
    pub(crate) created_at: SystemTime,
    pub(crate) updated_at: RefCell<SystemTime>,
    pub(crate) endpoint: String, // ws:// or vnc://
    pub(crate) shutdown_tx: RefCell<Option<oneshot::Sender<()>>>,
    pub(crate) browser: RefCell<Option<Browser>>,
    pub(crate) headless_handler: RefCell<Option<Handler>>,
    #[cfg(feature = "remote")]
    pub(crate) remote_handler: Option<crate::remote::RemoteHandler>,
}

impl Session {
    pub fn touch_updatedat(&self) {
        *self.updated_at.borrow_mut() = SystemTime::now();
    }
}

#[derive(Deserialize)]
pub struct CreateSessionParams {
    #[serde(rename = "device")]
    pub(crate) emulating_device: Option<String>,
}

impl From<&CreateSessionParams> for SessionOption {
    fn from(_params: &CreateSessionParams) -> Self {
        Self::default()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        #[cfg(feature = "remote")]
        self.remote_handler.take();

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

pub(crate) async fn list_session(State(state): State<StateRef>) -> Json<Value> {
    let sessions = state.sessions.lock().unwrap();
    let data = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "type": format!("{:?}", s.r#type),
                "data_dir": s.data_dir,
                "device": s.device,
                "cleanup": s.cleanup,
                "enable_cache": s.enable_cache,
                "created_at": s.created_at,
                "updated_at": s.updated_at,
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

#[derive(Deserialize)]
pub(crate) struct ScreenSessionParams {
    percentage: Option<i32>,
}

/// Take a screenshot of the current browser (headless or remote)
pub(crate) async fn screen_session(
    Path(session_id): Path<String>,
    Query(_params): Query<ScreenSessionParams>,
    State(state): State<StateRef>,
) -> Result<Response, crate::Error> {
    let session_type = state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == session_id)
        .ok_or_else(|| crate::Error::new(StatusCode::BAD_GATEWAY, "session not found"))?
        .r#type
        .clone();

    match session_type {
        #[cfg(feature = "headless")]
        SessionType::Headless => crate::headless::screen_headless_screen(session_id, state).await,
        #[cfg(feature = "remote")]
        SessionType::RemoteChrome => {
            let percentage = _params.percentage.unwrap_or(50);
            crate::remote::screen_remote_screen(percentage, session_id, state).await
        }
    }
}

pub(crate) async fn handle_index_page() -> Response {
    match read_to_string("dist/index.html").await {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html")
            .body(body.into())
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(e.to_string().into())
            .unwrap(),
    }
}

pub(crate) async fn background_cleanup_task(state: StateRef) -> Result<(), io::Error> {
    log::info!("background_cleanup_task started");
    loop {
        state
            .sessions
            .lock()
            .unwrap()
            .retain(|s| match s.created_at.elapsed() {
                Ok(elapsed) => {
                    if elapsed.as_secs() > state.max_timeout {
                        log::info!("session id: {} has timed out, elapsed: {:?}", s.id, elapsed);
                        false
                    } else {
                        true
                    }
                }
                Err(e) => {
                    log::error!("elapsed error: {:?}", e);
                    true
                }
            });
        // cleanup expired userdatadir
        clean_expired_dir(std::path::Path::new(&state.data_root)).await?;
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn clean_expired_dir(dir: &std::path::Path) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    let mut dir = tokio::fs::read_dir(dir).await?;
    let now = chrono::Utc::now().timestamp();

    while let Some(res) = dir.next_entry().await? {
        let dir_meta = res.metadata().await?;
        if !dir_meta.is_dir() {
            continue;
        }
        let path = res.path();
        let expire_file = path.join(".expire");
        if !expire_file.exists() || !expire_file.is_file() {
            continue;
        }

        let expire_time = tokio::fs::read_to_string(expire_file).await?;
        let expire_time = expire_time.trim();
        // format rfc3389 2024-03-19T07:30:14.970638011+00:00
        chrono::DateTime::parse_from_rfc3339(&expire_time)
            .map(|expire_time| {
                if now > expire_time.timestamp() {
                    log::info!("clean_expired_dir: {:?} at {:?}", path, expire_time);
                    std::fs::remove_dir_all(&path).unwrap_or_else(|e| {
                        log::error!("remove_dir_all error: {:?} path: {}", e, path.display());
                    });
                }
            })
            .unwrap_or_else(|e| {
                log::error!(
                    "parse_from_rfc3339 error: {:?} expire_time: {}",
                    e,
                    expire_time,
                );
            });
    }
    Ok(())
}
