import { Encoding } from './Encoding';
import { HarmonyRemoteCryptoCipher, RemoteCryptoCipher } from './RemoteCrypto';
import { X25519, X25519KeyPair } from './X25519';

export interface WatchSealedPayload {
  encryptedData: string;
  nonce: string;
}

/**
 * Seals a provisioned credential to the watch's ephemeral public key.
 *
 * Same construction as the room channel — raw X25519 shared secret used
 * directly as the AES-256-GCM key, ciphertext‖tag, base64 — so the watch can
 * open it with the `CryptoService` it already has. A fresh key pair per
 * provisioning attempt means a credential captured off the KV store stays
 * unreadable even if the watch's stored keys later leak.
 */
export class WatchProvisionCrypto {
  private readonly keyPair: X25519KeyPair;
  private readonly cipher: RemoteCryptoCipher;

  constructor(keyPair?: X25519KeyPair, cipher?: RemoteCryptoCipher) {
    this.keyPair = keyPair || X25519.generateKeyPair();
    this.cipher = cipher || new HarmonyRemoteCryptoCipher();
  }

  publicKeyBase64(): string {
    return Encoding.bytesToBase64(this.keyPair.publicKey);
  }

  async seal(watchPublicKeyBase64: string, plaintext: string): Promise<WatchSealedPayload> {
    const peerPublicKey = Encoding.base64ToBytes(watchPublicKeyBase64);
    if (peerPublicKey.length !== 32) {
      throw new Error('Watch public key must be 32 bytes.');
    }
    const key = X25519.scalarMult(this.keyPair.privateKey, peerPublicKey);
    const nonce = Encoding.randomBytes(12);
    const sealed = await this.cipher.encrypt(Encoding.utf8ToBytes(plaintext), key, nonce);
    return {
      encryptedData: Encoding.bytesToBase64(sealed),
      nonce: Encoding.bytesToBase64(nonce)
    };
  }
}
