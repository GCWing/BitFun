import { cryptoFramework } from '@kit.CryptoArchitectureKit';
import { Encoding } from './Encoding';
import { X25519 } from './X25519';

/** Per-device ECDH and the Relay v1.0.0 domain-separated HKDF contract. */
export class CloudAccountCrypto {
  static async deriveDeviceKey(secret: Uint8Array, peerPublicKey: Uint8Array): Promise<Uint8Array> {
    const shared = X25519.scalarMult(secret, peerPublicKey);
    if (shared.every((value: number): boolean => value === 0)) {
      throw new Error('Invalid peer public key.');
    }
    const own = X25519.scalarMultBase(secret);
    let ownFirst = true;
    for (let i = 0; i < 32; i++) {
      if (own[i] !== peerPublicKey[i]) { ownFirst = own[i] < peerPublicKey[i]; break; }
    }
    const params: cryptoFramework.HKDFSpec = {
      algName: 'HKDF', key: shared,
      salt: Encoding.utf8ToBytes('OpenBitFun Relay v1.0.0 device key'),
      info: ownFirst ? Encoding.concat(own, peerPublicKey) : Encoding.concat(peerPublicKey, own),
      keySize: 32
    };
    try {
      return (await cryptoFramework.createKdf('HKDF|SHA256|EXTRACT_AND_EXPAND').generateSecret(params)).data;
    } finally { shared.fill(0); }
  }
}
