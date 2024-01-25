use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chromiumoxide::error::CdpError;
use std::fmt;

#[derive(Debug)]
pub struct Error {
    status_code: StatusCode,
    message: String,
}

impl Error {
    pub fn new(status_code: StatusCode, message: &str) -> Self {
        Self {
            status_code,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.status_code, self.message)
    }
}

impl std::error::Error for Error {}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let builder = axum::http::Response::builder().status(self.status_code.as_u16());
        if self.message.is_empty() {
            builder.body(
                self.status_code
                    .canonical_reason()
                    .unwrap_or_default()
                    .into(),
            )
        } else {
            builder.body(self.message.into())
        }
        .unwrap()
    }
}

impl From<CdpError> for Error {
    fn from(err: CdpError) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string())
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err)
    }
}
