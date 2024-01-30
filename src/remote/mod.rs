use crate::StateRef;
use axum::{extract::State, response::Response};

pub(crate) async fn create_remote(State(state): State<StateRef>) -> Response {
    todo!()
}
pub(crate) async fn handle_remote(State(state): State<StateRef>) -> Response {
    todo!()
}
