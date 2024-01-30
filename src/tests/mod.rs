use crate::{create_router, AppState};
use axum::Router;
use std::sync::Arc;
use tokio::{spawn, sync::oneshot};
mod test_browser;
mod test_client;
mod test_content;
use tower_http::services::ServeDir;

#[allow(unused)]
pub(crate) fn open_port() -> String {
    // random port
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for port in 1..10 {
        let port = rng.gen_range(30000..30100);
        let addr = format!("127.0.0.1:{}", port);
        if let Ok(_) = std::net::TcpListener::bind(addr.clone()) {
            return addr;
        }
    }
    panic!("no port available");
}

async fn serve_test_server(shutdown_rx: oneshot::Receiver<()>, addr: String) {
    let state =
        Arc::new(AppState::new("/tmp/browserlify_unittest".to_string(), 0).allow_private_ip());
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .unwrap();
    });
}

async fn serve_test_http_server(
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<String, crate::Error> {
    let addr = open_port();
    let listener = tokio::net::TcpListener::bind(&addr.clone()).await.unwrap();

    spawn(async move {
        let serve_dir = ServeDir::new("test_assets");
        let app = Router::new().route_service("/", serve_dir);
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .unwrap();
    });
    Ok(addr.to_string())
}
