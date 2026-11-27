import { describe, expect, it } from 'vitest';
import { aCaso, bozzeVuote, daAssegnare, dirittoSu, nominaCapo, problema, sposta } from './lavori';
import type { Gruppo } from './api';

const persone = ['a', 'b', 'c', 'd', 'e'].map((id) => ({ id }));

describe('lavagna dei gruppi', () => {
  it('il primo che entra è capogruppo; spostandolo il ruolo passa a chi resta', () => {
    let b = sposta(sposta(bozzeVuote(2), 'a', 0), 'b', 0);
    expect(b[0]).toMatchObject({ membri: ['a', 'b'], capo: 'a' });
    b = sposta(b, 'a', 1);
    expect(b[0]).toMatchObject({ membri: ['b'], capo: 'b' });
    expect(b[1]).toMatchObject({ membri: ['a'], capo: 'a' });
    expect(daAssegnare(persone, b).map((p) => p.id)).toEqual(['c', 'd', 'e']);
    expect(sposta(b, 'a', null)[1]).toMatchObject({ membri: [], capo: null });
  });

  it('rimettere nello stesso gruppo non cambia nulla, e il capogruppo si sceglie solo tra i membri', () => {
    const b = nominaCapo(sposta(sposta(bozzeVuote(1), 'a', 0), 'b', 0), 0, 'b');
    expect(sposta(b, 'b', 0)).toBe(b);
    expect(nominaCapo(b, 0, 'z')[0]!.capo).toBe('b');
  });

  it('a caso: tutti una volta sola, in gruppi equilibrati', () => {
    const b = aCaso(bozzeVuote(2), persone.map((p) => p.id), () => 0.42);
    expect(b.flatMap((g) => g.membri).sort()).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(b.map((g) => g.membri.length).sort()).toEqual([2, 3]);
    expect(b.every((g) => g.capo && g.membri.includes(g.capo))).toBe(true);
    expect(problema(b)).toBe('');
  });

  it('segnala gruppi vuoti e nomi doppi', () => {
    expect(problema(bozzeVuote(1))).toContain('vuoto');
    const b = sposta(sposta(bozzeVuote(2), 'a', 0), 'b', 1);
    b[1]!.nome = 'gruppo 1';
    expect(problema(b)).toContain('stesso nome');
  });
});

describe('diritti per gruppo', () => {
  const lavoro = (leader: string, fine = 2000): Gruppo => ({
    id: 'g', name: 'G', kind: 'gruppo', mine: true, manage: false, parent_id: 'c', members: [],
    work: { activity: 'x', leader_id: leader, created_at: 0, expires_at: fine, max_expires_at: 9999, deletes_at: 9999, closed: false, representative: false, can_manage: false },
  });
  it('nel gruppo di lavoro decide il capogruppo, anche se alunno; il rappresentante propone', () => {
    expect(dirittoSu({ id: 'u', role: 'alunno' }, lavoro('u'), 1000)).toBe('diretto');
    expect(dirittoSu({ id: 'r', role: 'rappresentante' }, lavoro('u'), 1000)).toBe('proposta');
    expect(dirittoSu({ id: 'u', role: 'alunno' }, lavoro('u', 500), 1000)).toBe('no');
    expect(dirittoSu({ id: 'u', role: 'alunno' }, { ...lavoro('u'), work: null }, 1000)).toBe('no');
  });
});
