import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  BASE_DISPATCH_CAPABILITIES,
  DISPATCH_PROTOCOL_VERSION,
} from './dispatchPreflight';

const OUTBOUND_DISPATCH_COMMANDS = [
  'dispatch_list_targets',
  'dispatch_probe_target',
  'dispatch_install_cli_start',
  'dispatch_install_cli_poll',
  'dispatch_install_cli_cancel',
  'dispatch_provision_target',
  'dispatch_sync_model_config',
  'dispatch_submit',
  'dispatch_status',
  'dispatch_query',
  'dispatch_cancel',
  'dispatch_list_jobs',
  'dispatch_answer',
  'dispatch_append',
  'dispatch_sync_result',
  'dispatch_load_transcript',
  'dispatch_save_transcript',
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

  // Preparing a target means installing the signed release and nothing else.
  // Compiling BitFun on someone else's machine is not a command this client
  // can issue, so the name must not survive anywhere in the routing surface.
  it('exposes no way to build the CLI on a target', () => {
    const sources = [
      ...tables.map(table => table.source),
      read('./dispatchApi.ts'),
      read('./types.ts'),
      read('../../../../../src/apps/server/src/routes/dispatch.rs'),
    ];
    for (const source of sources) {
      expect(source).not.toContain('dispatch_install_cli_source_start');
      expect(source).not.toContain('sourceBuild');
    }
  });
});

describe('dispatch wire contract single source', () => {
  // The Rust side has exactly one contract file; the Web UI's copies must
  // track it. A capability or version bump that misses one side fails here
  // instead of at runtime probe.
  const contractSource = read(
    '../../../../../src/crates/services/services-core/src/dispatch_contract.rs',
  );

  it('pins the protocol version to the shared Rust contract', () => {
    expect(contractSource).toContain(
      `pub const DISPATCH_PROTOCOL_VERSION: u32 = ${DISPATCH_PROTOCOL_VERSION};`,
    );
  });

  it('requires only capabilities the shared Rust contract defines', () => {
    for (const capability of BASE_DISPATCH_CAPABILITIES) {
      if (capability === 'detached_worker') {
        expect(contractSource).toContain(
          `DISPATCH_DETACHED_WORKER_CAPABILITY: &str = "${capability}"`,
        );
        continue;
      }
      expect(contractSource).toContain(`"${capability}",`);
    }
  });

  it('keeps the versioned feature capabilities required on both sides', () => {
    for (const capability of ['per_turn_options', 'session_query', 'inline_attachments', 'reasoning_presets']) {
      expect(BASE_DISPATCH_CAPABILITIES).toContain(capability);
      expect(contractSource).toContain(`"${capability}",`);
    }
  });
});

describe('dispatch preflight contract', () => {
  it('fails closed on protocol v5 target reasoning and Git worktree delivery', () => {
    expect(DISPATCH_PROTOCOL_VERSION).toBe(5);
    expect(BASE_DISPATCH_CAPABILITIES).toContain('per_turn_options');
    expect(BASE_DISPATCH_CAPABILITIES).toEqual(expect.arrayContaining([
      'workspace_serialization',
      'workspace_git_worktree',
      'workspace_git_bundle_upload',
      'workspace_git_sync',
    ]));
    expect(BASE_DISPATCH_CAPABILITIES.join(' ')).not.toContain('workspace_snapshot');
    expect(BASE_DISPATCH_CAPABILITIES).not.toContain('workspace_result_bundle');
  });

  it('uses one Git sync command and exposes no snapshot pull/apply fallback', () => {
    const api = read('./dispatchApi.ts');
    const types = read('./types.ts');
    const picker = read('./DispatchTargetPicker.tsx');
    expect(api).toContain("'dispatch_sync_result'");
    expect(api).not.toContain("'dispatch_pull_result'");
    expect(api).not.toContain("'dispatch_apply_result'");
    expect(types).not.toContain("'snapshot-source'");
    expect(types).not.toContain("'snapshot-exact'");
    expect(types).not.toContain('defaultWorkspace?:');
    expect(picker).not.toContain('option.defaultWorkspace');
  });
});

describe('dispatch navigation scope contract', () => {
  const sessionsSection = read(
    '../../app/components/NavPanel/sections/sessions/SessionsSection.tsx',
  );
  const sessionsSectionStyles = read(
    '../../app/components/NavPanel/sections/sessions/SessionsSection.scss',
  );

  it('keeps dispatch presentation on sessions without a workspace-level target filter', () => {
    expect(sessionsSection).toContain('session.config.dispatchTarget');
    expect(sessionsSection).toContain('session.config.dispatchJobState');
    expect(sessionsSection).not.toContain('dispatchTargetFilter');
    expect(sessionsSectionStyles).not.toContain('session-target-filter');
  });
});
