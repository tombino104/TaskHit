-- Calendario: inviti, cronologia degli eventi e i campi che servono all'interfaccia.

-- Chi si iscrive con un codice invito resta in attesa finche' l'admin non lo accetta.
ALTER TABLE users ADD COLUMN pending INTEGER NOT NULL DEFAULT 0 CHECK (pending IN (0,1));
ALTER TABLE users ADD COLUMN pending_group_id TEXT;

-- Eventi pubblici: alunno a cui si riferisce (facoltativo) e avvisi scelti (es. 'g2,sera').
ALTER TABLE events ADD COLUMN subject_id TEXT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE events ADD COLUMN alerts TEXT NOT NULL DEFAULT '';

-- Sotto-tipi di un'etichetta (es. 'teorica,pratica') ed etichette che anche gli alunni possono usare.
ALTER TABLE labels ADD COLUMN subs TEXT NOT NULL DEFAULT '';
ALTER TABLE labels ADD COLUMN personal INTEGER NOT NULL DEFAULT 0 CHECK (personal IN (0,1));

CREATE TABLE invites (
  id         TEXT PRIMARY KEY,
  code       TEXT NOT NULL UNIQUE,
  group_id   TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
  created_by TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  uses       INTEGER NOT NULL DEFAULT 0,
  revoked_at INTEGER
) STRICT;

-- Cronologia degli eventi pubblici: chi ha fatto cosa e chi l'ha approvato.
CREATE TABLE event_history (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id    TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  at          INTEGER NOT NULL,
  actor_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
  approver_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('added','modified','deleted')),
  changes     TEXT                          -- JSON {campo: [prima, dopo]}
) STRICT;
CREATE INDEX event_history_event ON event_history(event_id, id);

-- Valori di partenza: stanno nel database e si possono cambiare, non sono scritti nell'interfaccia.
INSERT OR IGNORE INTO tags (id, name, color, sort) VALUES
  (lower(hex(randomblob(16))), 'Matematica',     '#E4572E', 1),
  (lower(hex(randomblob(16))), 'Italiano',       '#8E5BD6', 2),
  (lower(hex(randomblob(16))), 'Storia',         '#C9962B', 3),
  (lower(hex(randomblob(16))), 'Inglese',        '#2A9D8F', 4),
  (lower(hex(randomblob(16))), 'Informatica',    '#D6457A', 5),
  (lower(hex(randomblob(16))), 'Sistemi e reti', '#4C9A2A', 6),
  (lower(hex(randomblob(16))), 'Generale',       '#5B6470', 99);

INSERT OR IGNORE INTO labels (id, name, has_sub, subs, personal, sort) VALUES
  (lower(hex(randomblob(16))), 'Verifica',             1, 'teorica,pratica',             0, 1),
  (lower(hex(randomblob(16))), 'Verifica di recupero', 1, 'teorica,pratica',             0, 2),
  (lower(hex(randomblob(16))), 'Interrogazione',       0, '',                            1, 3),
  (lower(hex(randomblob(16))), 'Corso',                1, 'scolastico,extrascolastico',  0, 4),
  (lower(hex(randomblob(16))), 'Rientro',              0, '',                            0, 5),
  (lower(hex(randomblob(16))), 'FSL',                  0, '',                            0, 6),
  (lower(hex(randomblob(16))), 'Ed. Civica',           0, '',                            0, 7),
  (lower(hex(randomblob(16))), 'Altro',                0, '',                            1, 99);
