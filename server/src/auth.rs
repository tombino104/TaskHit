//! Autenticazione.
//!
//! Il client non manda mai la password: la trasforma con Argon2id in due chiavi indipendenti,
//! una per autenticarsi (`auth_key`) e una per cifrare i propri dati. Il server riceve solo la
//! prima e ne conserva un secondo hash Argon2id. La seconda chiave non lascia mai il dispositivo.

use crate::{config::Config, error::ApiError, state::AppState};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, header::COOKIE, request::Parts},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

pub fn hash_auth_key(auth_key: &str) -> anyhow::Result<String> {
    let hash = Argon2::default().hash_password(auth_key.as_bytes()).map_err(|e| anyhow::anyhow!("argon2: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_auth_key(auth_key: &str, stored: &str) -> bool {
    match PasswordHash::new(stored) {
        Ok(parsed) => Argon2::default().verify_password(auth_key.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::fill(&mut buf[..]);
    hex::encode(buf)
}

/// Token di sessione: 256 bit casuali. Il server conserva solo il suo SHA-256.
pub fn new_token() -> String {
    let mut buf = [0u8; 32];
    rand::fill(&mut buf[..]);
    B64.encode(buf)
}

pub fn session_id(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Salt finto ma stabile per gli utenti che non esistono: impedisce di scoprire i nomi utente.
pub fn fake_salt(server_key: &[u8; 32], username: &str) -> String {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(server_key).expect("HMAC accetta chiavi di qualsiasi lunghezza");
    mac.update(b"taskhit/prelogin/v1:");
    mac.update(username.as_bytes());
    let out = mac.finalize().into_bytes();
    B64.encode(&out[..16])
}

/// Controlla che `s` sia base64url senza padding e, se richiesto, che decodifichi in `len` byte.
pub fn valid_b64(s: &str, len: Option<usize>, max_chars: usize) -> bool {
    if s.is_empty() || s.len() > max_chars {
        return false;
    }
    match B64.decode(s) {
        Ok(bytes) => len.is_none_or(|n| bytes.len() == n),
        Err(_) => false,
    }
}

pub fn normalize_username(raw: &str) -> Result<String, ApiError> {
    let u = raw.trim().to_ascii_lowercase();
    let ok = (3..=32).contains(&u.len()) && u.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    if ok { Ok(u) } else { Err(ApiError::BadRequest("il nome utente deve avere 3-32 caratteri: lettere minuscole, numeri, punto, trattino")) }
}

fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all(COOKIE) {
        let Ok(text) = value.to_str() else { continue };
        for part in text.split(';') {
            if let Some((k, v)) = part.trim().split_once('=') {
                if k == name && !v.is_empty() {
                    return Some(v.to_owned());
                }
            }
        }
    }
    None
}

pub fn session_cookie(cfg: &Config, token: &str) -> String {
    let max_age = u64::from(cfg.security.session_days) * 86_400;
    let secure = if cfg.security.cookie_secure { "; Secure" } else { "" };
    format!("{}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}", cfg.cookie_name())
}

pub fn clear_cookie(cfg: &Config) -> String {
    let secure = if cfg.security.cookie_secure { "; Secure" } else { "" };
    format!("{}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}", cfg.cookie_name())
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub session_id: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = read_cookie(&parts.headers, state.cfg.cookie_name()).ok_or(ApiError::Unauthorized)?;
        let sid = session_id(&token);
        let now = crate::ratelimit::now_secs() as i64;
        let sid2 = sid.clone();
        state
            .db
            .run(move |c| {
                let row = c.query_row(
                    "SELECT u.id, u.username, u.display_name, u.role, s.last_seen
                       FROM sessions s JOIN users u ON u.id = s.user_id
                      WHERE s.id = ?1 AND s.expires_at > ?2 AND u.disabled = 0 AND u.pending = 0",
                    rusqlite::params![sid2, now],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?)),
                );
                match row {
                    Ok((id, username, display_name, role, last_seen)) => {
                        // Aggiorna last_seen al massimo ogni 5 minuti: niente scrittura a ogni richiesta.
                        if now - last_seen > 300 {
                            c.execute("UPDATE sessions SET last_seen = ?1 WHERE id = ?2", rusqlite::params![now, sid2])?;
                        }
                        Ok(AuthUser { id, username, display_name, role, session_id: sid2 })
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => Err(ApiError::Unauthorized),
                    Err(e) => Err(e.into()),
                }
            })
            .await
    }
}
