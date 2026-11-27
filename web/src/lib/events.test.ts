import { describe, expect, it } from 'vitest';
import type { Meta } from './api';
import * as D from './dates';
import { differenze, filtra, fraseOggi, fraseStoria, iniziali, perGiorno, righeModifiche, SENZA_FILTRI, testoAvvisi, testoSu, type Evento } from './events';
import { codiceLeggibile, linkInvito } from './inviti';
import { pickTheme, PREFERENZE_BASE } from './theme';
import type { Theme } from './api';

const meta: Meta = {
  tags: [{ id: 'mat', name: 'Matematica', color: '#E4572E' }],
  labels: [{ id: 'ver', name: 'Verifica', subs: ['teorica'], personal: false }, { id: 'int', name: 'Interrogazione', subs: [], personal: true }],
  groups: [], pending: 0, settings: { instance_name: '', public_url: '', school_year: '', timezone: '' },
  plugins: { grafico: { user: true, instance: true } },
};
const quando = (giorno: string, ora = '08:00') => D.aSecondi(giorno, ora);
const evento = (id: string, giorno: string, extra: Partial<Evento> = {}): Evento => ({
  id, title: id, description: null, tag_id: 'mat', label_id: 'ver', sub_kind: null, starts_at: quando(giorno), ends_at: null, alerts: [],
  privato: false, rev: 1, owner_id: 'u', group_id: 'g', subject_id: null, created_at: 0, updated_at: 0,
  created_by: { id: 'u', name: 'U', role: 'admin' }, updated_by: { name: 'U', role: 'admin' }, approved_by: null, proposed: false, ...extra,
});

describe('date', () => {
  it('la settimana inizia di lunedì', () => {
    expect(D.settimana(D.daChiave('2026-11-29')).map(D.chiave)[0]).toBe('2026-11-23');
    expect(D.numeroSettimana(D.daChiave('2026-11-24'))).toBe(48);
    expect(D.numeroSettimana(D.daChiave('2027-01-01'))).toBe(53);
  });
  it('la griglia del mese ha sei settimane e comprende i giorni vicini', () => {
    const g = D.grigliaMese(D.daChiave('2026-11-15')).map(D.chiave);
    expect([g.length, g[0], g[41]]).toEqual([42, '2026-10-26', '2026-12-06']);
  });
  it('cambiare mese non salta febbraio', () => {
    expect(D.chiave(D.piuMesi(D.daChiave('2027-01-31'), 1))).toBe('2027-02-28');
  });
  it('rifiuta chiavi di giorno inesistenti', () => {
    expect([D.chiaveValida('2026-11-24'), D.chiaveValida('2026-02-30'), D.chiaveValida('ciao')]).toEqual([true, false, false]);
  });
  it('scrive date in italiano', () => {
    expect(D.dataBreve(D.daChiave('2026-11-26'))).toBe('Gio 26 nov');
    expect(D.dataLunga(D.daChiave('2026-11-24'))).toBe('Martedì 24 novembre');
  });
});

describe('eventi', () => {
  const lista = [evento('b', '2026-11-26'), evento('a', '2026-11-24', { label_id: 'int', privato: true }), evento('c', '2026-11-26', { tag_id: null })];

  it('filtra per tipo, materia e visibilità; liste vuote non filtrano', () => {
    expect(filtra(lista, SENZA_FILTRI)).toHaveLength(3);
    expect(filtra(lista, { ...SENZA_FILTRI, tipi: ['int'] }).map((e) => e.id)).toEqual(['a']);
    expect(filtra(lista, { ...SENZA_FILTRI, materie: ['mat'] }).map((e) => e.id)).toEqual(['b', 'a']);
    expect(filtra(lista, { ...SENZA_FILTRI, visibilita: 'pubblici' }).map((e) => e.id)).toEqual(['b', 'c']);
  });
  it('raggruppa per giorno', () => {
    expect([...perGiorno(lista)].map(([k, l]) => [k, l.length])).toEqual([['2026-11-24', 1], ['2026-11-26', 2]]);
  });
  it('riassume la giornata', () => {
    expect(fraseOggi(lista, meta, D.daChiave('2026-11-24'))).toBe('Oggi hai un impegno. Giovedì ci sono due verifiche.');
    expect(fraseOggi([], meta, D.daChiave('2026-11-24'))).toBe('Oggi non hai impegni.');
  });
  it('racconta uno spostamento senza ripetere la fine se la durata non cambia', () => {
    const changes = { starts_at: [quando('2026-11-24', '10:00'), quando('2026-11-24')] as [number, number], ends_at: [quando('2026-11-24', '11:50'), quando('2026-11-24', '09:50')] as [number, number] };
    expect(righeModifiche(meta, changes).map((r) => r.campo)).toEqual(['ORA']);
    expect(fraseStoria(meta, { at: 0, kind: 'modified', changes, actor: 'Luca', approver: 'Giulia' }))
      .toBe("Luca ha spostato l'ora da 10:00 a 08:00. Approvato da Giulia.");
  });
  it('trova solo i campi diversi', () => {
    expect(differenze({ title: 'a', alerts: ['g1'], tag_id: null }, { title: 'b', alerts: ['g1'], tag_id: null })).toEqual({ title: ['a', 'b'] });
  });
  it('formatta avvisi, iniziali e codici', () => {
    expect(testoAvvisi(['sera', 'g2'])).toBe('2 giorni prima e la sera prima');
    expect([iniziali('Giulia Ferri'), iniziali('Lorenzo')]).toEqual(['GF', 'LO']);
    expect(codiceLeggibile('K7Q4BX29M')).toBe('K7Q-4BX-29M');
    expect(linkInvito('https://esempio.test/', 'K7Q4BX29M', 'http://x')).toBe('https://esempio.test/#/entra/K7Q-4BX-29M');
  });
  it('sceglie un testo leggibile sul colore della materia', () => {
    expect([testoSu('#E4572E'), testoSu('#101050'), testoSu('#F2C94C')]).toEqual(['#17171B', '#FFFFFF', '#17171B']);
  });
});

describe('temi', () => {
  const tema = (id: string, tipo: Theme['tipo']) => ({ id, nome: id, tipo }) as Theme;
  const temi = [tema('carta', 'chiaro'), tema('lavagna', 'scuro'), tema('notte', 'scuro'), tema('quaderno', 'chiaro')];
  it('segue il sistema: Carta di giorno e Notte di sera', () => {
    expect([pickTheme(temi, PREFERENZE_BASE, false)?.id, pickTheme(temi, PREFERENZE_BASE, true)?.id]).toEqual(['carta', 'notte']);
  });
  it('rispetta le scelte e ripiega se un tema sparisce', () => {
    expect(pickTheme(temi, { modo: 'scuro', chiaro: 'carta', scuro: 'lavagna' }, false)?.id).toBe('lavagna');
    expect(pickTheme(temi, { modo: 'chiaro', chiaro: 'cancellato', scuro: 'notte' }, true)?.id).toBe('carta');
  });
});
