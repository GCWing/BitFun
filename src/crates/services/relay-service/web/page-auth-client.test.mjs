import assert from 'node:assert/strict';
import test from 'node:test';
import { argon2idAsync } from '@noble/hashes/argon2.js';

test('browser Argon2id output matches the native account client', async () => {
  const password = new TextEncoder().encode('correct horse battery staple');
  const salt = Uint8Array.from({ length: 16 }, (_, index) => index);
  const output = await argon2idAsync(password, salt, {
    m: 8 * 1024,
    t: 1,
    p: 1,
    dkLen: 32,
    version: 0x13,
    asyncTick: 10,
  });

  assert.equal(
    Buffer.from(output).toString('base64'),
    'mu73UxPlhfSSwzxeEtgumtJTt914Yy1Tfomc1O3deJw=',
  );
});
