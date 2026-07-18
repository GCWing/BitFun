import { Encoding } from './Encoding';

const P: bigint = (1n << 255n) - 19n;
const A24: bigint = 121665n;

export interface X25519KeyPair {
  privateKey: Uint8Array;
  publicKey: Uint8Array;
}

function mod(value: bigint): bigint {
  const result = value % P;
  return result >= 0n ? result : result + P;
}

function decodeLittleEndian(bytes: Uint8Array): bigint {
  let value = 0n;
  for (let index = bytes.length - 1; index >= 0; index--) {
    value = (value << 8n) + BigInt(bytes[index]);
  }
  return value;
}

function encodeLittleEndian(value: bigint): Uint8Array {
  let current = mod(value);
  const bytes = new Uint8Array(32);
  for (let index = 0; index < 32; index++) {
    bytes[index] = Number(current & 255n);
    current >>= 8n;
  }
  return bytes;
}

function powMod(base: bigint, exponent: bigint): bigint {
  let result = 1n;
  let current = mod(base);
  let exp = exponent;
  while (exp > 0n) {
    if ((exp & 1n) === 1n) {
      result = mod(result * current);
    }
    current = mod(current * current);
    exp >>= 1n;
  }
  return result;
}

export class X25519 {
  static generateKeyPair(): X25519KeyPair {
    const privateKey = Encoding.randomBytes(32);
    const publicKey = X25519.scalarMultBase(privateKey);
    return { privateKey, publicKey };
  }

  static scalarMultBase(privateKey: Uint8Array): Uint8Array {
    const base = new Uint8Array(32);
    base[0] = 9;
    return X25519.scalarMult(privateKey, base);
  }

  static scalarMult(privateKey: Uint8Array, publicKey: Uint8Array): Uint8Array {
    if (privateKey.length !== 32 || publicKey.length !== 32) {
      throw new Error('X25519 keys must be 32 bytes.');
    }

    const scalar = Encoding.copyBytes(privateKey);
    scalar[0] &= 248;
    scalar[31] &= 127;
    scalar[31] |= 64;

    const uBytes = Encoding.copyBytes(publicKey);
    uBytes[31] &= 127;

    const x1 = decodeLittleEndian(uBytes);
    let x2 = 1n;
    let z2 = 0n;
    let x3 = x1;
    let z3 = 1n;
    let swap = 0;
    const k = decodeLittleEndian(scalar);

    for (let bit = 254; bit >= 0; bit--) {
      const currentBit = Number((k >> BigInt(bit)) & 1n);
      swap = swap ^ currentBit;
      if (swap === 1) {
        const tx = x2;
        x2 = x3;
        x3 = tx;
        const tz = z2;
        z2 = z3;
        z3 = tz;
      }
      swap = currentBit;

      const a = mod(x2 + z2);
      const aa = mod(a * a);
      const b = mod(x2 - z2);
      const bb = mod(b * b);
      const e = mod(aa - bb);
      const c = mod(x3 + z3);
      const d = mod(x3 - z3);
      const da = mod(d * a);
      const cb = mod(c * b);
      x3 = mod((da + cb) * (da + cb));
      z3 = mod(x1 * mod((da - cb) * (da - cb)));
      x2 = mod(aa * bb);
      z2 = mod(e * mod(aa + A24 * e));
    }

    if (swap === 1) {
      const tx = x2;
      x2 = x3;
      x3 = tx;
      const tz = z2;
      z2 = z3;
      z3 = tz;
    }

    const inverse = powMod(z2, P - 2n);
    return encodeLittleEndian(x2 * inverse);
  }
}
