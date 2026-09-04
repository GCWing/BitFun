import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  auditMiniappSurface,
  auditNativeSurface,
  compareMirrorTrees,
  discoverMiniappRoots,
  loadRegistry,
  validateRegistry,
} from './audit-frontend-colors.mjs';

const repositoryRoot = process.cwd();

function writeText(filePath, content) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf8');
}

test('frontend color surface registry covers every discovered MiniApp and names valid owners', () => {
  const registry = loadRegistry();
  assert.deepEqual(validateRegistry(registry, { repositoryRoot }), []);

  const registered = new Set([
    ...registry.surfaces.filter(surface => surface.kind === 'miniapp').map(surface => surface.root),
    ...registry.mirrors.map(mirror => mirror.path),
  ]);
  assert.deepEqual(discoverMiniappRoots(registry, { repositoryRoot }), Array.from(registered).sort());
  assert.ok(registry.exclusions.every(exclusion => exclusion.owner && exclusion.reason.length >= 24));
});

test('MiniApp audit accepts a narrow data-viz owner and rejects ordinary raw colors or host fallbacks', (t) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-miniapp-colors-'));
  t.after(() => fs.rmSync(fixtureRoot, { recursive: true, force: true }));
  writeText(path.join(fixtureRoot, 'app/ui.js'), "const SERIES = ['#ff00ff'];\n");
  writeText(path.join(fixtureRoot, 'app/style.css'), '.app { color: #ffffff; background: var(--openbitfun-bg, #000000); }\n');
  const surface = {
    id: 'fixture-miniapp',
    root: 'app',
    audit: {
      engine: 'miniapp',
      rawColorOwners: [{
        kind: 'data-viz',
        file: 'ui.js',
        startMarker: 'const SERIES = [',
        endMarker: '];',
        reason: 'Fixture categorical renderer palette owner.',
      }],
    },
  };
  const contract = {
    variables: [{ name: '--openbitfun-bg' }],
  };

  const report = auditMiniappSurface(surface, contract, { repositoryRoot: fixtureRoot });
  assert.equal(report.specializedOwners[0].occurrences, 1);
  assert.match(report.failures.join('\n'), /unowned hex #ffffff/);
  assert.match(report.failures.join('\n'), /fallback to public host variable --openbitfun-bg/);
});

test('MiniApp generated bundle checks ignore checkout-only line ending differences', (t) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-miniapp-generated-eol-'));
  t.after(() => fs.rmSync(fixtureRoot, { recursive: true, force: true }));
  writeText(path.join(fixtureRoot, 'app/source/ui/part.js'), 'const ready = true;\r\n');
  writeText(
    path.join(fixtureRoot, 'app/source/ui.js'),
    '/* ui/part.js */\nconst ready = true;\n\n',
  );
  const surface = {
    id: 'fixture-generated-eol',
    root: 'app',
    audit: {
      engine: 'miniapp',
      generatedBundles: [{
        output: 'source/ui.js',
        inputs: ['source/ui/part.js'],
      }],
    },
  };

  const report = auditMiniappSurface(surface, { variables: [] }, { repositoryRoot: fixtureRoot });
  assert.deepEqual(report.failures, []);

  writeText(
    path.join(fixtureRoot, 'app/source/ui.js'),
    '/* ui/part.js */\nconst ready = false;\n\n',
  );
  const staleReport = auditMiniappSurface(surface, { variables: [] }, { repositoryRoot: fixtureRoot });
  assert.match(staleReport.failures.join('\n'), /generated bundle source\/ui\.js is stale/);
});

test('native source audit rejects raw platform colors while ignoring generated projections', (t) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-native-colors-'));
  t.after(() => fs.rmSync(fixtureRoot, { recursive: true, force: true }));
  writeText(path.join(fixtureRoot, 'android/ui/Screen.kt'), 'val color = Color.White\n');
  writeText(path.join(fixtureRoot, 'android/generated/Tokens.kt'), 'val color = Color.Black\n');
  const surface = {
    id: 'fixture-android',
    root: 'android',
    audit: {
      engine: 'native',
      platform: 'android',
      extensions: ['.kt'],
      excludePaths: ['generated'],
    },
  };

  const report = auditNativeSurface(surface, { repositoryRoot: fixtureRoot });
  assert.equal(report.filesScanned, 1);
  assert.equal(report.rawColorOccurrences, 1);
  assert.match(report.failures[0], /Color\.White/);
});

test('MiniApp reference mirrors are byte-exact, including file inventory', (t) => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-miniapp-mirror-'));
  t.after(() => fs.rmSync(fixtureRoot, { recursive: true, force: true }));
  const source = path.join(fixtureRoot, 'source');
  const mirror = path.join(fixtureRoot, 'mirror');
  writeText(path.join(source, 'style.css'), '.app {}\n');
  writeText(path.join(mirror, 'style.css'), '.app {}\n');
  assert.deepEqual(compareMirrorTrees(source, mirror), []);

  writeText(path.join(mirror, 'style.css'), '.app { display: block; }\n');
  assert.deepEqual(compareMirrorTrees(source, mirror), ['mirror differs at style.css']);
});
