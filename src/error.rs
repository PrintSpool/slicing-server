use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

// Make our own error that wraps `eyre::Error`.
pub struct AppError(eyre::Error);

pub type AppResult<T> = Result<T, AppError>;

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, eyre::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<eyre::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
