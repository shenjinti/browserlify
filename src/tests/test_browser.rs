use chromiumoxide::{browser::BrowserConfigBuilder, Browser};
use futures::StreamExt;

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

    let page = client
        .new_page("https://browserlify.com/?from=unittest")
        .await
        .expect("new page fail");

    let content = page.content().await.expect("content fail");
    assert!(content.contains("HANGZHOU"));

    client.close().await.expect("close fail");
    browser.close().await.expect("close fail");
}
