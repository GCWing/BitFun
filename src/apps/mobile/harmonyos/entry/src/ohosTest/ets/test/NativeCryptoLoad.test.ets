import { describe, it, expect } from '@ohos/hypium';
import { CloudAccountCrypto } from '../../../main/ets/services/CloudAccountCrypto';

export default function nativeCryptoLoadTest() {
  describe('NativeCryptoLoad', () => {
    it('keeps the ArkTS event loop responsive during concurrent Argon2id work', 0, async () => {
      const salt = new Uint8Array([0x42, 0x69, 0x74, 0x46, 0x75, 0x6e, 0x2d, 0x31,
        0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39]);
      let heartbeatTicks = 0;
      const heartbeatId = setInterval(() => {
        heartbeatTicks += 1;
      }, 5);
      const startedAt = Date.now();

      const results = await Promise.all([
        CloudAccountCrypto.derivePasswordHash('bitfun-device-load', salt, { m: 16 * 1024, t: 2, p: 1 }),
        CloudAccountCrypto.derivePasswordHash('bitfun-device-load', salt, { m: 16 * 1024, t: 2, p: 1 }),
        CloudAccountCrypto.derivePasswordHash('bitfun-device-load', salt, { m: 16 * 1024, t: 2, p: 1 })
      ]);
      clearInterval(heartbeatId);

      expect(results.length).assertEqual(3);
      expect(results[0].length).assertEqual(32);
      expect(bytesEqual(results[0], results[1])).assertTrue();
      expect(bytesEqual(results[1], results[2])).assertTrue();
      expect(heartbeatTicks > 0).assertTrue();
      expect(Date.now() - startedAt < 30000).assertTrue();
    });
  });
}

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) {
    return false;
  }
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) {
      return false;
    }
  }
  return true;
}
