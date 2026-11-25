//! Limitatore a finestra fissa, in memoria. Basta per un'istanza singola.
//! La difesa dagli attacchi volumetrici (DDoS) sta davanti, in Cloudflare: da qui non si puo'
//! fermare traffico che e' gia' arrivato sulla linea di casa.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Default)]
pub struct RateLimiter {
    map: Mutex<HashMap<String, (u64, u32)>>,
}

const MAX_KEYS: usize = 100_000;

impl RateLimiter {
    pub fn new() -> Self { Self::default() }

    /// Conta una richiesta per `key`. Restituisce false se supera `limit` nella finestra di `window_secs`.
    pub fn hit(&self, key: &str, limit: u32, window_secs: u64) -> bool {
        let now = now_secs();
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() >= MAX_KEYS {
            map.retain(|_, (start, _)| now.saturating_sub(*start) < 120);
            if map.len() >= MAX_KEYS {
                // Memoria sotto attacco: meglio dimenticare tutto che crescere senza limite.
                map.clear();
            }
        }
        let entry = map.entry(key.to_owned()).or_insert((now, 0));
        if now.saturating_sub(entry.0) >= window_secs {
            *entry = (now, 0);
        }
        entry.1 = entry.1.saturating_add(1);
        entry.1 <= limit
    }

    pub fn sweep(&self) {
        let now = now_secs();
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, (start, _)| now.saturating_sub(*start) < 120);
    }
}
