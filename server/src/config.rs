//! Configurazione: valori predefiniti, poi file `config.toml`, poi variabili d'ambiente.
//! Niente e' scritto nel codice: tutto cio' che dipende dall'installazione sta qui.

use anyhow::{Context, bail};
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerCfg {
    /// Indirizzo su cui ascolta il server. In Docker deve essere 0.0.0.0.
    pub bind: String,
    /// Indirizzo pubblico (tunnel Cloudflare), es. "https://taskhit-tombino104.net".
    /// Se impostato, le richieste di scrittura sono accettate solo da questa origine.
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PathsCfg {
    /// Cartella dei dati persistenti: database, segreti, temi, backup.
    pub data_dir: PathBuf,
    /// Cartella dell'interfaccia web gia' compilata.
    pub web_dir: PathBuf,
    /// Se la cartella esiste, ogni backup vi viene anche copiato (meglio su un altro disco).
    pub backup_copy_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityCfg {
    pub session_days: u32,
    /// true dietro HTTPS (tunnel Cloudflare). false solo per prove in locale su http.
    pub cookie_secure: bool,
    /// Intestazione che contiene l'IP reale del client. Vuota = usa l'indirizzo della connessione.
    /// Con Cloudflare e' "cf-connecting-ip". Se il server e' raggiungibile anche senza tunnel,
    /// chi si collega direttamente puo' falsificarla (e quindi eludere solo il proprio limite).
    pub client_ip_header: String,
    /// Consente la creazione del primo admin anche da richieste che arrivano dal tunnel.
    /// Di default no: il primo avvio si fa dalla rete locale, prima di pubblicare il tunnel.
    pub allow_remote_setup: bool,
    pub api_requests_per_minute: u32,
    pub login_attempts_per_minute_ip: u32,
    pub login_attempts_per_minute_user: u32,
    pub max_body_kb: usize,
    pub request_timeout_secs: u64,
    /// Richieste contemporanee oltre le quali il server risponde 503 invece di accodarle.
    pub max_in_flight: usize,
    /// Calcoli Argon2 contemporanei (ognuno usa ~19 MiB di RAM).
    pub max_parallel_hashes: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerCfg,
    pub paths: PathsCfg,
    pub security: SecurityCfg,
}

impl Default for ServerCfg {
    fn default() -> Self {
        Self { bind: "0.0.0.0:8080".into(), public_url: None }
    }
}

impl Default for PathsCfg {
    fn default() -> Self {
        Self { data_dir: "./data".into(), web_dir: "./web".into(), backup_copy_dir: None }
    }
}

impl Default for SecurityCfg {
    fn default() -> Self {
        Self {
            session_days: 30,
            cookie_secure: true,
            client_ip_header: "cf-connecting-ip".into(),
            allow_remote_setup: false,
            api_requests_per_minute: 240,
            login_attempts_per_minute_ip: 10,
            login_attempts_per_minute_user: 5,
            max_body_kb: 256,
            request_timeout_secs: 15,
            max_in_flight: 256,
            max_parallel_hashes: 4,
        }
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn env_bool(name: &str) -> anyhow::Result<Option<bool>> {
    match env(name).as_deref() {
        None => Ok(None),
        Some("1" | "true" | "yes") => Ok(Some(true)),
        Some("0" | "false" | "no") => Ok(Some(false)),
        Some(v) => bail!("{name}: valore non valido '{v}' (usa true o false)"),
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let data_dir_hint = env("TASKHIT_DATA_DIR").map(PathBuf::from);
        let file = match env("TASKHIT_CONFIG") {
            Some(p) => Some(PathBuf::from(p)),
            None => {
                let p = data_dir_hint.clone().unwrap_or_else(|| "./data".into()).join("config.toml");
                p.exists().then_some(p)
            }
        };
        let mut cfg = match file {
            Some(p) => {
                let text = std::fs::read_to_string(&p).with_context(|| format!("lettura di {}", p.display()))?;
                toml::from_str(&text).with_context(|| format!("config non valida in {}", p.display()))?
            }
            None => Config::default(),
        };
        if let Some(v) = env("TASKHIT_BIND") { cfg.server.bind = v; }
        if let Some(v) = env("TASKHIT_PUBLIC_URL") { cfg.server.public_url = Some(v); }
        if let Some(v) = data_dir_hint { cfg.paths.data_dir = v; }
        if let Some(v) = env("TASKHIT_WEB_DIR") { cfg.paths.web_dir = v.into(); }
        if let Some(v) = env("TASKHIT_BACKUP_COPY_DIR") { cfg.paths.backup_copy_dir = Some(v.into()); }
        if let Some(v) = env_bool("TASKHIT_COOKIE_SECURE")? { cfg.security.cookie_secure = v; }
        if let Some(v) = env_bool("TASKHIT_ALLOW_REMOTE_SETUP")? { cfg.security.allow_remote_setup = v; }
        if let Some(v) = std::env::var("TASKHIT_CLIENT_IP_HEADER").ok() { cfg.security.client_ip_header = v; }
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.server.bind.parse::<SocketAddr>().with_context(|| format!("server.bind '{}' non e' un indirizzo valido", self.server.bind))?;
        if let Some(u) = &self.server.public_url {
            if !(u.starts_with("https://") || u.starts_with("http://")) {
                bail!("server.public_url deve iniziare con https:// o http://");
            }
        }
        let s = &self.security;
        if s.session_days == 0 || s.max_body_kb == 0 || s.request_timeout_secs == 0 || s.max_in_flight == 0 || s.max_parallel_hashes == 0 {
            bail!("i valori numerici di [security] devono essere maggiori di zero");
        }
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf { self.paths.data_dir.join("taskhit.db") }
    pub fn secrets_dir(&self) -> PathBuf { self.paths.data_dir.join("secrets") }
    pub fn themes_dir(&self) -> PathBuf { self.paths.data_dir.join("themes") }
    pub fn languages_dir(&self) -> PathBuf { self.paths.data_dir.join("languages") }
    pub fn backups_dir(&self) -> PathBuf { self.paths.data_dir.join("backups") }

    /// Nome del cookie di sessione. Con HTTPS usa il prefisso __Host-, che il browser accetta
    /// solo se il cookie e' Secure, senza Domain e con Path=/.
    pub fn cookie_name(&self) -> &'static str {
        if self.security.cookie_secure { "__Host-th_session" } else { "th_session" }
    }

    /// Verifica l'intestazione Origin di una richiesta di scrittura.
    pub fn origin_allowed(&self, origin: &str, host: Option<&str>) -> bool {
        let authority = |s: &str| s.split("://").nth(1).unwrap_or(s).split('/').next().unwrap_or("").to_ascii_lowercase();
        let o = authority(origin);
        if let Some(p) = &self.server.public_url {
            return o == authority(p);
        }
        host.is_some_and(|h| o == h.to_ascii_lowercase())
    }
}
