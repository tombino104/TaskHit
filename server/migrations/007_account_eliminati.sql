-- Un account eliminato resta come riga vuota, senza dati: gli eventi pubblicati per la classe
-- e la loro cronologia continuano ad avere un autore, mostrato come «Account eliminato».
ALTER TABLE users ADD COLUMN removed_at INTEGER;
