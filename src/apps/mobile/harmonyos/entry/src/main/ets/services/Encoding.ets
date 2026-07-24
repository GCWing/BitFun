import buffer from '@ohos.buffer';
import { cryptoFramework } from '@kit.CryptoArchitectureKit';
import util from '@ohos.util';

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

  private static copyBuffer(value: buffer.Buffer): Uint8Array {
    const copy = new Uint8Array(value.length);
    for (let index = 0; index < value.length; index++) {
      copy[index] = value[index];
    }
    return copy;
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
