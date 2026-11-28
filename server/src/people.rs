//! Persone, gruppi e inviti. L'admin gestisce tutto; i rappresentanti inviti, richieste e gruppi della propria classe.
//! Chi riceve un codice puo' chiedere di entrare.

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
    routing::{get, patch, post, put},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

const INVITE_DAYS: i64 = 7;
const MAX_PENDING: i64 = 100;
const MAX_CLASS_GROUPS: i64 = 30;
/// Senza caratteri che si confondono (0/O, 1/I/L).
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 9;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list))
        .route("/users/{id}", patch(update))
        .route("/users/{id}/accept", post(accept))
        .route("/users/{id}/reject", post(reject))
        .route("/users/{id}/delete", post(remove))
        .route("/groups", post(create_group))
        .route("/groups/{id}/members", put(set_members))
        .route("/invites", get(invites).post(create_invite))
        .route("/invites/{id}/revoke", post(revoke_invite))
        .route("/join", post(join))
}

fn group_ids(c: &Connection, user_id: &str) -> Result<Vec<String>, ApiError> {
    let mut stmt = c.prepare("SELECT group_id FROM group_members WHERE user_id = ?1")?;
    Ok(stmt.query_map(params![user_id], |r| r.get(0))?.collect::<Result<_, _>>()?)
}

/// L'admin riceve persone e richieste; il rappresentante solo le richieste dei gruppi che gestisce.
async fn list(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    common::require_manager(&user)?;
    let res = st
        .db
        .run(move |c| {
            let managed = common::managed_groups(c, &user)?;
            let mut stmt = c.prepare(
                "SELECT id, username, display_name, role, disabled, pending, pending_group_id, created_at FROM users
                  WHERE removed_at IS NULL
                  ORDER BY CASE role WHEN 'admin' THEN 0 WHEN 'rappresentante' THEN 1 WHEN 'nerd' THEN 2 ELSE 3 END, display_name COLLATE NOCASE",
            )?;
            type Row = (String, String, String, String, bool, bool, Option<String>, i64);
            let rows: Vec<Row> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?
                .collect::<Result<_, _>>()?;
            let (mut users, mut waiting) = (vec![], vec![]);
            for (id, username, display_name, role, disabled, pending, group, created_at) in rows {
                if pending {
                    if managed.is_some() && !group.as_deref().is_some_and(|g| common::manages(&managed, g)) {
                        continue;
                    }
                    waiting.push(json!({ "id": id, "username": username, "display_name": display_name, "group_id": group, "created_at": created_at }));
                } else if managed.is_none() {
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
            if !common::exists(&tx, "SELECT 1 FROM users WHERE id = ?1 AND pending = 0 AND removed_at IS NULL", &id)? {
                return Err(ApiError::NotFound);
            }
            if let Some(role) = &req.role {
                tx.execute("UPDATE users SET role = ?2, updated_at = ?3 WHERE id = ?1", params![id, role, now()])?;
                common::audit(&tx, &user.id, "role_changed", Some(&format!("{id}:{role}")))?;
            }
            if let Some(disabled) = req.disabled {
                if disabled {
                    tx.execute(
                        "UPDATE users SET disabled = 1, block_kind = 'sospeso', block_until = NULL, blocked_by_role = 'admin', updated_at = ?2 WHERE id = ?1",
                        params![id, now()],
                    )?;
                } else {
                    crate::moderation::clear_block(&tx, &id)?;
                }
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
                // I gruppi di lavoro li compongono i rappresentanti: l'admin non li tocca.
                tx.execute(
                    "DELETE FROM group_members WHERE user_id = ?1 AND group_id IN (SELECT id FROM groups WHERE expires_at IS NULL)",
                    params![id],
                )?;
                for g in groups {
                    if !common::exists(&tx, "SELECT 1 FROM groups WHERE id = ?1", g)? {
                        return Err(ApiError::BadRequest("il gruppo scelto non esiste"));
                    }
                    if crate::work::is_work_group(&tx, g)? {
                        continue;
                    }
                    tx.execute("INSERT OR IGNORE INTO group_members (group_id, user_id) VALUES (?1, ?2)", params![g, id])?;
                }
                common::audit(&tx, &user.id, "groups_changed", Some(&id))?;
            }
            drop_orphan_memberships(&tx)?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Chi non fa parte di una classe non resta nei gruppi creati al suo interno.
pub fn drop_orphan_memberships(c: &Connection) -> Result<(), ApiError> {
    c.execute(
        "DELETE FROM group_members WHERE EXISTS (
            SELECT 1 FROM groups g WHERE g.id = group_members.group_id AND g.parent_id IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM group_members p WHERE p.group_id = g.parent_id AND p.user_id = group_members.user_id))",
        [],
    )?;
    // E chi non fa piu' parte di un gruppo non ne resta capogruppo.
    c.execute(
        "UPDATE groups SET leader_id = NULL WHERE leader_id IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM group_members m WHERE m.group_id = groups.id AND m.user_id = groups.leader_id)",
        [],
    )?;
    Ok(())
}

/// La richiesta in attesa, se chi chiede la puo' decidere. Quelle di altre classi risultano inesistenti.
fn pending_request(c: &Connection, user: &AuthUser, id: &str) -> Result<Option<String>, ApiError> {
    let group: Option<String> = c
        .query_row("SELECT pending_group_id FROM users WHERE id = ?1 AND pending = 1", params![id], |r| r.get(0))
        .optional()?
        .ok_or(ApiError::NotFound)?;
    let managed = common::managed_groups(c, user)?;
    if managed.is_some() && !group.as_deref().is_some_and(|g| common::manages(&managed, g)) {
        return Err(ApiError::NotFound);
    }
    Ok(group)
}

async fn accept(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    common::require_manager(&user)?;
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let group = pending_request(&tx, &user, &id)?;
            tx.execute("UPDATE users SET pending = 0, pending_group_id = NULL, updated_at = ?2 WHERE id = ?1", params![id, now()])?;
            if let Some(g) = group {
                tx.execute(
                    // Entrando in un gruppo di una classe si entra anche nella classe.
                    "INSERT OR IGNORE INTO group_members (group_id, user_id)
                     SELECT id, ?2 FROM groups WHERE id = ?1 OR id = (SELECT parent_id FROM groups WHERE id = ?1)",
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
    common::require_manager(&user)?;
    st.db
        .run(move |c| {
            pending_request(c, &user, &id)?;
            // Si cancellano solo le richieste non ancora accettate.
            if c.execute("DELETE FROM users WHERE id = ?1 AND pending = 1", params![id])? == 0 {
                return Err(ApiError::NotFound);
            }
            common::audit(c, &user.id, "user_rejected", Some(&id))
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Il nome che resta sugli eventi di chi non c'e' piu'. L'interfaccia lo traduce.
const REMOVED_NAME: &str = "Account eliminato";

/// L'admin elimina un account, di qualsiasi ruolo, tranne il proprio: cosi' ne resta sempre uno.
async fn remove(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    if id == user.id {
        return Err(ApiError::Forbidden("non puoi eliminare il tuo account"));
    }
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            erase(&tx, &id)?;
            common::audit(&tx, &user.id, "user_deleted", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Cancella accesso e dati personali. Restano gli eventi pubblicati per la classe; spariscono quelli
/// privati e quelli che riguardano solo questa persona.
fn erase(c: &Connection, id: &str) -> Result<(), ApiError> {
    let name: String = c
        .query_row("SELECT display_name FROM users WHERE id = ?1 AND removed_at IS NULL", params![id], |r| r.get(0))
        .optional()?
        .ok_or(ApiError::NotFound)?;

    let about: Vec<(String, String, i64)> = c
        .prepare("SELECT id, group_id, rev FROM events WHERE subject_id = ?1 AND visibility = 'public'")?
        .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (event, group, rev) in about {
        common::log_change(c, "event", &event, "delete", Some(&group), None, rev + 1)?;
        c.execute("DELETE FROM inbox WHERE event_id = ?1", params![event])?;
        c.execute("DELETE FROM events WHERE id = ?1", params![event])?;
    }

    // Proposte fatte da questa persona o che la riguardano: con le relative richieste nell'Inbox.
    let theirs = "SELECT id FROM proposals WHERE proposed_by = ?1 OR (status = 'pending' AND json_extract(data, '$.subject_id') = ?1)";
    c.execute(
        &format!("DELETE FROM inbox WHERE kind = 'proposal' AND json_extract(diff, '$.proposal_id') IN ({theirs})"),
        params![id],
    )?;
    c.execute(&format!("DELETE FROM proposals WHERE id IN ({theirs})"), params![id])?;

    c.execute("UPDATE inbox SET diff = json_set(diff, '$.approver', ?2) WHERE json_extract(diff, '$.approver') = ?1", params![name, REMOVED_NAME])?;
    for sql in [
        "DELETE FROM events WHERE owner_id = ?1 AND visibility = 'private'",
        "DELETE FROM sessions WHERE user_id = ?1",
        "DELETE FROM inbox WHERE user_id = ?1",
        "DELETE FROM reminders WHERE user_id = ?1",
        "DELETE FROM push_subscriptions WHERE user_id = ?1",
        "DELETE FROM plugin_state WHERE user_id = ?1",
        "DELETE FROM avatars WHERE user_id = ?1",
        "DELETE FROM group_members WHERE user_id = ?1",
    ] {
        c.execute(sql, params![id])?;
    }
    c.execute("UPDATE invites SET revoked_at = ?2 WHERE created_by = ?1 AND revoked_at IS NULL", params![id, now()])?;
    drop_orphan_memberships(c)?;
    // La riga resta, vuota: il nome torna libero e con queste chiavi non entra nessuno.
    c.execute(
        "UPDATE users SET username = 'eliminato:' || id, display_name = ?2, first_name = NULL, last_name = NULL, role = 'alunno',
                auth_hash = '', kdf_salt = '', wrapped_key = '', recovery_wrapped_key = NULL, profile_blob = NULL,
                rename_request = NULL, pending = 0, pending_group_id = NULL, disabled = 1,
                block_kind = NULL, block_until = NULL, block_reason = NULL, block_tags = '', blocked_by_role = NULL,
                removed_at = ?3, updated_at = ?3 WHERE id = ?1",
        params![id, REMOVED_NAME, now()],
    )?;
    Ok(())
}

#[derive(Deserialize)]
struct GroupReq {
    name: String,
    kind: String,
    /// Classe in cui nasce il gruppo: obbligatoria per i rappresentanti.
    parent_id: Option<String>,
}

/// L'admin crea classi e gruppi, senza farne parte. Il rappresentante crea gruppi dentro la propria classe.
async fn create_group(State(st): State<AppState>, user: AuthUser, Json(req): Json<GroupReq>) -> Result<Json<Value>, ApiError> {
    common::require_manager(&user)?;
    let name = common::required_text(&req.name, 40, "il nome del gruppo deve avere da 1 a 40 caratteri")?;
    if req.kind != "classe" && req.kind != "gruppo" {
        return Err(ApiError::BadRequest("il tipo deve essere classe o gruppo"));
    }
    if user.role != "admin" && (req.kind != "gruppo" || req.parent_id.is_none()) {
        return Err(ApiError::Forbidden("puoi creare solo gruppi all'interno della tua classe"));
    }
    if req.kind == "classe" && req.parent_id.is_some() {
        return Err(ApiError::BadRequest("una classe non puo' stare dentro un'altra"));
    }
    let id = auth::random_hex(16);
    let gid = id.clone();
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            if let Some(parent) = &req.parent_id {
                let managed = common::managed_groups(&tx, &user)?;
                if !common::manages(&managed, parent) {
                    return Err(ApiError::Forbidden("puoi creare solo gruppi all'interno della tua classe"));
                }
                if !common::exists(&tx, "SELECT 1 FROM groups WHERE id = ?1 AND kind = 'classe'", parent)? {
                    return Err(ApiError::BadRequest("la classe scelta non esiste"));
                }
                let n: i64 = tx.query_row("SELECT COUNT(*) FROM groups WHERE parent_id = ?1 AND expires_at IS NULL", params![parent], |r| r.get(0))?;
                if n >= MAX_CLASS_GROUPS {
                    return Err(ApiError::Conflict("questa classe ha gia' troppi gruppi"));
                }
            }
            if common::exists(&tx, "SELECT 1 FROM groups WHERE name = ?1", &name)? {
                return Err(ApiError::Conflict("esiste gia' un gruppo con questo nome"));
            }
            tx.execute(
                "INSERT INTO groups (id, name, kind, parent_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![gid, name, req.kind, req.parent_id, now()],
            )?;
            // Chi lo crea da rappresentante ne fa subito parte, per poterci pubblicare.
            if user.role != "admin" {
                tx.execute("INSERT INTO group_members (group_id, user_id) VALUES (?1, ?2)", params![gid, user.id])?;
            }
            common::audit(&tx, &user.id, "group_created", Some(&gid))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct MembersReq {
    user_ids: Vec<String>,
}

/// Sceglie chi fa parte di un gruppo interno a una classe, tra i membri di quella classe.
async fn set_members(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<MembersReq>) -> Result<Json<Value>, ApiError> {
    common::require_manager(&user)?;
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let managed = common::managed_groups(&tx, &user)?;
            if !common::manages(&managed, &id) {
                return Err(ApiError::NotFound);
            }
            let parent: Option<String> = tx
                .query_row("SELECT parent_id FROM groups WHERE id = ?1 AND expires_at IS NULL", params![id], |r| r.get(0))
                .optional()?
                .ok_or(ApiError::NotFound)?;
            let parent = parent.ok_or(ApiError::Forbidden("i membri di questo gruppo li gestisce l'admin"))?;
            tx.execute("DELETE FROM group_members WHERE group_id = ?1", params![id])?;
            for member in &req.user_ids {
                let ok = tx
                    .prepare(
                        "SELECT 1 FROM group_members m JOIN users u ON u.id = m.user_id
                          WHERE m.group_id = ?1 AND m.user_id = ?2 AND u.pending = 0 AND u.role <> 'admin'",
                    )?
                    .exists(params![parent, member])?;
                if !ok {
                    return Err(ApiError::BadRequest("nel gruppo puo' entrare solo chi fa parte della classe"));
                }
                tx.execute("INSERT OR IGNORE INTO group_members (group_id, user_id) VALUES (?1, ?2)", params![id, member])?;
            }
            common::audit(&tx, &user.id, "members_changed", Some(&id))?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
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
    common::require_manager(&user)?;
    let items = st
        .db
        .run(move |c| {
            let managed = common::managed_groups(c, &user)?;
            let mut stmt = c.prepare(
                "SELECT id, code, group_id, expires_at, uses FROM invites WHERE revoked_at IS NULL AND expires_at > ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![now()], |r| {
                Ok(json!({ "id": r.get::<_, String>(0)?, "code": r.get::<_, String>(1)?, "group_id": r.get::<_, String>(2)?,
                           "expires_at": r.get::<_, i64>(3)?, "uses": r.get::<_, i64>(4)? }))
            })?;
            let mut found = rows.collect::<Result<Vec<_>, _>>()?;
            found.retain(|i| common::manages(&managed, i["group_id"].as_str().unwrap_or("")));
            Ok(found)
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
    common::require_manager(&user)?;
    let code = new_code();
    let out = code.clone();
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            let managed = common::managed_groups(&tx, &user)?;
            if !common::manages(&managed, &req.group_id) {
                return Err(ApiError::Forbidden("puoi invitare solo nella tua classe"));
            }
            if !common::exists(&tx, "SELECT 1 FROM groups WHERE id = ?1 AND expires_at IS NULL", &req.group_id)? {
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
    common::require_manager(&user)?;
    st.db
        .run(move |c| {
            let group: Option<String> = c.query_row("SELECT group_id FROM invites WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
            let managed = common::managed_groups(c, &user)?;
            if !group.is_some_and(|g| common::manages(&managed, &g)) {
                return Err(ApiError::NotFound);
            }
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
                return Err(ApiError::Conflict(crate::meta::NAME_TAKEN));
            }
            tx.execute(
                "INSERT INTO users (id, username, display_name, role, auth_hash, kdf_salt, wrapped_key, recovery_wrapped_key, pending, pending_group_id, created_at, updated_at, first_name, last_name)
                 VALUES (?1, ?2, ?3, 'alunno', ?4, ?5, ?6, ?7, 1, ?8, ?9, ?9, ?10, ?11)",
                params![auth::random_hex(16), account.username, account.display_name, hash, account.kdf_salt, account.wrapped_key,
                        account.recovery_wrapped_key, group, now(), account.first_name, account.last_name],
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
