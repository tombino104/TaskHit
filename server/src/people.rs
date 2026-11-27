//! Persone, gruppi e inviti. Li gestisce solo l'admin; chi riceve un codice puo' chiedere di entrare.

use crate::{
    api::{AccountKeys, hash_blocking},
    auth::{self, AuthUser},
    common::{self, ROLES, now},
    error::ApiError,
    state::{AppState, ClientIp},
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{get, patch, post},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

const INVITE_DAYS: i64 = 7;
const MAX_PENDING: i64 = 100;
/// Senza caratteri che si confondono (0/O, 1/I/L).
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 9;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list))
        .route("/users/{id}", patch(update))
        .route("/users/{id}/accept", post(accept))
        .route("/users/{id}/reject", post(reject))
        .route("/groups", post(create_group))
        .route("/invites", get(invites).post(create_invite))
        .route("/invites/{id}/revoke", post(revoke_invite))
        .route("/join", post(join))
}

fn group_ids(c: &Connection, user_id: &str) -> Result<Vec<String>, ApiError> {
    let mut stmt = c.prepare("SELECT group_id FROM group_members WHERE user_id = ?1")?;
    Ok(stmt.query_map(params![user_id], |r| r.get(0))?.collect::<Result<_, _>>()?)
}

async fn list(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let res = st
        .db
        .run(|c| {
            let mut stmt = c.prepare(
                "SELECT id, username, display_name, role, disabled, pending, pending_group_id, created_at FROM users
                  ORDER BY CASE role WHEN 'admin' THEN 0 WHEN 'rappresentante' THEN 1 WHEN 'nerd' THEN 2 ELSE 3 END, display_name COLLATE NOCASE",
            )?;
            type Row = (String, String, String, String, bool, bool, Option<String>, i64);
            let rows: Vec<Row> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?
                .collect::<Result<_, _>>()?;
            let (mut users, mut waiting) = (vec![], vec![]);
            for (id, username, display_name, role, disabled, pending, group, created_at) in rows {
                if pending {
                    waiting.push(json!({ "id": id, "username": username, "display_name": display_name, "group_id": group, "created_at": created_at }));
                } else {
                    let groups = group_ids(c, &id)?;
                    users.push(json!({ "id": id, "username": username, "display_name": display_name, "role": role, "disabled": disabled, "group_ids": groups }));
                }
            }
            Ok(json!({ "users": users, "pending": waiting }))
        })
        .await?;
    Ok(Json(res))
}

#[derive(Deserialize)]
struct UpdateReq {
    role: Option<String>,
    disabled: Option<bool>,
    group_ids: Option<Vec<String>>,
}

async fn update(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<UpdateReq>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    if req.role.as_deref().is_some_and(|r| !ROLES.contains(&r)) {
        return Err(ApiError::BadRequest("ruolo non valido"));
    }
    // L'admin non puo' togliersi i poteri da solo: cosi' ne resta sempre almeno uno.
    if id == user.id && (req.role.as_deref().is_some_and(|r| r != "admin") || req.disabled == Some(true)) {
        return Err(ApiError::Forbidden("non puoi cambiare il tuo ruolo o disattivare il tuo account"));
    }
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            if !common::exists(&tx, "SELECT 1 FROM users WHERE id = ?1 AND pending = 0", &id)? {
                return Err(ApiError::NotFound);
            }
            if let Some(role) = &req.role {
                tx.execute("UPDATE users SET role = ?2, updated_at = ?3 WHERE id = ?1", params![id, role, now()])?;
                common::audit(&tx, &user.id, "role_changed", Some(&format!("{id}:{role}")))?;
            }
            if let Some(disabled) = req.disabled {
                tx.execute("UPDATE users SET disabled = ?2, updated_at = ?3 WHERE id = ?1", params![id, disabled, now()])?;
                if disabled {
                    tx.execute("DELETE FROM sessions WHERE user_id = ?1", params![id])?;
                }
                common::audit(&tx, &user.id, if disabled { "user_disabled" } else { "user_enabled" }, Some(&id))?;
            }
            // L'admin non fa parte della classe: perde i gruppi e non ne puo' ricevere.
            let admin = common::exists(&tx, "SELECT 1 FROM users WHERE id = ?1 AND role = 'admin'", &id)?;
            if admin {
                tx.execute("DELETE FROM group_members WHERE user_id = ?1", params![id])?;
                tx.execute("UPDATE events SET subject_id = NULL WHERE subject_id = ?1", params![id])?;
                tx.execute("DELETE FROM inbox WHERE user_id = ?1", params![id])?;
            }
            if let Some(groups) = req.group_ids.as_ref().filter(|_| !admin) {
                tx.execute("DELETE FROM group_members WHERE user_id = ?1", params![id])?;
                for g in groups {
                    if !common::exists(&tx, "SELECT 1 FROM groups WHERE id = ?1", g)? {
                        return Err(ApiError::BadRequest("il gruppo scelto non esiste"));
                    }
                    tx.execute("INSERT OR IGNORE INTO group_members (group_id, user_id) VALUES (?1, ?2)", params![g, id])?;
                }
                common::audit(&tx, &user.id, "groups_changed", Some(&id))?;
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn accept(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let group: Option<String> = tx
                .query_row("SELECT pending_group_id FROM users WHERE id = ?1 AND pending = 1", params![id], |r| r.get(0))
                .optional()?
                .ok_or(ApiError::NotFound)?;
            tx.execute("UPDATE users SET pending = 0, pending_group_id = NULL, updated_at = ?2 WHERE id = ?1", params![id, now()])?;
            if let Some(g) = group {
                tx.execute(
                    "INSERT OR IGNORE INTO group_members (group_id, user_id) SELECT id, ?2 FROM groups WHERE id = ?1",
                    params![g, id],
                )?;
            }
            common::audit(&tx, &user.id, "user_accepted", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn reject(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    st.db
        .run(move |c| {
            // Si cancellano solo le richieste non ancora accettate.
            if c.execute("DELETE FROM users WHERE id = ?1 AND pending = 1", params![id])? == 0 {
                return Err(ApiError::NotFound);
            }
            common::audit(c, &user.id, "user_rejected", Some(&id))
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct GroupReq {
    name: String,
    kind: String,
}

/// I gruppi li crea solo l'admin, che pero' non ne fa parte.
async fn create_group(State(st): State<AppState>, user: AuthUser, Json(req): Json<GroupReq>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let name = common::required_text(&req.name, 40, "il nome del gruppo deve avere da 1 a 40 caratteri")?;
    if req.kind != "classe" && req.kind != "gruppo" {
        return Err(ApiError::BadRequest("il tipo deve essere classe o gruppo"));
    }
    let id = auth::random_hex(16);
    let gid = id.clone();
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            if common::exists(&tx, "SELECT 1 FROM groups WHERE name = ?1", &name)? {
                return Err(ApiError::Conflict("esiste gia' un gruppo con questo nome"));
            }
            tx.execute("INSERT INTO groups (id, name, kind, created_at) VALUES (?1, ?2, ?3, ?4)", params![gid, name, req.kind, now()])?;
            common::audit(&tx, &user.id, "group_created", Some(&gid))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "id": id })))
}

fn new_code() -> String {
    let mut out = String::with_capacity(CODE_LEN);
    // Rifiuto dei byte oltre l'ultimo multiplo: ogni carattere ha la stessa probabilita'.
    let limit = 256 - 256 % CODE_ALPHABET.len();
    while out.len() < CODE_LEN {
        let mut b = [0u8; 1];
        rand::fill(&mut b[..]);
        if usize::from(b[0]) < limit {
            out.push(char::from(CODE_ALPHABET[usize::from(b[0]) % CODE_ALPHABET.len()]));
        }
    }
    out
}

/// "k7q-4bx 29m" -> "K7Q4BX29M".
fn normalize_code(raw: &str) -> String {
    raw.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).take(CODE_LEN + 1).collect()
}

async fn invites(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let items = st
        .db
        .run(|c| {
            let mut stmt = c.prepare(
                "SELECT id, code, group_id, expires_at, uses FROM invites WHERE revoked_at IS NULL AND expires_at > ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![now()], |r| {
                Ok(json!({ "id": r.get::<_, String>(0)?, "code": r.get::<_, String>(1)?, "group_id": r.get::<_, String>(2)?,
                           "expires_at": r.get::<_, i64>(3)?, "uses": r.get::<_, i64>(4)? }))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(Json(json!({ "invites": items })))
}

#[derive(Deserialize)]
struct InviteReq {
    group_id: String,
}

/// Un solo codice attivo per gruppo: crearne uno nuovo revoca il precedente.
async fn create_invite(State(st): State<AppState>, user: AuthUser, Json(req): Json<InviteReq>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let code = new_code();
    let out = code.clone();
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            if !common::exists(&tx, "SELECT 1 FROM groups WHERE id = ?1", &req.group_id)? {
                return Err(ApiError::BadRequest("il gruppo scelto non esiste"));
            }
            tx.execute("UPDATE invites SET revoked_at = ?2 WHERE group_id = ?1 AND revoked_at IS NULL", params![req.group_id, now()])?;
            tx.execute(
                "INSERT INTO invites (id, code, group_id, created_by, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![auth::random_hex(16), code, req.group_id, user.id, now(), now() + INVITE_DAYS * 86_400],
            )?;
            common::audit(&tx, &user.id, "invite_created", Some(&req.group_id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "code": out })))
}

async fn revoke_invite(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    st.db
        .run(move |c| {
            if c.execute("UPDATE invites SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL", params![id, now()])? == 0 {
                return Err(ApiError::NotFound);
            }
            common::audit(c, &user.id, "invite_revoked", Some(&id))
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct JoinReq {
    code: String,
    #[serde(flatten)]
    account: AccountKeys,
}

/// Richiesta di iscrizione con codice invito. L'account resta in attesa finche' l'admin non lo accetta.
async fn join(
    State(st): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Json(req): Json<JoinReq>,
) -> Result<Json<Value>, ApiError> {
    // Limite per IP e limite complessivo: indovinare un codice a tentativi non e' praticabile.
    if !st.limiter.hit(&format!("join:{ip}"), 10, 60) || !st.limiter.hit("join-globale", 120, 60) {
        return Err(ApiError::TooManyRequests);
    }
    let account = req.account.checked()?;
    let code = normalize_code(&req.code);
    const INVALID: &str = "codice invito non valido o scaduto";
    let find = |c: &Connection, code: &str| -> Result<(String, String), ApiError> {
        c.query_row(
            "SELECT id, group_id FROM invites WHERE code = ?1 AND revoked_at IS NULL AND expires_at > ?2",
            params![code, now()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(ApiError::BadRequest(INVALID))
    };
    // Il codice si controlla prima di spendere un calcolo Argon2.
    let probe = code.clone();
    st.db.run(move |c| find(c, &probe).map(|_| ())).await?;
    let hash = hash_blocking(&st, account.auth_key.clone()).await?;
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let (invite, group) = find(&tx, &code)?;
            let waiting: i64 = tx.query_row("SELECT COUNT(*) FROM users WHERE pending = 1", [], |r| r.get(0))?;
            if waiting >= MAX_PENDING {
                return Err(ApiError::Conflict("troppe richieste in attesa: chiedi all'admin di smaltirle"));
            }
            if common::exists(&tx, "SELECT 1 FROM users WHERE username = ?1", &account.username)? {
                return Err(ApiError::Conflict("nome utente gia' in uso"));
            }
            tx.execute(
                "INSERT INTO users (id, username, display_name, role, auth_hash, kdf_salt, wrapped_key, recovery_wrapped_key, pending, pending_group_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'alunno', ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9)",
                params![auth::random_hex(16), account.username, account.display_name, hash, account.kdf_salt, account.wrapped_key,
                        account.recovery_wrapped_key, group, now()],
            )?;
            tx.execute("UPDATE invites SET uses = uses + 1 WHERE id = ?1", params![invite])?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_use_only_unambiguous_characters() {
        for _ in 0..50 {
            let c = new_code();
            assert_eq!(c.len(), CODE_LEN);
            assert!(c.bytes().all(|b| CODE_ALPHABET.contains(&b)));
            assert_eq!(normalize_code(&format!("{}-{}-{}", &c[..3], &c[3..6], &c[6..]).to_lowercase()), c);
        }
    }
}
