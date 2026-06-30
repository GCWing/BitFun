import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

function readText(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

test('startup theme bootstrap check is stable across line endings', () => {
  const generatorSource = readText('scripts/generate-startup-theme-bootstrap.mjs');

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
