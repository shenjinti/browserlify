use crate::StateRef;
use axum::{extract::State, response::Response};

pub(crate) async fn firefox_remote(State(state): State<StateRef>) -> Response {
    todo!()
}
