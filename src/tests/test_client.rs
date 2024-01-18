use crate::{init_log, tests::serve_test_server};
use chromiumoxide::Browser;
use futures::StreamExt;

#[tokio::test]
async fn test_connect() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9002";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let url = format!("ws://{}/?from=unittest", addr);
    match tokio_tungstenite::connect_async(url).await {
        Ok((_, _)) => {}
        Err(e) => {
            panic!("connect to {} failed: {}", addr, e);
        }
    }
    server.abort();
}
#[tokio::test]
async fn test_dump_content() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9003";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let url = format!("ws://{}/?from=unittest", addr);

    let (mut browser, mut handler) = Browser::connect(url)
        .await
        .expect("connect to server failed");

    tokio::spawn(async move { while let Some(_) = handler.next().await {} });

    let page = browser
        .new_page("https://browserlify.com/?from=unittest")
        .await
        .unwrap();
    let content = page.content().await.unwrap();
    assert!(content.contains("HANGZHOU"));

    browser.close().await.unwrap();
    browser.wait().await.unwrap();
    server.abort();
}
