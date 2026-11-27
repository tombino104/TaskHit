// Moderazione degli account: testi di motivi, durate e blocchi. Le regole stanno sul server.
import type { Blocco } from './api';
import { daSecondi, mese, ora } from './dates';

export const MOTIVI: { id: string; nome: string }[] = [
  { id: 'nome', nome: 'Nome non adeguato' },
  { id: 'foto', nome: 'Foto non adeguata' },
  { id: 'comportamento', nome: 'Comportamento scorretto' },
  { id: 'altro', nome: 'Altro' },
];
/** `null` = finché qualcuno non riattiva l'account. */
export const DURATE: [number | null, string][] = [[1, '1 giorno'], [2, '2 giorni'], [7, '1 settimana'], [14, '2 settimane'], [30, '1 mese'], [null, 'Fino a riattivazione']];

export const nomiMotivi = (ids: string[]): string => MOTIVI.filter((m) => ids.includes(m.id)).map((m) => m.nome).join(', ');

const quando = (s: number) => `${daSecondi(s).getDate()} ${mese(daSecondi(s))} alle ${ora(s)}`;
/** "Sospeso fino al 3 dicembre alle 10:00", "Sospeso", "Espulso". */
export const statoBlocco = (b: Blocco): string => (b.kind === 'espulso' ? 'Espulso' : b.until ? `Sospeso fino al ${quando(b.until)}` : 'Sospeso');
