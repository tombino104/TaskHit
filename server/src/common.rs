//! Regole comuni alle API: ruoli, appartenenza ai gruppi, validazione e registri.
//! I permessi si applicano qui, sul server: l'interfaccia li rispecchia ma non li decide.

use crate::{auth::AuthUser, error::ApiError, ratelimit::now_secs};
use rusqlite::{Connection, params};

pub const ROLES: [&str; 4] = ["admin", "rappresentante", "nerd", "alunno"];

/// Cosa puo' fare un ruolo sugli eventi pubblici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publish {
    /// Modifica diretta (admin, rappresentante).
    Direct,
    /// La modifica diventa una proposta da approvare (nerd).
    Propose,
    /// Nessuna modifica (alunno).
    No,
}

pub fn publish_right(role: &str) -> Publish {
    match role {
        "admin" | "rappresentante" => Publish::Direct,
        "nerd" => Publish::Propose,
        _ => Publish::No,
    }
}

pub fn require_admin(u: &AuthUser) -> Result<(), ApiError> {
    if u.role == "admin" { Ok(()) } else { Err(ApiError::Forbidden("operazione riservata all'amministratore")) }
}

pub fn now() -> i64 {
    now_secs() as i64
}

/// L'admin vede tutti i gruppi; gli altri solo quelli di cui fanno parte.
pub fn can_access_group(c: &Connection, u: &AuthUser, group_id: &str) -> Result<bool, ApiError> {
    if u.role == "admin" {
        return exists(c, "SELECT 1 FROM groups WHERE id = ?1", group_id);
    }
    Ok(c.query_row(
        "SELECT EXISTS(SELECT 1 FROM group_members WHERE group_id = ?1 AND user_id = ?2)",
        params![group_id, u.id],
        |r| r.get::<_, bool>(0),
    )?)
}

pub fn exists(c: &Connection, sql: &str, id: &str) -> Result<bool, ApiError> {
    Ok(c.prepare(sql)?.exists(params![id])?)
}

/// Identificativi: 32 caratteri esadecimali minuscoli.
pub fn valid_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Testo su una riga: senza spazi ai bordi, non vuoto, entro `max` caratteri.
pub fn required_text(raw: &str, max: usize, error: &'static str) -> Result<String, ApiError> {
    let t = raw.trim();
    if t.is_empty() || t.chars().count() > max || t.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(error));
    }
    Ok(t.to_owned())
}

/// Testo facoltativo, anche su piu' righe. Vuoto equivale ad assente.
pub fn optional_text(raw: Option<String>, max: usize, error: &'static str) -> Result<Option<String>, ApiError> {
    match raw {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) if s.chars().count() > max || s.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') => {
            Err(ApiError::BadRequest(error))
        }
        Some(s) => Ok(Some(s.trim().to_owned())),
    }
}

/// Registro per la sincronizzazione incrementale.
pub fn log_change(c: &Connection, entity: &str, id: &str, op: &str, group_id: Option<&str>, user_id: Option<&str>, rev: i64) -> Result<(), ApiError> {
    c.execute(
        "INSERT INTO changes (entity, entity_id, op, group_id, user_id, rev, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![entity, id, op, group_id, user_id, rev, now()],
    )?;
    Ok(())
}

/// Registro di sicurezza: chi ha fatto cosa. Mai i contenuti degli eventi.
pub fn audit(c: &Connection, actor: &str, action: &str, detail: Option<&str>) -> Result<(), ApiError> {
    c.execute("INSERT INTO audit_log (at, actor_id, action, detail) VALUES (?1, ?2, ?3, ?4)", params![now(), actor, action, detail])?;
    Ok(())
}
