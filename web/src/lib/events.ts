// Il modello degli eventi dell'interfaccia e le funzioni pure che lo trasformano.
import type { Campi, EventoServer, Fotografia, Gruppo, Meta, Modifiche, Ruolo, VoceStoria } from './api';
import { chiave, chiaveDi, daSecondi, dataBreve, nomeGiorno, maiuscola, ora, piuGiorni } from './dates';

/** Un evento pronto da mostrare: per quelli privati i campi arrivano dalla decifratura. */
export interface Evento extends Campi {
  id: string; privato: boolean; rev: number; owner_id: string; group_id: string | null; subject_id: string | null;
  created_at: number; updated_at: number; created_by: EventoServer['created_by']; updated_by: EventoServer['updated_by'];
  approved_by: string | null; proposed: boolean;
}

export type Visibilita = 'tutti' | 'pubblici' | 'privati';
/** Liste vuote = nessun filtro. */
export interface Filtri { tipi: string[]; materie: string[]; visibilita: Visibilita }
export const SENZA_FILTRI: Filtri = { tipi: [], materie: [], visibilita: 'tutti' };
export const filtriAttivi = (f: Filtri): number => f.tipi.length + f.materie.length + (f.visibilita === 'tutti' ? 0 : 1);

export function filtra(eventi: Evento[], f: Filtri): Evento[] {
  return eventi.filter((e) =>
    (!f.tipi.length || (e.label_id !== null && f.tipi.includes(e.label_id))) &&
    (!f.materie.length || (e.tag_id !== null && f.materie.includes(e.tag_id))) &&
    (f.visibilita === 'tutti' || (f.visibilita === 'privati') === e.privato));
}

/** Aggiunge o toglie un valore da una lista. */
export const commuta = <T>(lista: T[], v: T): T[] => (lista.includes(v) ? lista.filter((x) => x !== v) : [...lista, v]);

/** Raggruppa per giorno di inizio, in ordine di orario. */
export function perGiorno(eventi: Evento[]): Map<string, Evento[]> {
  const out = new Map<string, Evento[]>();
  for (const e of [...eventi].sort((a, b) => a.starts_at - b.starts_at)) {
    const k = chiaveDi(e.starts_at);
    const lista = out.get(k);
    if (lista) lista.push(e); else out.set(k, [e]);
  }
  return out;
}

export const materiaDi = (meta: Meta | null, id: string | null) => meta?.tags.find((t) => t.id === id);
export const tipoDi = (meta: Meta | null, id: string | null) => meta?.labels.find((t) => t.id === id);
export const colore = (meta: Meta | null, tagId: string | null): string => materiaDi(meta, tagId)?.color ?? 'var(--attenuato)';

/** "Verifica teorica", "Corso extrascolastico", "Interrogazione". */
export function nomeTipo(meta: Meta | null, labelId: string | null, sub: string | null): string {
  const t = tipoDi(meta, labelId);
  if (!t) return 'Evento';
  if (sub === 'recupero') return `${t.name} di recupero`;
  return sub ? `${t.name} ${sub}` : t.name;
}

/** "Scritta", "Pratica", "Di recupero". */
export const nomeModo = (sub: string): string => (sub === 'recupero' ? 'Di recupero' : sub.charAt(0).toUpperCase() + sub.slice(1));

/** "Matematica · Verifica teorica" */
export function riassunto(meta: Meta | null, e: Pick<Campi, 'tag_id' | 'label_id' | 'sub_kind'>): string {
  return [materiaDi(meta, e.tag_id)?.name, nomeTipo(meta, e.label_id, e.sub_kind)].filter(Boolean).join(' · ');
}

export const gruppoDi = (meta: Meta | null, id: string | null): Gruppo | undefined => meta?.groups.find((g) => g.id === id);
export const etichettaGruppo = (g: Gruppo | undefined): string => (!g ? 'Gruppo' : g.kind === 'classe' ? `Classe ${g.name}` : g.work ? g.name : `Gruppo ${g.name}`);

export const NOMI_RUOLO: Record<Ruolo, string> = { admin: 'Admin', rappresentante: 'Rappresentante', nerd: 'Nerd', alunno: 'Alunno' };
/** Inviti, richieste, gruppi e plugin: admin e rappresentanti. */
export const puoGestire = (r: Ruolo): boolean => r === 'admin' || r === 'rappresentante';
export const puoPubblicare = (r: Ruolo): 'diretto' | 'proposta' | 'no' =>
  r === 'rappresentante' ? 'diretto' : r === 'nerd' ? 'proposta' : 'no';

export const AVVISI: { id: string; nome: string }[] = [
  { id: 'g1', nome: '1 giorno prima' },
  { id: 'g2', nome: '2 giorni prima' },
  { id: 'sera', nome: 'La sera prima' },
  { id: 'sett', nome: '1 settimana prima' },
];
export function testoAvvisi(avvisi: string[]): string {
  const nomi = AVVISI.filter((a) => avvisi.includes(a.id)).map((a, i) => (i ? a.nome.charAt(0).toLowerCase() + a.nome.slice(1) : a.nome));
  if (!nomi.length) return 'Nessuno';
  return nomi.length === 1 ? nomi[0]! : `${nomi.slice(0, -1).join(', ')} e ${nomi.at(-1)}`;
}

export const iniziali = (nome: string): string => {
  const parti = nome.trim().split(/\s+/).filter(Boolean);
  const lettere = parti.length > 1 ? [parti[0]!, parti.at(-1)!].map((p) => [...p][0]) : [...(parti[0] ?? '?')].slice(0, 2);
  return lettere.join('').toUpperCase();
};

/** Colore del testo (scuro o chiaro) più leggibile sopra una tinta scelta dall'utente. */
export function testoSu(sfondo: string): string {
  if (!/^#[0-9a-fA-F]{6}$/.test(sfondo)) return '#17171B';
  const [r, g, b] = [1, 3, 5].map((i) => {
    const s = parseInt(sfondo.slice(i, i + 2), 16) / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  const l = 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
  const scuro = (l + 0.05) / (0.0091 + 0.05); // contrasto con #17171B
  const chiaro = 1.05 / (l + 0.05);
  return scuro >= chiaro ? '#17171B' : '#FFFFFF';
}

const NUMERI = ['zero', 'un', 'due', 'tre', 'quattro', 'cinque', 'sei', 'sette', 'otto', 'nove', 'dieci'];
const inParole = (n: number) => NUMERI[n] ?? String(n);

/** La frase d'apertura di «Oggi»: quanti impegni ci sono e il prossimo giorno impegnativo. */
export function fraseOggi(eventi: Evento[], meta: Meta | null, adesso: Date): string {
  const giorni = perGiorno(eventi);
  const n = giorni.get(chiave(adesso))?.length ?? 0;
  const prima = n === 0 ? 'Oggi non hai impegni.' : n === 1 ? 'Oggi hai un impegno.' : `Oggi hai ${inParole(n)} impegni.`;
  const verifiche = (k: string) => (giorni.get(k) ?? []).filter((e) => tipoDi(meta, e.label_id)?.name.startsWith('Verifica')).length;
  const prossimi = Array.from({ length: 6 }, (_, i) => piuGiorni(adesso, i + 1));
  const quando = (d: Date, i: number) => (i === 0 ? 'Domani' : maiuscola(nomeGiorno(d)));
  const pieno = prossimi.findIndex((d) => verifiche(chiave(d)) >= 2);
  if (pieno >= 0) return `${prima} ${quando(prossimi[pieno]!, pieno)} ci sono ${inParole(verifiche(chiave(prossimi[pieno]!)))} verifiche.`;
  const primaVerifica = prossimi.findIndex((d) => verifiche(chiave(d)) === 1);
  if (primaVerifica >= 0) return `${prima} ${quando(prossimi[primaVerifica]!, primaVerifica)} c'è una verifica.`;
  return prima;
}

export interface RigaModifica { campo: string; prima: string; dopo: string }

/** Le differenze tra due versioni, in parole: usate da Inbox, approvazioni e cronologia. */
export function righeModifiche(meta: Meta | null, modifiche: Modifiche): RigaModifica[] {
  const out: RigaModifica[] = [];
  const aggiungi = (campo: string, prima: string, dopo: string) => { if (prima !== dopo) out.push({ campo, prima, dopo }); };
  const m = modifiche as Record<string, [any, any] | undefined>;
  const persone = new Map(meta?.groups.flatMap((g) => g.members.map((p) => [p.id, p.name] as const)) ?? []);
  if (m.title) aggiungi('TITOLO', m.title[0], m.title[1]);
  if (m.starts_at) {
    const [a, b] = m.starts_at as [number, number];
    aggiungi('DATA', dataBreve(daSecondi(a)), dataBreve(daSecondi(b)));
    aggiungi('ORA', ora(a), ora(b));
  }
  // Se inizio e fine si spostano insieme, la durata non cambia e basta dire il nuovo orario.
  const stessaDurata = m.starts_at && m.ends_at && m.ends_at[0] !== null && m.ends_at[1] !== null
    && m.ends_at[0] - m.starts_at[0] === m.ends_at[1] - m.starts_at[1];
  if (m.ends_at && !stessaDurata) aggiungi('FINE', m.ends_at[0] ? ora(m.ends_at[0]) : '—', m.ends_at[1] ? ora(m.ends_at[1]) : '—');
  if (m.tag_id) aggiungi('MATERIA', materiaDi(meta, m.tag_id[0])?.name ?? '—', materiaDi(meta, m.tag_id[1])?.name ?? '—');
  if (m.label_id) aggiungi('TIPO', tipoDi(meta, m.label_id[0])?.name ?? '—', tipoDi(meta, m.label_id[1])?.name ?? '—');
  if (m.sub_kind) aggiungi('MODALITÀ', m.sub_kind[0] ?? '—', m.sub_kind[1] ?? '—');
  if (m.group_id) aggiungi('GRUPPO', gruppoDi(meta, m.group_id[0])?.name ?? '—', gruppoDi(meta, m.group_id[1])?.name ?? '—');
  if (m.subject_id) aggiungi('ALUNNO', persone.get(m.subject_id[0]) ?? '—', persone.get(m.subject_id[1]) ?? '—');
  if (m.description) aggiungi('TESTO', m.description[0] ?? '—', m.description[1] ?? '—');
  if (m.alerts) aggiungi('AVVISO', testoAvvisi(m.alerts[0] ?? []), testoAvvisi(m.alerts[1] ?? []));
  return out;
}

/** Le modifiche tra la versione attuale e quella proposta. */
export function differenze(prima: object, dopo: object): Modifiche {
  const a = prima as Record<string, unknown>;
  return Object.fromEntries(
    Object.entries(dopo).filter(([k, v]) => JSON.stringify(a[k] ?? null) !== JSON.stringify(v ?? null)).map(([k, v]) => [k, [a[k] ?? null, v]]),
  ) as Modifiche;
}

const FRASI: Record<string, string> = { TITOLO: 'il titolo', DATA: 'la data', ORA: "l'ora", FINE: "l'ora di fine", MATERIA: 'la materia', TIPO: 'il tipo', 'MODALITÀ': 'la modalità', GRUPPO: 'il gruppo', ALUNNO: "l'alunno" };

/** "Luca Conti ha spostato l'ora da 10:00 a 08:00. Approvato da Giulia Ferri." */
export function fraseStoria(meta: Meta | null, v: VoceStoria): string {
  const chi = v.actor ?? 'Qualcuno';
  let testo: string;
  if (v.kind === 'added') testo = `${chi} ha creato l'evento.`;
  else if (v.kind === 'deleted') testo = `${chi} ha eliminato l'evento.`;
  else {
    const parti = righeModifiche(meta, v.changes ?? {}).map((r) => {
      if (r.campo === 'TESTO') return 'ha aggiornato la descrizione';
      if (r.campo === 'AVVISO') return 'ha cambiato gli avvisi';
      const verbo = r.campo === 'DATA' || r.campo === 'ORA' ? 'ha spostato' : 'ha cambiato';
      return `${verbo} ${FRASI[r.campo] ?? 'un campo'} da ${r.prima} a ${r.dopo}`;
    });
    testo = `${chi} ${parti.length ? parti.join(', ') : "ha modificato l'evento"}.`;
  }
  return v.approver ? `${testo} Approvato da ${v.approver}.` : testo;
}

/** La notifica di un evento in una riga: "Matematica · Verifica teorica · Mar 24 nov, 08:00". */
export const metaNotifica = (meta: Meta | null, f: Fotografia): string =>
  `${riassunto(meta, f)} · ${dataBreve(daSecondi(f.starts_at))}, ${ora(f.starts_at)}`;
