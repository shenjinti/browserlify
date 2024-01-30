use crate::StateRef;
use axum::{extract::State, response::Response};

pub(crate) async fn chrome_remote(State(state): State<StateRef>) -> Response {
    todo!()
}
