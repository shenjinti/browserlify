use crate::{init_log, tests::serve_test_server};

#[tokio::test]
async fn test_render_pdf() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9004";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let target = "https://browserlify.com/?from=unittest";
    let url = format!(
        "http://{}/pdf?url={}&author=browserlify_test",
        addr,
        urlencoding::encode(target)
    );

    let resp = reqwest::get(&url).await.expect("get api/pdf fail");
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "application/pdf");
    server.abort();
    let content = resp.bytes().await.expect("get pdf content fail");
    let doc = lopdf::Document::load_mem(&content).expect("load pdf fail");
    // get author from pdf
    let info = doc
        .trailer
        .get(b"Info")
        .and_then(|obj| doc.get_object(obj.as_reference().unwrap()))
        .expect("get info fail");
    match info {
        lopdf::Object::Dictionary(dict) => {
            let author = dict.get(b"Author").expect("get author fail");
            match author {
                lopdf::Object::String(author, _) => {
                    assert_eq!(author, b"browserlify_test");
                }
                _ => {
                    panic!("author is not string");
                }
            }
        }
        _ => {
            panic!("info is not dictionary");
        }
    }
}

#[tokio::test]
async fn test_render_screenshot() {
    assert_eq!(0.max(1), 1);
    assert_eq!(2.max(1), 2);
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9005";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let target = "https://browserlify.com/?from=unittest";
    let url = format!(
        "http://{}/screenshot?url={}&wait_images=true",
        addr,
        urlencoding::encode(target)
    );

    let resp = reqwest::get(&url).await.expect("get api/screenshot fail");
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "image/png");
    server.abort();
}

#[tokio::test]
async fn test_render_text() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9006";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let target = "https://browserlify.com/?from=unittest";
    let url = format!("http://{}/text?url={}", addr, urlencoding::encode(target));

    let resp = reqwest::get(&url).await.expect("get api/screenshot fail");
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "plain/text");
    let body = resp.text().await.expect("get text fail");
    assert!(!body.contains("<body>"));
    server.abort();
}

#[tokio::test]
async fn test_render_html() {
    init_log("info".to_string(), false);
    let addr = "127.0.0.1:9007";
    let server = tokio::spawn(serve_test_server(addr.to_string()));
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let target = "https://browserlify.com/?from=unittest";
    let url = format!("http://{}/html?url={}", addr, urlencoding::encode(target));

    let resp = reqwest::get(&url).await.expect("get api/screenshot fail");
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "text/html");
    let body = resp.text().await.expect("get text fail");
    assert!(body.contains("<!DOCTYPE html>"));
    server.abort();
}
