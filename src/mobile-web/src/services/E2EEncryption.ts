/**
 * E2E encryption for the mobile web client.
 *
 * Uses pure-JS implementations that work on plain HTTP pages:
 *   - X25519 key exchange: @noble/curves
 *   - AES-256-GCM:         @noble/ciphers
 *
 * The shared secret (32 bytes from X25519) is used directly as the AES key,
 * matching the Rust desktop side which uses x25519-dalek + aes-gcm the same way.
 */

// @ts-ignore — @noble/curves v2 exports with .js suffix in package.json
import { x25519 } from '@noble/curves/ed25519.js';
// @ts-ignore — @noble/ciphers v2 exports with .js suffix in package.json
import { gcm } from '@noble/ciphers/aes.js';

const NONCE_LEN = 12;

export interface MobileKeyPair {
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}

export async function generateKeyPair(): Promise<MobileKeyPair> {
  const priv = randomBytes(32);
  const pub = x25519.getPublicKey(priv);
  return { publicKey: new Uint8Array(pub), privateKey: priv };
}

export async function deriveSharedKey(
  kp: MobileKeyPair,
  peerPub: Uint8Array,
): Promise<Uint8Array> {
  const shared = x25519.getSharedSecret(kp.privateKey, peerPub);
  return new Uint8Array(shared);
}

export async function encrypt(
  key: Uint8Array,
  plaintext: string,
): Promise<{ data: string; nonce: string }> {
  const nonce = randomBytes(NONCE_LEN);
  const cipher = gcm(key, nonce);
  const ct = cipher.encrypt(new TextEncoder().encode(plaintext));
  return { data: toB64(ct), nonce: toB64(nonce) };
}

export async function decrypt(
  key: Uint8Array,
  dataB64: string,
  nonceB64: string,
): Promise<string> {
  const nonce = fromB64(nonceB64);
  const data = fromB64(dataB64);
  const cipher = gcm(key, nonce);
  const pt = cipher.decrypt(data);
  return new TextDecoder().decode(pt);
}

export function toB64(buf: Uint8Array): string {
  let s = '';
  buf.forEach(b => (s += String.fromCharCode(b)));
  return btoa(s);
}

export function fromB64(b64: string): Uint8Array {
  const s = atob(b64);
  const buf = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) buf[i] = s.charCodeAt(i);
  return buf;
}

function randomBytes(len: number): Uint8Array {
  const buf = new Uint8Array(len);
  // crypto.getRandomValues works on HTTP too (it's not part of subtle)
  (globalThis.crypto || (globalThis as any).msCrypto).getRandomValues(buf);
  return buf;
}
