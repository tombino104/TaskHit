pub mod api;
pub mod auth;
pub mod backup;
pub mod common;
pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod meta;
pub mod moderation;
pub mod people;
pub mod photos;
pub mod proposals;
pub mod ratelimit;
pub mod secrets;
pub mod state;
pub mod themes;
pub mod languages;
pub mod web;
pub mod work;

use axum::{Router, extract::DefaultBodyLimit, http::StatusCode, middleware::from_fn_with_state, response::IntoResponse};
use config::Config;
use state::AppState;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use tower_http::{
    compression::CompressionLayer,
    limit::RequestBodyLimitLayer,
    services::ServeDir,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

/// Prepara cartelle, segreti, database (con migrazioni) e temi predefiniti.
pub fn build_state(cfg: Config) -> anyhow::Result<AppState> {
    std::fs::create_dir_all(&cfg.paths.data_dir)?;
    backup::clean_leftovers(&cfg.paths.data_dir);
    let secrets_dir = cfg.secrets_dir();
    let server_key = secrets::load_or_create(&secrets_dir, "server.key")?;

    #[cfg(feature = "sqlcipher")]
    let db_key = Some(secrets::load_or_create(&secrets_dir, "db.key")?);
    #[cfg(not(feature = "sqlcipher"))]
    let db_key: Option<[u8; 32]> = {
        tracing::warn!("build senza sqlcipher: il database NON e' cifrato su disco. Usa --features sqlcipher in produzione.");
        None
    };

    let db = db::Db::open(&cfg.db_path(), db_key.as_ref())?;
    themes::prepare_dir(&cfg.themes_dir())?;
    std::fs::create_dir_all(cfg.languages_dir())?;
    let dummy_hash = auth::hash_auth_key("taskhit-hash-fittizio")?;
    let sec = &cfg.security;
    Ok(AppState {
        inflight: Arc::new(Semaphore::new(sec.max_in_flight)),
        hash_slots: Arc::new(Semaphore::new(sec.max_parallel_hashes)),
        limiter: Arc::new(ratelimit::RateLimiter::new()),
        server_key: Arc::new(server_key),
        dummy_hash: Arc::new(dummy_hash),
        db,
        backup_lock: Arc::default(),
        cfg: Arc::new(cfg),
    })
}

pub fn router(state: AppState) -> Router {
    let sec = state.cfg.security.clone();
    let web_dir = state.cfg.paths.web_dir.clone();
    // Percorsi "da app" (senza estensione) -> index.html. File mancanti (/assets/x.js) -> 404 vero.
    let index = web_dir.join("index.html");
    let spa_index = Router::new().fallback(move |uri: axum::http::Uri| {
        let index = index.clone();
        async move {
            let last = uri.path().rsplit('/').next().unwrap_or("");
            if last.contains('.') {
                return StatusCode::NOT_FOUND.into_response();
            }
            match tokio::fs::read(&index).await {
                Ok(html) => ([(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        }
    });
    let spa = ServeDir::new(&web_dir).fallback(spa_index);

    // Il primo layer e' il piu' interno; l'ultimo e' il primo a vedere la richiesta.
    let app = Router::new()
        .nest(
            "/api",
            api::routes()
                .layer(RequestBodyLimitLayer::new(sec.max_body_kb * 1024))
                .merge(backup::upload_routes().layer(DefaultBodyLimit::max(backup::MAX_UPLOAD_BYTES)).layer(RequestBodyLimitLayer::new(backup::MAX_UPLOAD_BYTES))),
        )
        .fallback_service(spa)
        .layer(from_fn_with_state(state.clone(), web::origin_guard))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(sec.request_timeout_secs)))
        .layer(axum::middleware::from_fn(web::cache_headers))
        .layer(CompressionLayer::new())
        .layer(from_fn_with_state(state.clone(), web::guard))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    web::with_security_headers(app, sec.cookie_secure)
}
