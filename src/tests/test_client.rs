use crate::{
    init_log,
    tests::{serve_test_http_server, serve_test_server},
};
use chromiumoxide::Browser;
use futures::StreamExt;

#[tokio::test]
async fn test_connect() {
    init_log("info".to_string(), false, None);
    let addr = "127.0.0.1:9002";
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    serve_test_server(shutdown_rx, addr.to_string()).await;
    let url = format!("ws://{}/?from=unittest", addr);
    match tokio_tungstenite::connect_async(url).await {
        Ok((_, _)) => {}
        Err(e) => {
            panic!("connect to {} failed: {}", addr, e);
        }
    }
    drop(shutdown_tx);
}
#[tokio::test]
async fn test_dump_content() {
    init_log("info".to_string(), false, None);
    let addr = "127.0.0.1:9003";
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    serve_test_server(shutdown_rx, addr.to_string()).await;
    let url = format!("ws://{}/?from=unittest", addr);

    let (mut browser, mut handler) = Browser::connect(url)
        .await
        .expect("connect to server failed");

    tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    let (http_shutdown_tx, http_shutdown_rx) = tokio::sync::oneshot::channel();
    let http_addr = serve_test_http_server(http_shutdown_rx)
        .await
        .expect("serve http fail");
    let target = format!("http://{http_addr}/?from=unittest");

    let page = browser.new_page(target).await.unwrap();
    let content = page.content().await.unwrap();
    assert!(content.contains("MADE WITH CARE IN HANGZHOU"));

    browser.close().await.unwrap();
    browser.wait().await.unwrap();
    drop(shutdown_tx);
    drop(http_shutdown_tx);
}
