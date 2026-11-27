import type { Theme } from './api';

const KEY = 'th-theme-cache';
const PREF = 'th-theme-prefs';
const PREF_VECCHIA = 'th-theme-pref'; // 'system' oppure l'id di un tema

const SAFE_FONT = /^[A-Za-z0-9 \-]{1,40}$/;

export type Modo = 'chiaro' | 'scuro' | 'sistema';
export type Tipo = 'chiaro' | 'scuro';
/** Si sceglie un tema e, separatamente, la modalità: ogni tema ha la versione chiara e quella scura. */
export interface Preferenze { modo: Modo; tema: string }
export const PREFERENZE_BASE: Preferenze = { modo: 'sistema', tema: 'carta' };

export const tipoDi = (pref: Preferenze, prefersDark: boolean): Tipo => (pref.modo === 'sistema' ? (prefersDark ? 'scuro' : 'chiaro') : pref.modo);

/** I temi arrivano dal server ma sono trattati come dati: solo valori colore e nomi font sicuri finiscono nel CSS. */
export function applyTheme(t: Theme, tipo: Tipo): void {
  const root = document.documentElement.style;
  const colori = t[tipo];
  const map: Record<string, string> = {
    '--sfondo': colori.sfondo, '--superficie': colori.superficie, '--inchiostro': colori.inchiostro,
    '--inchiostro-2': colori.inchiostro2, '--attenuato': colori.attenuato, '--filetto': colori.filetto,
    '--accento': colori.accento, '--su-accento': colori.suAccento,
  };
  for (const [k, v] of Object.entries(map)) if (/^#[0-9a-fA-F]{6}$/.test(v)) root.setProperty(k, v);
  const font = (v: string | undefined, fallback: string) => (v && SAFE_FONT.test(v) ? `'${v}', ${fallback}` : undefined);
  const f = {
    '--font-titoli': font(t.font?.titoli, 'Georgia, serif'),
    '--font-testo': font(t.font?.testo, 'system-ui, sans-serif'),
    '--font-dati': font(t.font?.dati, 'ui-monospace, monospace'),
  };
  for (const [k, v] of Object.entries(f)) v ? root.setProperty(k, v) : root.removeProperty(k);
  document.documentElement.dataset.tema = t.id;
  document.documentElement.dataset.tipo = tipo;
  root.colorScheme = tipo === 'scuro' ? 'dark' : 'light';
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', colori.sfondo);
}

export const pickTheme = (themes: Theme[], pref: Preferenze): Theme | undefined =>
  themes.find((t) => t.id === pref.tema) ?? themes.find((t) => t.id === PREFERENZE_BASE.tema) ?? themes[0];

/** Fino alla 0.6 si sceglievano due temi, uno di giorno e uno di sera: «Notte» ora è la versione scura di «Carta». */
export function aggiornaPreferenze(salvate: { modo?: Modo; tema?: string; chiaro?: string; scuro?: string }): Preferenze {
  const modo = salvate.modo ?? PREFERENZE_BASE.modo;
  if (salvate.tema) return { modo, tema: salvate.tema };
  const nuovo = (id: string | undefined) => (id === 'notte' ? 'carta' : id);
  const giorno = nuovo(salvate.chiaro), sera = nuovo(salvate.scuro);
  const preferito = modo === 'scuro' ? sera : modo === 'chiaro' ? giorno : giorno && giorno !== PREFERENZE_BASE.tema ? giorno : sera;
  return { modo, tema: preferito ?? PREFERENZE_BASE.tema };
}

export function getPrefs(): Preferenze {
  try {
    const salvata = localStorage.getItem(PREF);
    if (salvata) return aggiornaPreferenze(JSON.parse(salvata));
    // La primissima versione salvava un solo id di tema.
    const vecchia = localStorage.getItem(PREF_VECCHIA);
    if (vecchia && vecchia !== 'system') {
      const scura = vecchia === 'notte' || vecchia === 'lavagna';
      return aggiornaPreferenze({ modo: scura ? 'scuro' : 'chiaro', [scura ? 'scuro' : 'chiaro']: vecchia });
    }
  } catch { /* storage non disponibile */ }
  return { ...PREFERENZE_BASE };
}
export const setPrefs = (p: Preferenze): void => { try { localStorage.setItem(PREF, JSON.stringify(p)); } catch { /* storage non disponibile */ } };

export function cacheThemes(themes: Theme[]): void { try { localStorage.setItem(KEY, JSON.stringify(themes)); } catch { /* ignora */ } }
/** Scarta quello che era rimasto in memoria nel formato precedente. */
export function cachedThemes(): Theme[] {
  try { return (JSON.parse(localStorage.getItem(KEY) ?? '[]') as Theme[]).filter((t) => t.chiaro && t.scuro); } catch { return []; }
}
