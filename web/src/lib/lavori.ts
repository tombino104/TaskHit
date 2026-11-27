// Gruppi di lavoro: diritti per gruppo e composizione sulla lavagna. Il server resta l'unico a decidere:
// qui si rispecchiano le sue regole per mostrare le azioni giuste.
import { t } from './i18n';
import type { Gruppo, Me, Meta } from './api';
import { puoPubblicare } from './events';

export const MAX_GIORNI = 90;
const GIORNO = 86_400;

export type Diritto = ReturnType<typeof puoPubblicare>;

/** Cosa si può fare sugli eventi di un gruppo: in un gruppo di lavoro conta il capogruppo, non il ruolo. */
export function dirittoSu(utente: Pick<Me, 'id' | 'role'>, g: Gruppo | undefined, adesso = Date.now() / 1000): Diritto {
  if (!g || utente.role === 'admin') return 'no';
  if (!g.work) return puoPubblicare(utente.role);
  if (!g.mine || g.work.expires_at <= adesso) return 'no';
  return g.work.leader_id === utente.id ? 'diretto' : 'proposta';
}

export const gruppiDiLavoro = (meta: Meta | null): Gruppo[] => meta?.groups.filter((g) => g.work) ?? [];

/** Chi ha la schermata delle approvazioni: ruolo della classe, oppure un gruppo di lavoro. */
export const vedeProposte = (utente: Pick<Me, 'role'>, meta: Meta | null): boolean =>
  utente.role !== 'admin' && (puoPubblicare(utente.role) !== 'no' || gruppiDiLavoro(meta).some((g) => g.mine));

/** Chi decide su qualcosa: il rappresentante, o un capogruppo. */
export const decideProposte = (utente: Pick<Me, 'id' | 'role'>, meta: Meta | null): boolean =>
  utente.role === 'rappresentante' || gruppiDiLavoro(meta).some((g) => g.mine && g.work!.leader_id === utente.id && !g.work!.closed);

/** "Gruppo 1 · Ricerca di storia" -> "Gruppo 1". */
export const nomeBreve = (g: Gruppo): string => (g.work?.activity ? g.name.replace(` · ${g.work.activity}`, '') : g.name);

/** Con un'ora di tolleranza: l'orologio dell'interfaccia può essere indietro di qualche minuto. */
export const giorniRimasti = (scadenza: number, adesso: number): number => Math.max(0, Math.ceil((scadenza - adesso - 3600) / GIORNO));

/** Di quanti giorni interi si può ancora prorogare. */
export const prorogaPossibile = (w: { expires_at: number; max_expires_at: number }): number =>
  Math.max(0, Math.floor((w.max_expires_at - w.expires_at) / GIORNO));

// --- La lavagna ---
export interface Bozza { id?: string; nome: string; membri: string[]; capo: string | null }

export const bozzeVuote = (n: number): Bozza[] => Array.from({ length: n }, (_, i) => ({ nome: t('Gruppo {n}', { n: i + 1 }), membri: [], capo: null }));

/** Se il capogruppo è uscito, o non c'è, lo diventa il primo del gruppo. */
const conCapo = (b: Bozza): Bozza => ({ ...b, capo: b.capo && b.membri.includes(b.capo) ? b.capo : (b.membri[0] ?? null) });

/** Mette una persona in un gruppo (o la rimette tra quelle da assegnare con `null`). Sta in un gruppo solo. */
export function sposta(bozze: Bozza[], persona: string, destinazione: number | null): Bozza[] {
  if (destinazione !== null && bozze[destinazione]?.membri.includes(persona)) return bozze;
  return bozze.map((b, i) => {
    const senza = b.membri.filter((m) => m !== persona);
    return conCapo({ ...b, membri: i === destinazione ? [...senza, persona] : senza });
  });
}

export const nominaCapo = (bozze: Bozza[], gruppo: number, persona: string): Bozza[] =>
  bozze.map((b, i) => (i === gruppo && b.membri.includes(persona) ? { ...b, capo: persona } : b));

export const daAssegnare = <P extends { id: string }>(persone: P[], bozze: Bozza[]): P[] =>
  persone.filter((p) => !bozze.some((b) => b.membri.includes(p.id)));

/** Distribuisce tutti a caso, in gruppi che differiscono al massimo di una persona. */
export function aCaso(bozze: Bozza[], persone: string[], caso: () => number = Math.random): Bozza[] {
  const mescolate = [...persone];
  for (let i = mescolate.length - 1; i > 0; i--) {
    const j = Math.floor(caso() * (i + 1));
    [mescolate[i], mescolate[j]] = [mescolate[j]!, mescolate[i]!];
  }
  return bozze.map((b, g) => conCapo({ ...b, capo: null, membri: mescolate.filter((_, i) => i % bozze.length === g) }));
}

/** Perché non si può ancora salvare; stringa vuota se va tutto bene. */
export function problema(bozze: Bozza[]): string {
  if (!bozze.length) return t('Aggiungi almeno un gruppo.');
  if (bozze.some((b) => !b.nome.trim())) return t('Dai un nome a ogni gruppo.');
  const nomi = bozze.map((b) => b.nome.trim().toLowerCase());
  if (new Set(nomi).size !== nomi.length) return t('Due gruppi hanno lo stesso nome.');
  const vuoto = bozze.find((b) => !b.membri.length);
  if (vuoto) return t('«{nome}» è vuoto: mettici qualcuno o toglilo.', { nome: vuoto.nome });
  return '';
}
