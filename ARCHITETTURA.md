# TaskHit — Architettura

Stato del documento: **fase 3 (calendario e interfaccia)**. Dice cosa è stato deciso, cosa è già implementato e verificato, cosa è ancora da fare e cosa non è stato possibile verificare. Non è una consulenza legale o di sicurezza professionale.

## 1. Decisioni

| Tema | Scelta | Perché |
|---|---|---|
| Server | Rust, Axum 0.8, Tokio | Un solo binario piccolo (3,5 MiB senza cifratura del database, 7,9 MiB con SQLCipher), poca RAM, nessun runtime da installare. |
| Database | SQLite (WAL, tabelle `STRICT`), accesso serializzato con un solo `Connection` | Un file solo, backup semplici, più che sufficiente per qualche decina di utenti. Le scritture sono brevi. |
| Cifratura a riposo | SQLCipher, attivo con `--features sqlcipher` (usato nell'immagine Docker) | Il file del database non è leggibile senza la chiave. Vedi i limiti in §3. |
| Client | PWA: Svelte 5 + Vite + TypeScript | Una sola base di codice per PC e telefono, installabile, offline con service worker. Niente app store. |
| Sicurezza dati | **Ibrido** | Eventi/profili **privati** cifrati sul dispositivo (il server vede solo blob). Eventi **pubblici** in chiaro sul server, perché devono essere condivisi, filtrati e notificati. |
| Accesso | Argon2id nel browser → chiave di autenticazione; il server ne salva l'hash Argon2id | La password non arriva mai al server. |
| Sync | Registro `changes` con cursore, revisione per evento, coda delle modifiche offline | Vedi §5. |
| Pacchetto | Immagine Docker multi-arch + store Umbrel personale | Vedi §9. |
| Licenza | PolyForm Noncommercial 1.0.0 | Uso personale/scolastico consentito, uso commerciale e rivendita no. Vedi §11. |

## 2. Struttura

```
Browser (PWA)  ──HTTPS──►  Cloudflare Tunnel  ──►  Umbrel app  ──►  taskhit-server (Rust)
 │  Argon2id, HKDF, AES-GCM                                          │  Axum + tower layers
 │  IndexedDB (cache offline)                                        │  SQLite/SQLCipher in /data
 └─ service worker                                                   └─ /data/{taskhit.db, secrets/, themes/, backups/}
```

Il server serve sia le API (`/api/*`) sia i file della PWA. I percorsi senza estensione ricadono su `index.html`; un file mancante (`/assets/x.js`) risponde 404.

Livelli applicati a ogni richiesta, dall'esterno all'interno: limite per IP e load shedding (503 oltre `max_in_flight`) → tracciamento → compressione → intestazioni di cache → timeout → limite del corpo → controllo dell'origine (CSRF) → route. Intestazioni di sicurezza su tutte le risposte (CSP restrittiva, con `'wasm-unsafe-eval'` per il solo Argon2id in WebAssembly, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, HSTS con HTTPS).

## 3. Modello di sicurezza

### Chiavi

```
password ──Argon2id(salt casuale, m=64 MiB, t=3, p=1)──► master (32 B)
master ──HKDF "th-auth"──► auth_key   → al server (che salva Argon2id(auth_key))
master ──HKDF "th-enc" ──► wrap_key   → resta nel browser
DEK  (32 B casuali)  ──AES-256-GCM(wrap_key)──► wrapped_key  → al server (60 byte: IV 12 + 32 + tag 16)
DEK  ──AES-256-GCM(chiave di recupero)──► recovery_wrapped_key  → al server
```

- Gli eventi privati e il profilo sono cifrati con la DEK (AES-256-GCM, IV casuale, dato aggiuntivo = contesto, p.es. `event:42`, così un blob spostato altrove non si decifra).
- Cambiare password = ri-avvolgere la DEK, senza ricifrare i dati.
- La chiave di recupero (160 bit, base32) è mostrata una volta sola alla creazione dell'account (admin o iscrizione con invito) e non è conservata dal server.
- La DEK sta solo in memoria. Dopo un riavvio l'app mostra gli eventi privati come bloccati finché non si riscrive la password.
- Il parametro KDF è dichiarato dal server (`/api/setup/status`, `/api/auth/prelogin`) così si può rafforzare in futuro.

### Sessione e abuso

- Cookie `__Host-th_session`: `HttpOnly`, `Secure`, `SameSite=Strict`. Sul server si salva solo lo SHA-256 del token.
- Controllo `Origin` / `Sec-Fetch-Site` sulle scritture; con `public_url` impostato vale solo quell'origine.
- Limiti: richieste per IP, tentativi di login per IP e **per nome utente** (blocca anche la password giusta durante la pausa), calcoli Argon2 in parallelo limitati (`max_parallel_hashes`) per non esaurire la RAM.
- `prelogin` restituisce un salt finto ma stabile (HMAC con una chiave del server) per i nomi inesistenti, e il login usa un hash fittizio per uguagliare i tempi: non si scoprono gli utenti esistenti.
- Il primo admin si crea **solo dalla rete locale**: le richieste che portano l'intestazione di Cloudflare ricevono 403 finché `allow_remote_setup` è `false`. La creazione è transazionale (un solo admin possibile).
- I temi sono dati non fidati: il server valida colori, nomi file, font e **contrasto WCAG** (≥ 4,5:1 per i testi); il client applica solo valori colore esadecimali e nomi font con caratteri sicuri.

### Cosa resta sul dispositivo

Per la consultazione offline il browser conserva in `localStorage` l'ultima risposta del server (profilo, gruppi con i nomi dei membri, eventi pubblici **in chiaro**, eventi privati **cifrati**, Inbox) e il necessario per sbloccare senza rete (salt, parametri Argon2id e DEK **cifrata**). Chi ha in mano il dispositivo legge i dati pubblici e può tentare di indovinare la password contro Argon2id; non legge i privati senza la password. Tutto viene cancellato con «Esci» o quando la sessione scade.

### Limiti dichiarati (leggere prima di fidarsi)

1. **Cloudflare vede i dati pubblici.** Il tunnel termina il TLS da loro: eventi pubblici, nomi utente e metadati passano in chiaro per Cloudflare. Vedrebbe anche `auth_key` in transito (non la password). Gli eventi privati sono cifrati prima di partire.
2. **Un operatore malevolo del server può comunque rubare la password**, servendo al browser un JavaScript modificato. La cifratura end-to-end protegge da database, disco e backup letti da altri e da un amministratore *curioso ma onesto*; non da chi controlla attivamente il codice servito. Il requisito «invisibile anche all'admin» è soddisfatto in questo senso, non in senso assoluto.
3. **La chiave di SQLCipher sta in `/data/secrets/db.key`**, accanto al database, e i backup di Umbrel includono la cartella dati. SQLCipher quindi protegge da una copia del solo file `.db` o da un disco estratto da un sistema che non conserva anche i segreti; non è una protezione contro chi legge l'intera cartella. Per questo i contenuti sensibili sono comunque cifrati dal client.
4. **I promemoria hanno un `fire_at` in chiaro** (il server deve sapere quando mandare la notifica): rivela *quando*, non *cosa*. Il testo della notifica per eventi privati va cifrato e letto dal service worker.
5. **L'IP del client viene da `cf-connecting-ip`.** Se il server è raggiungibile anche dalla LAN senza tunnel, chi si collega direttamente può falsificarlo e aggirare solo il *proprio* limite per IP; il limite per nome utente resta.
6. **La difesa DDoS vera è Cloudflare.** Il server aggiunge solo limiti locali e load shedding.
7. **Non esistono ancora** reimpostazione password, cambio password o rotazione della chiave di recupero (in roadmap). Si possono invece chiudere le sessioni sugli altri dispositivi.
8. L'admin vede gli eventi **pubblici** di tutti i gruppi; non riceve mai quelli privati degli altri.

## 4. Dati

Schema in `server/migrations/` (`001_init.sql`, `002_calendario.sql`; applicate all'avvio, idempotenti). Materie ed etichette di partenza sono righe del database, modificabili.

| Tabella | Contenuto |
|---|---|
| `users` | nome utente normalizzato, ruolo (`admin`, `rappresentante`, `nerd`, `alunno`), hash di `auth_key`, `kdf_salt`, `wrapped_key`, `recovery_wrapped_key`, `profile_blob` cifrato |
| `sessions` | SHA-256 del token, scadenza |
| `groups`, `group_members` | classi e gruppi (creabili solo dall'admin) |
| `tags`, `labels` | materie con colore esadecimale; tipi testuali (interrogazione, ed. Civica, verifica, verifica di recupero, corso scolastico/extrascolastico, rientro, FSL, altro), con sotto-tipo opzionale |
| `events` | pubblici: campi in chiaro; privati: **solo** `payload` cifrato. Un `CHECK` del database impone la separazione. Creatore/modificatore e timestamp registrati dal server, `rev` per la concorrenza, cancellazione logica |
| `reminders` | promemoria opzionali per evento |
| `proposals` | modifiche dei «Nerd» in attesa di approvazione (`create`/`update`/`delete`, stato) |
| `inbox` | notifiche per utente: aggiunto, modificato, eliminato, proposta, decisione |
| `changes` | registro ordinato (`seq`) per la sincronizzazione |
| `push_subscriptions` | sottoscrizioni Web Push |
| `invites` | codici invito per gruppo (7 giorni, revocabili, uno attivo per gruppo) |
| `event_history` | cronologia degli eventi pubblici: chi ha cambiato cosa e chi ha approvato |
| `settings`, `plugin_state`, `audit_log`, `backups` | impostazioni di istanza, plugin attivi (per istanza o per utente), registro di audit, elenco dei backup |

## 5. Sincronizzazione e offline

**Oggi**
- Il client rilegge profilo, dati di contorno, eventi e Inbox all'avvio, dopo ogni scrittura, quando torna visibile o online e ogni 60 secondi.
- Ogni evento ha `rev`: la scrittura la dichiara e, se il server ne ha una più recente, risponde 409 invece di sovrascrivere. Vale anche per le proposte: una proposta superata si può solo rifiutare.
- Offline si **consulta** (copia sul dispositivo, §3); le scritture sono disattivate.

**Da fare**: lettura incrementale `GET /api/sync?since=<seq>` sul registro `changes` (già compilato dal server), canale SSE, coda delle modifiche offline.

## API

| Percorso | Contenuto |
|---|---|
| `GET /api/events`, `POST /api/events`, `PUT /api/events/{id}`, `POST …/delete`, `GET …/history` | eventi; per un nerd le scritture pubbliche diventano proposte |
| `GET /api/proposals`, `POST …/{id}/decide` | approvazioni |
| `GET /api/inbox`, `POST /api/inbox/read` | notifiche |
| `GET /api/meta`, `POST /api/tags`, `POST /api/labels`, `PUT /api/settings`, `PUT /api/plugins/{id}` | materie, etichette, gruppi, impostazioni |
| `GET /api/users`, `PATCH …/{id}`, `POST …/accept`, `…/reject`, `POST /api/groups` | persone e gruppi (admin) |
| `GET /api/invites`, `POST /api/invites`, `POST …/revoke`, `POST /api/join` | inviti; chi si iscrive resta in attesa dell'admin |
| `GET /api/backups`, `POST /api/backups`, `PUT …/settings`, `GET …/{nome}`, `POST …/{nome}/delete`, `…/restore`, `POST …/upload`, `POST /api/setup/restore` | backup e ripristino (admin; l'ultimo solo su un'installazione vuota) |
| `GET /api/me`, `PATCH /api/me`, `POST /api/me/sessions/revoke-others`, `GET /api/admin/status` | profilo e stato del server |

## Backup

- Un backup è un `.tar` autosufficiente: `manifest.json`, copia coerente del database (`VACUUM INTO`, cifrata se si usa SQLCipher), `db.key` e temi. L'elenco è il contenuto della cartella, non la tabella `backups` (non usata).
- Pianificazione (giorni, ora nel fuso dell'istanza, quanti conservarne) nelle `settings`; il server controlla ogni minuto e recupera l'ultimo orario saltato. L'eliminazione dei vecchi avviene solo quando ne nasce uno nuovo.
- Ripristino: l'archivio è controllato in una cartella di lavoro privata (si leggono solo le voci previste, `integrity_check`, schema non più recente, almeno un admin, migrazioni), poi il contenuto sostituisce il database attivo in un'unica transazione (API di backup di SQLite). La chiave dell'installazione resta la sua, le sessioni vengono chiuse. Dall'admin richiede la password e salva prima una copia di sicurezza.
- Limiti: il file caricato non può superare 64 MB; la copia esterna non è verificata né sfoltita; il contenuto dell'archivio non è protetto da una password propria.

## 6. Ruoli

| | Admin | Rappresentanti | Nerd | Alunni |
|---|---|---|---|---|
| Creare gruppi, gestire utenti, backup, plugin per tutti | ✓ | — | — | — |
| Approvare/rifiutare proposte | ✓ | ✓ | — | — |
| Modificare eventi pubblici direttamente | ✓ | ✓ | proposta da approvare | — |
| Interrogazioni ed eventi personali | ✓ | ✓ | ✓ | ✓ |
| Proporre modifiche agli eventi pubblici | — | — | ✓ | non può |

I permessi si applicano **sul server** (mai solo nell'interfaccia). Rappresentanti e nerd agiscono solo sui gruppi di cui fanno parte. La visibilità di un evento non si cambia dopo la creazione. Accessi, ruoli, gruppi, inviti e decisioni sulle proposte vanno nell'`audit_log`.

## 7. Temi

Un tema è **un solo file JSON** in `/data/themes/`. Il server ne carica l'elenco a ogni richiesta, quindi aggiungerne uno non richiede riavvii.

```json
{
  "nome": "Carta", "tipo": "chiaro", "abbinato": "notte",
  "colori": { "sfondo": "#F1F0EB", "superficie": "#FAFAF7", "inchiostro": "#17171B", "inchiostro2": "#4F4F58",
              "attenuato": "#6B6B73", "filetto": "#D8D7D0", "accento": "#2C3FD1", "suAccento": "#FFFFFF" },
  "font": { "titoli": "Instrument Serif", "testo": "Geist", "dati": "Geist Mono" }
}
```

Inclusi: Carta, Notte, Quaderno, Lavagna. In «Aspetto» si sceglie la modalità (chiaro, scuro, sistema) e un tema per il giorno e uno per la sera. I file non validi non fanno fallire l'app: compaiono in `invalid` con il motivo. Il client segue «sistema» (chiaro/scuro) o il tema scelto.

## 8. Plugin

Oggi c'è un solo plugin interno, il **grafico del carico**, attivabile per l'istanza (admin/rappresentanti) o per utente. Il sistema qui sotto è **da implementare**.

Manifest JSON con id, nome, permessi richiesti e punti di aggancio (vista, widget, impostazioni). I plugin di prima parte sono compilati nell'app; quelli di terzi girerebbero in un `iframe` con `sandbox` e comunicazione via `postMessage`, senza accesso alla DEK. Stato (`plugin_state`): attivo per istanza (decide admin/rappresentanti) o per utente.

## 9. Distribuzione: Docker, Umbrel, Cloudflare

- **Immagine** (`Dockerfile`): 3 fasi (web → server con `--features sqlcipher` → `distroless/cc` non root). Nessuna shell nell'immagine; `HEALTHCHECK` con `taskhit-server healthcheck`. Si costruisce per `linux/amd64` e `linux/arm64` con `docker buildx` (workflow GitHub `.github/workflows/docker.yml`, parte sui tag `v*`).
- **Store Umbrel** (`umbrel-store/`): `umbrel-app-store.yml` + `tombino104-taskhit/` (manifest e compose). Il compose usa `user: "1000:1000"`, `read_only`, `cap_drop: ALL`, `no-new-privileges`, dati in `${APP_DATA_DIR}/data/app` e `PROXY_AUTH_ADD: "false"`, perché i compagni accedono dal tunnel con le credenziali di TaskHit e non con quelle di Umbrel.
- **Tunnel**: punta il tunnel Cloudflare all'indirizzo dell'app su Umbrel. Imposta `TASKHIT_PUBLIC_URL` con l'indirizzo pubblico. Crea l'admin **prima** di pubblicare il tunnel (o abilita `allow_remote_setup` per il solo primo avvio).
- Tutta la configurazione è in `config.toml` o in variabili `TASKHIT_*` (vedi `config.example.toml`): nulla è scritto nel codice.

## 10. Stato dell'implementazione

**Fatto e verificato** (novembre 2026, su macOS arm64)
- Server: tutto ciò che c'era più eventi, proposte, Inbox, persone, gruppi, inviti, impostazioni. più backup e ripristino. 24 test di integrazione e 4 unitari passati, anche con `--features sqlcipher`.
- Client: tutte le schermate del design (Oggi, Calendario giorno/settimana/mese, vista larga con barra laterale, Filtri, Evento, Nuovo evento, Inbox, Approvazioni, Profilo, Aspetto, Plugin, Persone, Backup, Setup guidato) più accesso, iscrizione e ricerca. 22 test, controllo dei tipi senza errori, build PWA.
- Test end-to-end del client contro il server: passato.
- Nel browser, contro un server locale con dati di prova: setup in cinque passi, accesso, creazione di un evento privato, blocco e sblocco (anche a server spento), approvazione con «Annulla», accettazione di un iscritto, temi chiaro e scuro, 390 px e 1440 px.
- Backup: creazione, scaricamento, password errata e ripristino nel browser; ripristino su un server locale vuoto.
- Corretti due difetti che nel browser impedivano l'uso: la CSP bloccava WebAssembly (quindi setup e accesso) e i font incorporati come `data:`.

**Presente nell'interfaccia ma non funzionante** (comandi disattivati e indicati «in arrivo»): notifiche push, caricamento di un tema, installazione di plugin, foto profilo. Gli avvisi di un evento si salvano ma non inviano nulla.

**Non ancora implementato**: sincronizzazione incrementale e coda offline, push, sistema dei plugin, cambio/recupero password, modifica o eliminazione di gruppi, materie ed etichette.

**Non verificato**
- Su un Umbrel vero la 0.1.2 è installata e raggiungibile dal tunnel. Restano da provare lì i backup e la cartella per la seconda copia.
- `requiresHttps: true` con `manifestVersion: 2.0` presume UmbrelOS 2.x; il cookie `__Host-` richiede HTTPS. Se la tua versione è precedente, va riverificato.
- Prove su telefoni veri (Safari/iOS compreso), tempo di Argon2id su dispositivi lenti, installazione come PWA, comportamento di `Origin`/`Host` dietro il proxy di Umbrel.
- Uso con molti utenti contemporanei.

## 11. Licenza

PolyForm Noncommercial 1.0.0: consente uso, modifica e distribuzione per scopi non commerciali (personale, scolastico, di ricerca); vieta l'uso commerciale e la rivendita, comprese le versioni modificate. È una licenza *source-available*, **non** open source secondo l'OSI: su GitHub il repository è pubblico, ma non si può chiamare «open source». Le dipendenze mantengono le proprie licenze. Il file `LICENSE` riporta come titolare «Lorenzo»: sostituisci con nome o handle corretto. Se serve certezza legale, fatti consigliare da un professionista.

## 12. Roadmap

1. **Sync** incrementale e live (SSE), coda offline, risoluzione dei conflitti.
2. **Push** (VAPID) e promemoria.
3. Cambio/recupero password, rotazione della chiave di recupero.
4. Gestione di gruppi, materie ed etichette; foto profilo; caricamento dei temi.
5. Sistema dei **plugin**.
6. Idee future: widget desktop, consigliere «giorno troppo carico», sincronizzazione con calendari nativi.

## 13. Rischi aperti

- Dipendenze molto recenti (Axum 0.8, Argon2 0.6, rusqlite 0.43, TypeScript 7, Vite 8): vanno aggiornate con regolarità e tenute con `Cargo.lock` / `package-lock.json`.
- Un `Mutex<Connection>` basta oggi; se le scritture crescono, valutare un pool di sola lettura.
- `npm audit` segnala 4 avvisi di gravità media in dipendenze di strumenti di build (`workbox-build` → `glob`/`minimatch`); non finiscono nell'app distribuita, ma vanno ricontrollati.
