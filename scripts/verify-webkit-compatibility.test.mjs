import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { findWebKitCompatibilityViolations } = require('./verify-webkit-compatibility.cjs');

function withTempDist(callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-webkit-compat-'));
  try {
    return callback(directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

test('accepts production JavaScript without the incompatible lookbehind', () => {
  withTempDist((directory) => {
    fs.writeFileSync(path.join(directory, 'app.js'), 'const email = /[-.\\w+]+@example\\.com/gu;');
    assert.deepEqual(findWebKitCompatibilityViolations(directory), []);
  });
});

test('rejects the remark-gfm variable-length email lookbehind', () => {
  withTempDist((directory) => {
    const filePath = path.join(directory, 'app.js');
    fs.writeFileSync(
      filePath,
      String.raw`const email = /(?<=^|\s|\p{P}|\p{S})([-.\w+]+)@example/gu;`
    );

    assert.deepEqual(findWebKitCompatibilityViolations(directory), [
      {
        filePath,
        label: 'remark-gfm variable-length email lookbehind',
      },
    ]);
  });
});
