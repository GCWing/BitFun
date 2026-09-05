import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  BASE_DISPATCH_CAPABILITIES,
  DISPATCH_PROTOCOL_VERSION,
} from './dispatchPreflight';
import { PEER_CONTROLLER_LOCAL_COMMANDS } from '@/infrastructure/api/generated/remoteSurface';

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
  // The three peer surfaces (Web transport, Desktop host, CLI host) no longer
  // keep their own deny tables: all derive from the Product Operation
  // Registry. Pinning the registry rows therefore pins every surface at once.
  const registry = JSON.parse(
    read('../../../../../src/crates/contracts/product-domains/src/generated/remote-surface-registry.json'),
  ) as {
    operations: Array<{ id: string; surface: string; peer: { kind: string } }>;
  };
  const byId = new Map(registry.operations.map(operation => [operation.id, operation]));

  it('keeps every outbound command controller-local in the registry', () => {
    for (const command of [...OUTBOUND_DISPATCH_COMMANDS, 'dispatch_continue']) {
      const operation = byId.get(command);
      expect(operation, command).toBeDefined();
      expect(operation?.peer.kind, command).toBe('controller_local');
      expect(PEER_CONTROLLER_LOCAL_COMMANDS.has(command), command).toBe(true);
    }
  });

  it('routes the target verb family to the host control plane, never the controller', () => {
    const targets = registry.operations.filter(operation => operation.id.startsWith('dispatch_target_'));
    expect(targets).toHaveLength(15);
    for (const operation of targets) {
      expect(operation.peer.kind, operation.id).toBe('host_control_plane');
      expect(operation.surface, operation.id).toBe('host_invoke_only');
      expect(PEER_CONTROLLER_LOCAL_COMMANDS.has(operation.id), operation.id).toBe(false);
    }
  });

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

  // Preparing a target means installing the signed release and nothing else.
  // Compiling OpenBitFun on someone else's machine is not a command this client
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

  it('requires exactly the capabilities the shared Rust contract requires', () => {
    // `dispatch_required_target_capabilities()` = the unconditional base list
    // plus the platform-conditional detached worker. Parse both from the Rust
    // source so a capability added on either side fails here, in both
    // directions, instead of at runtime probe.
    const baseBlock = /DISPATCH_BASE_TARGET_CAPABILITIES: &\[&str\] = &\[([\s\S]*?)\n\];/u.exec(contractSource);
    expect(baseBlock).not.toBeNull();
    const rustRequired = new Set(
      Array.from(baseBlock![1].replace(/\/\/[^\n]*/g, '').matchAll(/"([^"]+)"/g), match => match[1]),
    );
    const detached = /DISPATCH_DETACHED_WORKER_CAPABILITY: &str = "([^"]+)"/u.exec(contractSource);
    expect(detached).not.toBeNull();
    rustRequired.add(detached![1]);
    expect(new Set(BASE_DISPATCH_CAPABILITIES)).toEqual(rustRequired);
  });

  it('keeps the versioned feature capabilities required on both sides', () => {
    for (const capability of ['product_identity', 'per_turn_options', 'session_query', 'inline_attachments', 'reasoning_presets']) {
      expect(BASE_DISPATCH_CAPABILITIES).toContain(capability);
      expect(contractSource).toContain(`"${capability}",`);
    }
  });

  it('requires the compiled product and data identities on every target probe', () => {
    const productIdentity = read(
      '../../../../../src/crates/contracts/core-types/src/product_identity.rs',
    );
    const targetDispatch = read('../../../../../src/apps/cli/src/dispatch/mod.rs');
    const targetProtocol = read('../../../../../src/apps/cli/src/dispatch/protocol.rs');
    const transportValidator = read(
      '../../../../../src/crates/services/services-integrations/src/remote_ssh/dispatch_ssh.rs',
    );

    expect(productIdentity).toContain('pub const fn product_id()');
    expect(productIdentity).toContain('pub const fn data_namespace()');
    expect(targetDispatch).toContain(
      'openbitfun_services_core::product_identity::product_id()',
    );
    expect(targetDispatch).toContain(
      'openbitfun_services_core::product_identity::data_namespace()',
    );
    expect(targetProtocol).toContain('pub(crate) product_id: String');
    expect(targetProtocol).toContain('pub(crate) data_namespace: String');
    expect(transportValidator).toContain(
      'openbitfun_services_core::product_identity::product_id()',
    );
    expect(transportValidator).toContain(
      'openbitfun_services_core::product_identity::data_namespace()',
    );
    expect(transportValidator).toContain('.get("productId")');
    expect(transportValidator).toContain('.get("dataNamespace")');
  });
});

describe('dispatch preflight contract', () => {
  it('fails closed on protocol v6 product identity, reasoning, and Git worktree delivery', () => {
    expect(DISPATCH_PROTOCOL_VERSION).toBe(6);
    expect(BASE_DISPATCH_CAPABILITIES).toContain('product_identity');
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
