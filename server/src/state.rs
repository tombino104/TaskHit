use crate::{config::Config, db::Db, ratelimit::RateLimiter};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub db: Db,
    pub limiter: Arc<RateLimiter>,
    /// Chiave del server per valori derivati (es. salt finti anti-enumerazione). Mai inviata ai client.
    pub server_key: Arc<[u8; 32]>,
    /// Hash precalcolato: serve a spendere lo stesso tempo anche quando l'utente non esiste.
    pub dummy_hash: Arc<String>,
    /// Richieste in corso: oltre il limite il server risponde 503 (load shedding).
    pub inflight: Arc<Semaphore>,
    /// Calcoli Argon2 contemporanei: limitano la RAM usata da chi manda molti login.
    pub hash_slots: Arc<Semaphore>,
}

/// Indirizzo del client, calcolato una volta per richiesta dal middleware `guard`.
#[derive(Clone, Debug)]
pub struct ClientIp(pub String);
