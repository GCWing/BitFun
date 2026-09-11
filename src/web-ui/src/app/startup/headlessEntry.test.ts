// @vitest-environment node
import { access } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveConfig } from 'vite';
import { expect, it } from 'vitest';

it.each(['serve', 'build'] as const)('resolves the published headless runtime with Vite in %s mode', async (command) => {
  const configFile = fileURLToPath(new URL('../../../vite.config.ts', import.meta.url));
  const root = path.dirname(configFile);
  // Resolving configuration does not create a server or open a listening port.
  const config = await resolveConfig({ root, configFile, logLevel: 'silent' }, command);
  const entry = await config.createResolver()('@xterm/headless', path.join(root,
    'src/flow_chat/components/background-command/backgroundTerminalReplay.ts'));
  expect(entry).toBeDefined();
  expect(entry).toMatch(/\/lib-headless\/xterm-headless\.mjs$/);
  await expect(access(entry!)).resolves.toBeUndefined();
});
