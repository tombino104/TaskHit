//! Proposte in attesa di approvazione (dei nerd, e dei membri dei gruppi di lavoro) e Inbox delle notifiche.

use crate::{
    auth::{self, AuthUser},
    common::{self, Publish, now},
    error::ApiError,
    events::{self, Actor, Data},
    state::AppState,
    work,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/proposals", get(list))
        .route("/proposals/{id}/decide", post(decide))
        .route("/inbox", get(inbox))
        .route("/inbox/read", post(mark_read))
}

/// Gruppi di cui l'utente fa parte (l'admin non ne ha).
fn my_groups(c: &Connection, u: &AuthUser) -> Result<HashSet<String>, ApiError> {
    let mut stmt = c.prepare("SELECT group_id FROM group_members WHERE user_id = ?1")?;
    let ids = stmt.query_map(params![u.id], |r| r.get::<_, String>(0))?.collect::<Result<HashSet<_>, _>>()?;
    Ok(ids)
}

/// Le proposte in attesa che l'utente puo' decidere (rappresentante o capogruppo), piu' le proprie.
fn pending(c: &Connection, u: &AuthUser) -> Result<Vec<Value>, ApiError> {
    let mut stmt = c.prepare(
        "SELECT p.id, p.kind, p.event_id, p.base_rev, p.data, p.note, p.created_at, a.id, a.display_name, a.role
           FROM proposals p JOIN users a ON a.id = p.proposed_by
          WHERE p.status = 'pending' ORDER BY p.created_at DESC",
    )?;
    type Row = (String, String, Option<String>, Option<i64>, String, Option<String>, i64, String, String, String);
    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut out = vec![];
    for (id, kind, event_id, base_rev, data, note, created_at, by_id, by_name, by_role) in rows {
        let Ok(data) = serde_json::from_str::<Data>(&data) else { continue };
        let current = match &event_id {
            Some(e) => events::load(c, e)?,
            None => None,
        };
        // Per decidere servono i diritti sul gruppo proposto e su quello in cui l'evento si trova ora.
        let mut can_decide = work::right(c, u, &data.group_id)? == Publish::Direct;
        if let Some(d) = current.as_ref().and_then(|e| e.data.as_ref()) {
            can_decide &= work::right(c, u, &d.group_id)? == Publish::Direct;
        }
        if !can_decide && by_id != u.id {
            continue;
        }
        let stale = current.as_ref().is_some_and(|e| e.deleted || Some(e.rev) != base_rev);
        out.push(json!({
            "id": id, "kind": kind, "event_id": event_id, "note": note, "created_at": created_at,
            "proposer": { "id": by_id, "name": by_name, "role": by_role },
            "data": data, "current": current.and_then(|e| e.data), "stale": stale, "can_decide": can_decide,
        }));
    }
    Ok(out)
}

async fn list(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    let items = st.db.run(move |c| pending(c, &user)).await?;
    Ok(Json(json!({ "proposals": items })))
}

#[derive(Deserialize)]
struct DecideReq {
    approve: bool,
}

async fn decide(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<DecideReq>) -> Result<Json<Value>, ApiError> {
    common::require_class_member(&user)?;
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            type Row = (String, Option<String>, Option<i64>, String, String, String);
            let (kind, event_id, base_rev, data, proposer, status): Row = tx
                .query_row(
                    "SELECT kind, event_id, base_rev, data, proposed_by, status FROM proposals WHERE id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .optional()?
                .ok_or(ApiError::NotFound)?;
            let data: Data = serde_json::from_str(&data).map_err(ApiError::internal)?;
            let current = match &event_id {
                Some(e) => events::load(&tx, e)?,
                None => None,
            };
            if !common::can_access_group(&tx, &user, &data.group_id)? {
                return Err(ApiError::NotFound);
            }
            if let Some(d) = current.as_ref().and_then(|e| e.data.as_ref()) {
                if !common::can_access_group(&tx, &user, &d.group_id)? {
                    return Err(ApiError::NotFound);
                }
            }
            let mut can_decide = work::right(&tx, &user, &data.group_id)? == Publish::Direct;
            if let Some(d) = current.as_ref().and_then(|e| e.data.as_ref()) {
                can_decide &= work::right(&tx, &user, &d.group_id)? == Publish::Direct;
            }
            if !can_decide {
                return Err(ApiError::Forbidden("decidono i rappresentanti, o il capogruppo nel suo gruppo"));
            }
            if status != "pending" {
                return Err(ApiError::Conflict("la proposta e' gia' stata decisa"));
            }

            let mut applied = event_id.clone();
            if req.approve {
                let who = Actor { id: &proposer, approver: Some(&user.id) };
                if kind == "create" {
                    let data = events::clean(&tx, data.clone())?;
                    let new_id = auth::random_hex(16);
                    events::insert_public(&tx, &new_id, &data, &who)?;
                    applied = Some(new_id);
                } else {
                    let (eid, ev) = event_id.as_deref().zip(current.as_ref()).ok_or(ApiError::Conflict("l'evento non esiste piu'"))?;
                    let old = ev.data.as_ref().ok_or_else(|| ApiError::Internal("evento pubblico senza dati".into()))?;
                    if ev.deleted || Some(ev.rev) != base_rev {
                        return Err(ApiError::Conflict("l'evento e' cambiato dopo la proposta: rifiutala e chiedine una nuova"));
                    }
                    if kind == "update" {
                        let data = events::clean(&tx, data.clone())?;
                        events::update_public(&tx, eid, ev.rev, old, &data, &who)?;
                    } else {
                        // Chiude prima questa proposta, cosi' l'eliminazione non la segna come rifiutata.
                        tx.execute("UPDATE proposals SET status = 'approved' WHERE id = ?1", params![id])?;
                        events::delete_public(&tx, eid, ev.rev, old, &who)?;
                    }
                }
            }

            let outcome = if req.approve { "approved" } else { "rejected" };
            tx.execute(
                "UPDATE proposals SET status = ?2, decided_by = ?3, decided_at = ?4, event_id = ?5 WHERE id = ?1",
                params![id, outcome, user.id, now(), applied],
            )?;
            // Chi ha proposto riceve l'esito.
            let payload = json!({ "event": events::snapshot(&data), "proposal_kind": kind, "outcome": outcome }).to_string();
            tx.execute(
                "INSERT INTO inbox (id, user_id, kind, event_id, actor_id, diff, created_at) VALUES (lower(hex(randomblob(16))), ?1, 'decision', ?2, ?3, ?4, ?5)",
                params![proposer, applied, user.id, payload, now()],
            )?;
            // La richiesta di approvazione e' evasa per tutti.
            tx.execute(
                "UPDATE inbox SET read_at = ?2 WHERE kind = 'proposal' AND read_at IS NULL AND diff LIKE '%' || ?1 || '%'",
                params![id, now()],
            )?;
            common::audit(&tx, &user.id, if req.approve { "proposal_approved" } else { "proposal_rejected" }, Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn inbox(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    let res = st
        .db
        .run(move |c| {
            let groups = my_groups(c, &user)?;
            let mut stmt = c.prepare(
                "SELECT i.id, i.kind, i.event_id, i.created_at, i.read_at IS NOT NULL, i.diff, a.display_name
                   FROM inbox i LEFT JOIN users a ON a.id = i.actor_id
                  WHERE i.user_id = ?1 ORDER BY i.created_at DESC, i.rowid DESC LIMIT 200",
            )?;
            type Row = (String, String, Option<String>, i64, bool, Option<String>, Option<String>);
            let rows: Vec<Row> = stmt
                .query_map(params![user.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
                .collect::<Result<_, _>>()?;
            let mut items = vec![];
            let mut unread = 0;
            for (id, kind, event_id, at, read, payload, actor) in rows {
                let payload: Value = payload.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
                // Chi e' uscito da un gruppo non ne rilegge le vecchie notifiche.
                if kind != "decision" && !groups.contains(payload["event"]["group_id"].as_str().unwrap_or("")) {
                    continue;
                }
                if !read {
                    unread += 1;
                }
                items.push(json!({ "id": id, "kind": kind, "event_id": event_id, "at": at, "read": read, "actor": actor, "payload": payload }));
            }
            let to_approve = pending(c, &user)?.iter().filter(|p| p["can_decide"] == true).count();
            Ok(json!({ "items": items, "unread": unread, "to_approve": to_approve }))
        })
        .await?;
    Ok(Json(res))
}

async fn mark_read(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    st.db
        .run(move |c| {
            c.execute("UPDATE inbox SET read_at = ?2 WHERE user_id = ?1 AND read_at IS NULL", params![user.id, now()])?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}
