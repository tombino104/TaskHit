use std::{io::{Read, Write}, net::{SocketAddr, TcpStream}, time::Duration};
use taskhit_server::{build_state, config::Config, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=warn".into()))
        .init();

    let cfg = Config::load()?;

    // `taskhit-server healthcheck`: usato da Docker, l'immagine finale non ha ne' shell ne' curl.
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck(&cfg);
    }

    let bind = cfg.server.bind.clone();
    let state = build_state(cfg)?;

    // Pulizia periodica di limitatore e sessioni scadute.
    let (limiter, db) = (state.limiter.clone(), state.db.clone());
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            limiter.sweep();
            let now = taskhit_server::ratelimit::now_secs() as i64;
            if let Err(e) = db.run(move |c| { c.execute("DELETE FROM sessions WHERE expires_at < ?1", [now])?; Ok(()) }).await {
                tracing::warn!("pulizia sessioni fallita: {e:?}");
            }
        }
    });

    // Ogni minuto, e subito all'avvio: gruppi di lavoro scaduti e backup (recuperando quelli persi).
    let backups = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            match taskhit_server::work::purge_expired(&backups).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("gruppi di lavoro scaduti eliminati: {n}"),
                Err(e) => tracing::error!("pulizia dei gruppi di lavoro non riuscita: {e:?}"),
            }
            match taskhit_server::backup::run_due(&backups).await {
                Ok(Some(item)) => tracing::info!("backup automatico creato: {}", item.name),
                Ok(None) => {}
                Err(e) => tracing::error!("backup automatico non riuscito: {e:?}"),
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("TaskHit {} in ascolto su {bind}", env!("CARGO_PKG_VERSION"));
    axum::serve(listener, router(state).into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn healthcheck(cfg: &Config) -> anyhow::Result<()> {
    let addr: SocketAddr = cfg.server.bind.parse()?;
    let target = SocketAddr::from(([127, 0, 0, 1], addr.port()));
    let mut s = TcpStream::connect_timeout(&target, Duration::from_secs(3))?;
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    s.write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    if out.starts_with("HTTP/1.1 200") { Ok(()) } else { anyhow::bail!("health non ok") }
}

async fn shutdown_signal() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("arresto in corso");
}
