import { Encoding } from './Encoding';
import bitfunCrypto from 'libbitfun_crypto.so';

export interface CloudAccountKdfParams {
  m: number;
  t: number;
  p: number;
}

/**
 * Desktop-compatible Argon2id derivation. Keep this adapter isolated so the
 * relay protocol and the UI never depend on a particular crypto provider.
 */
export class CloudAccountCrypto {
  private static readonly KDF_TIMEOUT_MS: number = 30000;

  static async derivePasswordHash(password: string, salt: Uint8Array, params: CloudAccountKdfParams): Promise<Uint8Array> {
    CloudAccountCrypto.validateParams(params, salt);
    return CloudAccountCrypto.withTimeout(
      bitfunCrypto.argon2idRaw(Encoding.utf8ToBytes(password), salt, params.m, params.t, params.p),
      CloudAccountCrypto.KDF_TIMEOUT_MS
    );
  }

  private static withTimeout(operation: Promise<Uint8Array>, timeoutMs: number): Promise<Uint8Array> {
    return new Promise<Uint8Array>((resolve, reject) => {
      let settled = false;
      const timeoutId = setTimeout(() => {
        if (settled) {
          return;
        }
        settled = true;
        reject(new Error('Argon2id operation timed out.'));
      }, timeoutMs);
      operation.then((value: Uint8Array) => {
        if (settled) {
          return;
        }
        settled = true;
        clearTimeout(timeoutId);
        resolve(value);
      }).catch((err: Object) => {
        if (settled) {
          return;
        }
        settled = true;
        clearTimeout(timeoutId);
        reject(err);
      });
    });
  }

  private static validateParams(params: CloudAccountKdfParams, salt: Uint8Array): void {
    if (salt.length < 8 || salt.length > 64 ||
      params.m < 8 * 1024 || params.m > 256 * 1024 ||
      params.t < 1 || params.t > 10 || params.p < 1 || params.p > 16) {
      throw new Error('Invalid Argon2id parameters.');
    }
  }
}
