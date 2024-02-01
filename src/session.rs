use crate::{devices::Device, StateRef};
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chromiumoxide::{
    cdp::browser_protocol::page::CaptureScreenshotFormat, page::ScreenshotParams, Browser, Handler,
};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{cell::RefCell, time::SystemTime};
use tokio::sync::oneshot;

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
pub(crate) enum SessionType {
    Headless,
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
    pub(crate) remote_handler_tx: Option<crate::remote::RemoteHandler>,
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

impl Drop for Session {
    fn drop(&mut self) {
        #[cfg(feature = "remote")]
        self.remote_handler_tx.take();

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

/// Take a screenshot of the current browser (headless or remote)
pub(crate) async fn screen_session(
    Path(session_id): Path<String>,
    State(state): State<StateRef>,
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
