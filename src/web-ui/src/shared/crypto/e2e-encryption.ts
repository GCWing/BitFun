/**
 * End-to-end encryption for Remote Connect using Web Crypto API.
 *
 * Key exchange: X25519 ECDH (Chrome 113+, Safari 17+).
 * Symmetric encryption: AES-256-GCM.
 *
 * For older browsers that lack X25519 support in Web Crypto, this module
 * falls back to the @noble/curves library (must be installed separately).
 */

const ALGO_AES = 'AES-GCM';
const KEY_LENGTH = 256;
const NONCE_LENGTH = 12;

// X25519 is available in Web Crypto starting Chrome 113 / Safari 17.
// We detect support at runtime and fall back to @noble/curves if needed.

let _useNobleFallback: boolean | null = null;

async function supportsWebCryptoX25519(): Promise<boolean> {
  if (_useNobleFallback !== null) return !_useNobleFallback;
  try {
    await crypto.subtle.generateKey(
      { name: 'X25519' } as any,
      true,
      ['deriveKey'],
    );
    _useNobleFallback = false;
    return true;
  } catch {
    _useNobleFallback = true;
    return false;
  }
}

// ── Key types ──────────────────────────────────────────────────────

export interface E2EKeyPair {
  publicKey: Uint8Array;
  /** Opaque handle — either a CryptoKeyPair or noble private key bytes. */
  _internal: any;
}

// ── Key generation ─────────────────────────────────────────────────

export async function generateKeyPair(): Promise<E2EKeyPair> {
  if (await supportsWebCryptoX25519()) {
    return generateKeyPairWebCrypto();
  }
  return generateKeyPairNoble();
}

async function generateKeyPairWebCrypto(): Promise<E2EKeyPair> {
  const keyPair = await crypto.subtle.generateKey(
    { name: 'X25519' } as any,
    true,
    ['deriveKey'],
  );
  const rawPub = await crypto.subtle.exportKey('raw', (keyPair as any).publicKey);
  return {
    publicKey: new Uint8Array(rawPub),
    _internal: keyPair,
  };
}

async function generateKeyPairNoble(): Promise<E2EKeyPair> {
  const { x25519 } = await import('@noble/curves/ed25519');
  const privateKey = crypto.getRandomValues(new Uint8Array(32));
  const publicKey = x25519.getPublicKey(privateKey);
  return {
    publicKey,
    _internal: privateKey,
  };
}

// ── Shared secret derivation ───────────────────────────────────────

export async function deriveSharedSecret(
  keyPair: E2EKeyPair,
  peerPublicKey: Uint8Array,
): Promise<CryptoKey> {
  if (await supportsWebCryptoX25519()) {
    return deriveSharedSecretWebCrypto(keyPair, peerPublicKey);
  }
  return deriveSharedSecretNoble(keyPair, peerPublicKey);
}

async function deriveSharedSecretWebCrypto(
  keyPair: E2EKeyPair,
  peerPublicKey: Uint8Array,
): Promise<CryptoKey> {
  const peerKey = await crypto.subtle.importKey(
    'raw',
    peerPublicKey,
    { name: 'X25519' } as any,
    true,
    [],
  );
  return crypto.subtle.deriveKey(
    { name: 'X25519', public: peerKey } as any,
    (keyPair._internal as CryptoKeyPair).privateKey,
    { name: ALGO_AES, length: KEY_LENGTH },
    false,
    ['encrypt', 'decrypt'],
  );
}

async function deriveSharedSecretNoble(
  keyPair: E2EKeyPair,
  peerPublicKey: Uint8Array,
): Promise<CryptoKey> {
  const { x25519 } = await import('@noble/curves/ed25519');
  const sharedBytes = x25519.getSharedSecret(keyPair._internal as Uint8Array, peerPublicKey);
  return crypto.subtle.importKey(
    'raw',
    sharedBytes,
    { name: ALGO_AES, length: KEY_LENGTH },
    false,
    ['encrypt', 'decrypt'],
  );
}

// ── Encrypt / Decrypt ──────────────────────────────────────────────

export async function encrypt(
  sharedKey: CryptoKey,
  plaintext: string,
): Promise<{ data: string; nonce: string }> {
  const nonce = crypto.getRandomValues(new Uint8Array(NONCE_LENGTH));
  const encoded = new TextEncoder().encode(plaintext);
  const ciphertext = await crypto.subtle.encrypt(
    { name: ALGO_AES, iv: nonce },
    sharedKey,
    encoded,
  );
  return {
    data: uint8ToBase64(new Uint8Array(ciphertext)),
    nonce: uint8ToBase64(nonce),
  };
}

export async function decrypt(
  sharedKey: CryptoKey,
  dataBase64: string,
  nonceBase64: string,
): Promise<string> {
  const ciphertext = base64ToUint8(dataBase64);
  const nonce = base64ToUint8(nonceBase64);
  const plainBuffer = await crypto.subtle.decrypt(
    { name: ALGO_AES, iv: nonce },
    sharedKey,
    ciphertext,
  );
  return new TextDecoder().decode(plainBuffer);
}

// ── Public key encoding helpers ────────────────────────────────────

export function publicKeyToBase64(key: Uint8Array): string {
  return uint8ToBase64(key);
}

export function base64ToPublicKey(b64: string): Uint8Array {
  return base64ToUint8(b64);
}

// ── Base64 utilities ───────────────────────────────────────────────

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function base64ToUint8(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
