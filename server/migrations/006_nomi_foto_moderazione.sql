-- Si entra con nome e cognome. `username` resta la chiave d'accesso (il nome normalizzato);
-- chi ha un account precedente ha `first_name` vuoto finche' non completa il nome.
ALTER TABLE users ADD COLUMN first_name TEXT;
ALTER TABLE users ADD COLUMN last_name TEXT;
-- Richiesta di cambiare nome (JSON con motivi e testo); NULL = nessuna.
ALTER TABLE users ADD COLUMN rename_request TEXT;

-- Sospensioni ed espulsioni. `disabled` resta l'interruttore effettivo; qui c'e' il perche'.
ALTER TABLE users ADD COLUMN block_kind TEXT CHECK (block_kind IN ('sospeso','espulso'));
ALTER TABLE users ADD COLUMN block_until INTEGER;           -- NULL = finche' qualcuno non riattiva
ALTER TABLE users ADD COLUMN block_reason TEXT;
ALTER TABLE users ADD COLUMN block_tags TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN blocked_by_role TEXT;
UPDATE users SET block_kind = 'sospeso', blocked_by_role = 'admin' WHERE disabled = 1;

-- Foto profilo: visibili ai compagni, quindi non cifrate sul dispositivo (il database lo e' sul disco).
CREATE TABLE avatars (
  user_id    TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  mime       TEXT NOT NULL,
  data       BLOB NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;

-- Verifiche: scritta, pratica o di recupero in un tipo solo. Il vecchio tipo «Verifica di recupero»
-- resta nascosto, perche' gli eventi privati (cifrati) possono ancora citarlo.
ALTER TABLE labels ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0,1));
UPDATE labels SET subs = 'scritta,pratica,recupero' WHERE name = 'Verifica';
UPDATE events SET sub_kind = 'scritta'
 WHERE sub_kind = 'teorica' AND label_id IN (SELECT id FROM labels WHERE name = 'Verifica');
UPDATE events SET sub_kind = 'recupero', label_id = (SELECT id FROM labels WHERE name = 'Verifica')
 WHERE label_id IN (SELECT id FROM labels WHERE name = 'Verifica di recupero')
   AND EXISTS (SELECT 1 FROM labels WHERE name = 'Verifica');
UPDATE proposals SET data = json_set(data, '$.sub_kind', 'scritta')
 WHERE status = 'pending' AND json_extract(data, '$.sub_kind') = 'teorica'
   AND json_extract(data, '$.label_id') IN (SELECT id FROM labels WHERE name = 'Verifica');
UPDATE proposals SET data = json_set(data, '$.sub_kind', 'recupero', '$.label_id', (SELECT id FROM labels WHERE name = 'Verifica'))
 WHERE status = 'pending' AND json_extract(data, '$.label_id') IN (SELECT id FROM labels WHERE name = 'Verifica di recupero')
   AND EXISTS (SELECT 1 FROM labels WHERE name = 'Verifica');
UPDATE labels SET hidden = 1
 WHERE name = 'Verifica di recupero' AND EXISTS (SELECT 1 FROM labels WHERE name = 'Verifica');
