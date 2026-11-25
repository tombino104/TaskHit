import type { Theme } from './api';

const KEY = 'th-theme-cache';
const PREF = 'th-theme-prefs';
const PREF_VECCHIA = 'th-theme-pref'; // 'system' oppure l'id di un tema

const SAFE_FONT = /^[A-Za-z0-9 \-]{1,40}$/;

export type Modo = 'chiaro' | 'scuro' | 'sistema';
/** Si sceglie la modalità e, separatamente, il tema da usare di giorno e di sera. */
export interface Preferenze { modo: Modo; chiaro: string; scuro: string }
export const PREFERENZE_BASE: Preferenze = { modo: 'sistema', chiaro: 'carta', scuro: 'notte' };

/** I temi arrivano dal server ma sono trattati come dati: solo valori colore e nomi font sicuri finiscono nel CSS. */
export function applyTheme(t: Theme): void {
  const root = document.documentElement.style;
  const map: Record<string, string> = {
    '--sfondo': t.colori.sfondo, '--superficie': t.colori.superficie, '--inchiostro': t.colori.inchiostro,
    '--inchiostro-2': t.colori.inchiostro2, '--attenuato': t.colori.attenuato, '--filetto': t.colori.filetto,
    '--accento': t.colori.accento, '--su-accento': t.colori.suAccento,
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
  document.documentElement.dataset.tipo = t.tipo;
  root.colorScheme = t.tipo === 'scuro' ? 'dark' : 'light';
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', t.colori.sfondo);
}

export function pickTheme(themes: Theme[], pref: Preferenze, prefersDark: boolean): Theme | undefined {
  const tipo = pref.modo === 'sistema' ? (prefersDark ? 'scuro' : 'chiaro') : pref.modo;
  const delTipo = themes.filter((t) => t.tipo === tipo);
  return delTipo.find((t) => t.id === pref[tipo]) ?? delTipo.find((t) => t.id === PREFERENZE_BASE[tipo]) ?? delTipo[0] ?? themes[0];
}

export function getPrefs(): Preferenze {
  try {
    const salvata = localStorage.getItem(PREF);
    if (salvata) return { ...PREFERENZE_BASE, ...JSON.parse(salvata) };
    // Versione precedente: un solo id di tema.
    const vecchia = localStorage.getItem(PREF_VECCHIA);
    const t = cachedThemes().find((x) => x.id === vecchia);
    if (t) return { ...PREFERENZE_BASE, modo: t.tipo, [t.tipo]: t.id };
  } catch { /* storage non disponibile */ }
  return { ...PREFERENZE_BASE };
}
export const setPrefs = (p: Preferenze): void => { try { localStorage.setItem(PREF, JSON.stringify(p)); } catch { /* storage non disponibile */ } };

export function cacheThemes(themes: Theme[]): void { try { localStorage.setItem(KEY, JSON.stringify(themes)); } catch { /* ignora */ } }
export function cachedThemes(): Theme[] { try { return JSON.parse(localStorage.getItem(KEY) ?? '[]'); } catch { return []; } }
