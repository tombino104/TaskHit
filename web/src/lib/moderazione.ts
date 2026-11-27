// Moderazione degli account: testi di motivi, durate e blocchi. Le regole stanno sul server.
import { t } from './i18n';
import type { Blocco } from './api';
import { daSecondi, mese, ora } from './dates';

export const MOTIVI: { id: string; nome: string }[] = [
  { id: 'nome', nome: t('Nome non adeguato') },
  { id: 'foto', nome: t('Foto non adeguata') },
  { id: 'comportamento', nome: t('Comportamento scorretto') },
  { id: 'altro', nome: t('Altro') },
];
/** `null` = finché qualcuno non riattiva l'account. */
export const DURATE: [number | null, string][] = [[1, t('1 giorno')], [2, t('2 giorni')], [7, t('1 settimana')], [14, t('2 settimane')], [30, t('1 mese')], [null, t('Fino a riattivazione')]];

export const nomiMotivi = (ids: string[]): string => MOTIVI.filter((m) => ids.includes(m.id)).map((m) => m.nome).join(', ');

const quando = (s: number) => t('{giorno} {mese} alle {ora}', { giorno: daSecondi(s).getDate(), mese: mese(daSecondi(s)), ora: ora(s) });
/** "Sospeso fino al 3 dicembre alle 10:00", "Sospeso", "Espulso". */
export const statoBlocco = (b: Blocco): string => (b.kind === 'espulso' ? t('Espulso') : b.until ? t('Sospeso fino al {quando}', { quando: quando(b.until) }) : t('Sospeso'));
