// Le lingue: un file JSON per lingua, come i temi. L'italiano è la lingua di partenza ed è scritto nel
// codice: ogni testo italiano è anche la sua chiave, e un file di lingua dice come tradurlo.
// Un testo senza traduzione resta in italiano.

export interface Lingua { id: string; nome: string }
export interface FileLingua extends Lingua { testi: Record<string, string> }

export const ITALIANO: Lingua = { id: 'it', nome: 'Italiano' };
const SCELTA = 'th-lingua';
const COPIA = 'th-lingua-testi';
const ELENCO = 'th-lingue';

let testi: Record<string, string> = {};
let attiva = ITALIANO.id;
let disponibili: Lingua[] = [ITALIANO];

/** Il testo nella lingua scelta; i segnaposto `{nome}` si riempiono con `valori`. */
export function t(chiave: string, valori?: Record<string, unknown>): string {
  const testo = Object.hasOwn(testi, chiave) ? testi[chiave]! : chiave;
  if (!valori) return testo;
  return testo.replace(/\{(\w+)\}/g, (segno, nome: string) => (Object.hasOwn(valori, nome) ? String(valori[nome]) : segno));
}

export const lingue = (): Lingua[] => disponibili;
export const linguaAttiva = (): string => attiva;
export function usaTesti(nuovi: Record<string, string>): void { testi = nuovi; }

/** La lingua del dispositivo se c'è; altrimenti l'inglese, che capisce più gente. */
export function preferita(delDispositivo: readonly string[], elenco: Lingua[]): string {
  for (const voce of delDispositivo) {
    const id = voce.toLowerCase().split('-')[0]!;
    if (elenco.some((l) => l.id === id)) return id;
  }
  return elenco.some((l) => l.id === 'en') ? 'en' : ITALIANO.id;
}

const leggi = <T>(chiave: string): T | null => { try { return JSON.parse(localStorage.getItem(chiave) ?? 'null') as T | null; } catch { return null; } };
const scrivi = (chiave: string, valore: unknown): void => { try { localStorage.setItem(chiave, JSON.stringify(valore)); } catch { /* storage non disponibile */ } };

async function scarica<T>(percorso: string): Promise<T | null> {
  try {
    const r = await fetch(percorso, { signal: AbortSignal.timeout(4000) });
    return r.ok ? ((await r.json()) as T) : null;
  } catch { return null; }
}

/** Si chiama prima di caricare l'interfaccia, così ogni testo nasce già nella lingua giusta. */
export async function preparaLingua(): Promise<void> {
  const elenco = (await scarica<{ languages: Lingua[] }>('/api/languages'))?.languages;
  if (elenco) scrivi(ELENCO, elenco);
  const altre = (elenco ?? leggi<Lingua[]>(ELENCO) ?? []).filter((l) => l.id !== ITALIANO.id);
  disponibili = [ITALIANO, ...altre];
  const scelta = leggi<string>(SCELTA);
  attiva = scelta && disponibili.some((l) => l.id === scelta) ? scelta : preferita(navigator.languages ?? [navigator.language], disponibili);
  if (attiva !== ITALIANO.id) {
    let file = await scarica<FileLingua>(`/api/languages/${encodeURIComponent(attiva)}`);
    if (file) scrivi(COPIA, file);
    else file = leggi<FileLingua>(COPIA); // offline: l'ultima copia
    if (file?.id === attiva) testi = file.testi;
    else attiva = ITALIANO.id;
  }
  document.documentElement.lang = attiva;
}

/** Cambiare lingua ricarica l'app. */
export function scegliLingua(id: string): void {
  scrivi(SCELTA, id);
  location.reload();
}
