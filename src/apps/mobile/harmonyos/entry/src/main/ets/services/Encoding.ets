import buffer from '@ohos.buffer';
import { cryptoFramework } from '@kit.CryptoArchitectureKit';
import util from '@ohos.util';

/**
 * Byte/text conversions on the path every relay RPC takes.
 *
 * These run inline on whichever thread issues the call, which for the device
 * RPC is the UI thread, and they run over the whole request and the whole
 * response — a transcript-sized `poll_session` reply is hundreds of kilobytes.
 * So the cost per byte is what decides whether the frame that should be
 * painting a skeleton gets to run at all. Everything here has to stay on a
 * native bulk primitive; anything that touches bytes one at a time from ArkTS
 * shows up directly as a blocked main thread.
 */
export class Encoding {
  static utf8ToBytes(text: string): Uint8Array {
    const value = buffer.from(text, 'utf-8');
    return Encoding.copyBuffer(value);
  }

  static bytesToUtf8(bytes: Uint8Array): string {
    return buffer.from(bytes).toString('utf-8');
  }

  static base64ToBytes(text: string): Uint8Array {
    const value = buffer.from(text, 'base64');
    return Encoding.copyBuffer(value);
  }

  static bytesToBase64(bytes: Uint8Array): string {
    return buffer.from(bytes).toString('base64');
  }

  static randomBytes(size: number): Uint8Array {
    const random = cryptoFramework.createRandom();
    const data = random.generateRandomSync(size).data;
    return Encoding.copyRandomData(data, size);
  }

  static randomId(prefix: string): string {
    return `${prefix}-${util.generateRandomUUID(false)}`;
  }

  static concat(left: Uint8Array, right: Uint8Array): Uint8Array {
    const out = new Uint8Array(left.length + right.length);
    out.set(left, 0);
    out.set(right, left.length);
    return out;
  }

  static copyBytes(bytes: Uint8Array): Uint8Array {
    const copy = new Uint8Array(bytes.length);
    copy.set(bytes);
    return copy;
  }

  static parseJsonObject<T>(text: string): T {
    return JSON.parse(text) as T;
  }

  /**
   * Detaches a `Buffer` into a plain `Uint8Array` in one native copy.
   *
   * This used to read `value[index]` in a loop. On a `Buffer` each of those is
   * a proxied property access rather than a typed-array load, so a
   * transcript-sized response — hundreds of kilobytes, hundreds of thousands of
   * reads — spent about a second of the UI thread here, inside the window where
   * the conversation pane was meant to be painting its skeleton.
   *
   * `value.buffer` is already scoped to this Buffer's own bytes: measured on
   * device, a 32-byte Buffer sitting at pool offset 856 reports a 32-byte
   * ArrayBuffer, so the offset must not be applied to it. `util.Base64Helper`
   * and `util.TextEncoder` would skip the Buffer entirely, but both return
   * empty on this runtime.
   */
  private static copyBuffer(value: buffer.Buffer): Uint8Array {
    return new Uint8Array(value.buffer);
  }

  private static copyRandomData(value: Uint8Array | ArrayBuffer, expectedSize: number): Uint8Array {
    const bytes = value as Uint8Array;
    if (bytes.length === expectedSize) {
      return Encoding.copyBytes(bytes);
    }

    const rawBuffer = value as ArrayBuffer;
    if (rawBuffer.byteLength === expectedSize) {
      return Encoding.copyBytes(new Uint8Array(rawBuffer));
    }

    return Encoding.randomBytesFromUuid(expectedSize);
  }

  private static randomBytesFromUuid(size: number): Uint8Array {
    const out = new Uint8Array(size);
    let offset = 0;
    while (offset < size) {
      const uuid = util.generateRandomUUID(false).replace(/-/g, '');
      if (uuid.length < 2) {
        throw new Error(`Expected ${size} random bytes.`);
      }
      for (let index = 0; index + 1 < uuid.length && offset < size; index += 2) {
        out[offset] = Number.parseInt(uuid.slice(index, index + 2), 16);
        offset += 1;
      }
    }
    return out;
  }
}
