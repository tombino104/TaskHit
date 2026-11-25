use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(&'static str),
    Unauthorized,
    Forbidden(&'static str),
    NotFound,
    Conflict(&'static str),
    TooManyRequests,
    Unavailable,
    /// Dettaglio solo nei log: al client va un messaggio generico.
    Internal(String),
}

impl ApiError {
    pub fn internal(e: impl std::fmt::Display) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self { ApiError::Internal(format!("database: {e}")) }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self { ApiError::Internal(format!("{e:#}")) }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg): (StatusCode, &str) = match &self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "credenziali non valide o sessione scaduta"),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "non trovato"),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "troppe richieste, riprova tra poco"),
            ApiError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "server occupato, riprova tra poco"),
            ApiError::Internal(d) => {
                tracing::error!("errore interno: {d}");
                (StatusCode::INTERNAL_SERVER_ERROR, "errore interno")
            }
        };
        let mut res = (status, Json(json!({ "error": msg }))).into_response();
        if matches!(self, ApiError::TooManyRequests | ApiError::Unavailable) {
            res.headers_mut().insert(RETRY_AFTER, HeaderValue::from_static("30"));
        }
        res
    }
}
