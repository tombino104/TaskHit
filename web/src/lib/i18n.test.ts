import { afterEach, describe, expect, it } from 'vitest';
import { chiaviServer, chiaviWeb } from './chiavi-lingua';
import { preferita, t, usaTesti } from './i18n';

/** Testi scelti mentre l'app gira: nel codice non compaiono dentro `t('…')`. */
const DINAMICI = [
  // campi di una modifica
  'TITOLO', 'DATA', 'ORA', 'FINE', 'MATERIA', 'TIPO', 'MODALITÀ', 'GRUPPO', 'ALUNNO',
  'ha spostato {cosa} da {prima} a {dopo}', 'ha cambiato {cosa} da {prima} a {dopo}',
  'La modifica sarà applicata quando {chi} approva.', 'L’evento sarà pubblicato quando {chi} approva.',
  '1 evento; il giorno più pieno è {giorno}.', '{n} eventi; il giorno più pieno è {giorno}.',
  'chiaro', 'scuro',
  // il nome che il server lascia sugli eventi di un account eliminato
  'Account eliminato',
  // tipi di evento e modalità creati insieme al database
  'Verifica', 'Verifica di recupero', 'Interrogazione', 'Corso', 'Rientro', 'FSL',
  'teorica', 'pratica', 'scritta', 'recupero', 'scolastico', 'extrascolastico',
  'Teorica', 'Pratica', 'Scritta', 'Scolastico', 'Extrascolastico',
];

const grezzi = (file: Record<string, unknown>) =>
  Object.entries(file).filter(([f]) => !/\.test\.ts$|chiavi-lingua|i18n\.ts/.test(f)).map(([, testo]) => testo as string);
const web = grezzi(import.meta.glob('../**/*.{ts,svelte}', { query: '?raw', import: 'default', eager: true }));
const server = grezzi(import.meta.glob('../../../server/src/**/*.rs', { query: '?raw', import: 'default', eager: true }));
const inglese = grezzi(import.meta.glob('../../../server/languages-default/en.json', { query: '?raw', import: 'default', eager: true }));
const segnaposti = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe('traduzione', () => {
  afterEach(() => usaTesti({}));
  it('senza traduzione resta l’italiano, e riempie i segnaposto', () => {
    expect(t('Settimana {n}', { n: 4 })).toBe('Settimana 4');
    usaTesti({ 'Settimana {n}': 'Week {n}' });
    expect(t('Settimana {n}', { n: 4 })).toBe('Week 4');
    expect(t('constructor')).toBe('constructor');
  });
  it('sceglie la lingua del dispositivo, altrimenti l’inglese', () => {
    const elenco = [{ id: 'it', nome: 'Italiano' }, { id: 'en', nome: 'English' }];
    expect([preferita(['it-IT'], elenco), preferita(['en-GB'], elenco), preferita(['de-DE', 'it'], elenco), preferita(['de-DE'], elenco)]).toEqual(['it', 'en', 'it', 'en']);
  });
});

describe('file inglese', () => {
  const en = (JSON.parse(inglese[0]!) as { testi: Record<string, string> }).testi;
  const usati = new Set([...web.flatMap(chiaviWeb), ...server.flatMap(chiaviServer), ...DINAMICI]);
  it('traduce ogni testo dell’app e del server', () => {
    expect([...usati].filter((k) => !(k in en))).toEqual([]);
  });
  it('non ha voci rimaste indietro', () => {
    expect(Object.keys(en).filter((k) => !usati.has(k))).toEqual([]);
  });
  it('usa gli stessi segnaposto dell’italiano', () => {
    expect(Object.entries(en).filter(([k, v]) => segnaposti(v).some((s) => !segnaposti(k).includes(s)))).toEqual([]);
  });
});
