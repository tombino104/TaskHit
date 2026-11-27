// Crittografia lato client. Il server non vede mai la password né la chiave dei dati.
//
//   password --Argon2id(salt)--> master (32 B)
//   master --HKDF "th-auth"--> auth_key   (va al server, che ne salva solo l'hash)
//   master --HKDF "th-enc" --> wrap_key   (resta qui, cifra la chiave dei dati)
//   DEK (casuale, 32 B)  --AES-256-GCM(wrap_key)--> wrapped_key (al server)
//
// Gli eventi privati e il profilo sono cifrati con la DEK. Cambiare password = ri-cifrare solo la DEK.
import { t } from './i18n';
import { argon2id } from 'hash-wasm';

const enc = new TextEncoder();
const subtle = () => globalThis.crypto.subtle;

export interface KdfParams { m_kib: number; t: number; p: number; len: number }
export const DEFAULT_KDF: KdfParams = { m_kib: 65536, t: 3, p: 1, len: 32 };

export const b64 = {
  enc(bytes: Uint8Array): string {
    let s = '';
    for (const b of bytes) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  },
  dec(text: string): Uint8Array {
    const pad = '='.repeat((4 - (text.length % 4)) % 4);
    const bin = atob(text.replace(/-/g, '+').replace(/_/g, '/') + pad);
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
  },
};

export const randomBytes = (n: number): Uint8Array => globalThis.crypto.getRandomValues(new Uint8Array(n));

export interface DerivedKeys { authKey: string; wrapKey: CryptoKey }

export async function deriveKeys(password: string, salt: Uint8Array, kdf: KdfParams = DEFAULT_KDF): Promise<DerivedKeys> {
  const master = await argon2id({
    password: password.normalize('NFKC'),
    salt,
    parallelism: kdf.p,
    iterations: kdf.t,
    memorySize: kdf.m_kib,
    hashLength: kdf.len,
    outputType: 'binary',
  });
  const base = await subtle().importKey('raw', master as BufferSource, 'HKDF', false, ['deriveBits', 'deriveKey']);
  const info = (s: string) => ({ name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(0), info: enc.encode(s) });
  const authBits = await subtle().deriveBits(info('th-auth'), base, 256);
  const wrapKey = await subtle().deriveKey(info('th-enc'), base, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
  master.fill(0);
  return { authKey: b64.enc(new Uint8Array(authBits)), wrapKey };
}

async function seal(key: CryptoKey, data: Uint8Array, aad?: string): Promise<Uint8Array> {
  const iv = randomBytes(12);
  const params: AesGcmParams = { name: 'AES-GCM', iv: iv as BufferSource };
  if (aad) params.additionalData = enc.encode(aad);
  const ct = new Uint8Array(await subtle().encrypt(params, key, data as BufferSource));
  const out = new Uint8Array(iv.length + ct.length);
  out.set(iv);
  out.set(ct, iv.length);
  return out;
}

async function open(key: CryptoKey, blob: Uint8Array, aad?: string): Promise<Uint8Array> {
  if (blob.length < 12 + 16) throw new Error('blob troppo corto');
  const params: AesGcmParams = { name: 'AES-GCM', iv: blob.slice(0, 12) as BufferSource };
  if (aad) params.additionalData = enc.encode(aad);
  return new Uint8Array(await subtle().decrypt(params, key, blob.slice(12) as BufferSource));
}

/** Chiave dei dati: casuale, mai uguale alla password né derivata da essa. */
export const newDek = (): Uint8Array => randomBytes(32);

export async function wrapDek(wrapKey: CryptoKey, dek: Uint8Array): Promise<string> {
  return b64.enc(await seal(wrapKey, dek, 'th-dek'));
}

export async function unwrapDek(wrapKey: CryptoKey, wrapped: string): Promise<Uint8Array> {
  return open(wrapKey, b64.dec(wrapped), 'th-dek');
}

export async function dataKey(dek: Uint8Array): Promise<CryptoKey> {
  return subtle().importKey('raw', dek as BufferSource, 'AES-GCM', false, ['encrypt', 'decrypt']);
}

/** Cifra un oggetto JSON (evento privato, profilo). `context` lega il blob al suo posto: spostarlo altrove fa fallire la lettura. */
export async function encryptJson(key: CryptoKey, value: unknown, context: string): Promise<string> {
  return b64.enc(await seal(key, enc.encode(JSON.stringify(value)), context));
}

export async function decryptJson<T = unknown>(key: CryptoKey, blob: string, context: string): Promise<T> {
  return JSON.parse(new TextDecoder().decode(await open(key, b64.dec(blob), context))) as T;
}

// --- Chiave di recupero: 160 bit in base32, mostrata una sola volta all'admin. ---
const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

export function newRecoveryKey(): { text: string; bytes: Uint8Array } {
  const bytes = randomBytes(20);
  let bits = '';
  for (const b of bytes) bits += b.toString(2).padStart(8, '0');
  let out = '';
  for (let i = 0; i < bits.length; i += 5) out += ALPHABET[parseInt(bits.slice(i, i + 5), 2)];
  return { text: out.match(/.{4}/g)!.join('-'), bytes };
}

export function parseRecoveryKey(text: string): Uint8Array {
  const clean = text.toUpperCase().replace(/[^A-Z2-7]/g, '');
  if (clean.length !== 32) throw new Error(t('La chiave di recupero ha 32 caratteri.'));
  let bits = '';
  for (const ch of clean) bits += ALPHABET.indexOf(ch).toString(2).padStart(5, '0');
  const out = new Uint8Array(20);
  for (let i = 0; i < 20; i++) out[i] = parseInt(bits.slice(i * 8, i * 8 + 8), 2);
  return out;
}

/** La chiave di recupero (casuale e già forte) avvolge la DEK tramite HKDF, senza bisogno di Argon2. */
export async function recoveryWrapKey(recovery: Uint8Array): Promise<CryptoKey> {
  const base = await subtle().importKey('raw', recovery as BufferSource, 'HKDF', false, ['deriveKey']);
  return subtle().deriveKey(
    { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(0), info: enc.encode('th-recovery') },
    base,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}
