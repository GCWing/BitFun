import { readFileSync } from 'fs';
import { join } from 'path';

/**
 * Peer Device Mode controller/peer command ownership boundary.
 *
 * The three peer surfaces (Web UI transport adapter, Desktop peer host, CLI
 * peer host) used to carry their own hand-written `LOCAL_ONLY_COMMANDS`
 * tables, and nothing but review kept them in agreement. They now derive
 * from one Product Operation Registry
 * (`src/crates/contracts/product-domains/src/remote_surface/`), whose
 * committed export is the artifact checked here.
 *
 * This check enforces two things:
 *
 * 1. No migrated surface grows a hand-written table again. A literal
 *    `LOCAL_ONLY_COMMANDS = ...[` declaration in any of these files is a fork
 *    of the registry and fails the check.
 * 2. The committed registry export still says what the surfaces rely on: the
 *    control plane, the dispatch outbound verbs, and every account command
 *    stay controller-owned; `git_trust_repository` stays operator-only.
 */

const FE_ADAPTER = 'src/web-ui/src/infrastructure/api/adapters/peer-device-adapter.ts';
const FE_GENERATED = 'src/web-ui/src/infrastructure/api/generated/remoteSurface.ts';
const DESKTOP_HOST = 'src/apps/desktop/src/api/peer_host_invoke.rs';
const CLI_HOST = 'src/apps/cli/src/peer_host/deny.rs';
const REGISTRY_EXPORT =
  'src/crates/contracts/product-domains/src/generated/remote-surface-registry.json';

/** Surfaces that must not re-introduce a hand-written deny table. */
const MIGRATED_FILES = [FE_ADAPTER, DESKTOP_HOST, CLI_HOST];

const HAND_WRITTEN_TABLE = /LOCAL_ONLY_COMMANDS(?:\s*:\s*[^=]+)?\s*=\s*(?:new Set\(\s*)?&?\[/u;

function parseTypeScriptSet(source, name) {
  const match = new RegExp(`const ${name}[^=]*=\\s*new Set\\(\\[(.*?)\\n\\]\\);`, 's').exec(source);
  if (!match) {
    return null;
  }
  return new Set(Array.from(match[1].replace(/\/\/[^\n]*/g, '').matchAll(/"([^"]+)"/g), m => m[1]));
}

export function checkPeerCommandPolicySync(root) {
  const failures = [];

  const read = (relativePath) => {
    try {
      return readFileSync(join(root, relativePath), 'utf8');
    } catch {
      failures.push({
        path: relativePath,
        line: 1,
        message:
          'Peer command policy check could not read this file; update scripts/core-boundaries/peer-command-policy.mjs if it moved',
      });
      return null;
    }
  };

  for (const relativePath of MIGRATED_FILES) {
    const source = read(relativePath);
    if (source && HAND_WRITTEN_TABLE.test(source)) {
      failures.push({
        path: relativePath,
        line: 1,
        message:
          'This peer surface derives its controller-owned command set from the Product Operation Registry; ' +
          'do not re-introduce a hand-written LOCAL_ONLY_COMMANDS table. Add or change the row in ' +
          'src/crates/contracts/product-domains/src/remote_surface/table.rs instead',
      });
    }
  }

  const registrySource = read(REGISTRY_EXPORT);
  const generatedSource = read(FE_GENERATED);
  if (!registrySource || !generatedSource) {
    return failures;
  }

  let registry;
  try {
    registry = JSON.parse(registrySource);
  } catch (error) {
    failures.push({
      path: REGISTRY_EXPORT,
      line: 1,
      message: `Product Operation Registry export is not valid JSON: ${error.message}`,
    });
    return failures;
  }

  const byId = new Map((registry.operations ?? []).map(operation => [operation.id, operation]));
  const controllerLocal = new Set(
    (registry.operations ?? [])
      .filter(operation => operation.surface === 'tauri_command')
      .filter(operation => ['controller_local', 'host_control_plane'].includes(operation?.peer?.kind))
      .map(operation => operation.id),
  );

  const generatedLocal = parseTypeScriptSet(generatedSource, 'PEER_CONTROLLER_LOCAL_COMMANDS');
  if (!generatedLocal) {
    failures.push({
      path: FE_GENERATED,
      line: 1,
      message:
        'Could not parse PEER_CONTROLLER_LOCAL_COMMANDS; regenerate with `pnpm run capabilities:generate`',
    });
  } else {
    const stale = [...generatedLocal].filter(command => !controllerLocal.has(command));
    const missing = [...controllerLocal].filter(command => !generatedLocal.has(command));
    if (stale.length || missing.length) {
      failures.push({
        path: FE_GENERATED,
        line: 1,
        message:
          `Generated controller-local set drifted from the registry export (stale: ${stale.sort().join(', ') || '-'}; ` +
          `missing: ${missing.sort().join(', ') || '-'}). Run: pnpm run capabilities:generate`,
      });
    }
  }

  const expectStance = (command, kinds, why) => {
    const operation = byId.get(command);
    const kind = operation?.peer?.kind;
    if (!operation || !kinds.includes(kind)) {
      failures.push({
        path: REGISTRY_EXPORT,
        line: 1,
        message: `Registry row '${command}' must have peer stance ${kinds.join('|')} (found ${kind ?? 'no row'}): ${why}`,
      });
    }
  };

  for (const command of ['peer_control_attach', 'peer_control_detach', 'peer_mode_ping']) {
    expectStance(command, ['host_control_plane'], 'the peer control plane is answered before dispatch');
  }
  for (const operation of registry.operations ?? []) {
    if (operation.id.startsWith('dispatch_target_')) {
      expectStance(operation.id, ['host_control_plane'], 'dispatch target verbs route to the durable CLI runner before the peer bridge');
    } else if (operation.id.startsWith('dispatch_')) {
      expectStance(operation.id, ['controller_local'], 'outbound dispatch is controller-owned');
    } else if (operation.id.startsWith('account_')) {
      expectStance(operation.id, ['controller_local'], 'account identity and cloud session APIs stay on the controller');
    }
  }
  expectStance(
    'git_trust_repository',
    ['operator_only'],
    'granting Git ownership trust is decided at the machine that owns the repository and refused on every peer host',
  );

  return failures;
}
