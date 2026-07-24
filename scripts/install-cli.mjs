import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function installerCommand(platform = process.platform) {
  if (platform === 'win32') {
    return {
      command: 'powershell.exe',
      args: [
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        path.join(repoRoot, 'src', 'apps', 'cli', 'install.ps1'),
      ],
    };
  }

  if (platform === 'darwin' || platform === 'linux') {
    return {
      command: 'bash',
      args: [path.join(repoRoot, 'src', 'apps', 'cli', 'install.sh')],
    };
  }

  throw new Error(`Unsupported platform for BitFun CLI installation: ${platform}`);
}

function main() {
  const installer = installerCommand();
  const result = spawnSync(installer.command, [...installer.args, ...process.argv.slice(2)], {
    cwd: repoRoot,
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }
  process.exit(result.status ?? 1);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
