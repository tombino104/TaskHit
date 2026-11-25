-- TaskHit: schema iniziale.
-- Convenzioni: tempi in secondi Unix (UTC), id = 32 caratteri esadecimali casuali.
-- Le colonne marcate [E2EE] contengono dati cifrati sul dispositivo: il server non li legge.

CREATE TABLE users (
  id                   TEXT PRIMARY KEY,
  username             TEXT NOT NULL UNIQUE COLLATE NOCASE,
  display_name         TEXT NOT NULL,
  role                 TEXT NOT NULL CHECK (role IN ('admin','rappresentante','nerd','alunno')),
  auth_hash            TEXT NOT NULL,      -- Argon2id dell'auth key derivata dal client (mai la password)
  kdf_salt             TEXT NOT NULL,      -- salt della derivazione chiavi lato client (base64url)
  wrapped_key          TEXT NOT NULL,      -- [E2EE] chiave dati dell'utente, cifrata con la chiave derivata dalla password
  recovery_wrapped_key TEXT,               -- [E2EE] stessa chiave, cifrata con la chiave di recupero (mostrata una volta sola)
  profile_blob         TEXT,               -- [E2EE] dati generici del profilo e foto, cifrati
  disabled             INTEGER NOT NULL DEFAULT 0 CHECK (disabled IN (0,1)),
  created_at           INTEGER NOT NULL,
  updated_at           INTEGER NOT NULL
) STRICT;

CREATE TABLE sessions (
  id         TEXT PRIMARY KEY,             -- SHA-256 del token: il token vero esiste solo nel cookie
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  last_seen  INTEGER NOT NULL,
  user_agent TEXT
) STRICT;
CREATE INDEX sessions_user ON sessions(user_id);
CREATE INDEX sessions_expiry ON sessions(expires_at);

-- Gruppi: li crea solo l'admin.
CREATE TABLE groups (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
  kind       TEXT NOT NULL CHECK (kind IN ('classe','gruppo')),
  created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE group_members (
  group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  user_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  PRIMARY KEY (group_id, user_id)
) STRICT;
CREATE INDEX group_members_user ON group_members(user_id);

-- TAG colorati (materie, FSL, ...) e ETICHETTE di tipo (verifica, interrogazione, ...).
CREATE TABLE tags (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE COLLATE NOCASE,
  color TEXT NOT NULL CHECK (color GLOB '#[0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]'),
  sort  INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE labels (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
  has_sub    INTEGER NOT NULL DEFAULT 0 CHECK (has_sub IN (0,1)),  -- es. teorica/pratica, scolastico/extrascolastico
  sort       INTEGER NOT NULL DEFAULT 0
) STRICT;

-- Eventi.
--  pubblico: campi in chiaro (il server deve poterli leggere per Inbox, approvazioni e push);
--  privato:  solo `payload` cifrato [E2EE]; il server non conosce titolo, data o materia.
CREATE TABLE events (
  id          TEXT PRIMARY KEY,
  visibility  TEXT NOT NULL CHECK (visibility IN ('public','private')),
  owner_id    TEXT NOT NULL REFERENCES users(id),
  group_id    TEXT REFERENCES groups(id) ON DELETE CASCADE,
  title       TEXT,
  description TEXT,
  tag_id      TEXT REFERENCES tags(id) ON DELETE SET NULL,
  label_id    TEXT REFERENCES labels(id) ON DELETE SET NULL,
  sub_kind    TEXT,
  starts_at   INTEGER,
  ends_at     INTEGER,
  payload     TEXT,                        -- [E2EE] base64url(nonce || ciphertext) con tutti i campi dell'evento privato
  created_by  TEXT NOT NULL REFERENCES users(id),
  created_at  INTEGER NOT NULL,
  updated_by  TEXT NOT NULL REFERENCES users(id),
  updated_at  INTEGER NOT NULL,
  rev         INTEGER NOT NULL DEFAULT 1,  -- revisione, per rilevare conflitti
  deleted_at  INTEGER,                     -- cancellazione logica (serve alla sincronizzazione)
  CHECK (
    (visibility = 'public'  AND group_id IS NOT NULL AND title IS NOT NULL AND starts_at IS NOT NULL AND payload IS NULL)
 OR (visibility = 'private' AND group_id IS NULL AND payload IS NOT NULL AND title IS NULL AND starts_at IS NULL)
  )
) STRICT;
CREATE INDEX events_group_time ON events(group_id, starts_at) WHERE visibility = 'public' AND deleted_at IS NULL;
CREATE INDEX events_owner ON events(owner_id) WHERE visibility = 'private';

-- Promemoria. fire_at e' in chiaro: il server sa QUANDO notificare, non COSA. Compromesso dichiarato.
CREATE TABLE reminders (
  id       TEXT PRIMARY KEY,
  event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  user_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  fire_at  INTEGER NOT NULL,
  sent_at  INTEGER
) STRICT;
CREATE INDEX reminders_due ON reminders(fire_at) WHERE sent_at IS NULL;

-- Proposte dei Nerd: modifiche agli eventi pubblici che aspettano un rappresentante o l'admin.
CREATE TABLE proposals (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('create','update','delete')),
  event_id    TEXT REFERENCES events(id) ON DELETE CASCADE,
  base_rev    INTEGER,
  proposed_by TEXT NOT NULL REFERENCES users(id),
  data        TEXT NOT NULL,               -- JSON con i campi proposti (solo dati pubblici)
  note        TEXT,
  status      TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
  decided_by  TEXT REFERENCES users(id),
  decided_at  INTEGER,
  created_at  INTEGER NOT NULL
) STRICT;
CREATE INDEX proposals_pending ON proposals(status, created_at);

-- Inbox: notifiche di aggiunte, modifiche ed eliminazioni di eventi pubblici.
CREATE TABLE inbox (
  id         TEXT PRIMARY KEY,
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind       TEXT NOT NULL CHECK (kind IN ('added','modified','deleted','proposal','decision')),
  event_id   TEXT,
  actor_id   TEXT REFERENCES users(id) ON DELETE SET NULL,
  diff       TEXT,                         -- JSON {campo: [prima, dopo]}, solo dati pubblici
  created_at INTEGER NOT NULL,
  read_at    INTEGER
) STRICT;
CREATE INDEX inbox_user ON inbox(user_id, created_at DESC);

-- Registro delle modifiche, per la sincronizzazione incrementale (cursore = seq).
CREATE TABLE changes (
  seq       INTEGER PRIMARY KEY AUTOINCREMENT,
  entity    TEXT NOT NULL,                 -- 'event', 'tag', 'label', 'group', ...
  entity_id TEXT NOT NULL,
  op        TEXT NOT NULL CHECK (op IN ('upsert','delete')),
  group_id  TEXT,                          -- chi lo riceve: i membri del gruppo
  user_id   TEXT,                          -- oppure solo questo utente (eventi privati)
  rev       INTEGER,
  at        INTEGER NOT NULL
) STRICT;
CREATE INDEX changes_group ON changes(group_id, seq);
CREATE INDEX changes_user ON changes(user_id, seq);

CREATE TABLE push_subscriptions (
  id         TEXT PRIMARY KEY,
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  endpoint   TEXT NOT NULL UNIQUE,
  p256dh     TEXT NOT NULL,
  auth       TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_ok_at INTEGER
) STRICT;

-- Impostazioni dell'istanza: niente e' scritto nel codice.
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

CREATE TABLE plugin_state (
  plugin_id TEXT NOT NULL,
  scope     TEXT NOT NULL CHECK (scope IN ('instance','user')),
  user_id   TEXT NOT NULL DEFAULT '',      -- '' per l'impostazione di istanza
  enabled   INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  settings  TEXT,
  PRIMARY KEY (plugin_id, scope, user_id)
) STRICT;

-- Registro di sicurezza: accessi, ruoli, backup. Mai dati degli eventi.
CREATE TABLE audit_log (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  at       INTEGER NOT NULL,
  actor_id TEXT,
  action   TEXT NOT NULL,
  detail   TEXT
) STRICT;

CREATE TABLE backups (
  id         TEXT PRIMARY KEY,
  file       TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('auto','manual')),
  created_at INTEGER NOT NULL
) STRICT;
