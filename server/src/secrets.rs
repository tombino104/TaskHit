//! Segreti generati al primo avvio e conservati nella cartella dei dati (mai nel codice).

use anyhow::{Context, bail};
use std::{fs, io::Write, path::Path};

/// Legge 32 byte da `dir/name`, oppure li genera con il generatore casuale del sistema.
/// Il file e' leggibile solo dall'utente che esegue il server (0600).
pub fn load_or_create(dir: &Path, name: &str) -> anyhow::Result<[u8; 32]> {
    fs::create_dir_all(dir).with_context(|| format!("creazione di {}", dir.display()))?;
    set_mode(dir, 0o700);
    let path = dir.join(name);
    if path.exists() {
        let text = fs::read_to_string(&path).with_context(|| format!("lettura di {}", path.display()))?;
        let bytes = hex::decode(text.trim()).with_context(|| format!("{} non e' esadecimale", path.display()))?;
        let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) else {
            bail!("{} deve contenere 32 byte (64 caratteri esadecimali)", path.display());
        };
        return Ok(key);
    }
    let mut key = [0u8; 32];
    rand::fill(&mut key[..]);
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&path).with_context(|| format!("creazione di {}", path.display()))?;
    f.write_all(hex::encode(key).as_bytes())?;
    f.sync_all()?;
    set_mode(&path, 0o600);
    Ok(key)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}
