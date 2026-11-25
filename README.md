# TaskHit

Calendario scolastico condiviso, per una classe: eventi pubblici, eventi privati cifrati sul dispositivo, ruoli, temi a file singolo. Si ospita da sé (UmbrelOS + Cloudflare Tunnel). **Stato: calendario funzionante** (eventi, ruoli, approvazioni, Inbox, inviti, temi, grafico del carico); notifiche push, backup automatici e installazione di plugin sono in roadmap. Tutti i dettagli, i limiti di sicurezza e ciò che non è stato verificato sono in [`ARCHITETTURA.md`](ARCHITETTURA.md).

## Provarlo in locale

```bash
# 1. interfaccia
cd web && npm ci && npm run build && cd ..

# 2. server (senza cifratura del database, solo per prove; cookie non Secure perché http)
cd server
TASKHIT_DATA_DIR=./data TASKHIT_WEB_DIR=../web/dist TASKHIT_BIND=127.0.0.1:8080 TASKHIT_COOKIE_SECURE=false \
  cargo run --release
# apri http://127.0.0.1:8080 e segui la configurazione guidata
```

Sviluppo dell'interfaccia con ricarica a caldo: `cd web && npm run dev` (inoltra `/api` a `127.0.0.1:8080`).

## Test

```bash
cd server && cargo test                 # 20 test di integrazione e 1 unitario
cd web && npm test                      # crittografia, date, eventi, temi
cd web && npx svelte-check --tsgo       # tipi

# end-to-end del client contro un server vuoto in esecuzione
cd web && TASKHIT_E2E_URL=http://127.0.0.1:8080 npx vitest run src/lib/e2e.test.ts
```

## Produzione

- Immagine: `docker buildx build --platform linux/amd64,linux/arm64 -t ghcr.io/<utente>/taskhit:0.1.0 --push .` (include SQLCipher).
- Umbrel: copia `umbrel-store/` in un repository pubblico e aggiungilo come store di terze parti; poi installa TaskHit. Sostituisci l'immagine nel compose con quella fissata per digest.
- Configurazione: `config.example.toml` o variabili `TASKHIT_*`. Crea l'amministratore dalla rete di casa **prima** di esporre il tunnel.

## Licenza

[PolyForm Noncommercial 1.0.0](LICENSE): uso non commerciale libero, niente uso commerciale né rivendita. Non è una licenza open source.
