import assert from 'node:assert/strict';
import test from 'node:test';

import {
  analyzeSourceEntries,
  compareInventories,
} from './check-design-system-adoption.mjs';

test('inventory tracks legacy imports, new-library progress, and native controls', () => {
  const inventory = analyzeSourceEntries([
    [
      'src/web-ui/src/feature/Example.tsx',
      `
        import { IconButton, type SelectOption } from '@/component-library';
        import { Button } from '@bitfun/ui';
        export function Example() {
          return <button><input /></button>;
        }
      `,
    ],
    [
      'src/web-ui/src/feature/direct.ts',
      "import { Tooltip } from '@components/Tooltip';",
    ],
  ]);

  assert.equal(inventory.legacy.moduleReferences, 2);
  assert.equal(inventory.legacy.aliasReferences, 1);
  assert.deepEqual(inventory.legacy.bySymbol, {
    IconButton: 1,
    SelectOption: 1,
    Tooltip: 1,
  });
  assert.equal(inventory.newLibrary.moduleReferences, 1);
  assert.equal(inventory.nativeControls.total, 2);
});

test('ratchet rejects a new debt dimension even when the total is unchanged', () => {
  const baseline = analyzeSourceEntries([
    ['src/web-ui/src/feature/One.tsx', "import { Input } from '@/component-library';"],
  ]);
  const current = analyzeSourceEntries([
    ['src/web-ui/src/feature/One.tsx', "import { Select } from '@/component-library';"],
  ]);

  const comparison = compareInventories(current, baseline);

  assert.deepEqual(
    comparison.increases.map(item => item.key),
    ['legacy.bySymbol.Select'],
  );
  assert.equal(
    comparison.reductions.some(item => item.key === 'legacy.bySymbol.Input'),
    true,
  );
});

test('native controls are ratcheted per file and tag', () => {
  const baseline = analyzeSourceEntries([
    ['src/web-ui/src/feature/One.tsx', 'export const One = () => <button />;'],
  ]);
  const current = analyzeSourceEntries([
    ['src/web-ui/src/feature/Two.tsx', 'export const Two = () => <button />;'],
  ]);

  const comparison = compareInventories(current, baseline);

  assert.equal(comparison.increases.length, 1);
  assert.equal(
    comparison.increases[0].key,
    'nativeControls.byFile.src/web-ui/src/feature/Two.tsx.<button>',
  );
});

test('allowlisted files are excluded from native-control debt', () => {
  const inventory = analyzeSourceEntries(
    [['src/web-ui/src/feature/NativePrimitive.tsx', 'export const Root = () => <button />;']],
    { nativeControlAllowlist: ['src/web-ui/src/feature/NativePrimitive.tsx'] },
  );

  assert.equal(inventory.nativeControls.total, 0);
  assert.deepEqual(inventory.nativeControls.byFile, {});
});
