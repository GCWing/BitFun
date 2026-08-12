import { readFileSync } from 'fs';
import { join } from 'path';

/**
 * Peer Device Mode controller/peer command ownership boundary.
 *
 * The controller-side deny list in the Web UI transport adapter is an
 * optimization: it keeps a controller-owned command on the controller without
 * a round trip. It is not the boundary. A controller running an older build,
 * or any non-Web-UI controller, still reaches a peer host over HostInvoke, so
 * each peer host must independently refuse every controller-owned command.
 *
 * The enforced direction is therefore one-way: whatever the controller refuses
 * to send, a peer host must also refuse to run. A host denying more than the
 * controller is safe and stays allowed.
 */

const FE_ADAPTER = 'src/web-ui/src/infrastructure/api/adapters/peer-device-adapter.ts';
const DESKTOP_HOST = 'src/apps/desktop/src/api/peer_host_invoke.rs';
const CLI_HOST = 'src/apps/cli/src/peer_host/deny.rs';

/**
 * Commands the CLI peer host answers before the deny-list check, so they are
 * intentionally absent from its list. See `src/apps/cli/src/peer_host/dispatch.rs`.
 */
const CLI_PRE_HANDLED_COMMANDS = new Set([
  'peer_control_attach',
  'peer_control_detach',
  'peer_mode_ping',
  'account_cancel_pending_login',
]);

function stripLineComments(text) {
  return text.replace(/\/\/[^\n]*/g, '');
}

function parseTypeScriptSet(source, name) {
  const match = new RegExp(`const ${name}\\s*=\\s*new Set\\(\\[(.*?)\\n\\]\\);`, 's').exec(source);
  if (!match) {
    return null;
  }
  return new Set(Array.from(stripLineComments(match[1]).matchAll(/'([^']+)'/g), m => m[1]));
}

function parseRustSlice(source, name) {
  const match = new RegExp(`static ${name}[^=]*=\\s*&\\[(.*?)\\n\\];`, 's').exec(source);
  if (!match) {
    return null;
  }
  return new Set(Array.from(stripLineComments(match[1]).matchAll(/"([^"]+)"/g), m => m[1]));
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

  const feSource = read(FE_ADAPTER);
  const desktopSource = read(DESKTOP_HOST);
  const cliSource = read(CLI_HOST);
  if (!feSource || !desktopSource || !cliSource) {
    return failures;
  }

  const controllerDenied = parseTypeScriptSet(feSource, 'LOCAL_ONLY_COMMANDS');
  const desktopDenied = parseRustSlice(desktopSource, 'LOCAL_ONLY_COMMANDS');
  const cliDenied = parseRustSlice(cliSource, 'LOCAL_ONLY_COMMANDS');

  for (const [path, parsed] of [
    [FE_ADAPTER, controllerDenied],
    [DESKTOP_HOST, desktopDenied],
    [CLI_HOST, cliDenied],
  ]) {
    if (!parsed) {
      failures.push({
        path,
        line: 1,
        message:
          'Could not parse LOCAL_ONLY_COMMANDS; keep the declaration shape the peer command policy check expects',
      });
    }
  }
  if (!controllerDenied || !desktopDenied || !cliDenied) {
    return failures;
  }

  const missingOnDesktop = [...controllerDenied].filter(command => !desktopDenied.has(command));
  if (missingOnDesktop.length > 0) {
    failures.push({
      path: DESKTOP_HOST,
      line: 1,
      message:
        `Desktop peer host must refuse every controller-owned command. Missing from LOCAL_ONLY_COMMANDS: ${missingOnDesktop.sort().join(', ')}. ` +
        'An older or non-Web-UI controller can still HostInvoke these onto this peer',
    });
  }

  const missingOnCli = [...controllerDenied].filter(
    command => !cliDenied.has(command) && !CLI_PRE_HANDLED_COMMANDS.has(command),
  );
  if (missingOnCli.length > 0) {
    failures.push({
      path: CLI_HOST,
      line: 1,
      message:
        `CLI peer host must refuse every controller-owned command. Missing from LOCAL_ONLY_COMMANDS: ${missingOnCli.sort().join(', ')}. ` +
        'An older or non-Web-UI controller can still HostInvoke these onto this peer',
    });
  }

  const staleCliExceptions = [...CLI_PRE_HANDLED_COMMANDS].filter(
    command => !controllerDenied.has(command),
  );
  if (staleCliExceptions.length > 0) {
    failures.push({
      path: CLI_HOST,
      line: 1,
      message:
        `Stale CLI pre-handled exception(s) in scripts/core-boundaries/peer-command-policy.mjs: ${staleCliExceptions.sort().join(', ')}. ` +
        'Remove the exception once the controller no longer treats the command as controller-owned',
    });
  }

  return failures;
}
