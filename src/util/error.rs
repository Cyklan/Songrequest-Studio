use poem::{error::ResponseError, http::StatusCode};

#[derive(Debug, thiserror::Error)]
#[error("Internal Server Error")]
pub struct HttpError;

impl ResponseError for HttpError {
    fn status(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl From<sqlx::Error> for HttpError {
    fn from(_: sqlx::Error) -> Self {
        HttpError
    }
}

impl From<reqwest::Error> for HttpError {
    fn from(_: reqwest::Error) -> Self {
        HttpError
    }
}