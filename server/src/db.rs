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
];

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
fn apply_key(conn: &Connection, key: Option<&[u8; 32]>) -> anyhow::Result<()> {
    let key = key.context("compilato con sqlcipher ma senza chiave del database")?;
    conn.pragma_update(None, "key", format!("x'{}'", hex::encode(key)))?;
    // Se la chiave e' sbagliata o il file non e' cifrato, questa query fallisce subito.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .context("il database non si apre con la chiave fornita")?;
    Ok(())
}

#[cfg(not(feature = "sqlcipher"))]
fn apply_key(_conn: &Connection, _key: Option<&[u8; 32]>) -> anyhow::Result<()> {
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
