use core::time;

use crate::remote::x11vnc::{create_x11_session, X11SessionOption};

#[tokio::test]
async fn test_x11_session() {
    crate::init_log("info".to_string(), false);
    tokio::fs::create_dir_all("/tmp/browserlify_unittest")
        .await
        .expect("create tmp dir fail");

    let option = X11SessionOption {
        id: "unittest".to_string(),
        name: None,
        data_dir: "/tmp/browserlify_unittest".to_string(),
        screen: None,
        binary: None,
        lc_ctype: None,
        timezone: None,
        homepage: None,
        http_proxy: None,
    };

    let (shutdown_tx, _) = tokio::sync::oneshot::channel();
    let session = create_x11_session(option, shutdown_tx)
        .await
        .expect("create x11vnc fail");

    drop(session);
    tokio::time::sleep(time::Duration::from_secs(1)).await;
}
