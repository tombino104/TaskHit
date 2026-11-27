// Date in italiano e conti sul calendario, nel fuso del dispositivo. La settimana inizia di lunedì.
import { t } from './i18n';

export const MESI = [t('gennaio'), t('febbraio'), t('marzo'), t('aprile'), t('maggio'), t('giugno'), t('luglio'), t('agosto'), t('settembre'), t('ottobre'), t('novembre'), t('dicembre')];
export const GIORNI = [t('lunedì'), t('martedì'), t('mercoledì'), t('giovedì'), t('venerdì'), t('sabato'), t('domenica')];

export const maiuscola = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);
const due = (n: number) => String(n).padStart(2, '0');

/** Secondi Unix -> Date. */
export const daSecondi = (s: number): Date => new Date(s * 1000);

/** Chiave di un giorno locale, es. "2026-11-24": ordinabile e confrontabile. */
export const chiave = (d: Date): string => `${d.getFullYear()}-${due(d.getMonth() + 1)}-${due(d.getDate())}`;
export const chiaveDi = (secondi: number): string => chiave(daSecondi(secondi));

export function daChiave(k: string): Date {
  const [a, m, g] = k.split('-').map(Number);
  return new Date(a!, m! - 1, g!);
}
export const chiaveValida = (k: string): boolean => /^\d{4}-\d{2}-\d{2}$/.test(k) && chiave(daChiave(k)) === k;

export const piuGiorni = (d: Date, n: number): Date => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
/** Stesso giorno in un altro mese; se non esiste (31 -> febbraio) si ferma all'ultimo. */
export function piuMesi(d: Date, n: number): Date {
  const ultimo = new Date(d.getFullYear(), d.getMonth() + n + 1, 0).getDate();
  return new Date(d.getFullYear(), d.getMonth() + n, Math.min(d.getDate(), ultimo));
}

/** 0 = lunedì … 6 = domenica. */
export const indiceGiorno = (d: Date): number => (d.getDay() + 6) % 7;
export const lunedi = (d: Date): Date => piuGiorni(d, -indiceGiorno(d));
export const settimana = (d: Date): Date[] => Array.from({ length: 7 }, (_, i) => piuGiorni(lunedi(d), i));

/** Le 42 caselle (6 settimane) della griglia di un mese. */
export function grigliaMese(d: Date): Date[] {
  const inizio = lunedi(new Date(d.getFullYear(), d.getMonth(), 1));
  return Array.from({ length: 42 }, (_, i) => piuGiorni(inizio, i));
}

/** Numero di settimana ISO 8601. */
export function numeroSettimana(d: Date): number {
  const t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
  t.setUTCDate(t.getUTCDate() + 4 - (t.getUTCDay() || 7));
  const inizio = Date.UTC(t.getUTCFullYear(), 0, 1);
  return Math.ceil(((t.getTime() - inizio) / 86_400_000 + 1) / 7);
}

export const nomeGiorno = (d: Date): string => GIORNI[indiceGiorno(d)]!;
export const giornoBreve = (d: Date): string => nomeGiorno(d).slice(0, 3);
export const mese = (d: Date): string => MESI[d.getMonth()]!;
export const meseBreve = (d: Date): string => mese(d).slice(0, 3);

export const ora = (secondi: number): string => { const d = daSecondi(secondi); return `${due(d.getHours())}:${due(d.getMinutes())}`; };
export const orario = (inizio: number, fine: number | null): string => (fine ? `${ora(inizio)}–${ora(fine)}` : ora(inizio));

/** "Gio 26 nov" */
export const dataBreve = (d: Date): string => `${maiuscola(giornoBreve(d))} ${d.getDate()} ${meseBreve(d)}`;
/** "Martedì 24 novembre" */
export const dataLunga = (d: Date): string => `${maiuscola(nomeGiorno(d))} ${d.getDate()} ${mese(d)}`;
/** "20 nov, 17:42" */
export const momento = (secondi: number): string => { const d = daSecondi(secondi); return `${d.getDate()} ${meseBreve(d)}, ${ora(secondi)}`; };

/** "Oggi", "Ieri" oppure "Venerdì 20 novembre". */
export function giornoRelativo(secondi: number, adesso: Date): string {
  const k = chiaveDi(secondi);
  if (k === chiave(adesso)) return t('Oggi');
  if (k === chiave(piuGiorni(adesso, -1))) return t('Ieri');
  return dataLunga(daSecondi(secondi));
}

/** "2 ore fa", "ieri, 19:20", "20 nov, 17:42" */
export function quantoFa(secondi: number, adesso: Date): string {
  const minuti = Math.floor((adesso.getTime() / 1000 - secondi) / 60);
  if (minuti < 1) return t('adesso');
  if (minuti < 60) return t('{n} min fa', { n: minuti });
  if (chiaveDi(secondi) === chiave(adesso)) { const h = Math.floor(minuti / 60); return h === 1 ? t("un'ora fa") : t('{n} ore fa', { n: h }); }
  if (chiaveDi(secondi) === chiave(piuGiorni(adesso, -1))) return t('ieri, {ora}', { ora: ora(secondi) });
  return momento(secondi);
}

/** Da "2026-11-26" e "08:00" a secondi Unix. */
export function aSecondi(giorno: string, hhmm: string): number {
  const d = daChiave(giorno);
  const [h, m] = hhmm.split(':').map(Number);
  d.setHours(h!, m!, 0, 0);
  return Math.floor(d.getTime() / 1000);
}
