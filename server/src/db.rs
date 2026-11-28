//! SQLite: una connessione protetta da un Mutex, usata fuori dal thread asincrono.
//! Per 10-30 utenti contemporanei e' piu' che sufficiente (WAL, scritture brevi).
//! Se servisse, il passo successivo e' un pool di lettori separato dallo scrittore.

use crate::error::ApiError;
use anyhow::Context;
use rusqlite::Connection;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_calendario.sql"),
    include_str!("../migrations/003_admin_fuori_dalla_classe.sql"),
    include_str!("../migrations/004_gruppi_della_classe.sql"),
    include_str!("../migrations/005_gruppi_di_lavoro.sql"),
    include_str!("../migrations/006_nomi_foto_moderazione.sql"),
    include_str!("../migrations/007_account_eliminati.sql"),
];

/// Versione dello schema che questa build conosce.
pub const SCHEMA_VERSION: usize = MIGRATIONS.len();

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    pub fn open(path: &Path, key: Option<&[u8; 32]>) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path).with_context(|| format!("apertura di {}", path.display()))?;
        apply_key(&conn, key)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        migrate(&mut conn)?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    /// Esegue `f` su un thread dedicato, cosi' le query non bloccano le richieste.
    pub async fn run<T, F>(&self, f: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, ApiError> + Send + 'static,
    {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = db.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut guard)
        })
        .await
        .map_err(ApiError::internal)?
    }
}

#[cfg(feature = "sqlcipher")]
pub(crate) fn apply_key(conn: &Connection, key: Option<&[u8; 32]>) -> anyhow::Result<()> {
    let key = key.context("compilato con sqlcipher ma senza chiave del database")?;
    conn.pragma_update(None, "key", format!("x'{}'", hex::encode(key)))?;
    // Se la chiave e' sbagliata o il file non e' cifrato, questa query fallisce subito.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .context("il database non si apre con la chiave fornita")?;
    Ok(())
}

#[cfg(not(feature = "sqlcipher"))]
pub(crate) fn apply_key(_conn: &Connection, _key: Option<&[u8; 32]>) -> anyhow::Result<()> {
    Ok(())
}

/// Migrazioni automatiche, idempotenti e a prova di riavvio: la versione e' in `PRAGMA user_version`.
pub fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    let current: usize = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql).with_context(|| format!("migrazione {}", i + 1))?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
        tracing::info!("migrazione {} applicata", i + 1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrading_takes_the_admin_out_of_the_class() {
        let mut c = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..2] {
            c.execute_batch(sql).unwrap();
        }
        c.pragma_update(None, "user_version", 2).unwrap();
        c.execute_batch(
            "INSERT INTO users (id, username, display_name, role, auth_hash, kdf_salt, wrapped_key, created_at, updated_at)
             VALUES ('a','admin','A','admin','h','s','w',0,0), ('r','rap','R','rappresentante','h','s','w',0,0);
             INSERT INTO groups (id, name, kind, created_at) VALUES ('g','4B','classe',0);
             INSERT INTO group_members (group_id, user_id) VALUES ('g','a'), ('g','r');
             INSERT INTO events (id, visibility, owner_id, group_id, title, starts_at, subject_id, created_by, created_at, updated_by, updated_at)
             VALUES ('e','public','a','g','Compito',1,'a','a',0,'a',0);",
        )
        .unwrap();
        migrate(&mut c).unwrap();
        let one = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(one("SELECT COUNT(*) FROM group_members WHERE user_id = 'a'"), 0);
        assert_eq!(one("SELECT COUNT(*) FROM group_members WHERE user_id = 'r'"), 1);
        // L'evento gia' pubblicato resta alla classe, senza riferimento all'admin.
        assert_eq!(one("SELECT COUNT(*) FROM events WHERE id = 'e' AND subject_id IS NULL"), 1);
    }

    #[test]
    fn upgrading_merges_the_test_types() {
        let mut c = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..5] {
            c.execute_batch(sql).unwrap();
        }
        c.pragma_update(None, "user_version", 5).unwrap();
        c.execute_batch(
            "INSERT INTO users (id, username, display_name, role, auth_hash, kdf_salt, wrapped_key, disabled, created_at, updated_at)
             VALUES ('r','rap','R','rappresentante','h','s','w',0,0,0), ('x','fermo','F','alunno','h','s','w',1,0,0);
             INSERT INTO groups (id, name, kind, created_at) VALUES ('g','4B','classe',0);
             INSERT INTO events (id, visibility, owner_id, group_id, title, label_id, sub_kind, starts_at, created_by, created_at, updated_by, updated_at)
             SELECT 'a','public','r','g','A', id, 'teorica', 1,'r',0,'r',0 FROM labels WHERE name = 'Verifica';
             INSERT INTO events (id, visibility, owner_id, group_id, title, label_id, sub_kind, starts_at, created_by, created_at, updated_by, updated_at)
             SELECT 'b','public','r','g','B', id, 'pratica', 1,'r',0,'r',0 FROM labels WHERE name = 'Verifica di recupero';
             INSERT INTO proposals (id, kind, proposed_by, data, created_at)
             SELECT 'p','create','r', json_object('label_id', id, 'sub_kind', 'teorica'), 0 FROM labels WHERE name = 'Verifica di recupero';",
        )
        .unwrap();
        migrate(&mut c).unwrap();
        let one = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        let verifica = "(SELECT id FROM labels WHERE name = 'Verifica')";
        assert_eq!(one(&format!("SELECT COUNT(*) FROM events WHERE id = 'a' AND sub_kind = 'scritta' AND label_id = {verifica}")), 1);
        assert_eq!(one(&format!("SELECT COUNT(*) FROM events WHERE id = 'b' AND sub_kind = 'recupero' AND label_id = {verifica}")), 1);
        assert_eq!(one(&format!("SELECT COUNT(*) FROM proposals WHERE json_extract(data, '$.sub_kind') = 'recupero' AND json_extract(data, '$.label_id') = {verifica}")), 1);
        assert_eq!(one("SELECT COUNT(*) FROM labels WHERE hidden = 1"), 1);
        assert_eq!(one("SELECT COUNT(*) FROM users WHERE id = 'x' AND block_kind = 'sospeso'"), 1);
    }
}
