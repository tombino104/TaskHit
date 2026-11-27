//! Dati di contorno (materie, tipi, gruppi, impostazioni, plugin) e profilo personale.

use crate::{
    auth::{self, AuthUser},
    common::{self, now},
    error::ApiError,
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post, put},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Map, Value, json};

const SETTINGS: [&str; 4] = ["instance_name", "public_url", "school_year", "timezone"];
const PLUGINS: [&str; 1] = ["grafico"];
const MAX_PROFILE_CHARS: usize = 100_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/meta", get(meta))
        .route("/tags", post(create_tag))
        .route("/labels", post(create_label))
        .route("/settings", put(save_settings))
        .route("/plugins/{id}", put(save_plugin))
        .route("/me", get(me).patch(update_me))
        .route("/me/sessions/revoke-others", post(revoke_others))
        .route("/admin/status", get(admin_status))
}

fn rows(c: &Connection, sql: &str, f: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<Value>) -> Result<Vec<Value>, ApiError> {
    let mut stmt = c.prepare(sql)?;
    Ok(stmt.query_map([], |r| f(r))?.collect::<Result<_, _>>()?)
}

fn plugin_enabled(c: &Connection, plugin: &str, scope: &str, user_id: &str) -> Result<bool, ApiError> {
    Ok(c.query_row(
        "SELECT enabled FROM plugin_state WHERE plugin_id = ?1 AND scope = ?2 AND user_id = ?3",
        params![plugin, scope, user_id],
        |r| r.get::<_, bool>(0),
    )
    .optional()?
    .unwrap_or(true))
}

async fn meta(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    let res = st
        .db
        .run(move |c| {
            let tags = rows(c, "SELECT id, name, color FROM tags ORDER BY sort, name COLLATE NOCASE", |r| {
                Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "color": r.get::<_, String>(2)? }))
            })?;
            let labels = rows(c, "SELECT id, name, subs, personal FROM labels ORDER BY sort, name COLLATE NOCASE", |r| {
                let subs: String = r.get(2)?;
                Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?,
                           "subs": subs.split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>(), "personal": r.get::<_, bool>(3)? }))
            })?;

            // Ognuno vede i propri gruppi e i compagni che ne fanno parte. L'admin vede tutti i gruppi
            // e il rappresentante anche quelli interni alla sua classe, per gestirli. L'admin non compare mai tra i membri.
            let managed = if common::require_manager(&user).is_ok() { Some(common::managed_groups(c, &user)?) } else { None };
            let mut stmt = c.prepare(
                "SELECT g.id, g.name, g.kind, g.parent_id,
                        EXISTS (SELECT 1 FROM group_members m WHERE m.group_id = g.id AND m.user_id = ?1)
                   FROM groups g ORDER BY g.kind, g.name COLLATE NOCASE",
            )?;
            type Group = (String, String, String, Option<String>, bool);
            let found: Vec<Group> = stmt
                .query_map(params![user.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
                .collect::<Result<_, _>>()?;
            let mut groups = vec![];
            let mut members = c.prepare(
                "SELECT u.id, u.display_name, u.role FROM group_members m JOIN users u ON u.id = m.user_id
                  WHERE m.group_id = ?1 AND u.disabled = 0 AND u.pending = 0 AND u.role <> 'admin' ORDER BY u.display_name COLLATE NOCASE",
            )?;
            let mut pending = 0;
            for (id, name, kind, parent_id, mine) in found {
                let manage = managed.as_ref().is_some_and(|set| common::manages(set, &id));
                if !mine && !manage {
                    continue;
                }
                if manage {
                    pending += c.query_row("SELECT COUNT(*) FROM users WHERE pending = 1 AND pending_group_id = ?1", params![id], |r| r.get::<_, i64>(0))?;
                }
                let list: Vec<Value> = members
                    .query_map(params![id], |r| {
                        Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "role": r.get::<_, String>(2)? }))
                    })?
                    .collect::<Result<_, _>>()?;
                groups.push(json!({ "id": id, "name": name, "kind": kind, "parent_id": parent_id, "mine": mine, "manage": manage, "members": list }));
            }

            let mut settings = Map::new();
            for key in SETTINGS {
                let v: Option<String> = c.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional()?;
                settings.insert(key.into(), json!(v.unwrap_or_default()));
            }
            let mut plugins = Map::new();
            for p in PLUGINS {
                plugins.insert(p.into(), json!({ "user": plugin_enabled(c, p, "user", &user.id)?, "instance": plugin_enabled(c, p, "instance", "")? }));
            }
            Ok(json!({ "tags": tags, "labels": labels, "groups": groups, "settings": settings, "plugins": plugins, "pending": pending }))
        })
        .await?;
    Ok(Json(res))
}

#[derive(Deserialize)]
struct TagReq {
    name: String,
    color: String,
}

async fn create_tag(State(st): State<AppState>, user: AuthUser, Json(req): Json<TagReq>) -> Result<Json<Value>, ApiError> {
    common::require_representative(&user)?;
    let name = common::required_text(&req.name, 40, "il nome della materia deve avere da 1 a 40 caratteri")?;
    if crate::themes::contrast(&req.color, "#000000").is_none() {
        return Err(ApiError::BadRequest("il colore deve essere #RRGGBB"));
    }
    let id = auth::random_hex(16);
    let tid = id.clone();
    st.db
        .run(move |c| {
            if common::exists(c, "SELECT 1 FROM tags WHERE name = ?1", &name)? {
                return Err(ApiError::Conflict("esiste gia' una materia con questo nome"));
            }
            c.execute("INSERT INTO tags (id, name, color, sort) VALUES (?1, ?2, ?3, 50)", params![tid, name, req.color])?;
            common::log_change(c, "tag", &tid, "upsert", None, None, 1)
        })
        .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct LabelReq {
    name: String,
}

async fn create_label(State(st): State<AppState>, user: AuthUser, Json(req): Json<LabelReq>) -> Result<Json<Value>, ApiError> {
    common::require_representative(&user)?;
    let name = common::required_text(&req.name, 40, "il nome del tipo deve avere da 1 a 40 caratteri")?;
    let id = auth::random_hex(16);
    let lid = id.clone();
    st.db
        .run(move |c| {
            if common::exists(c, "SELECT 1 FROM labels WHERE name = ?1", &name)? {
                return Err(ApiError::Conflict("esiste gia' un tipo con questo nome"));
            }
            c.execute("INSERT INTO labels (id, name, sort) VALUES (?1, ?2, 50)", params![lid, name])?;
            common::log_change(c, "label", &lid, "upsert", None, None, 1)
        })
        .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsReq {
    instance_name: Option<String>,
    public_url: Option<String>,
    school_year: Option<String>,
    timezone: Option<String>,
}

/// Dati dell'istanza mostrati nell'interfaccia. Le impostazioni di sicurezza restano in config.toml.
async fn save_settings(State(st): State<AppState>, user: AuthUser, Json(req): Json<SettingsReq>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let mut values = vec![];
    for (key, value) in [
        ("instance_name", req.instance_name),
        ("public_url", req.public_url),
        ("school_year", req.school_year),
        ("timezone", req.timezone),
    ] {
        if let Some(v) = value {
            let v = common::optional_text(Some(v), 200, "valore troppo lungo")?.unwrap_or_default();
            if key == "public_url" && !v.is_empty() && !(v.starts_with("https://") || v.starts_with("http://")) {
                return Err(ApiError::BadRequest("l'indirizzo pubblico deve iniziare con https://"));
            }
            values.push((key, v));
        }
    }
    st.db
        .run(move |c| {
            let tx = c.transaction()?;
            for (k, v) in values {
                tx.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![k, v],
                )?;
            }
            common::audit(&tx, &user.id, "settings_changed", None)?;
            tx.commit()?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PluginReq {
    scope: String,
    enabled: bool,
}

async fn save_plugin(State(st): State<AppState>, user: AuthUser, Path(id): Path<String>, Json(req): Json<PluginReq>) -> Result<Json<Value>, ApiError> {
    if !PLUGINS.contains(&id.as_str()) {
        return Err(ApiError::NotFound);
    }
    let owner = match req.scope.as_str() {
        "user" => user.id.clone(),
        "instance" => {
            common::require_manager(&user)?;
            String::new()
        }
        _ => return Err(ApiError::BadRequest("ambito non valido")),
    };
    st.db
        .run(move |c| {
            c.execute(
                "INSERT INTO plugin_state (plugin_id, scope, user_id, enabled) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(plugin_id, scope, user_id) DO UPDATE SET enabled = excluded.enabled",
                params![id, req.scope, owner, req.enabled],
            )?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn me(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    let uid = user.id.clone();
    let (profile, sessions) = st
        .db
        .run(move |c| {
            let profile: Option<String> = c.query_row("SELECT profile_blob FROM users WHERE id = ?1", params![uid], |r| r.get(0))?;
            let sessions: i64 = c.query_row("SELECT COUNT(*) FROM sessions WHERE user_id = ?1 AND expires_at > ?2", params![uid, now()], |r| r.get(0))?;
            Ok((profile, sessions))
        })
        .await?;
    Ok(Json(json!({
        "id": user.id, "username": user.username, "display_name": user.display_name, "role": user.role,
        "profile_blob": profile, "sessions": sessions,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeReq {
    display_name: Option<String>,
    /// Dati del profilo cifrati sul dispositivo.
    profile_blob: Option<String>,
}

async fn update_me(State(st): State<AppState>, user: AuthUser, Json(req): Json<MeReq>) -> Result<Json<Value>, ApiError> {
    let name = req.display_name.map(|n| common::required_text(&n, 60, "il nome deve avere da 1 a 60 caratteri")).transpose()?;
    if req.profile_blob.as_deref().is_some_and(|b| !auth::valid_b64(b, None, MAX_PROFILE_CHARS)) {
        return Err(ApiError::BadRequest("profilo cifrato non valido"));
    }
    st.db
        .run(move |c| {
            c.execute(
                "UPDATE users SET display_name = coalesce(?2, display_name), profile_blob = coalesce(?3, profile_blob), updated_at = ?4 WHERE id = ?1",
                params![user.id, name, req.profile_blob, now()],
            )?;
            Ok(())
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn revoke_others(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    st.db
        .run(move |c| {
            c.execute("DELETE FROM sessions WHERE user_id = ?1 AND id <> ?2", params![user.id, user.session_id])?;
            common::audit(c, &user.id, "sessions_revoked", None)
        })
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Stato reale della sicurezza, letto dalla configurazione: l'interfaccia lo mostra senza inventarlo.
async fn admin_status(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let s = &st.cfg.security;
    Ok(Json(json!({
        "encrypted_at_rest": cfg!(feature = "sqlcipher"),
        "login_attempts_per_minute": s.login_attempts_per_minute_user,
        "remote_setup_blocked": !s.allow_remote_setup && !s.client_ip_header.is_empty(),
        "origin_pinned": st.cfg.server.public_url.is_some(),
    })))
}
