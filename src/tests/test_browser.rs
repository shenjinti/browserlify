use chromiumoxide::{browser::BrowserConfigBuilder, Browser};
use futures::StreamExt;

use crate::tests::serve_test_http_server;

#[tokio::test]
async fn test_launch() {
    let config = BrowserConfigBuilder::default()
        .build()
        .expect("config fail");

    let (_, _) = Browser::launch(config).await.expect("launch fail");
}

#[tokio::test]
async fn test_connect() {
    let config = BrowserConfigBuilder::default()
        .build()
        .expect("config fail");

    let (mut browser, mut handler) = Browser::launch(config).await.expect("launch fail");
    let ws_url = browser.websocket_address().to_string();
    tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    let (mut client, mut client_handler) = Browser::connect(ws_url).await.expect("connect fail");
    tokio::spawn(async move { while let Some(_) = client_handler.next().await {} });

    let (http_shutdown_tx, http_shutdown_rx) = tokio::sync::oneshot::channel();
    let http_addr = serve_test_http_server(http_shutdown_rx)
        .await
        .expect("serve http fail");
    let target = format!("http://{http_addr}/?from=unittest");

    let page = client.new_page(target).await.expect("new page fail");

    let content = page.content().await.expect("content fail");
    assert!(content.contains("MADE WITH CARE IN HANGZHOU"));

    client.close().await.expect("close fail");
    browser.close().await.expect("close fail");
    drop(http_shutdown_tx);
}
