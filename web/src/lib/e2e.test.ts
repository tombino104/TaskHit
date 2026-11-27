// Test end-to-end opzionale: parte solo con TASKHIT_E2E_URL (server in esecuzione, database vuoto).
import { describe, expect, it } from 'vitest';
import * as C from './crypto';

// Vitest gira in Node: qui serve solo questa variabile.
declare const process: { env: Record<string, string | undefined> };

const base = process.env.TASKHIT_E2E_URL;

describe.skipIf(!base)('client contro server reale', () => {
  it('setup, login con altra "sessione", decifrazione della DEK, recupero', async () => {
    const post = async (path: string, body: unknown, cookie?: string) => {
      const r = await fetch(base + path, { method: 'POST', headers: { 'content-type': 'application/json', ...(cookie ? { cookie } : {}) }, body: JSON.stringify(body) });
      return { status: r.status, body: await r.json().catch(() => ({})), cookie: r.headers.get('set-cookie')?.split(';')[0] };
    };
    const st = await (await fetch(base + '/api/setup/status')).json();
    expect(st.needs_setup).toBe(true);

    const salt = C.randomBytes(16);
    const t0 = performance.now();
    const { authKey, wrapKey } = await C.deriveKeys('una password lunga', salt, st.kdf);
    const kdfMs = Math.round(performance.now() - t0);
    const dek = C.newDek();
    const rec = C.newRecoveryKey();
    const setup = await post('/api/setup', {
      first_name: 'Lorenzo', last_name: 'Prova', auth_key: authKey, kdf_salt: C.b64.enc(salt),
      wrapped_key: await C.wrapDek(wrapKey, dek),
      recovery_wrapped_key: await C.wrapDek(await C.recoveryWrapKey(rec.bytes), dek),
    });
    expect(setup.status, JSON.stringify(setup.body)).toBe(200);

    // "Altro dispositivo": rifà tutto da zero partendo da nome utente e password.
    const pre = await post('/api/auth/prelogin', { username: 'lorenzo prova' });
    const k2 = await C.deriveKeys('una password lunga', C.b64.dec(pre.body.kdf_salt), pre.body.kdf);
    const login = await post('/api/auth/login', { username: 'lorenzo prova', auth_key: k2.authKey });
    expect(login.status).toBe(200);
    expect(await C.unwrapDek(k2.wrapKey, login.body.wrapped_key)).toEqual(dek);

    // Password sbagliata: il server rifiuta, e comunque la DEK non si aprirebbe.
    const bad = await C.deriveKeys('password sbagliata!', C.b64.dec(pre.body.kdf_salt), pre.body.kdf);
    expect((await post('/api/auth/login', { username: 'lorenzo prova', auth_key: bad.authKey })).status).toBe(401);

    // Dati privati: cifrati col client, il server non li vede (qui verifichiamo solo il giro di chiavi).
    const me = await fetch(base + '/api/me', { headers: { cookie: login.cookie! } });
    expect((await me.json()).role).toBe('admin');
    console.log(`Argon2id (64 MiB, t=3) in Node: ${kdfMs} ms`);
  }, 60_000);
});
