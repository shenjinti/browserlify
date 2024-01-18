use crate::{init_log, tests::serve_test_server};

#[tokio::test]
async fn test_render_pdf() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9003";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let target = "https://browserlify.com/?from=unittest";
    let url = format!("http://{}/pdf?url={}", addr, urlencoding::encode(target));

    let resp = reqwest::get(&url).await.expect("get api/pdf fail");
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "application/pdf");
    server.abort();
}

#[tokio::test]
async fn test_render_screenshot() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9003";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let target = "https://browserlify.com/?from=unittest";
    let url = format!(
        "http://{}/screenshot?url={}",
        addr,
        urlencoding::encode(target)
    );

    let resp = reqwest::get(&url).await.expect("get api/screenshot fail");
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "image/png");
    server.abort();
}
