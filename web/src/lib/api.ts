// Client HTTP minimo. Il cookie di sessione è HttpOnly: da qui non si legge e non serve.
export class ApiError extends Error {
  constructor(public status: number, message: string, public retryAfter?: number) { super(message); }
}

/** `keepalive` fa partire la richiesta anche se la pagina si sta chiudendo. */
async function call<T>(method: string, path: string, body?: unknown, keepalive = false): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      method,
      credentials: 'same-origin',
      keepalive,
      headers: body === undefined ? undefined : { 'content-type': body instanceof Blob ? 'application/octet-stream' : 'application/json' },
      body: body === undefined || body instanceof Blob ? body : JSON.stringify(body),
    });
  } catch {
    throw new ApiError(0, 'Server non raggiungibile.');
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    const retry = Number(res.headers.get('retry-after')) || undefined;
    throw new ApiError(res.status, data?.error ?? `Errore ${res.status}`, retry);
  }
  return data as T;
}

export type Ruolo = 'admin' | 'rappresentante' | 'nerd' | 'alunno';
export interface User { id: string; username: string; display_name: string; role: Ruolo }
export interface Me extends User { profile_blob: string | null; sessions: number }
export interface Kdf { alg: string; m_kib: number; t: number; p: number; len: number }
export interface SetupStatus { needs_setup: boolean; kdf: Kdf }
export interface LoginResult { user: User; kdf_salt: string; wrapped_key: string }
export interface ThemeColors {
  sfondo: string; superficie: string; inchiostro: string; inchiostro2: string;
  attenuato: string; filetto: string; accento: string; suAccento: string;
}
export interface Theme {
  id: string; nome: string; tipo: 'chiaro' | 'scuro'; abbinato?: string;
  colori: ThemeColors; font?: { titoli?: string; testo?: string; dati?: string };
}
export interface NuovoAccount {
  username: string; display_name: string; auth_key: string; kdf_salt: string;
  wrapped_key: string; recovery_wrapped_key?: string;
}

export interface Materia { id: string; name: string; color: string }
export interface Tipo { id: string; name: string; subs: string[]; personal: boolean }
export interface Membro { id: string; name: string; role: Ruolo }
export interface Gruppo { id: string; name: string; kind: 'classe' | 'gruppo'; mine: boolean; members: Membro[] }
export interface Impostazioni { instance_name: string; public_url: string; school_year: string; timezone: string }
export interface Meta {
  tags: Materia[]; labels: Tipo[]; groups: Gruppo[]; settings: Impostazioni;
  plugins: { grafico: { user: boolean; instance: boolean } };
}

/** Campi di un evento: in chiaro se pubblico, dentro il blob cifrato se privato. */
export interface Campi {
  title: string; description: string | null; tag_id: string | null; label_id: string | null; sub_kind: string | null;
  starts_at: number; ends_at: number | null; alerts: string[];
}
export interface DatiPubblici extends Campi { group_id: string; subject_id: string | null }
export interface EventoServer {
  id: string; visibility: 'public' | 'private'; owner_id: string; rev: number; created_at: number; updated_at: number;
  created_by: { id: string; name: string; role: Ruolo }; updated_by: { name: string; role: Ruolo };
  approved_by: string | null; proposed: boolean; payload: string | null; data: DatiPubblici | null;
}
export type Modifiche = Partial<Record<keyof DatiPubblici, [unknown, unknown]>>;
export interface VoceStoria { at: number; kind: 'added' | 'modified' | 'deleted'; changes: Modifiche | null; actor: string | null; approver: string | null }
export type EsitoScrittura = 'published' | 'proposed' | 'saved';

export type Fotografia = Pick<DatiPubblici, 'title' | 'tag_id' | 'label_id' | 'sub_kind' | 'group_id' | 'starts_at'>;
export type TipoProposta = 'create' | 'update' | 'delete';
export interface Notifica {
  id: string; kind: 'added' | 'modified' | 'deleted' | 'proposal' | 'decision'; event_id: string | null; at: number; read: boolean;
  actor: string | null;
  payload: { event: Fotografia; changes?: Modifiche | null; approver?: string | null; proposal_kind?: TipoProposta; outcome?: 'approved' | 'rejected' };
}
export interface Inbox { items: Notifica[]; unread: number; to_approve: number }
export interface Proposta {
  id: string; kind: TipoProposta; event_id: string | null; note: string | null; created_at: number;
  proposer: { id: string; name: string; role: Ruolo }; data: DatiPubblici; current: DatiPubblici | null; stale: boolean;
}

export interface Persona { id: string; username: string; display_name: string; role: Ruolo; disabled: boolean; group_ids: string[] }
export interface InAttesa { id: string; username: string; display_name: string; group_id: string | null; created_at: number }
export interface Invito { id: string; code: string; group_id: string; expires_at: number; uses: number }
export interface StatoAdmin {
  encrypted_at_rest: boolean; login_attempts_per_minute: number; remote_setup_blocked: boolean;
  origin_pinned: boolean;
}

export interface ImpostazioniBackup { enabled: boolean; /** 1 = lunedì … 7 = domenica */ days: number[]; time: string; keep: number }
export interface Backup { name: string; kind: 'auto' | 'manuale' | 'caricato' | 'sicurezza'; created_at: number; size: number }
export interface StatoBackup {
  settings: ImpostazioniBackup; timezone: string; next: number | null; error: string | null;
  copy: { active: boolean; latest_copied: boolean }; items: Backup[]; dir: string;
}

type Ok = { ok: boolean };
const id = encodeURIComponent;

export const api = {
  setupStatus: () => call<SetupStatus>('GET', '/api/setup/status'),
  setup: (b: NuovoAccount) => call<{ user: User }>('POST', '/api/setup', b),
  join: (b: NuovoAccount & { code: string }) => call<Ok>('POST', '/api/join', b),
  prelogin: (username: string) => call<{ kdf_salt: string; kdf: Kdf }>('POST', '/api/auth/prelogin', { username }),
  login: (username: string, auth_key: string) => call<LoginResult>('POST', '/api/auth/login', { username, auth_key }),
  logout: () => call<unknown>('POST', '/api/auth/logout', {}),
  me: () => call<Me>('GET', '/api/me'),
  salvaMe: (b: { display_name?: string; profile_blob?: string }) => call<Ok>('PATCH', '/api/me', b),
  esciAltrove: () => call<Ok>('POST', '/api/me/sessions/revoke-others', {}),
  themes: () => call<{ themes: Theme[]; invalid: { file: string; error: string }[] }>('GET', '/api/themes'),

  meta: () => call<Meta>('GET', '/api/meta'),
  creaMateria: (name: string, color: string) => call<{ id: string }>('POST', '/api/tags', { name, color }),
  creaTipo: (name: string) => call<{ id: string }>('POST', '/api/labels', { name }),
  salvaImpostazioni: (b: Partial<Impostazioni>) => call<Ok>('PUT', '/api/settings', b),
  salvaPlugin: (plugin: string, scope: 'user' | 'instance', enabled: boolean) => call<Ok>('PUT', `/api/plugins/${id(plugin)}`, { scope, enabled }),
  statoAdmin: () => call<StatoAdmin>('GET', '/api/admin/status'),

  backup: () => call<StatoBackup>('GET', '/api/backups'),
  backupAdesso: () => call<Backup>('POST', '/api/backups', {}),
  salvaBackup: (b: ImpostazioniBackup) => call<Ok>('PUT', '/api/backups/settings', b),
  scaricaBackup: (name: string) => `/api/backups/${id(name)}`,
  eliminaBackup: (name: string) => call<Ok>('POST', `/api/backups/${id(name)}/delete`, {}),
  ripristinaBackup: (name: string, auth_key: string) => call<Ok>('POST', `/api/backups/${id(name)}/restore`, { auth_key }),
  caricaBackup: (file: Blob) => call<Backup>('POST', '/api/backups/upload', file),
  ripristinaAlPrimoAvvio: (file: Blob) => call<Ok>('POST', '/api/setup/restore', file),

  eventi: () => call<{ events: EventoServer[] }>('GET', '/api/events'),
  creaPubblico: (data: DatiPubblici, note?: string) =>
    call<{ status: EsitoScrittura; id: string }>('POST', '/api/events', { visibility: 'public', data, note }),
  creaPrivato: (eventId: string, payload: string) =>
    call<{ status: EsitoScrittura; id: string }>('POST', '/api/events', { visibility: 'private', id: eventId, payload }),
  modifica: (eventId: string, b: { rev: number; data?: DatiPubblici; payload?: string; note?: string }) =>
    call<{ status: EsitoScrittura }>('PUT', `/api/events/${id(eventId)}`, b),
  elimina: (eventId: string, rev: number, note?: string) =>
    call<{ status: EsitoScrittura }>('POST', `/api/events/${id(eventId)}/delete`, { rev, note }),
  storia: (eventId: string) => call<{ history: VoceStoria[] }>('GET', `/api/events/${id(eventId)}/history`),

  inbox: () => call<Inbox>('GET', '/api/inbox'),
  leggiTutto: () => call<Ok>('POST', '/api/inbox/read', {}),
  proposte: () => call<{ proposals: Proposta[] }>('GET', '/api/proposals'),
  decidi: (proposalId: string, approve: boolean) => call<Ok>('POST', `/api/proposals/${id(proposalId)}/decide`, { approve }, true),

  persone: () => call<{ users: Persona[]; pending: InAttesa[] }>('GET', '/api/users'),
  salvaPersona: (userId: string, b: { role?: Ruolo; disabled?: boolean; group_ids?: string[] }) => call<Ok>('PATCH', `/api/users/${id(userId)}`, b),
  accetta: (userId: string) => call<Ok>('POST', `/api/users/${id(userId)}/accept`, {}),
  rifiuta: (userId: string) => call<Ok>('POST', `/api/users/${id(userId)}/reject`, {}),
  creaGruppo: (name: string, kind: Gruppo['kind']) => call<{ id: string }>('POST', '/api/groups', { name, kind }),
  inviti: () => call<{ invites: Invito[] }>('GET', '/api/invites'),
  creaInvito: (group_id: string) => call<{ code: string }>('POST', '/api/invites', { group_id }),
  revocaInvito: (inviteId: string) => call<Ok>('POST', `/api/invites/${id(inviteId)}/revoke`, {}),
};
