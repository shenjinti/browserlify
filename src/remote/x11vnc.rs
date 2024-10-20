use crate::remote::RemoteHandler;
use crate::session::Session;
use axum::http::StatusCode;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::SystemTime;
use tokio::{process::Command, select, sync::oneshot};
pub struct X11SessionOption {
    pub id: String,
    pub name: Option<String>,
    pub data_dir: String,
    pub screen: Option<String>,
    pub binary: Option<String>,
    pub lc_ctype: Option<String>,
    pub timezone: Option<String>,
    pub homepage: Option<String>,
    pub http_proxy: Option<String>,
}

lazy_static! {
    static ref X11VNC_PORT: AtomicI32 = AtomicI32::new(5900);
}
const DEFAULT_HOMEPAGE: &str = "https://browserlify.com/?startup=docker";

fn allow_xvfb_port() -> Result<i32, crate::Error> {
    for idx in 100..1024 {
        let fname = format!("/tmp/.X{}-lock", idx);
        let lock_file = Path::new(&fname);
        if lock_file.exists() {
            continue;
        }
        return Ok(idx);
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
        if next_port >= 8000 {
            X11VNC_PORT.store(5900, Ordering::Relaxed);
            return Err(crate::Error::new(
                StatusCode::BAD_GATEWAY,
                "no port available",
            ));
        }
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

    let data_dir_str = option.data_dir.clone();
    let data_dir = Path::new(&data_dir_str);
    let display_num = allow_xvfb_port()?;
    let display_num_str = format!(":{display_num}");

    let screen = option.screen.unwrap_or("1200x900x24".to_string());

    let re = regex::Regex::new(r"(\d+)x(\d+)").unwrap();
    let dimensions = re.captures(&screen).and_then(|cap| {
        let width = cap.get(1)?.as_str().parse().ok()?;
        let height = cap.get(2)?.as_str().parse().ok()?;
        Some((width, height))
    });
    let (width, height) = dimensions.unwrap_or((1200, 900));

    let args = vec![
        &display_num_str,
        "-nolisten",
        "tcp",
        "-screen",
        "scrn",
        &screen,
    ];

    let xvfb = Command::new("Xvfb")
        .kill_on_drop(true)
        .args(&args)
        .spawn()?;

    log::info!(
        "xvfb id:{} pid: {} Xfvb {display_num_str} {}",
        option.id,
        xvfb.id().unwrap_or_default(),
        args.join(" "),
    );

    // create x11vnc subprocess
    let x11vnc_port = allow_vnc_port().await?;
    let x11vnc_port = x11vnc_port.to_string();
    let desktop_name = format!("{}", option.name.unwrap_or(option.id.clone()));
    let x11vnc_outout_file = data_dir
        .join("x11vnc.log")
        .to_str()
        .unwrap_or("/dev/stdout")
        .to_string();

    let args = vec![
        "-noxdamage",
        "-display",
        &display_num_str,
        "-nopw",
        "-forever",
        "-shared",
        "-o",
        &x11vnc_outout_file,
        "-listen",
        "localhost",
        "-rfbport",
        &x11vnc_port,
        "-desktop",
        &desktop_name,
    ];

    let x11vnc = Command::new("x11vnc")
        .kill_on_drop(true)
        .args(&args)
        .spawn()?;

    log::info!(
        "x11vnc id: {} pid: {} x11vnc {}",
        option.id,
        x11vnc.id().unwrap_or_default(),
        args.join(" ")
    );

    let (remote_handler_tx, remote_handler_rx) = oneshot::channel::<()>();
    let id_ref = option.id.clone();

    let remote_handler = RemoteHandler {
        display_num: Some(display_num),
        child_x11vnc: Some(x11vnc),
        child_xvfb: Some(xvfb),
        shutdown_tx: Some(remote_handler_tx),
    };

    let serve_browser = async move {
        let homepage = option
            .homepage
            .clone()
            .unwrap_or(DEFAULT_HOMEPAGE.to_string());

        let data_dir_path = Path::new(&option.data_dir);
        let output_file = data_dir_path.join("stdout.log");
        let user_data_dir = format!("--user-data-dir={}", option.data_dir);
        let window_size = format!("--window-size={width},{height}");

        loop {
            vec![
                data_dir_path.join("SingletonCookie"),
                data_dir_path.join("SingletonLock"),
                data_dir_path.join("SingletonSocket"),
            ]
            .into_iter()
            .for_each(|f| {
                std::fs::remove_file(&f).ok();
            });

            let args = vec![
                &user_data_dir,
                "--disable-breakpad",
                "--no-first-run",
                "--password-store=basic",
                "--disable-hang-monitor",
                "--disable-default-apps",
                "--disable-renderer-backgrounding",
                "--force-color-profile=srgb",
                "--no-default-browser-check",
                "--start-maximized",
                &window_size,
                &homepage,
            ];

            let mut cmd = Command::new(&browser_bin);
            cmd.kill_on_drop(true);
            cmd.env("DISPLAY", &display_num_str);
            cmd.args(&args);

            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&output_file)
                .map(|f| cmd.stderr(Stdio::from(f)))
                .ok();

            option.lc_ctype.as_ref().map(|v| cmd.env("LC_CTYPE", v));
            option.timezone.as_ref().map(|v| cmd.env("TZ", v));
            option.http_proxy.as_ref().map(|v| {
                cmd.env("http_proxy", v);
                cmd.env("https_proxy", v);
            });

            match cmd.spawn() {
                Ok(mut child) => {
                    log::info!("start browser {} {}", browser_bin, args.join(" "));
                    child.wait().await.ok();
                    log::info!("browser process exit, restart");
                }
                Err(_) => {}
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    };

    tokio::spawn(async move {
        select! {
            _ = serve_browser => {
                log::info!("serve_browser shutdown id: {}", id_ref);
            }
            _ = remote_handler_rx => {
                log::info!("remote_handler_rx shutdown id: {}", id_ref);
            }
        }
        log::info!("shutdown remote sesson id: {} exit", id_ref);
    });

    let session = Session {
        id: option.id.clone(),
        r#type: crate::session::SessionType::RemoteChrome,
        data_dir: data_dir_str,
        device: None,
        cleanup: false,
        enable_cache: false,
        shutdown_tx: RefCell::new(Some(shutdown_tx)),
        browser: RefCell::new(None),
        headless_handler: RefCell::new(None),
        created_at: SystemTime::now(),
        updated_at: RefCell::new(SystemTime::now()),
        endpoint: format!("vnc://127.0.0.1:{}", x11vnc_port),
        remote_handler: Some(remote_handler),
    };
    Ok(session)
}
