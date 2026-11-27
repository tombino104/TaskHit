//! Gruppi di lavoro: li compongono i rappresentanti dentro la propria classe, durano al massimo 90 giorni
//! e hanno un capogruppo, che al loro interno decide sugli eventi. Gli eventi li vedono solo i membri.
//! Alla scadenza restano in sola lettura per 30 giorni, poi vengono eliminati con i loro eventi.

use crate::{
    auth::{self, AuthUser},
    common::{self, Publish, now},
    error::ApiError,
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{post, put},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

const DAY: i64 = 86_400;
/// Durata massima, proroghe comprese.
pub const MAX_DAYS: i64 = 90;
/// Giorni in cui un gruppo chiuso resta consultabile.
pub const GRACE_DAYS: i64 = 30;
const MAX_AT_ONCE: usize = 15;
const MAX_ACTIVE_PER_CLASS: i64 = 60;
const NOT_YOURS: &str = "i gruppi di lavoro li gestiscono i rappresentanti della classe e il capogruppo";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/work-groups", post(create))
        .route("/work-groups/{id}", put(update))
        .route("/work-groups/{id}/extend", post(extend))
        .route("/work-groups/{id}/close", post(close))
        .route("/work-groups/{id}/delete", post(delete))
}

pub fn max_expiry(created_at: i64) -> i64 {
    created_at + MAX_DAYS * DAY
}

pub fn deletion_time(expires_at: i64) -> i64 {
    expires_at + GRACE_DAYS * DAY
}

pub fn is_work_group(c: &Connection, group_id: &str) -> Result<bool, ApiError> {
    common::exists(c, "SELECT 1 FROM groups WHERE id = ?1 AND expires_at IS NOT NULL", group_id)
}

/// Il rappresentante di una classe: ha il ruolo e ne fa parte.
pub fn represents(c: &Connection, u: &AuthUser, class_id: &str) -> Result<bool, ApiError> {
    Ok(u.role == "rappresentante" && common::can_access_group(c, u, class_id)?)
}

/// Cosa puo' fare l'utente sugli eventi di un gruppo. In un gruppo di lavoro conta il capogruppo,
/// non il ruolo; negli altri conta il ruolo.
pub fn right(c: &Connection, u: &AuthUser, group_id: &str) -> Result<Publish, ApiError> {
    if u.role == "admin" || !common::can_access_group(c, u, group_id)? {
        return Ok(Publish::No);
    }
    let (expires, leader): (Option<i64>, Option<String>) =
        c.query_row("SELECT expires_at, leader_id FROM groups WHERE id = ?1", params![group_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(match expires {
        None => common::publish_right(&u.role),
        Some(end) if end <= now() => Publish::No,
        Some(_) if leader.as_deref() == Some(u.id.as_str()) => Publish::Direct,
        Some(_) => Publish::Propose,
    })
}

/// Elimina i gruppi chiusi da piu' di 30 giorni, con eventi e proposte. Restituisce quanti.
pub fn purge(c: &mut Connection, at: i64) -> Result<usize, ApiError> {
    let tx = c.transaction()?;
    let old: Vec<String> = {
        let mut stmt = tx.prepare("SELECT id FROM groups WHERE expires_at IS NOT NULL AND expires_at <= ?1")?;
        stmt.query_map(params![at - GRACE_DAYS * DAY], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for id in &old {
        remove(&tx, id)?;
        tx.execute("INSERT INTO audit_log (at, action, detail) VALUES (?1, 'work_group_expired', ?2)", params![at, id])?;
    }
    tx.commit()?;
    Ok(old.len())
}

pub async fn purge_expired(st: &AppState) -> Result<usize, ApiError> {
    st.db.run(|c| purge(c, now())).await
}

fn remove(c: &Connection, id: &str) -> Result<(), ApiError> {
    // Le proposte di nuovi eventi citano il gruppo solo nei dati: vanno tolte a mano.
    c.execute("DELETE FROM proposals WHERE json_extract(data, '$.group_id') = ?1", params![id])?;
    c.execute("DELETE FROM groups WHERE id = ?1 AND expires_at IS NOT NULL", params![id])?;
    Ok(())
}

fn reject_open_proposals(c: &Connection, group_id: &str, by: &str) -> Result<(), ApiError> {
    c.execute(
        "UPDATE proposals SET status = 'rejected', decided_by = ?2, decided_at = ?3
          WHERE status = 'pending' AND json_extract(data, '$.group_id') = ?1",
        params![group_id, by, now()],
    )?;
    Ok(())
}

struct Group {
    class_id: String,
    created_at: i64,
    expires_at: i64,
    /// Chi chiede e' rappresentante della classe (altrimenti e' il capogruppo).
    representative: bool,
}

/// Il gruppo, se chi chiede lo puo' gestire. A chi non ne ha a che fare risulta inesistente.
fn manageable(c: &Connection, u: &AuthUser, id: &str) -> Result<Group, ApiError> {
    common::require_class_member(u)?;
    let (class_id, created_at, expires_at, leader): (String, i64, i64, Option<String>) = c
        .query_row(
            "SELECT parent_id, created_at, expires_at, leader_id FROM groups WHERE id = ?1 AND expires_at IS NOT NULL",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
        .ok_or(ApiError::NotFound)?;
    let representative = represents(c, u, &class_id)?;
    let member = common::can_access_group(c, u, id)?;
    if representative || (member && leader.as_deref() == Some(u.id.as_str())) {
        return Ok(Group { class_id, created_at, expires_at, representative });
    }
    Err(if member { ApiError::Forbidden(NOT_YOURS) } else { ApiError::NotFound })
}

fn still_open(g: &Group) -> Result<(), ApiError> {
    if g.expires_at <= now() { Err(ApiError::Conflict("il gruppo di lavoro e' chiuso")) } else { Ok(()) }
}

/// Sostituisce i membri: solo compagni della classe, e il capogruppo deve essere uno di loro.
fn put_members(c: &Connection, class_id: &str, group_id: &str, members: &[String], leader: &str) -> Result<(), ApiError> {
    if members.is_empty() {
        return Err(ApiError::BadRequest("ogni gruppo ha bisogno di almeno una persona"));
    }
    if !members.iter().any(|m| m == leader) {
        return Err(ApiError::BadRequest("il capogruppo deve far parte del gruppo"));
    }
    c.execute("DELETE FROM group_members WHERE group_id = ?1", params![group_id])?;
    for member in members {
        let ok = c
            .prepare(
                "SELECT 1 FROM group_members m JOIN users u ON u.id = m.user_id
                  WHERE m.group_id = ?1 AND m.user_id = ?2 AND u.pending = 0 AND u.disabled = 0 AND u.role <> 'admin'",
            )?
            .exists(params![class_id, member])?;
        if !ok {
            return Err(ApiError::BadRequest("nel gruppo puo' entrare solo chi fa parte della classe"));
        }
        c.execute("INSERT OR IGNORE INTO group_members (group_id, user_id) VALUES (?1, ?2)", params![group_id, member])?;
    }
    c.execute("UPDATE groups SET leader_id = ?2 WHERE id = ?1", params![group_id, leader])?;
    Ok(())
}

/// I nomi dei gruppi sono unici: se serve si aggiunge un numero.
fn free_name(c: &Connection, base: &str) -> Result<String, ApiError> {
    let mut name = base.to_owned();
    let mut n = 1;
    while common::exists(c, "SELECT 1 FROM groups WHERE name = ?1", &name)? {
        n += 1;
        name = format!("{base} ({n})");
    }
    Ok(name)
}

#[derive(Deserialize)]
struct NewGroup {
    name: String,
    member_ids: Vec<String>,
    leader_id: String,
}

#[derive(Deserialize)]
struct CreateReq {
    class_id: String,
    activity: String,
    days: i64,
    groups: Vec<NewGroup>,
}

/// Crea insieme tutti i gruppi di un lavoro: o nascono tutti, o nessuno.
async fn create(State(st): State<AppState>, user: AuthUser, Json(req): Json<CreateReq>) -> Result<Json<Value>, ApiError> {
    common::require_class_member(&user)?;
    let activity = common::required_text(&req.activity, 60, "il nome del lavoro deve avere da 1 a 60 caratteri")?;
    if !(1..=MAX_DAYS).contains(&req.days) {
        return Err(ApiError::BadRequest("la durata va da 1 a 90 giorni"));
    }
    if req.groups.is_empty() || req.groups.len() > MAX_AT_ONCE {
        return Err(ApiError::BadRequest("si creano da 1 a 15 gruppi alla volta"));
    }
    // In uno stesso lavoro nessuno sta in due gruppi.
    let mut seen = HashSet::new();
    for g in &req.groups {
        for m in g.member_ids.iter().collect::<HashSet<_>>() {
            if !seen.insert(m.clone()) {
                return Err(ApiError::BadRequest("una persona non puo' stare in due gruppi dello stesso lavoro"));
            }
        }
    }
    let ids = st
        .db
        .run(move |c| {
            let tx = c.transaction()?;
            if !represents(&tx, &user, &req.class_id)? {
                return Err(ApiError::Forbidden("i gruppi di lavoro li creano i rappresentanti della classe"));
            }
            if !common::exists(&tx, "SELECT 1 FROM groups WHERE id = ?1 AND kind = 'classe'", &req.class_id)? {
                return Err(ApiError::BadRequest("la classe scelta non esiste"));
            }
            let active: i64 = tx.query_row(
                "SELECT COUNT(*) FROM groups WHERE parent_id = ?1 AND expires_at > ?2",
                params![req.class_id, now()],
                |r| r.get(0),
            )?;
            if active + req.groups.len() as i64 > MAX_ACTIVE_PER_CLASS {
                return Err(ApiError::Conflict("la classe ha troppi gruppi di lavoro aperti: chiudine qualcuno"));
            }
            let mut ids = vec![];
            for g in &req.groups {
                let name = common::required_text(&g.name, 40, "il nome del gruppo deve avere da 1 a 40 caratteri")?;
                let id = auth::random_hex(16);
                let created = now();
                tx.execute(
                    "INSERT INTO groups (id, name, kind, parent_id, created_at, expires_at, activity) VALUES (?1, ?2, 'gruppo', ?3, ?4, ?5, ?6)",
                    params![id, free_name(&tx, &format!("{name} · {activity}"))?, req.class_id, created, created + req.days * DAY, activity],
                )?;
                put_members(&tx, &req.class_id, &id, &g.member_ids, &g.leader_id)?;
                common::audit(&tx, &user.id, "work_group_created", Some(&id))?;
                ids.push(id);
            }
            tx.commit()?;
            Ok(ids)
        })
        .await?;
    Ok(Json(json!({ "ids": ids })))
}

#[derive(Deserialize)]
struct UpdateReq {
    member_ids: Vec<String>,
    leader_id: String,
}

/// Membri e capogruppo: li cambiano il rappresentante e il capogruppo, che puo' passare il ruolo.
async fn update(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<UpdateReq>) -> Result<Json<Value>, ApiError> {
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let g = manageable(&tx, &user, &id)?;
            still_open(&g)?;
            put_members(&tx, &g.class_id, &id, &req.member_ids, &req.leader_id)?;
            // Chi esce non ha piu' proposte in sospeso nel gruppo.
            tx.execute(
                "UPDATE proposals SET status = 'rejected', decided_by = ?2, decided_at = ?3
                  WHERE status = 'pending' AND json_extract(data, '$.group_id') = ?1
                    AND proposed_by NOT IN (SELECT user_id FROM group_members WHERE group_id = ?1)",
                params![id, user.id, now()],
            )?;
            common::audit(&tx, &user.id, "work_group_changed", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ExtendReq {
    days: i64,
}

async fn extend(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<ExtendReq>) -> Result<Json<Value>, ApiError> {
    if !(1..=MAX_DAYS).contains(&req.days) {
        return Err(ApiError::BadRequest("la proroga va da 1 a 90 giorni"));
    }
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let g = manageable(&tx, &user, &id)?;
            still_open(&g)?;
            let end = g.expires_at + req.days * DAY;
            if end > max_expiry(g.created_at) {
                return Err(ApiError::BadRequest("un gruppo di lavoro dura al massimo 90 giorni in tutto"));
            }
            tx.execute("UPDATE groups SET expires_at = ?2 WHERE id = ?1", params![id, end])?;
            common::audit(&tx, &user.id, "work_group_extended", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Chiusura anticipata: da subito sola lettura, e tra 30 giorni l'eliminazione.
async fn close(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let g = manageable(&tx, &user, &id)?;
            still_open(&g)?;
            tx.execute("UPDATE groups SET expires_at = ?2 WHERE id = ?1", params![id, now()])?;
            reject_open_proposals(&tx, &id, &user.id)?;
            common::audit(&tx, &user.id, "work_group_closed", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Eliminazione immediata (un gruppo creato per sbaglio): solo il rappresentante.
async fn delete(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            if !manageable(&tx, &user, &id)?.representative {
                return Err(ApiError::Forbidden("solo un rappresentante puo' eliminare un gruppo di lavoro"));
            }
            remove(&tx, &id)?;
            common::audit(&tx, &user.id, "work_group_deleted", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}
