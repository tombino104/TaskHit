-- L'admin si occupa solo dell'impianto: non fa parte di classi e gruppi.
DELETE FROM group_members WHERE user_id IN (SELECT id FROM users WHERE role = 'admin');
-- Le notifiche sugli eventi ricevute finora non lo riguardano piu'.
DELETE FROM inbox WHERE user_id IN (SELECT id FROM users WHERE role = 'admin');
-- Un evento non puo' riferirsi a un admin come alunno.
UPDATE events SET subject_id = NULL WHERE subject_id IN (SELECT id FROM users WHERE role = 'admin');
