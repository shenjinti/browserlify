use tokio::{process::Command, sync::oneshot};

use crate::{session::Session, StateRef};

pub struct X11SessionOption {
    pub id: String,
    pub data_dir: String,
    pub screen: Option<String>,
    pub binary: Option<String>,
    pub lc_ctype: Option<String>,
    pub timezone: Option<String>,
}

pub(super) async fn create_x11_session(
    option: X11SessionOption,
    state: StateRef,
) -> Result<Session, crate::Error> {
    which::which("x11vnc")
        .map_err(|e| crate::Error::new(axum::http::StatusCode::BAD_GATEWAY, &e.to_string()))?;
    which::which("xvfb-run")
        .map_err(|e| crate::Error::new(axum::http::StatusCode::BAD_GATEWAY, &e.to_string()))?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    /*
    xvfb-run -s "-screen 0 1280x1024x24+32" -e /dev/stdout -f .Xauthority -n 1 chromium &
    sleep 1
    x11vnc -noxdamage  -display :1 -nopw -auth .Xauthority
        */
    let data_dir = std::path::Path::new(&option.data_dir);
    let auth_file = data_dir.join(".Xauthority");
    let browser_bin = option.binary.unwrap_or("chromium".to_string());
    let remoterc = data_dir.join(".remoterc");

    let timezone = option.timezone.unwrap_or("America/New_York".to_string());
    let lc_ctype = option.lc_ctype.unwrap_or("en_US.UTF-8".to_string());

    let remoterc_data = format!(
        r#"#!/bin/bash
export LC_CTYPE="{lc_ctype}"
export TZ="{timezone}"
{browser_bin} &"#
    );

    let run_args = vec![
        "-s",
        option
            .screen
            .as_ref()
            .unwrap_or(&"1280x1024x24+32".to_string()),
        "-e",
        "/dev/stdout",
        "-f",
        auth_file.to_str().unwrap(),
        "-n",
        "1",
        &remoterc.to_str().unwrap(),
    ];
    todo!()
}
