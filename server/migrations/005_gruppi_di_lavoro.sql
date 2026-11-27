-- Gruppi di lavoro: temporanei, dentro una classe, con un capogruppo.
-- Un gruppo e' "di lavoro" quando ha una scadenza.
ALTER TABLE groups ADD COLUMN expires_at INTEGER;
ALTER TABLE groups ADD COLUMN leader_id TEXT REFERENCES users(id) ON DELETE SET NULL;
-- Il lavoro per cui e' nato (es. "Ricerca di storia"): unisce i gruppi creati insieme.
ALTER TABLE groups ADD COLUMN activity TEXT;
