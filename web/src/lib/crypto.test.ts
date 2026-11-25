import { describe, expect, it } from 'vitest';
import * as C from './crypto';

// Parametri ridotti: stessa logica, test veloci. In produzione il server dichiara m=64 MiB, t=3.
const FAST = { m_kib: 256, t: 1, p: 1, len: 32 };
const salt = new Uint8Array(16).fill(7);

describe('derivazione chiavi', () => {
  it('e\' deterministica e separa auth_key da wrap_key', async () => {
    const a = await C.deriveKeys('correct horse battery', salt, FAST);
    const b = await C.deriveKeys('correct horse battery', salt, FAST);
    expect(a.authKey).toBe(b.authKey);
    expect(C.b64.dec(a.authKey)).toHaveLength(32);
    const altra = await C.deriveKeys('altra password!!', salt, FAST);
    expect(altra.authKey).not.toBe(a.authKey);
    const altroSalt = await C.deriveKeys('correct horse battery', new Uint8Array(16).fill(8), FAST);
    expect(altroSalt.authKey).not.toBe(a.authKey);
  });

  it('la wrap_key non puo\' aprire cio\' che e\' stato avvolto con un\'altra password', async () => {
    const giusta = await C.deriveKeys('password-giusta', salt, FAST);
    const sbagliata = await C.deriveKeys('password-sbagliata', salt, FAST);
    const dek = C.newDek();
    const wrapped = await C.wrapDek(giusta.wrapKey, dek);
    expect(await C.unwrapDek(giusta.wrapKey, wrapped)).toEqual(dek);
    await expect(C.unwrapDek(sbagliata.wrapKey, wrapped)).rejects.toThrow();
  });
});

describe('dati cifrati', () => {
  it('round trip con contesto, e fallisce se il blob cambia posto o viene alterato', async () => {
    const key = await C.dataKey(C.newDek());
    const evento = { titolo: 'Dentista', ora: '15:30' };
    const blob = await C.encryptJson(key, evento, 'event:42');
    expect(await C.decryptJson(key, blob, 'event:42')).toEqual(evento);
    await expect(C.decryptJson(key, blob, 'event:43')).rejects.toThrow();
    const bytes = C.b64.dec(blob);
    bytes[bytes.length - 1] ^= 1;
    await expect(C.decryptJson(key, C.b64.enc(bytes), 'event:42')).rejects.toThrow();
  });

  it('due cifrature dello stesso dato sono diverse (IV casuale)', async () => {
    const key = await C.dataKey(C.newDek());
    expect(await C.encryptJson(key, { a: 1 }, 'x')).not.toBe(await C.encryptJson(key, { a: 1 }, 'x'));
  });

  it('la chiave avvolta ha la lunghezza che il server si aspetta (12 + 32 + 16 = 60 byte)', async () => {
    const { wrapKey } = await C.deriveKeys('una password', salt, FAST);
    expect(C.b64.dec(await C.wrapDek(wrapKey, C.newDek()))).toHaveLength(60);
  });
});

describe('chiave di recupero', () => {
  it('si scrive e si rilegge identica, e riapre la DEK', async () => {
    const r = C.newRecoveryKey();
    expect(r.text).toMatch(/^([A-Z2-7]{4}-){7}[A-Z2-7]{4}$/);
    expect(C.parseRecoveryKey(r.text.toLowerCase().replace(/-/g, ' '))).toEqual(r.bytes);
    const dek = C.newDek();
    const wrapped = await C.wrapDek(await C.recoveryWrapKey(r.bytes), dek);
    expect(await C.unwrapDek(await C.recoveryWrapKey(C.parseRecoveryKey(r.text)), wrapped)).toEqual(dek);
  });
  it('rifiuta chiavi di lunghezza sbagliata', () => {
    expect(() => C.parseRecoveryKey('ABCD-EFGH')).toThrow();
  });
});

describe('base64url', () => {
  it('round trip senza padding', () => {
    for (const n of [0, 1, 2, 3, 31, 32, 33]) {
      const x = C.randomBytes(n);
      const s = C.b64.enc(x);
      expect(s).not.toMatch(/[+/=]/);
      expect(C.b64.dec(s)).toEqual(x);
    }
  });
});
