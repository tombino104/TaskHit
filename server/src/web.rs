//! Middleware: IP del client, limiti, controllo dell'origine, intestazioni di sicurezza e cache.

use crate::{
    error::ApiError,
    state::{AppState, ClientIp},
};
use axum::{
    Router,
    extract::{ConnectInfo, Request, State},
    http::{HeaderName, HeaderValue, Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{net::{IpAddr, SocketAddr}, sync::Arc};
use tower_http::set_header::SetResponseHeaderLayer;

/// Niente script o stili esterni: font, immagini e codice sono tutti serviti da qui.
/// 'wasm-unsafe-eval' consente solo di compilare WebAssembly (Argon2id nel browser), non l'eval di JavaScript.
const CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
font-src 'self'; connect-src 'self'; worker-src 'self'; manifest-src 'self'; object-src 'none'; base-uri 'self'; \
form-action 'self'; frame-ancestors 'none'";

pub fn client_ip(st: &AppState, req: &Request) -> String {
    let header_name = &st.cfg.security.client_ip_header;
    if !header_name.is_empty() {
        if let Some(ip) = req.headers().get(header_name.as_str()).and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<IpAddr>().ok()) {
            return ip.to_string();
        }
    }
    req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string()).unwrap_or_else(|| "sconosciuto".into())
}

/// Calcola l'IP, applica il limite generale per IP e scarta il traffico in eccesso (503) invece di accodarlo.
pub async fn guard(State(st): State<AppState>, mut req: Request, next: Next) -> Response {
    let ip = client_ip(&st, &req);
    let is_api = req.uri().path().starts_with("/api/");
    if is_api && !st.limiter.hit(&format!("api:{ip}"), st.cfg.security.api_requests_per_minute, 60) {
        return ApiError::TooManyRequests.into_response();
    }
    req.extensions_mut().insert(ClientIp(ip));
    let Ok(_permit) = Arc::clone(&st.inflight).try_acquire_owned() else {
        return ApiError::Unavailable.into_response();
    };
    next.run(req).await
}

/// Difesa CSRF in aggiunta al cookie SameSite=Strict: le scritture devono arrivare dalla nostra origine.
pub async fn origin_guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    if !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        let h = req.headers();
        let cross_site = h.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site");
        let origin = h.get(header::ORIGIN).and_then(|v| v.to_str().ok());
        let host = h.get(header::HOST).and_then(|v| v.to_str().ok());
        if cross_site || origin.is_some_and(|o| !st.cfg.origin_allowed(o, host)) {
            return ApiError::Forbidden("origine non consentita").into_response();
        }
    }
    next.run(req).await
}

/// Cache: API mai in cache; risorse con hash nel nome per sempre; il resto (index, service worker) sempre riconvalidato.
pub async fn cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let mut res = next.run(req).await;
    let value = if path.starts_with("/api/") {
        "no-store"
    } else if path.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    res.headers_mut().entry(header::CACHE_CONTROL).or_insert(HeaderValue::from_static(value));
    res
}

pub fn with_security_headers(router: Router, hsts: bool) -> Router {
    let h = |name: &'static str, value: &'static str| SetResponseHeaderLayer::overriding(HeaderName::from_static(name), HeaderValue::from_static(value));
    let mut r = router
        .layer(h("content-security-policy", CSP))
        .layer(h("x-content-type-options", "nosniff"))
        .layer(h("x-frame-options", "DENY"))
        .layer(h("referrer-policy", "no-referrer"))
        .layer(h("cross-origin-opener-policy", "same-origin"))
        .layer(h("permissions-policy", "camera=(), microphone=(), geolocation=(), payment=()"));
    if hsts {
        r = r.layer(h("strict-transport-security", "max-age=31536000"));
    }
    r
}
