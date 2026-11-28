//! Eventi.
//!
//! Pubblici: campi in chiaro, visibili ai membri del gruppo; i nerd li modificano solo tramite proposta.
//! Privati: il server riceve e restituisce un blob cifrato (`payload`) e non ne conosce il contenuto.

use crate::{
    auth::{self, AuthUser},
    common::{self, Publish, now},
    error::ApiError,
    state::AppState,
    work,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post, put},
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const MAX_PAYLOAD_CHARS: usize = 12_000;
const MAX_PENDING_PROPOSALS: i64 = 50;
const MAX_PRIVATE_EVENTS: i64 = 2_000;
/// 1 gennaio 2100: oltre e' certamente un errore.
const MAX_TIME: i64 = 4_102_444_800;
const ALERTS: [&str; 4] = ["g1", "g2", "sera", "sett"];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/events", get(list).post(create))
        .route("/events/{id}", put(update))
        .route("/events/{id}/delete", post(delete))
        .route("/events/{id}/history", get(history))
}

/// I campi di un evento pubblico.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Data {
    pub group_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tag_id: Option<String>,
    #[serde(default)]
    pub label_id: Option<String>,
    #[serde(default)]
    pub sub_kind: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
    pub starts_at: i64,
    #[serde(default)]
    pub ends_at: Option<i64>,
    #[serde(default)]
    pub alerts: Vec<String>,
}

/// Controlla i campi e li normalizza. Si usa sia in ingresso sia quando si approva una proposta.
pub fn clean(c: &Connection, mut d: Data) -> Result<Data, ApiError> {
    d.title = common::required_text(&d.title, 120, "il titolo deve avere da 1 a 120 caratteri")?;
    d.description = common::optional_text(d.description, 4000, "la descrizione e' troppo lunga")?;
    if !(0..MAX_TIME).contains(&d.starts_at) || d.ends_at.is_some_and(|e| e <= d.starts_at || e - d.starts_at > 31 * 86_400) {
        return Err(ApiError::BadRequest("data o orario non validi: la fine deve seguire l'inizio"));
    }
    if !common::exists(c, "SELECT 1 FROM groups WHERE id = ?1", &d.group_id)? {
        return Err(ApiError::BadRequest("il gruppo scelto non esiste"));
    }
    if let Some(t) = &d.tag_id {
        if !common::exists(c, "SELECT 1 FROM tags WHERE id = ?1", t)? {
            return Err(ApiError::BadRequest("la materia scelta non esiste"));
        }
    }
    let subs: Option<String> = match &d.label_id {
        None => None,
        Some(l) => Some(
            c.query_row("SELECT subs FROM labels WHERE id = ?1", params![l], |r| r.get(0))
                .optional()?
                .ok_or(ApiError::BadRequest("il tipo scelto non esiste"))?,
        ),
    };
    if let Some(s) = &d.sub_kind {
        if !subs.as_deref().unwrap_or("").split(',').any(|x| !x.is_empty() && x == s) {
            return Err(ApiError::BadRequest("modalita' non valida per questo tipo"));
        }
    }
    if let Some(user) = &d.subject_id {
        let member = c
            .prepare("SELECT 1 FROM group_members m JOIN users u ON u.id = m.user_id
                       WHERE m.group_id = ?1 AND m.user_id = ?2 AND u.role <> 'admin'")?
            .exists(params![d.group_id, user])?;
        if !member {
            return Err(ApiError::BadRequest("l'alunno scelto non fa parte del gruppo"));
        }
    }
    if d.alerts.iter().any(|a| !ALERTS.contains(&a.as_str())) {
        return Err(ApiError::BadRequest("avviso non valido"));
    }
    // Ordine fisso e senza doppioni, cosi' il confronto tra versioni e' stabile.
    d.alerts = ALERTS.iter().filter(|a| d.alerts.iter().any(|x| x == *a)).map(|a| (*a).to_owned()).collect();
    Ok(d)
}

pub struct Stored {
    pub visibility: String,
    pub owner_id: String,
    pub rev: i64,
    pub deleted: bool,
    /// Presente solo per gli eventi pubblici.
    pub data: Option<Data>,
}

fn split_alerts(s: &str) -> Vec<String> {
    s.split(',').filter(|x| !x.is_empty()).map(str::to_owned).collect()
}

/// Legge le colonne pubbliche a partire dall'indice `at`, nell'ordine di `DATA_COLUMNS`.
fn data_from_row(r: &Row<'_>, at: usize) -> rusqlite::Result<Option<Data>> {
    let Some(group_id) = r.get::<_, Option<String>>(at)? else { return Ok(None) };
    Ok(Some(Data {
        group_id,
        title: r.get(at + 1)?,
        description: r.get(at + 2)?,
        tag_id: r.get(at + 3)?,
        label_id: r.get(at + 4)?,
        sub_kind: r.get(at + 5)?,
        subject_id: r.get(at + 6)?,
        starts_at: r.get(at + 7)?,
        ends_at: r.get(at + 8)?,
        alerts: split_alerts(&r.get::<_, String>(at + 9)?),
    }))
}

const DATA_COLUMNS: &str =
    "e.group_id, e.title, e.description, e.tag_id, e.label_id, e.sub_kind, e.subject_id, e.starts_at, e.ends_at, e.alerts";

pub fn load(c: &Connection, id: &str) -> Result<Option<Stored>, ApiError> {
    let sql = format!("SELECT e.visibility, e.owner_id, e.rev, e.deleted_at IS NOT NULL, {DATA_COLUMNS} FROM events e WHERE e.id = ?1");
    Ok(c.query_row(&sql, params![id], |r| {
        Ok(Stored { visibility: r.get(0)?, owner_id: r.get(1)?, rev: r.get(2)?, deleted: r.get(3)?, data: data_from_row(r, 4)? })
    })
    .optional()?)
}

/// Fotografia dei campi che l'Inbox mostra, cosi' resta leggibile anche se l'evento cambia o sparisce.
pub fn snapshot(d: &Data) -> Value {
    json!({ "title": d.title, "tag_id": d.tag_id, "label_id": d.label_id, "sub_kind": d.sub_kind, "group_id": d.group_id, "starts_at": d.starts_at, "subject_id": d.subject_id })
}

fn diff(old: &Data, new: &Data) -> Map<String, Value> {
    let (a, b) = (json!(old), json!(new));
    let mut out = Map::new();
    if let (Value::Object(a), Value::Object(b)) = (a, b) {
        for (k, v) in b {
            let before = a.get(&k).cloned().unwrap_or(Value::Null);
            if before != v {
                out.insert(k, json!([before, v]));
            }
        }
    }
    out
}

/// Chi ha fatto la modifica e, se era una proposta, chi l'ha approvata.
pub struct Actor<'a> {
    pub id: &'a str,
    pub approver: Option<&'a str>,
}

/// Scrive la voce di cronologia, quella di sincronizzazione e le notifiche nell'Inbox dei membri.
fn record(c: &Connection, kind: &str, event_id: &str, who: &Actor<'_>, d: &Data, old_group: Option<&str>, changes: Option<&Map<String, Value>>, rev: i64) -> Result<(), ApiError> {
    let changes_json = changes.map(|m| Value::Object(m.clone()).to_string());
    c.execute(
        "INSERT INTO event_history (event_id, at, actor_id, approver_id, kind, changes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![event_id, now(), who.id, who.approver, kind, changes_json],
    )?;
    let op = if kind == "deleted" { "delete" } else { "upsert" };
    common::log_change(c, "event", event_id, op, Some(&d.group_id), None, rev)?;
    if let Some(old) = old_group.filter(|g| *g != d.group_id) {
        common::log_change(c, "event", event_id, "delete", Some(old), None, rev)?;
    }

    let approver_name: Option<String> = match who.approver {
        Some(a) => c.query_row("SELECT display_name FROM users WHERE id = ?1", params![a], |r| r.get(0)).optional()?,
        None => None,
    };
    let payload = json!({ "event": snapshot(d), "changes": changes, "approver": approver_name }).to_string();
    c.execute(
        "INSERT INTO inbox (id, user_id, kind, event_id, actor_id, diff, created_at)
         SELECT lower(hex(randomblob(16))), u.id, ?1, ?2, ?3, ?4, ?5 FROM users u
          WHERE u.disabled = 0 AND u.pending = 0 AND u.id <> ?3 AND u.id <> ifnull(?6, '')
            AND EXISTS (SELECT 1 FROM group_members m WHERE m.user_id = u.id AND m.group_id IN (?7, ?8))",
        params![kind, event_id, who.id, payload, now(), who.approver, d.group_id, old_group.unwrap_or(&d.group_id)],
    )?;
    Ok(())
}

pub fn insert_public(c: &Connection, id: &str, d: &Data, who: &Actor<'_>) -> Result<(), ApiError> {
    c.execute(
        "INSERT INTO events (id, visibility, owner_id, group_id, title, description, tag_id, label_id, sub_kind, subject_id, starts_at, ends_at, alerts, created_by, created_at, updated_by, updated_at)
         VALUES (?1, 'public', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?2, ?13, ?2, ?13)",
        params![id, who.id, d.group_id, d.title, d.description, d.tag_id, d.label_id, d.sub_kind, d.subject_id, d.starts_at, d.ends_at, d.alerts.join(","), now()],
    )?;
    record(c, "added", id, who, d, None, None, 1)
}

/// Restituisce `false` se non cambia nulla (in quel caso non scrive niente).
pub fn update_public(c: &Connection, id: &str, rev: i64, old: &Data, d: &Data, who: &Actor<'_>) -> Result<bool, ApiError> {
    let changes = diff(old, d);
    if changes.is_empty() {
        return Ok(false);
    }
    c.execute(
        "UPDATE events SET group_id = ?2, title = ?3, description = ?4, tag_id = ?5, label_id = ?6, sub_kind = ?7, subject_id = ?8,
                starts_at = ?9, ends_at = ?10, alerts = ?11, updated_by = ?12, updated_at = ?13, rev = rev + 1 WHERE id = ?1",
        params![id, d.group_id, d.title, d.description, d.tag_id, d.label_id, d.sub_kind, d.subject_id, d.starts_at, d.ends_at, d.alerts.join(","), who.id, now()],
    )?;
    record(c, "modified", id, who, d, Some(&old.group_id), Some(&changes), rev + 1)?;
    Ok(true)
}

pub fn delete_public(c: &Connection, id: &str, rev: i64, d: &Data, who: &Actor<'_>) -> Result<(), ApiError> {
    c.execute(
        "UPDATE events SET deleted_at = ?2, updated_by = ?3, updated_at = ?2, rev = rev + 1 WHERE id = ?1",
        params![id, now(), who.id],
    )?;
    // Le proposte ancora aperte su un evento eliminato non hanno piu' senso.
    c.execute(
        "UPDATE proposals SET status = 'rejected', decided_by = ?2, decided_at = ?3 WHERE event_id = ?1 AND status = 'pending'",
        params![id, who.approver.unwrap_or(who.id), now()],
    )?;
    record(c, "deleted", id, who, d, None, None, rev + 1)
}

/// Registra una proposta e avvisa chi puo' approvarla: i rappresentanti, o il capogruppo.
fn propose(c: &Connection, u: &AuthUser, kind: &str, event_id: Option<&str>, base_rev: Option<i64>, d: &Data, note: Option<&str>) -> Result<String, ApiError> {
    let open: i64 = c.query_row("SELECT COUNT(*) FROM proposals WHERE proposed_by = ?1 AND status = 'pending'", params![u.id], |r| r.get(0))?;
    if open >= MAX_PENDING_PROPOSALS {
        return Err(ApiError::Conflict("hai troppe proposte in attesa: aspetta che vengano decise"));
    }
    let id = auth::random_hex(16);
    c.execute(
        "INSERT INTO proposals (id, kind, event_id, base_rev, proposed_by, data, note, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, kind, event_id, base_rev, u.id, json!(d).to_string(), note, now()],
    )?;
    let payload = json!({ "event": snapshot(d), "proposal_id": id, "proposal_kind": kind }).to_string();
    c.execute(
        "INSERT INTO inbox (id, user_id, kind, event_id, actor_id, diff, created_at)
         SELECT lower(hex(randomblob(16))), u.id, 'proposal', ?1, ?2, ?3, ?4 FROM users u
          WHERE u.disabled = 0 AND u.pending = 0 AND u.id <> ?2
            AND EXISTS (SELECT 1 FROM group_members m JOIN groups g ON g.id = m.group_id
                         WHERE m.user_id = u.id AND g.id = ?5
                           AND CASE WHEN g.expires_at IS NULL THEN u.role = 'rappresentante' ELSE g.leader_id = u.id END)",
        params![event_id, u.id, payload, now(), d.group_id],
    )?;
    Ok(id)
}

async fn list(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    if user.role == "admin" {
        return Ok(Json(json!({ "events": [] })));
    }
    let events = st
        .db
        .run(move |c| {
            let sql = format!(
                "SELECT e.id, e.visibility, e.owner_id, e.rev, e.created_at, e.updated_at, e.created_by,
                        cu.display_name, cu.role, uu.display_name, uu.role,
                        (SELECT a.display_name FROM users a WHERE a.id =
                            (SELECT h.approver_id FROM event_history h WHERE h.event_id = e.id ORDER BY h.id DESC LIMIT 1)),
                        EXISTS (SELECT 1 FROM proposals p WHERE p.event_id = e.id AND p.status = 'pending' AND p.proposed_by = ?1),
                        e.payload, {DATA_COLUMNS}
                   FROM events e
                   JOIN users cu ON cu.id = e.created_by
                   JOIN users uu ON uu.id = e.updated_by
                  WHERE e.deleted_at IS NULL AND (
                        (e.visibility = 'private' AND e.owner_id = ?1)
                     OR (e.visibility = 'public' AND EXISTS
                            (SELECT 1 FROM group_members m WHERE m.group_id = e.group_id AND m.user_id = ?1)))
                  ORDER BY e.starts_at, e.created_at"
            );
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(params![user.id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "visibility": r.get::<_, String>(1)?,
                    "owner_id": r.get::<_, String>(2)?,
                    "rev": r.get::<_, i64>(3)?,
                    "created_at": r.get::<_, i64>(4)?,
                    "updated_at": r.get::<_, i64>(5)?,
                    "created_by": { "id": r.get::<_, String>(6)?, "name": r.get::<_, String>(7)?, "role": r.get::<_, String>(8)? },
                    "updated_by": { "name": r.get::<_, String>(9)?, "role": r.get::<_, String>(10)? },
                    "approved_by": r.get::<_, Option<String>>(11)?,
                    "proposed": r.get::<_, bool>(12)?,
                    "payload": r.get::<_, Option<String>>(13)?,
                    "data": data_from_row(r, 14)?,
                }))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(Json(json!({ "events": events })))
}

#[derive(Deserialize)]
struct CreateReq {
    visibility: String,
    /// Solo eventi privati: l'id e' scelto dal client perche' fa parte del contesto di cifratura.
    id: Option<String>,
    data: Option<Data>,
    payload: Option<String>,
    note: Option<String>,
}

fn check_payload(payload: Option<String>) -> Result<String, ApiError> {
    payload
        .filter(|p| auth::valid_b64(p, None, MAX_PAYLOAD_CHARS))
        .ok_or(ApiError::BadRequest("contenuto cifrato mancante o non valido"))
}

async fn create(State(st): State<AppState>, user: AuthUser, Json(req): Json<CreateReq>) -> Result<Json<Value>, ApiError> {
    common::require_class_member(&user)?;
    let res = st
        .db
        .run(move |c| {
            let tx = c.transaction()?;
            let out = match req.visibility.as_str() {
                "private" => {
                    if req.data.is_some() {
                        return Err(ApiError::BadRequest("un evento privato non accetta campi in chiaro"));
                    }
                    let id = req.id.filter(|i| common::valid_id(i)).ok_or(ApiError::BadRequest("identificativo non valido"))?;
                    let payload = check_payload(req.payload)?;
                    if common::exists(&tx, "SELECT 1 FROM events WHERE id = ?1", &id)? {
                        return Err(ApiError::Conflict("esiste gia' un evento con questo identificativo"));
                    }
                    let n: i64 = tx.query_row("SELECT COUNT(*) FROM events WHERE owner_id = ?1 AND visibility = 'private' AND deleted_at IS NULL", params![user.id], |r| r.get(0))?;
                    if n >= MAX_PRIVATE_EVENTS {
                        return Err(ApiError::Conflict("hai raggiunto il numero massimo di eventi privati"));
                    }
                    tx.execute(
                        "INSERT INTO events (id, visibility, owner_id, payload, created_by, created_at, updated_by, updated_at)
                         VALUES (?1, 'private', ?2, ?3, ?2, ?4, ?2, ?4)",
                        params![id, user.id, payload, now()],
                    )?;
                    common::log_change(&tx, "event", &id, "upsert", None, Some(&user.id), 1)?;
                    json!({ "status": "saved", "id": id })
                }
                "public" => {
                    if req.payload.is_some() {
                        return Err(ApiError::BadRequest("un evento pubblico non ha contenuto cifrato"));
                    }
                    let data = clean(&tx, req.data.ok_or(ApiError::BadRequest("mancano i dati dell'evento"))?)?;
                    if !common::can_access_group(&tx, &user, &data.group_id)? {
                        return Err(ApiError::Forbidden("non fai parte di questo gruppo"));
                    }
                    let right = work::right(&tx, &user, &data.group_id)?;
                    if right == Publish::No {
                        return Err(ApiError::Forbidden(refusal(&tx, &data.group_id)?));
                    }
                    let note = common::optional_text(req.note, 300, "la nota e' troppo lunga")?;
                    if right == Publish::Direct {
                        let id = auth::random_hex(16);
                        insert_public(&tx, &id, &data, &Actor { id: &user.id, approver: None })?;
                        json!({ "status": "published", "id": id })
                    } else {
                        let id = propose(&tx, &user, "create", None, None, &data, note.as_deref())?;
                        json!({ "status": "proposed", "id": id })
                    }
                }
                _ => return Err(ApiError::BadRequest("visibilita' non valida")),
            };
            tx.commit()?;
            Ok(out)
        })
        .await?;
    Ok(Json(res))
}

#[derive(Deserialize)]
struct UpdateReq {
    /// Revisione su cui si basa la modifica: se nel frattempo e' cambiata si risponde 409.
    rev: i64,
    data: Option<Data>,
    payload: Option<String>,
    note: Option<String>,
}

/// L'evento, se l'utente puo' vederlo. Gli eventi altrui o di altri gruppi risultano inesistenti.
fn visible(c: &Connection, user: &AuthUser, id: &str) -> Result<Stored, ApiError> {
    let ev = load(c, id)?.filter(|e| !e.deleted).ok_or(ApiError::NotFound)?;
    let ok = match &ev.data {
        Some(d) => common::can_access_group(c, user, &d.group_id)?,
        None => ev.owner_id == user.id,
    };
    if ok { Ok(ev) } else { Err(ApiError::NotFound) }
}

/// Perche' non si puo' scrivere in un gruppo di cui si fa parte.
fn refusal(c: &Connection, group_id: &str) -> Result<&'static str, ApiError> {
    Ok(if work::is_work_group(c, group_id)? {
        CLOSED
    } else {
        ROLE_REFUSAL
    })
}

const CLOSED: &str = "il gruppo di lavoro e' chiuso: si puo' solo consultare";
const ROLE_REFUSAL: &str = "il tuo ruolo non permette di modificare gli eventi pubblici";
const STALE: &str = "l'evento e' stato modificato nel frattempo: ricarica e riprova";

async fn update(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<UpdateReq>) -> Result<Json<Value>, ApiError> {
    common::require_class_member(&user)?;
    let res = st
        .db
        .run(move |c| {
            let tx = c.transaction()?;
            let ev = visible(&tx, &user, &id)?;
            let out = if ev.visibility == "private" {
                if req.data.is_some() {
                    return Err(ApiError::BadRequest("la visibilita' di un evento non si puo' cambiare"));
                }
                let payload = check_payload(req.payload)?;
                if ev.rev != req.rev {
                    return Err(ApiError::Conflict(STALE));
                }
                tx.execute(
                    "UPDATE events SET payload = ?2, updated_by = ?3, updated_at = ?4, rev = rev + 1 WHERE id = ?1",
                    params![id, payload, user.id, now()],
                )?;
                common::log_change(&tx, "event", &id, "upsert", None, Some(&user.id), ev.rev + 1)?;
                "saved"
            } else {
                if req.payload.is_some() {
                    return Err(ApiError::BadRequest("la visibilita' di un evento non si puo' cambiare"));
                }
                let data = clean(&tx, req.data.ok_or(ApiError::BadRequest("mancano i dati dell'evento"))?)?;
                if !common::can_access_group(&tx, &user, &data.group_id)? {
                    return Err(ApiError::Forbidden("non fai parte di questo gruppo"));
                }
                let old = ev.data.as_ref().ok_or_else(|| ApiError::Internal("evento pubblico senza dati".into()))?;
                // Gli eventi non entrano ne' escono dai gruppi di lavoro: li vedrebbe chi non deve.
                if old.group_id != data.group_id && (work::is_work_group(&tx, &old.group_id)? || work::is_work_group(&tx, &data.group_id)?) {
                    return Err(ApiError::BadRequest("un evento non si sposta da o verso un gruppo di lavoro"));
                }
                // Servono i diritti sia sul gruppo di partenza sia su quello di arrivo.
                let right = match (work::right(&tx, &user, &old.group_id)?, work::right(&tx, &user, &data.group_id)?) {
                    (Publish::No, _) => return Err(ApiError::Forbidden(refusal(&tx, &old.group_id)?)),
                    (_, Publish::No) => return Err(ApiError::Forbidden(refusal(&tx, &data.group_id)?)),
                    (Publish::Direct, Publish::Direct) => Publish::Direct,
                    _ => Publish::Propose,
                };
                if ev.rev != req.rev {
                    return Err(ApiError::Conflict(STALE));
                }
                let note = common::optional_text(req.note, 300, "la nota e' troppo lunga")?;
                if right == Publish::Direct {
                    update_public(&tx, &id, ev.rev, old, &data, &Actor { id: &user.id, approver: None })?;
                    "published"
                } else {
                    if *old == data {
                        return Err(ApiError::BadRequest("non hai cambiato nulla"));
                    }
                    propose(&tx, &user, "update", Some(&id), Some(ev.rev), &data, note.as_deref())?;
                    "proposed"
                }
            };
            tx.commit()?;
            Ok(json!({ "status": out, "id": id }))
        })
        .await?;
    Ok(Json(res))
}

#[derive(Deserialize)]
struct DeleteReq {
    rev: i64,
    note: Option<String>,
}

async fn delete(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<DeleteReq>) -> Result<Json<Value>, ApiError> {
    common::require_class_member(&user)?;
    let res = st
        .db
        .run(move |c| {
            let tx = c.transaction()?;
            let ev = visible(&tx, &user, &id)?;
            let right = match &ev.data {
                Some(d) => match work::right(&tx, &user, &d.group_id)? {
                    Publish::No => return Err(ApiError::Forbidden(refusal(&tx, &d.group_id)?)),
                    r => r,
                },
                None => Publish::No,
            };
            if ev.rev != req.rev {
                return Err(ApiError::Conflict(STALE));
            }
            let out = match &ev.data {
                None => {
                    // Il contenuto cifrato viene cancellato; resta solo il segno per la sincronizzazione.
                    tx.execute(
                        "UPDATE events SET payload = '', deleted_at = ?2, updated_at = ?2, rev = rev + 1 WHERE id = ?1",
                        params![id, now()],
                    )?;
                    common::log_change(&tx, "event", &id, "delete", None, Some(&user.id), ev.rev + 1)?;
                    "saved"
                }
                Some(d) if right == Publish::Direct => {
                    delete_public(&tx, &id, ev.rev, d, &Actor { id: &user.id, approver: None })?;
                    "published"
                }
                Some(d) => {
                    let note = common::optional_text(req.note, 300, "la nota e' troppo lunga")?;
                    propose(&tx, &user, "delete", Some(&id), Some(ev.rev), d, note.as_deref())?;
                    "proposed"
                }
            };
            tx.commit()?;
            Ok(json!({ "status": out, "id": id }))
        })
        .await?;
    Ok(Json(res))
}

async fn history(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    common::require_class_member(&user)?;
    let items = st
        .db
        .run(move |c| {
            visible(c, &user, &id)?;
            let mut stmt = c.prepare(
                "SELECT h.at, h.kind, h.changes, a.display_name, p.display_name
                   FROM event_history h
                   LEFT JOIN users a ON a.id = h.actor_id
                   LEFT JOIN users p ON p.id = h.approver_id
                  WHERE h.event_id = ?1 ORDER BY h.id DESC LIMIT 50",
            )?;
            let rows = stmt.query_map(params![id], |r| {
                let changes = r.get::<_, Option<String>>(2)?.and_then(|s| serde_json::from_str::<Value>(&s).ok());
                Ok(json!({
                    "at": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "changes": changes,
                    "actor": r.get::<_, Option<String>>(3)?, "approver": r.get::<_, Option<String>>(4)?,
                }))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await?;
    Ok(Json(json!({ "history": items })))
}
