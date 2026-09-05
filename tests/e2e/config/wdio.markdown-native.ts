import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';
import { createEmbeddedConfig } from './embedded-driver';

// Worker processes inherit this root; each runner invocation starts fresh.
const runtimeRoot = process.env.OPENBITFUN_MARKDOWN_E2E_ROOT
  ?? mkdtempSync(join(tmpdir(), 'openbitfun-markdown-profile-'));
process.env.OPENBITFUN_MARKDOWN_E2E_ROOT = runtimeRoot;
Object.assign(process.env, {
  OPENBITFUN_E2E_STORAGE_ROOT: runtimeRoot,
  OPENBITFUN_E2E_USER_ROOT: join(runtimeRoot, 'user'),
  OPENBITFUN_USER_ROOT: join(runtimeRoot, 'user'),
  OPENBITFUN_E2E_HOME: join(runtimeRoot, 'home'),
  OPENBITFUN_HOME: join(runtimeRoot, 'home'),
  OPENBITFUN_E2E_LOG_DIR: join(runtimeRoot, 'logs'),
  OPENBITFUN_E2E_STORAGE_GUARD: '1',
  OPENBITFUN_E2E_PACKAGED_FRONTEND: '1',
  OPENBITFUN_E2E_FRONTEND_DIR: resolve(fileURLToPath(new URL('../../../dist', import.meta.url))),
});
const base = createEmbeddedConfig(['../specs/markdown-native.spec.ts'], 'Markdown native');
export const config: Options.Testrunner = {
  ...base,
  async onComplete(...args) {
    try {
      if (typeof base.onComplete === 'function') await base.onComplete(...args);
    } finally {
      rmSync(runtimeRoot, { recursive: true, force: true });
    }
  },
};
