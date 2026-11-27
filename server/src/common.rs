//! Regole comuni alle API: ruoli, appartenenza ai gruppi, validazione e registri.
//! I permessi si applicano qui, sul server: l'interfaccia li rispecchia ma non li decide.

use crate::{auth::AuthUser, error::ApiError, ratelimit::now_secs};
use rusqlite::{Connection, params};
use std::collections::HashSet;

pub const ROLES: [&str; 4] = ["admin", "rappresentante", "nerd", "alunno"];

/// Cosa puo' fare un ruolo sugli eventi pubblici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publish {
    /// Modifica diretta (rappresentante).
    Direct,
    /// La modifica diventa una proposta da approvare (nerd).
    Propose,
    /// Nessuna modifica (alunno, e admin che non fa parte della classe).
    No,
}

pub fn publish_right(role: &str) -> Publish {
    match role {
        "rappresentante" => Publish::Direct,
        "nerd" => Publish::Propose,
        _ => Publish::No,
    }
}

pub fn require_admin(u: &AuthUser) -> Result<(), ApiError> {
    if u.role == "admin" { Ok(()) } else { Err(ApiError::Forbidden("operazione riservata all'amministratore")) }
}

/// L'admin gestisce l'impianto e non partecipa alla classe: niente eventi, materie o proposte.
pub fn require_class_member(u: &AuthUser) -> Result<(), ApiError> {
    if u.role == "admin" { Err(ApiError::Forbidden("l'amministratore non gestisce eventi: usa un account della classe")) } else { Ok(()) }
}

/// I contenuti della classe (materie, tipi) li decide il rappresentante.
pub fn require_representative(u: &AuthUser) -> Result<(), ApiError> {
    if publish_right(&u.role) == Publish::Direct { Ok(()) } else { Err(ApiError::Forbidden("operazione riservata ai rappresentanti")) }
}

/// Gruppi di cui l'utente gestisce inviti e richieste; `None` = tutti (admin).
/// Un rappresentante gestisce le classi di cui fa parte e i gruppi creati al loro interno.
pub fn managed_groups(c: &Connection, u: &AuthUser) -> Result<Option<HashSet<String>>, ApiError> {
    if u.role == "admin" {
        return Ok(None);
    }
    require_manager(u)?;
    let mut stmt = c.prepare(
        "WITH mie AS (SELECT g.id FROM groups g JOIN group_members m ON m.group_id = g.id WHERE m.user_id = ?1 AND g.kind = 'classe')
         SELECT id FROM mie UNION SELECT id FROM groups WHERE parent_id IN (SELECT id FROM mie)",
    )?;
    let ids = stmt.query_map(params![u.id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(Some(ids))
}

pub fn manages(groups: &Option<HashSet<String>>, group_id: &str) -> bool {
    groups.as_ref().is_none_or(|g| g.contains(group_id))
}

/// Admin e rappresentanti: inviti, richieste di iscrizione, gruppi e plugin.
pub fn require_manager(u: &AuthUser) -> Result<(), ApiError> {
    if u.role == "admin" || u.role == "rappresentante" { Ok(()) } else { Err(ApiError::Forbidden("operazione riservata ad admin e rappresentanti")) }
}

pub fn now() -> i64 {
    now_secs() as i64
}

/// Si accede solo ai gruppi di cui si fa parte (l'admin non ne ha).
pub fn can_access_group(c: &Connection, u: &AuthUser, group_id: &str) -> Result<bool, ApiError> {
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
