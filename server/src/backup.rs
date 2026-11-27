//! Backup e ripristino.
//!
//! Un backup e' un archivio tar che basta a se stesso: una copia coerente del database, la chiave che
//! lo apre (se cifrato) e i temi. Chi ha il file ha i dati: gli eventi privati restano comunque
//! cifrati dal dispositivo di chi li ha scritti.
//!
//! L'elenco dei backup e' il contenuto della cartella, non una tabella: sopravvive a un ripristino
//! e a una reinstallazione.

use crate::{
    api,
    auth::{self, AuthUser},
    common::{self, now},
    config::Config,
    db::{self, Db},
    error::ApiError,
    state::{AppState, ClientIp},
};
use anyhow::Context;
use axum::{
    Extension, Json, Router,
    body::{Body, Bytes},
    extract::{Path as UrlPath, Request, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use chrono::{DateTime, Datelike, Duration as Days, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension, backup::StepResult, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Dimensione massima di un archivio caricato. Il database di una classe e' di pochi MB.
pub const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;
const FORMAT: u32 = 1;
const MAX_KEEP: u32 = 90;
const MAX_THEME_BYTES: u64 = 64 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/backups", get(overview).post(run_now))
        .route("/backups/settings", put(save_settings))
        .route("/backups/{name}", get(download))
        .route("/backups/{name}/delete", post(delete))
        .route("/backups/{name}/restore", post(restore_stored))
}

/// Le sole rotte che accettano file grandi.
pub fn upload_routes() -> Router<AppState> {
    Router::new().route("/backups/upload", post(upload)).route("/setup/restore", post(restore_at_setup))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Auto,
    Manuale,
    Caricato,
    /// Copia fatta da sola subito prima di un ripristino.
    Sicurezza,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self { Kind::Auto => "auto", Kind::Manuale => "manuale", Kind::Caricato => "caricato", Kind::Sicurezza => "sicurezza" }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub name: String,
    pub kind: Kind,
    pub created_at: i64,
    pub size: u64,
}

/// `taskhit-AAAAMMGG-HHMMSS-tipo-xxxx.tar` (ora UTC). Qualsiasi altro nome viene ignorato,
/// quindi un nome arrivato da fuori non puo' uscire dalla cartella.
fn parse_name(name: &str) -> Option<(Kind, i64)> {
    let rest = name.strip_prefix("taskhit-")?.strip_suffix(".tar")?;
    let mut parts = rest.split('-');
    let (day, time, tag, id) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || id.len() != 4 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let kind = [Kind::Auto, Kind::Manuale, Kind::Caricato, Kind::Sicurezza].into_iter().find(|k| k.tag() == tag)?;
    let at = NaiveDateTime::parse_from_str(&format!("{day}{time}"), "%Y%m%d%H%M%S").ok()?;
    Some((kind, at.and_utc().timestamp()))
}

fn new_name(kind: Kind, at: i64) -> String {
    let when = DateTime::<Utc>::from_timestamp(at, 0).unwrap_or_default();
    format!("taskhit-{}-{}-{}.tar", when.format("%Y%m%d-%H%M%S"), kind.tag(), auth::random_hex(2))
}

/// Dal piu' recente al piu' vecchio.
pub fn list(dir: &Path) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut items: Vec<Item> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let (kind, created_at) = parse_name(&name)?;
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            Some(Item { name, kind, created_at, size: meta.len() })
        })
        .collect();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.name.cmp(&a.name)));
    items
}

fn prune(dir: &Path, keep: u32) {
    for old in list(dir).into_iter().skip(keep as usize) {
        if let Err(e) = fs::remove_file(dir.join(&old.name)) {
            tracing::warn!("impossibile eliminare il backup {}: {e}", old.name);
        }
    }
}

// --- Impostazioni e calendario ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    /// Giorni della settimana, 1 = lunedi' ... 7 = domenica.
    pub days: Vec<u8>,
    /// "HH:MM" nel fuso orario dell'istanza.
    pub time: String,
    /// Quanti backup conservare.
    pub keep: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { enabled: true, days: (1..=7).collect(), time: "03:00".into(), keep: 14 }
    }
}

impl Settings {
    fn checked(mut self) -> Result<Self, ApiError> {
        self.days.sort_unstable();
        self.days.dedup();
        if self.days.is_empty() || self.days.iter().any(|d| !(1..=7).contains(d)) {
            return Err(ApiError::BadRequest("scegli almeno un giorno"));
        }
        if self.time.len() != 5 || NaiveTime::parse_from_str(&self.time, "%H:%M").is_err() {
            return Err(ApiError::BadRequest("orario non valido (usa HH:MM)"));
        }
        if !(1..=MAX_KEEP).contains(&self.keep) {
            return Err(ApiError::BadRequest("si possono conservare da 1 a 90 backup"));
        }
        Ok(self)
    }

    /// Le occorrenze programmate intorno a `now`: l'ultima passata e la prossima.
    pub fn occurrences(&self, tz: Tz, now: i64) -> (Option<i64>, Option<i64>) {
        let (Ok(time), Some(here)) = (NaiveTime::parse_from_str(&self.time, "%H:%M"), DateTime::<Utc>::from_timestamp(now, 0)) else {
            return (None, None);
        };
        let today = here.with_timezone(&tz).date_naive();
        let (mut last, mut next) = (None, None);
        for offset in -8..=8 {
            let day = today + Days::days(offset);
            if !self.days.contains(&(day.weekday().number_from_monday() as u8)) {
                continue;
            }
            let local = day.and_time(time);
            // Con l'ora legale un orario puo' non esistere: si passa all'ora successiva.
            let Some(at) = tz.from_local_datetime(&local).earliest().or_else(|| tz.from_local_datetime(&(local + Days::hours(1))).earliest()) else {
                continue;
            };
            let at = at.timestamp();
            if at <= now {
                last = Some(at);
            } else if next.is_none() {
                next = Some(at);
            }
        }
        (last, next)
    }
}

fn setting(c: &Connection, key: &str) -> Result<Option<String>, ApiError> {
    Ok(c.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional()?)
}

fn put_setting(c: &Connection, key: &str, value: &str) -> Result<(), ApiError> {
    c.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

struct Schedule {
    settings: Settings,
    tz: Tz,
    users: i64,
    last_error: Option<String>,
}

async fn schedule(db: &Db) -> Result<Schedule, ApiError> {
    db.run(|c| {
        let settings = setting(c, "backup")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let tz = setting(c, "timezone")?.and_then(|s| s.parse().ok()).unwrap_or(chrono_tz::Europe::Rome);
        let users = c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        let last_error = setting(c, "backup_last_error")?.filter(|s| !s.is_empty());
        Ok(Schedule { settings, tz, users, last_error })
    })
    .await
}

// --- Creazione -------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Manifest {
    app: String,
    format: u32,
    version: String,
    schema: usize,
    encrypted: bool,
    created_at: i64,
}

/// Rimuove le cartelle di lavoro rimaste da un arresto improvviso: possono contenere la chiave.
pub fn clean_leftovers(data_dir: &Path) {
    let Ok(entries) = fs::read_dir(data_dir) else { return };
    for e in entries.flatten().filter(|e| e.file_name().to_string_lossy().starts_with(".lavoro-")) {
        let _ = fs::remove_dir_all(e.path());
    }
}

/// Cartella di lavoro privata, rimossa in ogni caso.
struct Staging(PathBuf);

impl Staging {
    fn new(cfg: &Config) -> anyhow::Result<Self> {
        let dir = cfg.paths.data_dir.join(format!(".lavoro-{}", auth::random_hex(8)));
        private_dir(&dir)?;
        Ok(Staging(dir))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn private_dir(dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creazione di {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Scrive in un file provvisorio e poi rinomina: un backup a meta' non compare mai nell'elenco.
fn write_file(dir: &Path, name: &str, fill: impl FnOnce(&mut fs::File) -> anyhow::Result<()>) -> anyhow::Result<()> {
    private_dir(dir)?;
    let tmp = dir.join(format!(".{name}.tmp"));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&tmp)?;
        fill(&mut f)?;
        f.sync_all()?;
        fs::rename(&tmp, dir.join(name))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn append(tar: &mut tar::Builder<&mut fs::File>, name: &str, data: &[u8]) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o600);
    header.set_mtime(now() as u64);
    tar.append_data(&mut header, name, data)
}

/// Crea un archivio completo. Il database e' copiato con `VACUUM INTO`: coerente anche mentre l'app lavora.
async fn create(st: &AppState, kind: Kind) -> Result<Item, ApiError> {
    let staging = Staging::new(&st.cfg)?;
    let snapshot = staging.0.join("taskhit.db");
    let target = snapshot.to_str().context("percorso non valido")?.to_owned();
    st.db.run(move |c| { c.execute("VACUUM INTO ?1", params![target])?; Ok(()) }).await?;

    let cfg = st.cfg.clone();
    let item = tokio::task::spawn_blocking(move || -> anyhow::Result<Item> {
        let _staging = staging;
        let created_at = now();
        let name = new_name(kind, created_at);
        let manifest = Manifest {
            app: "taskhit".into(),
            format: FORMAT,
            version: env!("CARGO_PKG_VERSION").into(),
            schema: db::SCHEMA_VERSION,
            encrypted: cfg!(feature = "sqlcipher"),
            created_at,
        };
        let dir = cfg.backups_dir();
        write_file(&dir, &name, |file| {
            let mut tar = tar::Builder::new(file);
            append(&mut tar, "manifest.json", &serde_json::to_vec_pretty(&manifest)?)?;
            append(&mut tar, "taskhit.db", &fs::read(&snapshot)?)?;
            if manifest.encrypted {
                append(&mut tar, "db.key", &fs::read(cfg.secrets_dir().join("db.key"))?)?;
            }
            if let Ok(themes) = fs::read_dir(cfg.themes_dir()) {
                for t in themes.flatten() {
                    let Ok(file_name) = t.file_name().into_string() else { continue };
                    if safe_theme_name(&file_name) && t.metadata().is_ok_and(|m| m.is_file() && m.len() <= MAX_THEME_BYTES) {
                        append(&mut tar, &format!("themes/{file_name}"), &fs::read(t.path())?)?;
                    }
                }
            }
            tar.finish()?;
            Ok(())
        })?;
        let size = fs::metadata(dir.join(&name))?.len();
        Ok(Item { name, kind, created_at, size })
    })
    .await
    .map_err(ApiError::internal)??;

    copy_outside(&st.cfg, &item.name).await;
    Ok(item)
}

/// Seconda copia in una cartella scelta da chi gestisce il server (su Umbrel: una cartella di Files),
/// che resta anche se l'app viene disinstallata. Se la cartella non c'e', non succede nulla.
async fn copy_outside(cfg: &Config, name: &str) {
    let Some(copy_dir) = cfg.paths.backup_copy_dir.clone().filter(|d| d.is_dir()) else { return };
    let source = cfg.backups_dir().join(name);
    let name = name.to_owned();
    let done = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let tmp = copy_dir.join(format!(".{name}.tmp"));
        fs::copy(&source, &tmp)?;
        fs::rename(&tmp, copy_dir.join(&name))
    })
    .await;
    match done {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("copia esterna del backup non riuscita: {e}"),
        Err(e) => tracing::warn!("copia esterna del backup non riuscita: {e}"),
    }
}

/// La cartella esterna non viene mai sfoltita da qui: quei file sono di chi gestisce il server.
fn copy_status(cfg: &Config, newest: Option<&Item>) -> Value {
    let dir = cfg.paths.backup_copy_dir.as_ref().filter(|d| d.is_dir());
    json!({
        "active": dir.is_some(),
        "latest_copied": dir.zip(newest).is_some_and(|(d, i)| d.join(&i.name).is_file()),
    })
}

/// Esegue il backup automatico se l'ultimo orario programmato non ha ancora il suo.
/// Vale anche dopo un periodo a server spento.
pub async fn run_due(st: &AppState) -> Result<Option<Item>, ApiError> {
    let _guard = st.backup_lock.lock().await;
    let plan = schedule(&st.db).await?;
    if !plan.settings.enabled || plan.users == 0 {
        return Ok(None);
    }
    let Some(due) = plan.settings.occurrences(plan.tz, now()).0 else { return Ok(None) };
    if list(&st.cfg.backups_dir()).iter().any(|i| i.kind == Kind::Auto && i.created_at >= due) {
        return Ok(None);
    }
    match create(st, Kind::Auto).await {
        Ok(item) => {
            prune(&st.cfg.backups_dir(), plan.settings.keep);
            st.db.run(|c| put_setting(c, "backup_last_error", "")).await?;
            Ok(Some(item))
        }
        Err(e) => {
            let message = "L'ultimo backup automatico non è riuscito: controlla spazio su disco e log del server.";
            st.db.run(move |c| put_setting(c, "backup_last_error", message)).await?;
            Err(e)
        }
    }
}

// --- Ripristino ------------------------------------------------------------------------------

fn safe_theme_name(name: &str) -> bool {
    name.len() <= 80
        && name.ends_with(".json")
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Un archivio controllato e pronto: il database e' gia' aperto e portato allo schema attuale.
struct Opened {
    source: Connection,
    themes: Vec<(String, Vec<u8>)>,
    _staging: Staging,
}

const NOT_A_BACKUP: ApiError = ApiError::BadRequest("il file non e' un backup di TaskHit valido");

/// Legge solo le voci previste, senza mai estrarre percorsi presi dall'archivio.
fn open_archive(cfg: &Config, bytes: &[u8]) -> Result<Opened, ApiError> {
    let (mut manifest, mut database, mut key, mut themes) = (None, None, None, vec![]);
    let mut archive = tar::Archive::new(bytes);
    for entry in archive.entries().map_err(|_| NOT_A_BACKUP)? {
        let mut entry = entry.map_err(|_| NOT_A_BACKUP)?;
        if !entry.header().entry_type().is_file() || entry.size() > MAX_UPLOAD_BYTES as u64 {
            return Err(NOT_A_BACKUP);
        }
        let path = entry.path().map_err(|_| NOT_A_BACKUP)?.to_string_lossy().into_owned();
        let mut data = vec![];
        entry.read_to_end(&mut data).map_err(|_| NOT_A_BACKUP)?;
        match path.as_str() {
            "manifest.json" => manifest = serde_json::from_slice::<Manifest>(&data).ok(),
            "taskhit.db" => database = Some(data),
            "db.key" => key = Some(data),
            other => {
                if let Some(name) = other.strip_prefix("themes/").filter(|n| safe_theme_name(n)) {
                    if data.len() as u64 <= MAX_THEME_BYTES && themes.len() < 200 {
                        themes.push((name.to_owned(), data));
                    }
                }
            }
        }
    }
    let (Some(manifest), Some(database)) = (manifest, database) else { return Err(NOT_A_BACKUP) };
    if manifest.app != "taskhit" {
        return Err(NOT_A_BACKUP);
    }
    if manifest.format > FORMAT || manifest.schema > db::SCHEMA_VERSION {
        return Err(ApiError::BadRequest("il backup viene da una versione piu' recente di TaskHit: aggiorna prima l'app"));
    }
    if manifest.encrypted != cfg!(feature = "sqlcipher") {
        return Err(ApiError::BadRequest("il backup viene da un'installazione con un tipo di cifratura diverso"));
    }
    let key: Option<[u8; 32]> = if manifest.encrypted {
        let text = key.and_then(|k| String::from_utf8(k).ok()).ok_or(NOT_A_BACKUP)?;
        Some(hex::decode(text.trim()).ok().and_then(|b| b.try_into().ok()).ok_or(NOT_A_BACKUP)?)
    } else {
        None
    };

    let staging = Staging::new(cfg)?;
    let path = staging.0.join("taskhit.db");
    fs::File::create(&path).and_then(|mut f| f.write_all(&database)).map_err(ApiError::internal)?;
    let mut source = Connection::open(&path)?;
    db::apply_key(&source, key.as_ref()).map_err(|_| NOT_A_BACKUP)?;
    let sound: String = source.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|_| NOT_A_BACKUP)?;
    let schema: i64 = source.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|_| NOT_A_BACKUP)?;
    if sound != "ok" || schema < 1 || schema as usize > db::SCHEMA_VERSION {
        return Err(NOT_A_BACKUP);
    }
    db::migrate(&mut source).map_err(|_| NOT_A_BACKUP)?;
    let admins: i64 = source
        .query_row("SELECT COUNT(*) FROM users WHERE role = 'admin' AND disabled = 0 AND pending = 0", [], |r| r.get(0))
        .map_err(|_| NOT_A_BACKUP)?;
    if admins == 0 {
        return Err(ApiError::BadRequest("il backup non contiene un amministratore: non si puo' ripristinare"));
    }
    Ok(Opened { source, themes, _staging: staging })
}

/// Sostituisce il contenuto del database con quello del backup, in un'unica transazione.
/// La chiave del database di questa installazione resta la stessa. Tutti dovranno accedere di nuovo.
async fn apply(st: &AppState, opened: Opened, only_if_empty: bool) -> Result<(), ApiError> {
    let themes_dir = st.cfg.themes_dir();
    st.db
        .run(move |live| {
            if only_if_empty {
                // Stesso blocco della copia: nel frattempo nessuno puo' aver creato un admin.
                let n: i64 = live.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
                if n > 0 {
                    return Err(ApiError::Conflict("la configurazione iniziale e' gia' stata completata"));
                }
            }
            {
                let copy = rusqlite::backup::Backup::new(&opened.source, live)?;
                if copy.step(-1)? != StepResult::Done {
                    return Err(ApiError::Internal("ripristino non completato".into()));
                }
            }
            live.execute("DELETE FROM sessions", [])?;
            live.execute("INSERT INTO audit_log (at, action) VALUES (?1, 'backup_restored')", params![now()])?;
            fs::create_dir_all(&themes_dir).map_err(ApiError::internal)?;
            for (name, data) in &opened.themes {
                fs::write(themes_dir.join(name), data).map_err(ApiError::internal)?;
            }
            // Un backup vecchio puo' riportare temi nel formato precedente.
            crate::themes::prepare_dir(&themes_dir).map_err(ApiError::internal)?;
            Ok(())
        })
        .await
}

async fn open_blocking(st: &AppState, bytes: Bytes) -> Result<Opened, ApiError> {
    let cfg = st.cfg.clone();
    tokio::task::spawn_blocking(move || open_archive(&cfg, &bytes)).await.map_err(ApiError::internal)?
}

// --- API -------------------------------------------------------------------------------------

async fn overview(State(st): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let plan = schedule(&st.db).await?;
    let items = list(&st.cfg.backups_dir());
    let next = plan.settings.enabled.then(|| plan.settings.occurrences(plan.tz, now()).1).flatten();
    Ok(Json(json!({
        "settings": plan.settings,
        "timezone": plan.tz.name(),
        "next": next,
        "error": plan.last_error,
        "copy": copy_status(&st.cfg, items.first()),
        "items": items,
        "dir": st.cfg.backups_dir().display().to_string(),
    })))
}

async fn save_settings(State(st): State<AppState>, user: AuthUser, Json(req): Json<Settings>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let settings = req.checked()?;
    let text = serde_json::to_string(&settings).map_err(ApiError::internal)?;
    let _guard = st.backup_lock.lock().await;
    st.db
        .run(move |c| {
            put_setting(c, "backup", &text)?;
            common::audit(c, &user.id, "backup_settings_changed", None)
        })
        .await?;
    // Niente eliminazioni qui: un limite abbassato per sbaglio vale solo dal prossimo backup.
    Ok(Json(json!({ "ok": true })))
}

async fn run_now(State(st): State<AppState>, user: AuthUser) -> Result<Json<Item>, ApiError> {
    common::require_admin(&user)?;
    if !st.limiter.hit(&format!("backup:{}", user.id), 6, 60) {
        return Err(ApiError::TooManyRequests);
    }
    let _guard = st.backup_lock.lock().await;
    let item = create(&st, Kind::Manuale).await?;
    let keep = schedule(&st.db).await?.settings.keep;
    prune(&st.cfg.backups_dir(), keep);
    st.db.run(move |c| { put_setting(c, "backup_last_error", "")?; common::audit(c, &user.id, "backup_created", None) }).await?;
    Ok(Json(item))
}

fn stored(st: &AppState, name: &str) -> Result<PathBuf, ApiError> {
    parse_name(name).ok_or(ApiError::NotFound)?;
    let path = st.cfg.backups_dir().join(name);
    if path.is_file() { Ok(path) } else { Err(ApiError::NotFound) }
}

async fn download(State(st): State<AppState>, user: AuthUser, UrlPath(name): UrlPath<String>) -> Result<Response, ApiError> {
    common::require_admin(&user)?;
    let path = stored(&st, &name)?;
    let bytes = tokio::fs::read(path).await.map_err(ApiError::internal)?;
    st.db.run(move |c| common::audit(c, &user.id, "backup_downloaded", None)).await?;
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")).map_err(ApiError::internal)?;
    Ok(([(header::CONTENT_TYPE, HeaderValue::from_static("application/x-tar")), (header::CONTENT_DISPOSITION, disposition)], Body::from(bytes)).into_response())
}

async fn delete(State(st): State<AppState>, user: AuthUser, UrlPath(name): UrlPath<String>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    let _guard = st.backup_lock.lock().await;
    fs::remove_file(stored(&st, &name)?).map_err(ApiError::internal)?;
    st.db.run(move |c| common::audit(c, &user.id, "backup_deleted", None)).await?;
    Ok(Json(json!({ "ok": true })))
}

/// Aggiunge all'elenco un backup salvato altrove, dopo averlo controllato. Non ripristina nulla.
async fn upload(State(st): State<AppState>, user: AuthUser, bytes: Bytes) -> Result<Json<Item>, ApiError> {
    common::require_admin(&user)?;
    if !st.limiter.hit(&format!("backup:{}", user.id), 6, 60) {
        return Err(ApiError::TooManyRequests);
    }
    let _guard = st.backup_lock.lock().await;
    drop(open_blocking(&st, bytes.clone()).await?);
    let created_at = now();
    let name = new_name(Kind::Caricato, created_at);
    write_file(&st.cfg.backups_dir(), &name, |f| Ok(f.write_all(&bytes)?))?;
    st.db.run(move |c| common::audit(c, &user.id, "backup_uploaded", None)).await?;
    Ok(Json(Item { name, kind: Kind::Caricato, created_at, size: bytes.len() as u64 }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreReq {
    /// Prova che chi chiede il ripristino conosce la password, non solo che ha una sessione aperta.
    auth_key: String,
}

async fn restore_stored(State(st): State<AppState>, user: AuthUser, UrlPath(name): UrlPath<String>, Json(req): Json<RestoreReq>) -> Result<Json<Value>, ApiError> {
    common::require_admin(&user)?;
    if !st.limiter.hit(&format!("restore:{}", user.id), st.cfg.security.login_attempts_per_minute_user, 60) {
        return Err(ApiError::TooManyRequests);
    }
    let uid = user.id.clone();
    let hash: String = st.db.run(move |c| Ok(c.query_row("SELECT auth_hash FROM users WHERE id = ?1", params![uid], |r| r.get(0))?)).await?;
    if !auth::valid_b64(&req.auth_key, Some(32), 64) || !api::verify_blocking(&st, req.auth_key, hash).await? {
        // Non 401: la sessione e' valida, e' sbagliata la password.
        return Err(ApiError::Forbidden("password errata"));
    }
    let _guard = st.backup_lock.lock().await;
    let bytes = Bytes::from(tokio::fs::read(stored(&st, &name)?).await.map_err(ApiError::internal)?);
    let opened = open_blocking(&st, bytes).await?;
    let safety = create(&st, Kind::Sicurezza).await?;
    apply(&st, opened, false).await?;
    tracing::info!("ripristinato il backup {name}; copia di sicurezza: {}", safety.name);
    Ok(Json(json!({ "ok": true, "safety": safety.name })))
}

/// Ripristino su un'installazione nuova, al posto della creazione del primo admin.
async fn restore_at_setup(State(st): State<AppState>, Extension(ClientIp(ip)): Extension<ClientIp>, req: Request) -> Result<Json<Value>, ApiError> {
    api::check_setup_allowed(&st, &ip, req.headers())?;
    if api::user_count(&st).await? > 0 {
        return Err(ApiError::Conflict("la configurazione iniziale e' gia' stata completata"));
    }
    let bytes = axum::body::to_bytes(req.into_body(), MAX_UPLOAD_BYTES).await.map_err(|_| ApiError::BadRequest("file troppo grande o incompleto"))?;
    let _guard = st.backup_lock.lock().await;
    let opened = open_blocking(&st, bytes).await?;
    apply(&st, opened, true).await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> i64 {
        DateTime::parse_from_rfc3339(text).unwrap().timestamp()
    }

    #[test]
    fn scheduled_times_follow_the_instance_timezone() {
        let s = Settings::default();
        // Novembre: Roma e' UTC+1.
        let (last, next) = s.occurrences(chrono_tz::Europe::Rome, at("2026-11-26T12:00:00Z"));
        assert_eq!(last, Some(at("2026-11-26T02:00:00Z")));
        assert_eq!(next, Some(at("2026-11-27T02:00:00Z")));
    }

    #[test]
    fn only_chosen_weekdays_count_and_skipped_hour_is_handled() {
        // Il 28 marzo 2027 e' domenica e a Roma le 02:30 non esistono.
        let s = Settings { days: vec![7], time: "02:30".into(), ..Settings::default() };
        let (last, next) = s.occurrences(chrono_tz::Europe::Rome, at("2027-03-27T12:00:00Z"));
        assert_eq!(last, Some(at("2027-03-21T01:30:00Z")));
        assert_eq!(next, Some(at("2027-03-28T01:30:00Z")));
    }

    #[test]
    fn only_well_formed_names_are_accepted() {
        assert!(parse_name("taskhit-20261126-030000-auto-1a2b.tar").is_some());
        for bad in ["../secrets/db.key", "taskhit-20261126-030000-auto-1a2b.tar.tmp", "taskhit-20261126-030000-altro-1a2b.tar", "taskhit-x-y-auto-1a2b.tar"] {
            assert!(parse_name(bad).is_none(), "{bad}");
        }
    }
}
