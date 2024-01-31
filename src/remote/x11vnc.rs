use crate::remote::RemoteHandler;
use crate::session::Session;
use axum::http::StatusCode;
use core::time;
use lazy_static::lazy_static;
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    process::Command,
    select,
    sync::oneshot,
};
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

fn allow_xvfb_port() -> Result<String, crate::Error> {
    for idx in 100..1024 {
        let fname = format!("/tmp/.X{}-lock", idx);
        let lock_file = std::path::Path::new(&fname);
        if lock_file.exists() {
            continue;
        }
        return Ok(format!(":{idx}"));
    }
    Err(crate::Error::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "not available xvfb num",
    ))
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
    which::which("Xvfb")
        .map_err(|_| crate::Error::new(StatusCode::BAD_GATEWAY, "Xvfb is required"))?;
    which::which(&browser_bin).map_err(|_| {
        crate::Error::new(
            StatusCode::BAD_GATEWAY,
            &format!("{} is required", browser_bin_ref),
        )
    })?;

    let data_dir = std::path::Path::new(&option.data_dir);
    let display_num = allow_xvfb_port()?;
    let screen = option.screen.unwrap_or("1280x1024x24+32".to_string());

    let args = vec![&display_num, "-nolisten", "tcp", "-screen", "scrn", &screen];
    let mut xvfb = Command::new("Xvfb")
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .args(&args)
        .spawn()?;

    let xvfb_stdout = match xvfb.stdout.take() {
        Some(stdout) => tokio::io::BufReader::new(stdout),
        None => {
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "Xvfb stdout is none",
            ))
        }
    };

    let mut lines = xvfb_stdout.lines();
    let mut xvfb_outout_file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("xvfb.log"))
        .await?;

    let xvfb_outout_capture = tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            xvfb_outout_file.write_all(line.as_bytes()).await.ok();
            xvfb_outout_file.write_all(b"\n").await.ok();
        }
    });

    log::info!(
        "xvfb id:{} pid: {} display: {display_num} args: {:?} ",
        option.id,
        xvfb.id().unwrap_or_default(),
        args,
    );

    // create x11vnc subprocess
    let x11vnc_port = allow_vnc_port().await?;
    let x11vnc_port = x11vnc_port.to_string();
    let desktop_name = format!(
        "{}",
        option
            .name
            .unwrap_or(format!("{}-browserlify.com", option.id.clone()))
    );
    let args = vec![
        "-noxdamage",
        "-display",
        &display_num,
        "-nopw",
        "-forever",
        "-listen",
        "localhost",
        "-rfbport",
        &x11vnc_port,
        "-desktop",
        &desktop_name,
    ];

    let mut x11vnc = Command::new("x11vnc")
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .args(&args)
        .spawn()?;

    log::info!(
        "x11vnc id: {} pid: {} port:{x11vnc_port} display: {display_num} args: {:?}",
        option.id,
        x11vnc.id().unwrap_or_default(),
        args
    );

    let x11vnc_stdout = match x11vnc.stdout.take() {
        Some(stdout) => tokio::io::BufReader::new(stdout),
        None => {
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "x11vnc stdout is none",
            ))
        }
    };

    let mut x11vnc_outout_file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("x11vnc.log"))
        .await?;

    let x11vnc_stdout_capture = tokio::spawn(async move {
        let mut lines = x11vnc_stdout.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            x11vnc_outout_file.write_all(line.as_bytes()).await.ok();
            x11vnc_outout_file.write_all(b"\n").await.ok();
        }
    });

    let (remote_handler_tx, remote_handler_rx) = oneshot::channel::<()>();
    let id_ref = option.id.clone();
    let browser_child_shutdown_tx_ref = Arc::new(Mutex::new(None));

    let remote_handler = RemoteHandler {
        child_x11vnc: Some(x11vnc),
        child_xvfb: Some(xvfb),
        shutdown_tx: Some(remote_handler_tx),
        browser_child_shutdown_tx: browser_child_shutdown_tx_ref.clone(),
    };

    let user_data_dir = option.data_dir.clone();
    let lc_ctype = option.lc_ctype.clone();
    let timezone = option.timezone.clone();

    let serve_browser = async move {
        loop {
            let args = vec!["--user-data-dir", &user_data_dir];
            let mut cmd = Command::new(&browser_bin);
            cmd.kill_on_drop(true);
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            cmd.args(&args);

            lc_ctype.clone().map(|v| cmd.env("LC_CTYPE", v));
            timezone.clone().map(|v| cmd.env("TZ", v));

            match cmd.spawn() {
                Ok(mut child) => {
                    let (browser_child_shutdown_tx, browser_child_shutdown_rx) =
                        oneshot::channel::<()>();

                    browser_child_shutdown_tx_ref
                        .lock()
                        .unwrap()
                        .replace(browser_child_shutdown_tx);

                    select! {
                        _ = child.wait() =>{
                            log::info!("browser process exit, restart");
                        }
                        _ = browser_child_shutdown_rx => {
                            break;
                        }
                    }
                }
                Err(e) => {
                    log::info!("create browser child fail {e}");
                }
            };
            tokio::time::sleep(time::Duration::from_secs(1)).await;
        }
    };

    tokio::spawn(async move {
        select! {
            _ = serve_browser => {
                log::info!("serve_browser shutdown id: {}", id_ref);
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
        remote_handler_tx: Some(remote_handler),
    };
    Ok(session)
}
