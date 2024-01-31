use crate::session::Session;
use axum::http::StatusCode;
use lazy_static::lazy_static;
use std::{
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicI32, Ordering},
};
use tokio::{io::AsyncBufReadExt, process::Command, select, sync::oneshot};
pub struct X11SessionOption {
    pub id: String,
    pub name: Option<String>,
    pub data_dir: String,
    pub screen: Option<String>,
    pub binary: Option<String>,
    pub lc_ctype: Option<String>,
    pub timezone: Option<String>,
}

lazy_static! {
    static ref X11VNC_PORT: AtomicI32 = AtomicI32::new(5900);
}

fn build_remote_rc(option: &X11SessionOption, browser_bin: &str) -> String {
    let timezone = option
        .timezone
        .clone()
        .unwrap_or("America/New_York".to_string());
    let lc_ctype = option.lc_ctype.clone().unwrap_or("en_US.UTF-8".to_string());

    format!(
        r#"#!/bin/sh
echo "DISPLAY="$DISPLAY
export LC_CTYPE={lc_ctype}
export TZ={timezone}
while true
do
    {browser_bin}
    echo "browser exit", $?, "restart browser"
    sleep 1
done
"#
    )
}

async fn allow_vnc_port() -> Result<i32, crate::Error> {
    let port = X11VNC_PORT.load(Ordering::Relaxed);
    for idx in 0..10 {
        let next_port = port + idx;
        let addr = format!("127.0.0.1:{next_port}");
        if let Ok(_) = tokio::net::TcpListener::bind(addr.clone()).await {
            X11VNC_PORT.store(next_port + 1, Ordering::Relaxed);
            return Ok(next_port);
        }
    }
    Err(crate::Error::new(
        StatusCode::BAD_GATEWAY,
        "no port available",
    ))
}

pub(super) async fn create_x11_session(
    option: X11SessionOption,
    shutdown_tx: oneshot::Sender<()>,
) -> Result<Session, crate::Error> {
    let browser_bin = option.binary.clone().unwrap_or("chromium".to_string());
    let browser_bin_ref = browser_bin.clone();
    which::which("x11vnc")
        .map_err(|_| crate::Error::new(StatusCode::BAD_GATEWAY, "x11vnc is required"))?;
    which::which("xvfb-run")
        .map_err(|_| crate::Error::new(StatusCode::BAD_GATEWAY, "xvfb-run is required"))?;
    which::which(&browser_bin).map_err(|_| {
        crate::Error::new(
            StatusCode::BAD_GATEWAY,
            &format!("{} is required", browser_bin_ref),
        )
    })?;

    let data_dir = std::path::Path::new(&option.data_dir);
    let remoterc = data_dir.join(".remoterc");
    // create remoterc when not exists
    if !remoterc.exists() {
        let remoterc_data = build_remote_rc(&option, &browser_bin);
        log::info!("create remoterc {:?} -> {}", remoterc, remoterc_data);
        tokio::fs::write(&remoterc, remoterc_data).await?;
        std::fs::set_permissions(&remoterc, Permissions::from_mode(0o755))?;
    }

    let auth_file = data_dir.join(".Xauthority");
    // create subprocess xvfb-run
    let mut display_num = ":99".to_string();
    let mut xvfb_run = Command::new("xvfb-run")
        .kill_on_drop(true)
        .args(&[
            "-s",
            option
                .screen
                .as_ref()
                .unwrap_or(&"1280x1024x24+32".to_string()),
            "-e",
            "/dev/stdout",
            "-f",
            auth_file.to_str().unwrap(),
            "-a",
            &remoterc.to_str().unwrap(),
        ])
        .spawn()?;

    let xvfb_run_stdout = match xvfb_run.stdout.take() {
        Some(stdout) => tokio::io::BufReader::new(stdout),
        None => {
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "xvfb-run stdout is none",
            ))
        }
    };
    let mut lines = xvfb_run_stdout.lines();
    while let Some(line) = lines.next_line().await? {
        log::info!("xvfb-run {} > {line}", option.id);
        if line.starts_with("DISPLAY=") {
            display_num = line.trim_start_matches("DISPLAY=").to_string();
            break;
        }
    }

    let id_ref = option.id.clone();
    let xvfb_outout_capture = tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            log::info!("xvfb-run {id_ref} > {line}");
        }
    });

    let xvfb_run_pid = xvfb_run.id();
    log::info!(
        "xvfb-run id:{} pid: {:?} display: {display_num}",
        option.id,
        xvfb_run_pid,
    );

    // create x11vnc subprocess
    let x11vnc_port = allow_vnc_port().await?;
    let mut x11vnc = Command::new("x11vnc")
        .kill_on_drop(true)
        .args(&[
            "-noxdamage",
            "-display",
            &display_num,
            "-nopw",
            "-auth",
            auth_file.to_str().unwrap(),
            "-forever",
            "-listen",
            "localhost",
            "-rfbport",
            &x11vnc_port.to_string(),
            "-desktop",
            &format!(
                "{}",
                option
                    .name
                    .unwrap_or(format!("{}-browserlify.com", option.id.clone()))
            ),
        ])
        .spawn()?;

    let x11vnc_pid = x11vnc.id();
    let x11vnc_stdout = match x11vnc.stdout.take() {
        Some(stdout) => tokio::io::BufReader::new(stdout),
        None => {
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "x11vnc stdout is none",
            ))
        }
    };

    let id_ref = option.id.clone();
    let x11vnc_stdout_capture = tokio::spawn(async move {
        let mut lines = x11vnc_stdout.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            log::info!("x11vnc {id_ref} > {line}");
        }
    });

    log::info!(
        "x11vnc id:{} pid: {:?} port:{} display: {display_num}",
        option.id,
        x11vnc_pid,
        x11vnc_port,
    );

    let (remote_handler_tx, remote_handler_rx) = oneshot::channel::<()>();
    let id_ref = option.id.clone();

    tokio::spawn(async move {
        select! {
            _ = x11vnc.wait() => {
                log::info!("x11vnc exit id: {}", id_ref);
            }
            _ = xvfb_run.wait() => {
                log::info!("xvfb-run exit id: {}", id_ref);
            }
            _ = xvfb_outout_capture => {
                log::info!("xvfb_outout_capture shutdown id: {}", id_ref);
            }
            _ = x11vnc_stdout_capture => {
                log::info!("x11vnc_stdout_capture shutdown id: {}", id_ref);
            }
            _ = remote_handler_rx => {
                log::info!("remote_handler_rx shutdown id: {}", id_ref);
            }
        }
        xvfb_run.kill().await.ok();
        x11vnc.kill().await.ok();
        log::info!("remote sesson id: {} exit", id_ref);
    });

    let session = Session {
        id: option.id.clone(),
        r#type: crate::session::SessionType::RemoteChrome,
        data_dir: option.data_dir.clone(),
        device: None,
        cleanup: false,
        enable_cache: false,
        shutdown_tx: std::cell::RefCell::new(Some(shutdown_tx)),
        browser: std::cell::RefCell::new(None),
        headless_handler: std::cell::RefCell::new(None),
        created_at: std::time::SystemTime::now(),
        endpoint: format!("vnc://127.0.0.1:{}", x11vnc_port),
        #[cfg(feature = "remote")]
        remote_handler_tx: Some(remote_handler_tx),
    };
    Ok(session)
}
