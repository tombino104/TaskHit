//! API HTTP: stato, primo avvio, accesso, sessione e temi; il resto e' nei moduli
//! `events`, `proposals`, `people` e `meta`. I backup stanno in `backup`; le push sono in roadmap (vedi ARCHITETTURA.md).

use crate::{
    auth::{self, AuthUser},
    error::ApiError,
    events, meta, people, proposals,
    ratelimit::now_secs,
    state::{AppState, ClientIp},
    themes,
};
use axum::{
    Extension, Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, header::{SET_COOKIE, USER_AGENT}},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

/// Parametri di derivazione delle chiavi lato client (Argon2id). Fissati dal server,
/// cosi' si possono rafforzare in futuro senza ricompilare i client.
fn kdf_params() -> Value {
    json!({ "alg": "argon2id", "m_kib": 65536, "t": 3, "p": 1, "len": 32 })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/setup/status", get(setup_status))
        .route("/setup", post(setup))
        .route("/auth/prelogin", post(prelogin))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/themes", get(list_themes))
        .merge(events::routes())
        .merge(proposals::routes())
        .merge(people::routes())
        .merge(meta::routes())
        .merge(crate::backup::routes())
        .fallback(|| async { ApiError::NotFound })
}

async fn health(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    st.db.run(|c| Ok(c.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?)).await?;
    Ok(Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") })))
}

pub async fn user_count(st: &AppState) -> Result<i64, ApiError> {
    st.db.run(|c| Ok(c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0))?)).await
}

async fn setup_status(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "needs_setup": user_count(&st).await? == 0, "kdf": kdf_params() })))
}

/// Dati di un nuovo account, uguali per il primo admin e per chi entra con un invito.
#[derive(Deserialize)]
pub struct AccountKeys {
    pub username: String,
    pub display_name: String,
    pub auth_key: String,
    pub kdf_salt: String,
    pub wrapped_key: String,
    pub recovery_wrapped_key: Option<String>,
}

impl AccountKeys {
    /// Normalizza nome utente e nome, e controlla la forma delle chiavi.
    pub fn checked(mut self) -> Result<Self, ApiError> {
        self.username = auth::normalize_username(&self.username)?;
        self.display_name = self.display_name.trim().to_owned();
        if self.display_name.is_empty() || self.display_name.chars().count() > 60 {
            return Err(ApiError::BadRequest("il nome deve avere da 1 a 60 caratteri"));
        }
        if !auth::valid_b64(&self.auth_key, Some(32), 64)
            || !auth::valid_b64(&self.kdf_salt, Some(16), 32)
            || !auth::valid_b64(&self.wrapped_key, None, 512)
            || self.recovery_wrapped_key.as_deref().is_some_and(|k| !auth::valid_b64(k, None, 512))
        {
            return Err(ApiError::BadRequest("chiavi non valide"));
        }
        Ok(self)
    }
}

fn session_response(st: &AppState, token: &str, body: Value) -> Response {
    let mut res = Json(body).into_response();
    if let Ok(v) = HeaderValue::from_str(&auth::session_cookie(&st.cfg, token)) {
        res.headers_mut().append(SET_COOKIE, v);
    }
    res
}

async fn create_session(st: &AppState, user_id: String, headers: &HeaderMap) -> Result<String, ApiError> {
    let token = auth::new_token();
    let sid = auth::session_id(&token);
    let now = now_secs() as i64;
    let expires = now + i64::from(st.cfg.security.session_days) * 86_400;
    let ua: String = headers.get(USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("").chars().take(200).collect();
    st.db
        .run(move |c| {
            c.execute(
                "INSERT INTO sessions (id, user_id, created_at, expires_at, last_seen, user_agent) VALUES (?1, ?2, ?3, ?4, ?3, ?5)",
                params![sid, user_id, now, expires, ua],
            )?;
            Ok(())
        })
        .await?;
    Ok(token)
}

/// Chi puo' fare il primo avvio (crearne l'admin o ripristinare un backup).
/// Si fa dalla rete locale: una richiesta che arriva dal tunnel porta l'intestazione di Cloudflare.
pub fn check_setup_allowed(st: &AppState, ip: &str, headers: &HeaderMap) -> Result<(), ApiError> {
    let sec = &st.cfg.security;
    if !sec.allow_remote_setup && !sec.client_ip_header.is_empty() && headers.contains_key(sec.client_ip_header.as_str()) {
        return Err(ApiError::Forbidden("completa la configurazione iniziale dalla rete locale, prima di pubblicare il tunnel"));
    }
    if !st.limiter.hit(&format!("setup:{ip}"), 10, 60) {
        return Err(ApiError::TooManyRequests);
    }
    Ok(())
}

/// Crea il primo admin. Funziona una volta sola, finche' non esiste nessun utente.
async fn setup(
    State(st): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
    Json(req): Json<AccountKeys>,
) -> Result<Response, ApiError> {
    check_setup_allowed(&st, &ip, &headers)?;
    let req = req.checked()?;
    let (username, display_name) = (req.username.clone(), req.display_name.clone());
    if user_count(&st).await? > 0 {
        return Err(ApiError::Conflict("la configurazione iniziale e' gia' stata completata"));
    }

    let hash = hash_blocking(&st, req.auth_key.clone()).await?;
    let id = auth::random_hex(16);
    let now = now_secs() as i64;
    let (uid, uname, dname, salt, wrapped) = (id.clone(), username.clone(), display_name.clone(), req.kdf_salt.clone(), req.wrapped_key.clone());
    let recovery = req.recovery_wrapped_key.clone();
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            // Controllo e inserimento nella stessa transazione: due richieste insieme non creano due admin.
            let n: i64 = tx.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
            if n > 0 {
                return Err(ApiError::Conflict("la configurazione iniziale e' gia' stata completata"));
            }
            tx.execute(
                "INSERT INTO users (id, username, display_name, role, auth_hash, kdf_salt, wrapped_key, recovery_wrapped_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'admin', ?4, ?5, ?6, ?7, ?8, ?8)",
                params![uid, uname, dname, hash, salt, wrapped, recovery, now],
            )?;
            tx.execute("INSERT INTO settings (key, value) VALUES ('setup_completed_at', ?1)", params![now.to_string()])?;
            tx.execute("INSERT INTO audit_log (at, actor_id, action) VALUES (?1, ?2, 'setup_completed')", params![now, uid])?;
            tx.commit()?;
            Ok(())
        })
        .await?;

    let token = create_session(&st, id.clone(), &headers).await?;
    Ok(session_response(&st, &token, json!({ "user": { "id": id, "username": username, "display_name": display_name, "role": "admin" } })))
}

#[derive(Deserialize)]
struct PreloginReq {
    username: String,
}

/// Restituisce il salt per derivare le chiavi. Per gli utenti inesistenti ne restituisce uno finto e stabile.
async fn prelogin(
    State(st): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Json(req): Json<PreloginReq>,
) -> Result<Json<Value>, ApiError> {
    if !st.limiter.hit(&format!("prelogin:{ip}"), 30, 60) {
        return Err(ApiError::TooManyRequests);
    }
    let username = auth::normalize_username(&req.username)?;
    let u = username.clone();
    let real: Option<String> = st.db.run(move |c| Ok(c.query_row("SELECT kdf_salt FROM users WHERE username = ?1 AND disabled = 0", params![u], |r| r.get(0)).optional()?)).await?;
    let salt = real.unwrap_or_else(|| auth::fake_salt(&st.server_key, &username));
    Ok(Json(json!({ "kdf_salt": salt, "kdf": kdf_params() })))
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    auth_key: String,
}

pub async fn hash_blocking(st: &AppState, auth_key: String) -> Result<String, ApiError> {
    let _slot = st.hash_slots.clone().acquire_owned().await.map_err(ApiError::internal)?;
    tokio::task::spawn_blocking(move || auth::hash_auth_key(&auth_key)).await.map_err(ApiError::internal)?.map_err(ApiError::from)
}

pub async fn verify_blocking(st: &AppState, auth_key: String, stored: String) -> Result<bool, ApiError> {
    let _slot = st.hash_slots.clone().acquire_owned().await.map_err(ApiError::internal)?;
    tokio::task::spawn_blocking(move || auth::verify_auth_key(&auth_key, &stored)).await.map_err(ApiError::internal)
}

async fn login(
    State(st): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> Result<Response, ApiError> {
    let sec = &st.cfg.security;
    if !st.limiter.hit(&format!("login-ip:{ip}"), sec.login_attempts_per_minute_ip, 60) {
        return Err(ApiError::TooManyRequests);
    }
    let username = auth::normalize_username(&req.username)?;
    if !st.limiter.hit(&format!("login-user:{username}"), sec.login_attempts_per_minute_user, 60) {
        return Err(ApiError::TooManyRequests);
    }
    if !auth::valid_b64(&req.auth_key, Some(32), 64) {
        return Err(ApiError::BadRequest("chiavi non valide"));
    }

    let u = username.clone();
    type Row = (String, String, String, String, String, String, bool, bool);
    let row: Option<Row> = st
        .db
        .run(move |c| {
            Ok(c.query_row(
                "SELECT id, display_name, role, auth_hash, kdf_salt, wrapped_key, disabled, pending FROM users WHERE username = ?1",
                params![u],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
            )
            .optional()?)
        })
        .await?;

    // Anche per un utente inesistente si esegue una verifica completa, per non rivelarlo dai tempi di risposta.
    let stored = row.as_ref().map_or_else(|| st.dummy_hash.as_str().to_owned(), |r| r.3.clone());
    let verified = verify_blocking(&st, req.auth_key.clone(), stored).await?;
    let now = now_secs() as i64;

    let Some((id, display_name, role, _hash, kdf_salt, wrapped_key, _, pending)) = row.filter(|r| verified && !r.6) else {
        let _ = st.db.run(move |c| { c.execute("INSERT INTO audit_log (at, action) VALUES (?1, 'login_failed')", params![now])?; Ok(()) }).await;
        return Err(ApiError::Unauthorized);
    };
    // Lo si dice solo a chi conosce la password: non rivela nulla a un estraneo.
    if pending {
        return Err(ApiError::Forbidden("la tua richiesta e' in attesa di conferma dell'admin"));
    }

    let token = create_session(&st, id.clone(), &headers).await?;
    let uid = id.clone();
    let _ = st.db.run(move |c| { c.execute("INSERT INTO audit_log (at, actor_id, action) VALUES (?1, ?2, 'login')", params![now, uid])?; Ok(()) }).await;
    Ok(session_response(
        &st,
        &token,
        json!({
            "user": { "id": id, "username": username, "display_name": display_name, "role": role },
            "kdf_salt": kdf_salt,
            "wrapped_key": wrapped_key,
        }),
    ))
}

async fn logout(State(st): State<AppState>, user: AuthUser) -> Result<Response, ApiError> {
    let sid = user.session_id.clone();
    st.db.run(move |c| { c.execute("DELETE FROM sessions WHERE id = ?1", params![sid])?; Ok(()) }).await?;
    let mut res = Json(json!({ "ok": true })).into_response();
    if let Ok(v) = HeaderValue::from_str(&auth::clear_cookie(&st.cfg)) {
        res.headers_mut().append(SET_COOKIE, v);
    }
    Ok(res)
}

async fn list_themes(State(st): State<AppState>) -> Result<Json<themes::ThemeList>, ApiError> {
    let dir = st.cfg.themes_dir();
    let list = tokio::task::spawn_blocking(move || themes::load_dir(&dir)).await.map_err(ApiError::internal)?;
    Ok(Json(list))
}
