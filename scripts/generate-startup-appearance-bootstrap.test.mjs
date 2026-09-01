import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

function readText(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

test('startup appearance bootstrap check is stable across line endings', () => {
  const generatorSource = readText('scripts/generate-startup-appearance-bootstrap.mjs');

  assert.match(generatorSource, /normalizeGeneratedText/, 'generator check should normalize line endings');
  assert.match(
    generatorSource,
    /replace\(?\/\\r\\n\?\/g,\s*'\\n'\)?/,
    'generator check should normalize CRLF and CR line endings to LF',
  );
  assert.match(
    generatorSource,
    /currentContentForCheck/,
    'generator check should compare normalized current content',
  );
});

test('startup appearance generation does not claim the Web UI HMR socket', () => {
  const generatorSource = readText('scripts/generate-startup-appearance-bootstrap.mjs');

  assert.match(
    generatorSource,
    /middlewareMode:\s*true,[\s\S]*?hmr:\s*false/,
    'the in-process Vite server should not collide with an active desktop dev server',
  );
});
