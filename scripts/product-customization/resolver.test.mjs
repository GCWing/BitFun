import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import test from 'node:test';

import { ProductDefinitionError, resolveProductDefinition } from './resolver.mjs';

const ROOT = resolve(import.meta.dirname, '..', '..');
const ACME = join(ROOT, 'products', 'fixtures', 'acme', 'product.jsonc');

test('default and custom members resolve through one deterministic contract', () => {
  const bitfun = resolveProductDefinition({ rootDir: ROOT, member: 'desktop' });
  const desktop = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'desktop' });
  const cli = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'cli' });

  assert.equal(bitfun.assembly.binaryName, 'bitfun-desktop');
  assert.equal(desktop.assembly.bundleId, 'com.acme.desktop');
  assert.equal(cli.assembly.binaryName, 'acme');
  assert.equal(cli.assembly.bundleId, undefined);
  assert.notEqual(desktop.assembly.assemblyDigest, cli.assembly.assemblyDigest);
  assert.equal(
    resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'desktop' })
      .assembly.assemblyDigest,
    desktop.assembly.assemblyDigest,
  );
});

test('localized names are validated against the shared locale contract', () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'desktop' });
  assert.equal(resolution.productNames['en-US'], 'Acme Desktop');
  assert.equal(resolution.productNames['zh-CN'], 'Acme 桌面版');
  assert.match(resolution.assembly.localeDigest, /^[a-f0-9]{64}$/);
});

test('schema version one rejects future owner sections instead of pretending to support them', () => {
  const directory = mkdtempSync(join(tmpdir(), 'bitfun-product-c0a-'));
  const source = readFileSync(ACME, 'utf8').replace(
    /\n}\s*$/,
    ',\n  "assets": { "desktopAppIcon": "icon.png" }\n}\n',
  );
  const config = join(directory, 'product.jsonc');
  writeFileSync(config, source, 'utf8');

  assert.throws(
    () => resolveProductDefinition({ rootDir: ROOT, productConfig: config, member: 'desktop' }),
    (error) => error instanceof ProductDefinitionError && error.code === 'unknown_field',
  );
});

test('locale paths cannot escape the product definition directory', () => {
  const directory = mkdtempSync(join(tmpdir(), 'bitfun-product-c0a-'));
  const source = readFileSync(ACME, 'utf8').replace('"./locales"', '"../"');
  const config = join(directory, 'product.jsonc');
  writeFileSync(config, source, 'utf8');

  assert.throws(
    () => resolveProductDefinition({ rootDir: ROOT, productConfig: config, member: 'desktop' }),
    (error) => error instanceof ProductDefinitionError && error.code === 'resource_path_escape',
  );
});

test('invalid member and unsafe binary identity fail with stable codes', () => {
  assert.throws(
    () => resolveProductDefinition({ rootDir: ROOT, member: 'server' }),
    (error) => error instanceof ProductDefinitionError && error.code === 'invalid_member',
  );

  const directory = mkdtempSync(join(tmpdir(), 'bitfun-product-c0a-'));
  cpSync(join(ROOT, 'products', 'fixtures', 'acme', 'locales'), join(directory, 'locales'), {
    recursive: true,
  });
  const source = readFileSync(ACME, 'utf8').replace('"binaryName": "acme"', '"binaryName": "CON"');
  const config = join(directory, 'product.jsonc');
  writeFileSync(config, source, 'utf8');
  assert.throws(
    () => resolveProductDefinition({ rootDir: ROOT, productConfig: config, member: 'cli' }),
    (error) => error instanceof ProductDefinitionError && error.code === 'invalid_binary_name',
  );
});
