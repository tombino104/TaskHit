-- I gruppi creati dai rappresentanti stanno dentro una classe: ne fanno parte solo i suoi membri.
-- NULL = classe, oppure gruppo creato dall'admin e non legato a una classe.
ALTER TABLE groups ADD COLUMN parent_id TEXT REFERENCES groups(id) ON DELETE CASCADE;
