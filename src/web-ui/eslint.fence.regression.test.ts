/**
 * Adapter-fence regression test.
 *
 * The `no-restricted-imports` / `no-restricted-syntax` rules in
 * `eslint.config.mjs` block direct and dynamic `invoke` imports from
 * `@tauri-apps/api/core` everywhere except `adapters/**` and the documented
 * `PeerHostInvokeBridge` exception. PR #2428 review #3 found that a global
 * `ignores` entry for `src/shared/context-system/core/types/**` let the whole
 * fence be bypassed in that directory — a direct `invoke` there passed lint.
 *
 * This test pins that the fence now applies to:
 *  - an ordinary business directory (always did), and
 *  - the context-system types directory (the regression).
 *
 * It also pins that the adapter exception still permits direct `invoke` inside
 * `adapters/**`. Run via `pnpm vitest run`.
 */
import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const webUiRoot = resolve(__dirname);

interface ProbeCase {
  name: string;
  filename: string;
  source: string;
  expectError: boolean;
}

const STATIC_PROBE = `import { invoke } from '@tauri-apps/api/core';
export const run = () => invoke('probe');
`;

const DYNAMIC_PROBE = `const mod = await import('@tauri-apps/api/core');
export const run = () => mod.invoke('probe');
`;

const cases: ProbeCase[] = [
  {
    name: 'ordinary business dir: static invoke is blocked',
    filename: 'src/app/__fence_probe_static.tsx',
    source: STATIC_PROBE,
    expectError: true,
  },
  {
    name: 'ordinary business dir: dynamic invoke is blocked',
    filename: 'src/app/__fence_probe_dynamic.tsx',
    source: DYNAMIC_PROBE,
    expectError: true,
  },
  {
    name: 'context-system types dir: static invoke is blocked (regression)',
    filename: 'src/shared/context-system/core/types/__fence_probe_static.tsx',
    source: STATIC_PROBE,
    expectError: true,
  },
  {
    name: 'context-system types dir: dynamic invoke is blocked (regression)',
    filename: 'src/shared/context-system/core/types/__fence_probe_dynamic.tsx',
    source: DYNAMIC_PROBE,
    expectError: true,
  },
  {
    name: 'adapter dir: static invoke is allowed (exception)',
    filename: 'src/infrastructure/api/adapters/__fence_probe_static.ts',
    source: STATIC_PROBE,
    expectError: false,
  },
];

/**
 * Resolve the eslint CLI entry as an absolute path and run it with `node`,
 * without a shell. Going through `pnpm`/`pnpm.cmd` needed `shell: true` on
 * Windows (a `.cmd` shim cannot be spawned with `shell: false`), which triggers
 * Node's DEP0190 security deprecation. Running the eslint JS entry directly
 * via `node` keeps `shell: false` on every platform and avoids the warning.
 */
function eslintBinPath(): string {
  return resolve(webUiRoot, 'node_modules/eslint/bin/eslint.js');
}

function lintProbe(probe: ProbeCase): { hasError: boolean; output: string } {
  // --stdin + --stdin-filename make the rule's path selectors see the probe as
  // if it lived at that path, so the fence applies per the probe's location.
  const args = [
    eslintBinPath(),
    '--stdin',
    '--stdin-filename',
    probe.filename,
  ];
  const result = spawnSync(process.execPath, args, {
    cwd: webUiRoot,
    input: probe.source,
    encoding: 'utf8',
    shell: false,
  });
  const combined = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  // ESLint exits non-zero and reports the restricted-imports/syntax error when
  // the fence fires; a clean probe exits 0 with no error lines.
  const hasError = /no-restricted-(imports|syntax)/.test(combined);
  return { hasError, output: combined };
}

describe('adapter fence regression', () => {
  for (const probe of cases) {
    it(probe.name, () => {
      const { hasError, output } = lintProbe(probe);
      if (probe.expectError) {
        expect(
          hasError,
          `expected the fence to block a direct/dynamic invoke at ${probe.filename}, but it did not:\n${output}`,
        ).toBe(true);
      } else {
        expect(
          hasError,
          `expected the adapter exception to allow invoke at ${probe.filename}, but the fence fired:\n${output}`,
        ).toBe(false);
      }
    });
  }
});
