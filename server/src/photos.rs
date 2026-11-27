//! Foto profilo. Le vede chi condivide un gruppo con la persona (e chi la gestisce).
//! Il client le riduce a un quadrato piccolo prima di inviarle; qui si controllano peso e formato.

use crate::{
    auth::AuthUser,
    common::{self, now},
    error::ApiError,
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::header::{CACHE_CONTROL, CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

pub const MAX_BYTES: usize = 150_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/photo", put(save))
        .route("/me/photo/delete", post(remove))
        .route("/users/{id}/photo", get(show))
}

/// Il tipo si riconosce dai primi byte: quello dichiarato dal client non conta.
fn kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[derive(Deserialize)]
struct PhotoReq {
    data: String,
}

async fn save(State(st): State<AppState>, user: AuthUser, Json(req): Json<PhotoReq>) -> Result<Json<Value>, ApiError> {
    const BAD: &str = "la foto deve essere un'immagine JPEG, PNG o WebP di al massimo 150 kB";
    let bytes = B64.decode(req.data).map_err(|_| ApiError::BadRequest(BAD))?;
    let mime = kind(&bytes).filter(|_| bytes.len() <= MAX_BYTES).ok_or(ApiError::BadRequest(BAD))?;
    st.db
        .run(move |c| {
            // La versione cambia sempre, anche due volte nello stesso secondo.
            c.execute(
                "INSERT INTO avatars (user_id, mime, data, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(user_id) DO UPDATE SET mime = excluded.mime, data = excluded.data, updated_at = max(excluded.updated_at, updated_at + 1)",
                params![user.id, mime, bytes, now()],
            )?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn remove(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    st.db
        .run(move |c| {
            c.execute("DELETE FROM avatars WHERE user_id = ?1", params![user.id])?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn show(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Response, ApiError> {
    let (mime, data) = st
        .db
        .run(move |c| {
            let mut allowed = user.id == id || user.role == "admin";
            if !allowed {
                allowed = c
                    .prepare("SELECT 1 FROM group_members a JOIN group_members b ON b.group_id = a.group_id WHERE a.user_id = ?1 AND b.user_id = ?2")?
                    .exists(params![user.id, id])?;
            }
            if !allowed && user.role == "rappresentante" {
                // Anche chi aspetta di entrare nella sua classe.
                let managed = common::managed_groups(c, &user)?;
                let waiting: Option<String> = c
                    .query_row("SELECT pending_group_id FROM users WHERE id = ?1 AND pending = 1", params![id], |r| r.get(0))
                    .optional()?
                    .flatten();
                allowed = waiting.is_some_and(|g| common::manages(&managed, &g));
            }
            if !allowed {
                return Err(ApiError::NotFound);
            }
            c.query_row("SELECT mime, data FROM avatars WHERE user_id = ?1", params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))
                .optional()?
                .ok_or(ApiError::NotFound)
        })
        .await?;
    // L'indirizzo porta la versione (?v=...): una foto nuova ha un indirizzo nuovo.
    Ok(([(CONTENT_TYPE, mime), (CACHE_CONTROL, "private, max-age=86400".to_owned())], data).into_response())
}
