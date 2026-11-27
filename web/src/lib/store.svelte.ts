// Stato dell'app e azioni che parlano col server.
//
// Offline: l'ultima risposta del server è tenuta nel dispositivo, con gli eventi privati ancora cifrati.
// La chiave dei dati (DEK) resta solo in memoria: dopo un riavvio si «sblocca» di nuovo con la password.
import { t } from './i18n';
import { api, ApiError, type Campi, type DatiPubblici, type EventoServer, type Inbox, type Kdf, type Me, type Meta, type Theme } from './api';
import * as C from './crypto';
import { SENZA_FILTRI, type Evento, type Filtri } from './events';
import { applyTheme, cacheThemes, cachedThemes, getPrefs, pickTheme, setPrefs, tipoDi, type Preferenze } from './theme';

const COPIA = 'th-copia';
const SBLOCCO = 'th-sblocco';
const INBOX_VUOTA: Inbox = { items: [], unread: 0, to_approve: 0 };

export const app = $state({
  utente: null as Me | null,
  meta: null as Meta | null,
  eventi: [] as Evento[],
  /** Eventi privati presenti ma non leggibili finché non si sblocca. */
  bloccati: 0,
  inbox: INBOX_VUOTA,
  temi: [] as Theme[],
  preferenze: getPrefs(),
  filtri: { ...SENZA_FILTRI } as Filtri,
  online: navigator.onLine,
  sbloccato: false,
  ultimaSync: 0,
  avviso: '',
});

let dek: CryptoKey | null = null; // resta solo in memoria
let grezzi: EventoServer[] = [];
/** Cambia a ogni uscita: le risposte arrivate dopo non devono riportare dati sul dispositivo. */
let epoca = 0;

// --- Temi ---
export function riapplicaTema(): void {
  const t = pickTheme(app.temi, app.preferenze);
  if (t) applyTheme(t, tipoDi(app.preferenze, matchMedia('(prefers-color-scheme: dark)').matches));
}

export function scegliPreferenze(p: Partial<Preferenze>): void {
  app.preferenze = { ...app.preferenze, ...p };
  setPrefs($state.snapshot(app.preferenze));
  riapplicaTema();
}

export async function caricaTemi(): Promise<void> {
  try {
    app.temi = (await api.themes()).themes;
    cacheThemes($state.snapshot(app.temi));
  } catch {
    app.temi = cachedThemes(); // offline: ultima lista nota
  }
  riapplicaTema();
}

// --- Messaggi ---
let timerAvviso: ReturnType<typeof setTimeout> | undefined;
export function avvisa(testo: string): void {
  app.avviso = testo;
  clearTimeout(timerAvviso);
  timerAvviso = setTimeout(() => (app.avviso = ''), 4500);
}

export const messaggio = (e: unknown): string =>
  e instanceof ApiError && e.status === 429 ? t('Troppi tentativi. Riprova tra {n} secondi.', { n: e.retryAfter ?? 60 })
  : e instanceof Error ? maiuscolaFrase(t(e.message)) : t('Errore sconosciuto');
const maiuscolaFrase = (s: string) => s.charAt(0).toUpperCase() + s.slice(1).replace(/e'/g, 'è');

// --- Memoria del dispositivo ---
const leggi = <T>(k: string): T | null => { try { return JSON.parse(localStorage.getItem(k) ?? 'null') as T; } catch { return null; } };
const scrivi = (k: string, v: unknown) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* spazio esaurito */ } };
const cancella = (k: string) => { try { localStorage.removeItem(k); } catch { /* ignora */ } };

interface Copia { utente: Me; meta: Meta; eventi: EventoServer[]; inbox: Inbox; at: number }
interface Sblocco { username: string; kdf_salt: string; kdf: Kdf; wrapped_key: string }

// --- Eventi ---
/** Decifra gli eventi privati (se si può) e ricostruisce la lista mostrata. */
async function componi(): Promise<void> {
  const inizio = epoca;
  const pronti: Evento[] = [];
  let bloccati = 0;
  for (const g of grezzi) {
    let campi: Campi | null = g.data;
    if (g.visibility === 'private') {
      campi = null;
      if (dek && g.payload) {
        try { campi = await C.decryptJson<Campi>(dek, g.payload, `event:${g.id}`); } catch { /* blob illeggibile */ }
      }
      if (!campi) { bloccati++; continue; }
    }
    if (!campi) continue;
    pronti.push({
      title: campi.title, description: campi.description ?? null, tag_id: campi.tag_id ?? null, label_id: campi.label_id ?? null,
      sub_kind: campi.sub_kind ?? null, starts_at: campi.starts_at, ends_at: campi.ends_at ?? null, alerts: campi.alerts ?? [],
      id: g.id, privato: g.visibility === 'private', rev: g.rev, owner_id: g.owner_id,
      group_id: g.data?.group_id ?? null, subject_id: g.data?.subject_id ?? null,
      created_at: g.created_at, updated_at: g.updated_at, created_by: g.created_by, updated_by: g.updated_by,
      approved_by: g.approved_by, proposed: g.proposed,
    });
  }
  if (inizio !== epoca) return;
  app.eventi = pronti.sort((a, b) => a.starts_at - b.starts_at);
  app.bloccati = bloccati;
}

/** Scarica tutto dal server. Se il server non risponde restano i dati già presenti. */
export async function sincronizza(): Promise<void> {
  const inizio = epoca;
  try {
    const [utente, meta, ev, inbox] = await Promise.all([api.me(), api.meta(), api.eventi(), api.inbox()]);
    if (inizio !== epoca) return;
    // Un altro utente sullo stesso dispositivo: la chiave in memoria non è la sua.
    if (app.utente && app.utente.id !== utente.id) blocca();
    app.utente = utente; app.meta = meta; app.inbox = inbox; grezzi = ev.events;
    app.ultimaSync = Date.now(); app.online = true;
    scrivi(COPIA, { utente, meta, eventi: ev.events, inbox, at: app.ultimaSync } satisfies Copia);
    await componi();
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) { dimentica(); return; }
    if (e instanceof ApiError && e.status === 0) { app.online = false; return; }
    throw e;
  }
}

/** Avvio: sessione valida, oppure copia sul dispositivo se il server non c'è. */
export async function riprendi(): Promise<boolean> {
  const copia = leggi<Copia>(COPIA);
  try {
    await sincronizza();
  } catch { /* errore del server: si prova con la copia */ }
  if (!app.utente && copia && !app.online) {
    app.utente = copia.utente; app.meta = copia.meta; app.inbox = copia.inbox; grezzi = copia.eventi; app.ultimaSync = copia.at;
    await componi();
  }
  return app.utente !== null;
}

// --- Accesso ---
export async function accedi(username: string, password: string): Promise<void> {
  const pre = await api.prelogin(username);
  const { authKey, wrapKey } = await C.deriveKeys(password, C.b64.dec(pre.kdf_salt), pre.kdf);
  const res = await api.login(username, authKey);
  await usaChiave(await C.unwrapDek(wrapKey, res.wrapped_key));
  scrivi(SBLOCCO, { username: res.user.username, kdf_salt: pre.kdf_salt, kdf: pre.kdf, wrapped_key: res.wrapped_key } satisfies Sblocco);
  await sincronizza();
}

/** Le chiavi di un nuovo account, calcolate sul dispositivo. */
export async function preparaChiavi(password: string, kdf: Kdf | undefined) {
  const salt = C.randomBytes(16);
  const { authKey, wrapKey } = await C.deriveKeys(password, salt, kdf);
  const nuova = C.newDek();
  const recupero = C.newRecoveryKey();
  return {
    dek: nuova,
    recupero: recupero.text,
    kdf_salt: C.b64.enc(salt),
    account: {
      auth_key: authKey, kdf_salt: C.b64.enc(salt),
      wrapped_key: await C.wrapDek(wrapKey, nuova),
      recovery_wrapped_key: await C.wrapDek(await C.recoveryWrapKey(recupero.bytes), nuova),
    },
  };
}

export async function usaChiave(grezza: Uint8Array, ricorda?: Sblocco): Promise<void> {
  dek = await C.dataKey(grezza);
  grezza.fill(0);
  app.sbloccato = true;
  if (ricorda) scrivi(SBLOCCO, ricorda);
  await componi();
}

/** Riapre gli eventi privati con la password: online rifà l'accesso, offline usa i dati tenuti sul dispositivo. */
export async function sblocca(password: string): Promise<void> {
  const utente = app.utente;
  if (!utente) return;
  const salvato = leggi<Sblocco>(SBLOCCO);
  if (salvato && salvato.username === utente.username) {
    const { wrapKey } = await C.deriveKeys(password, C.b64.dec(salvato.kdf_salt), salvato.kdf);
    try { await usaChiave(await C.unwrapDek(wrapKey, salvato.wrapped_key)); return; }
    catch { if (!app.online) throw new Error(t('Password errata.')); } // forse è cambiata: si chiede al server
  }
  if (!app.online) throw new Error(t('Per sbloccare la prima volta serve la connessione.'));
  try { await accedi(utente.username, password); }
  catch (e) { throw e instanceof ApiError && e.status === 401 ? new Error(t('Password errata.')) : e; }
}

function blocca(): void { dek = null; app.sbloccato = false; }

/** Toglie dal dispositivo tutto ciò che appartiene all'utente. */
function dimentica(): void {
  epoca++;
  blocca();
  grezzi = [];
  app.utente = null; app.meta = null; app.eventi = []; app.bloccati = 0; app.inbox = INBOX_VUOTA;
  app.filtri = { ...SENZA_FILTRI };
  cancella(COPIA); cancella(SBLOCCO);
}

export async function esci(): Promise<void> {
  try { await api.logout(); } catch { /* sessione già scaduta o server irraggiungibile */ }
  dimentica();
}

// --- Scritture sugli eventi ---
export async function salvaEvento(
  esistente: Evento | null,
  campi: Campi,
  destinazione: { privato: true } | { privato: false; group_id: string; subject_id: string | null; nota?: string },
) {
  let esito;
  if (destinazione.privato) {
    if (!dek) throw new Error(t('Sblocca gli eventi privati con la tua password.'));
    const id = esistente?.id ?? [...C.randomBytes(16)].map((b) => b.toString(16).padStart(2, '0')).join('');
    const payload = await C.encryptJson(dek, campi, `event:${id}`);
    esito = esistente ? await api.modifica(id, { rev: esistente.rev, payload }) : await api.creaPrivato(id, payload);
  } else {
    const data: DatiPubblici = { ...campi, group_id: destinazione.group_id, subject_id: destinazione.subject_id };
    esito = esistente
      ? await api.modifica(esistente.id, { rev: esistente.rev, data, note: destinazione.nota })
      : await api.creaPubblico(data, destinazione.nota);
  }
  await sincronizza();
  return esito.status;
}

export async function eliminaEvento(e: Evento, nota?: string) {
  const esito = await api.elimina(e.id, e.rev, nota);
  await sincronizza();
  return esito.status;
}

// --- Backup ---
/** Ripristina un backup. Serve la password; dopo, tutti i dispositivi devono accedere di nuovo. */
export async function ripristina(nome: string, password: string): Promise<void> {
  const utente = app.utente;
  if (!utente) return;
  const pre = await api.prelogin(utente.username);
  const { authKey } = await C.deriveKeys(password, C.b64.dec(pre.kdf_salt), pre.kdf);
  await api.ripristinaBackup(nome, authKey);
  dimentica(); // i dati sul dispositivo appartengono allo stato di prima
}

// --- Profilo cifrato ---
export interface Profilo { compleanno?: string }
export async function leggiProfilo(): Promise<Profilo | null> {
  const u = app.utente;
  if (!u?.profile_blob) return {};
  if (!dek) return null;
  try { return await C.decryptJson<Profilo>(dek, u.profile_blob, `profile:${u.id}`); } catch { return null; }
}
export async function salvaProfilo(nome: { first_name: string; last_name: string }, profilo: Profilo | null): Promise<void> {
  const u = app.utente;
  if (!u) return;
  const blob = profilo && dek ? await C.encryptJson(dek, profilo, `profile:${u.id}`) : undefined;
  await api.salvaMe({ ...nome, profile_blob: blob });
  await dopoIlNome();
}
export async function salvaNome(first_name: string, last_name: string): Promise<void> {
  await api.salvaMe({ first_name, last_name });
  await dopoIlNome();
}
/** Con un nome nuovo cambia anche ciò con cui si entra: lo sblocco sul dispositivo va aggiornato. */
async function dopoIlNome(): Promise<void> {
  await sincronizza();
  const salvato = leggi<Sblocco>(SBLOCCO);
  if (salvato && app.utente) scrivi(SBLOCCO, { ...salvato, username: app.utente.username });
}

export async function leggiTutto(): Promise<void> {
  await api.leggiTutto();
  await sincronizza();
}
