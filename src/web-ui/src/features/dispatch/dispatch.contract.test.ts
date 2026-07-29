import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  BASE_DISPATCH_CAPABILITIES,
  isDispatchWorkspaceReady,
} from './dispatchPreflight';

const OUTBOUND_DISPATCH_COMMANDS = [
  'dispatch_list_targets',
  'dispatch_probe_target',
  'dispatch_install_cli_start',
  'dispatch_install_cli_poll',
  'dispatch_install_cli_cancel',
  'dispatch_submit',
  'dispatch_status',
  'dispatch_cancel',
  'dispatch_list_jobs',
] as const;

function read(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('dispatch controller-only routing contract', () => {
  const tables = [
    {
      name: 'Web peer transport',
      source: read('../../infrastructure/api/adapters/peer-device-adapter.ts'),
    },
    {
      name: 'Desktop peer host bridge',
      source: read('../../../../../src/apps/desktop/src/api/peer_host_invoke.rs'),
    },
    {
      name: 'CLI peer host deny table',
      source: read('../../../../../src/apps/cli/src/peer_host/deny.rs'),
    },
  ];

  for (const table of tables) {
    it(`keeps every outbound command local in the ${table.name}`, () => {
      for (const command of OUTBOUND_DISPATCH_COMMANDS) {
        expect(table.source).toMatch(new RegExp(`['"]${command}['"]`));
      }
    });
  }
});

describe('dispatch preflight contract', () => {
  it('requires the workspace serialization capability enforced by submit', () => {
    expect(BASE_DISPATCH_CAPABILITIES).toContain('workspace_serialization');
  });

  it('invalidates workspace readiness when the input no longer matches the probe', () => {
    const probe = {
      path: '/repo',
      exists: true,
      isDirectory: true,
      isGitRepository: true,
    };

    expect(isDispatchWorkspaceReady(' /repo ', probe)).toBe(true);
    expect(isDispatchWorkspaceReady('/another-repo', probe)).toBe(false);
  });
});
