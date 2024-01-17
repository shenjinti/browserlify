use crate::{session, AppState};
use axum::{routing::get, Router};
use std::sync::Arc;
mod test_browser;
mod test_client;

async fn serve_test_server(addr: String) {
    let state = Arc::new(AppState::new("/tmp/browserlify_unittest".to_string(), 0));

    let app = Router::new()
        .route("/", get(session::create_session))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
