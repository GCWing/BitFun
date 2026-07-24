import { cryptoFramework } from '@kit.CryptoArchitectureKit';
import { Encoding } from './Encoding';
import { X25519, X25519KeyPair } from './X25519';
import { EncryptedPayload } from '../model/RemoteModels';

export interface RemoteCryptoCipher {
  encrypt(plainBytes: Uint8Array, key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array>;
  decrypt(encryptedBytes: Uint8Array, key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array>;
}

export interface RemoteCryptoOptions {
  cipher?: RemoteCryptoCipher;
  keyPair?: X25519KeyPair;
  randomBytes?: (size: number) => Uint8Array;
}

class HarmonyRemoteCryptoCipher implements RemoteCryptoCipher {
  async encrypt(plainBytes: Uint8Array, key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    const symKey = await cryptoFramework.createSymKeyGenerator('AES256').convertKey({ data: key });
    const cipher = cryptoFramework.createCipher('AES256|GCM|NoPadding');
    const params: cryptoFramework.GcmParamsSpec = {
      algName: 'GcmParamsSpec',
      iv: { data: nonce },
      aad: { data: new Uint8Array(0) },
      authTag: { data: new Uint8Array(0) }
    };
    await cipher.init(cryptoFramework.CryptoMode.ENCRYPT_MODE, symKey, params);
    const out = await cipher.doFinal({ data: plainBytes });
    const data = out.data;
    if (data.length > plainBytes.length) {
      return data;
    }
    const authTag = params.authTag.data;
    return Encoding.concat(data, authTag);
  }

  async decrypt(encryptedBytes: Uint8Array, key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    if (encryptedBytes.length < 16) {
      throw new Error('Encrypted payload is too short.');
    }
    const cipherText = encryptedBytes.slice(0, encryptedBytes.length - 16);
    const authTag = encryptedBytes.slice(encryptedBytes.length - 16);
    const symKey = await cryptoFramework.createSymKeyGenerator('AES256').convertKey({ data: key });
    const cipher = cryptoFramework.createCipher('AES256|GCM|NoPadding');
    const params: cryptoFramework.GcmParamsSpec = {
      algName: 'GcmParamsSpec',
      iv: { data: nonce },
      aad: { data: new Uint8Array(0) },
      authTag: { data: authTag }
    };
    await cipher.init(cryptoFramework.CryptoMode.DECRYPT_MODE, symKey, params);
    const out = await cipher.doFinal({ data: cipherText });
    return out.data;
  }
}

export class RemoteCrypto {
  readonly keyPair: X25519KeyPair;
  private sharedKey?: Uint8Array;
  private readonly cipher: RemoteCryptoCipher;
  private readonly randomBytes: (size: number) => Uint8Array;

  constructor(options: RemoteCryptoOptions = {}) {
    this.keyPair = options.keyPair || X25519.generateKeyPair();
    this.cipher = options.cipher || new HarmonyRemoteCryptoCipher();
    this.randomBytes = options.randomBytes || Encoding.randomBytes;
  }

  getPublicKeyBase64(): string {
    return Encoding.bytesToBase64(this.keyPair.publicKey);
  }

  deriveSharedKey(peerPublicKeyBase64: string): void {
    const peerPublicKey = Encoding.base64ToBytes(peerPublicKeyBase64);
    this.sharedKey = X25519.scalarMult(this.keyPair.privateKey, peerPublicKey);
  }

  async encryptJson(payload: Object): Promise<EncryptedPayload> {
    const json = JSON.stringify(payload);
    const plainBytes = Encoding.utf8ToBytes(json);
    const nonce = this.randomBytes(12);
    const encrypted = await this.encryptBytes(plainBytes, nonce);
    const result = new EncryptedPayload();
    result.encrypted_data = Encoding.bytesToBase64(encrypted);
    result.nonce = Encoding.bytesToBase64(nonce);
    return result;
  }

  async decryptJson<T>(payload: EncryptedPayload): Promise<T> {
    const encrypted = Encoding.base64ToBytes(payload.encrypted_data);
    const nonce = Encoding.base64ToBytes(payload.nonce);
    const plainBytes = await this.decryptBytes(encrypted, nonce);
    return Encoding.parseJsonObject<T>(Encoding.bytesToUtf8(plainBytes));
  }

  private async encryptBytes(plainBytes: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    const key = this.requireSharedKey();
    return this.cipher.encrypt(plainBytes, key, nonce);
  }

  private async decryptBytes(encryptedBytes: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    const key = this.requireSharedKey();
    return this.cipher.decrypt(encryptedBytes, key, nonce);
  }

  private requireSharedKey(): Uint8Array {
    if (!this.sharedKey) {
      throw new Error('Remote encryption has not been initialized.');
    }
    return this.sharedKey;
  }
}
