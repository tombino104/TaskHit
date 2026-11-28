//! Moderazione degli account: sospensione (anche a tempo), espulsione, obbligo di cambiare nome,
//! rimozione della foto. L'admin agisce su chiunque tranne se stesso; il rappresentante su alunni
//! e nerd della propria classe, e non puo' modificare un blocco deciso dall'admin.

use crate::{
    auth::AuthUser,
    common::{self, now},
    error::ApiError,
    people,
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

pub const TAGS: [&str; 4] = ["nome", "foto", "comportamento", "altro"];
const MAX_DAYS: i64 = 365;

pub fn routes() -> Router<AppState> {
    Router::new().route("/moderation", get(list)).route("/users/{id}/sanction", post(sanction))
}

/// Riapre gli account la cui sospensione a tempo e' finita.
pub fn lift_expired(c: &Connection, at: i64) -> Result<usize, ApiError> {
    Ok(c.execute(
        "UPDATE users SET disabled = 0, block_kind = NULL, block_until = NULL, block_reason = NULL, block_tags = '', blocked_by_role = NULL
          WHERE disabled = 1 AND block_until IS NOT NULL AND block_until <= ?1",
        params![at],
    )?)
}

pub fn clear_block(c: &Connection, user_id: &str) -> Result<(), ApiError> {
    c.execute(
        "UPDATE users SET disabled = 0, block_kind = NULL, block_until = NULL, block_reason = NULL, block_tags = '', blocked_by_role = NULL, updated_at = ?2 WHERE id = ?1",
        params![user_id, now()],
    )?;
    Ok(())
}

/// Il blocco di un account, come lo vedono chi modera e l'interessato.
pub fn block_json(kind: Option<String>, until: Option<i64>, reason: Option<String>, tags: String) -> Value {
    match kind {
        None => Value::Null,
        Some(kind) => json!({
            "kind": kind, "until": until, "reason": reason.unwrap_or_default(),
            "tags": tags.split(',').filter(|t| !t.is_empty()).collect::<Vec<_>>(),
        }),
    }
}

/// Le classi di cui l'utente fa parte.
fn classes_of(c: &Connection, user_id: &str) -> Result<Vec<String>, ApiError> {
    let mut stmt = c.prepare("SELECT g.id FROM groups g JOIN group_members m ON m.group_id = g.id WHERE m.user_id = ?1 AND g.kind = 'classe'")?;
    Ok(stmt.query_map(params![user_id], |r| r.get(0))?.collect::<Result<_, _>>()?)
}

/// L'admin riceve tutti gli account; il rappresentante quelli della sua classe, sospesi compresi.
async fn list(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    common::require_manager(&user)?;
    let users = st
        .db
        .run(move |c| {
            lift_expired(c, now())?;
            let mine = classes_of(c, &user.id)?;
            let mut stmt = c.prepare(
                "SELECT u.id, u.display_name, u.role, u.block_kind, u.block_until, u.block_reason, u.block_tags,
                        u.blocked_by_role, u.rename_request IS NOT NULL, a.updated_at
                   FROM users u LEFT JOIN avatars a ON a.user_id = u.id
                  WHERE u.pending = 0 AND u.removed_at IS NULL AND u.role <> 'admin' AND u.id <> ?1 ORDER BY u.display_name COLLATE NOCASE",
            )?;
            type Row = (String, String, String, Option<String>, Option<i64>, Option<String>, String, Option<String>, bool, Option<i64>);
            let rows: Vec<Row> = stmt
                .query_map(params![user.id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?))
                })?
                .collect::<Result<_, _>>()?;
            let mut out = vec![];
            for (id, name, role, kind, until, reason, tags, by, renaming, photo) in rows {
                let classes = classes_of(c, &id)?;
                if user.role != "admin" && !classes.iter().any(|g| mine.contains(g)) {
                    continue;
                }
                let can_moderate = user.role == "admin" || (role != "rappresentante" && (kind.is_none() || by.as_deref() != Some("admin")));
                out.push(json!({
                    "id": id, "name": name, "role": role, "class_ids": classes, "photo": photo,
                    "block": block_json(kind, until, reason, tags), "renaming": renaming, "can_moderate": can_moderate,
                }));
            }
            Ok(out)
        })
        .await?;
    Ok(Json(json!({ "users": users })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SanctionReq {
    /// "sospendi", "espelli", "nome", "foto" oppure "riattiva".
    action: String,
    #[serde(default)]
    tags: Vec<String>,
    reason: Option<String>,
    /// Solo per la sospensione: giorni; assente = finche' qualcuno non riattiva.
    days: Option<i64>,
}

async fn sanction(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<SanctionReq>) -> Result<Json<Value>, ApiError> {
    common::require_manager(&user)?;
    if id == user.id {
        return Err(ApiError::Forbidden("non puoi moderare il tuo account"));
    }
    if req.tags.iter().any(|t| !TAGS.contains(&t.as_str())) {
        return Err(ApiError::BadRequest("motivo non valido"));
    }
    let tags = TAGS.iter().filter(|t| req.tags.iter().any(|x| x == *t)).copied().collect::<Vec<_>>().join(",");
    let reason = common::optional_text(req.reason, 500, "il motivo e' troppo lungo")?;
    let needs_reason = matches!(req.action.as_str(), "sospendi" | "espelli" | "nome");
    if needs_reason && tags.is_empty() && reason.is_none() {
        return Err(ApiError::BadRequest("scrivi il motivo oppure scegline uno"));
    }
    if req.days.is_some_and(|d| !(1..=MAX_DAYS).contains(&d)) {
        return Err(ApiError::BadRequest("la durata va da 1 a 365 giorni"));
    }
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            lift_expired(&tx, now())?;
            let (role, by): (String, Option<String>) = tx
                .query_row("SELECT role, blocked_by_role FROM users WHERE id = ?1 AND pending = 0 AND removed_at IS NULL", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?
                .ok_or(ApiError::NotFound)?;
            let mine = classes_of(&tx, &user.id)?;
            let theirs = classes_of(&tx, &id)?;
            if user.role != "admin" {
                if !theirs.iter().any(|g| mine.contains(g)) {
                    return Err(ApiError::NotFound);
                }
                if role == "admin" || role == "rappresentante" {
                    return Err(ApiError::Forbidden("puoi moderare solo alunni e nerd della tua classe"));
                }
                if by.as_deref() == Some("admin") && matches!(req.action.as_str(), "sospendi" | "espelli" | "riattiva") {
                    return Err(ApiError::Forbidden("il blocco e' stato deciso dall'admin"));
                }
            }
            match req.action.as_str() {
                "sospendi" => block(&tx, &id, "sospeso", req.days.map(|d| now() + d * 86_400), reason.as_deref(), &tags, &user.role)?,
                "espelli" => {
                    // Un rappresentante espelle dalla propria classe: se l'account sta anche altrove, resta attivo.
                    let elsewhere = user.role != "admin" && theirs.iter().any(|g| !mine.contains(g));
                    if elsewhere {
                        for g in mine.iter() {
                            tx.execute("DELETE FROM group_members WHERE group_id = ?1 AND user_id = ?2", params![g, id])?;
                        }
                    } else {
                        tx.execute("DELETE FROM group_members WHERE user_id = ?1", params![id])?;
                        block(&tx, &id, "espulso", None, reason.as_deref(), &tags, &user.role)?;
                    }
                    people::drop_orphan_memberships(&tx)?;
                    tx.execute(
                        "UPDATE proposals SET status = 'rejected', decided_by = ?2, decided_at = ?3 WHERE proposed_by = ?1 AND status = 'pending'",
                        params![id, user.id, now()],
                    )?;
                }
                "nome" => {
                    let request = json!({ "reason": reason.clone().unwrap_or_default(), "tags": tags.split(',').filter(|t| !t.is_empty()).collect::<Vec<_>>() });
                    tx.execute("UPDATE users SET rename_request = ?2, updated_at = ?3 WHERE id = ?1", params![id, request.to_string(), now()])?;
                }
                "foto" => {
                    tx.execute("DELETE FROM avatars WHERE user_id = ?1", params![id])?;
                }
                "riattiva" => clear_block(&tx, &id)?,
                _ => return Err(ApiError::BadRequest("azione non valida")),
            }
            common::audit(&tx, &user.id, "moderation", Some(&format!("{id}:{}:{tags}", req.action)))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

fn block(c: &Connection, id: &str, kind: &str, until: Option<i64>, reason: Option<&str>, tags: &str, by: &str) -> Result<(), ApiError> {
    c.execute(
        "UPDATE users SET disabled = 1, block_kind = ?2, block_until = ?3, block_reason = ?4, block_tags = ?5, blocked_by_role = ?6, updated_at = ?7 WHERE id = ?1",
        params![id, kind, until, reason, tags, by, now()],
    )?;
    c.execute("DELETE FROM sessions WHERE user_id = ?1", params![id])?;
    Ok(())
}
