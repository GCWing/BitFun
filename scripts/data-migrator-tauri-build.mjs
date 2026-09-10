#!/usr/bin/env node
/** Builds the standalone, non-updating Data Migrator Tauri bundle. */
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { generateDataMigratorTheme } from './generate-data-migrator-theme.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const APP_DIR = join(ROOT, 'src', 'apps', 'data-migrator');

export function prepareDataMigratorTauriConfig(
  baseConfigPath,
  outputDirectory = join(APP_DIR, 'gen'),
) {
  const config = JSON.parse(readFileSync(baseConfigPath, 'utf8'));
  const manifest = readFileSync(join(APP_DIR, 'Cargo.toml'), 'utf8');
  const version = manifest.match(/^version = "([^"]+)"/m)?.[1];
  if (!version || config.version !== version) throw new Error('Migrator Cargo and Tauri versions must match.');
  config.build = {
    frontendDist: config.build?.frontendDist || 'ui',
  };
  if (config.plugins) {
    delete config.plugins.updater;
    if (Object.keys(config.plugins).length === 0) delete config.plugins;
  }
  if (config.bundle) delete config.bundle.createUpdaterArtifacts;

  mkdirSync(outputDirectory, { recursive: true });
  const output = join(
    outputDirectory,
    'tauri.generated.conf.json',
  );
  writeFileSync(output, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
  return output;
}

// The tool identity and the format destination are separate. Never inherit a
// branded Desktop build's data namespace or sibling-executable projections.
export function dataMigratorEnvironment(environment = process.env) {
  const result = { ...environment };
  for (const key of ['OPENBITFUN_DESKTOP_BINARY_NAME', 'OPENBITFUN_DATA_MIGRATOR_BINARY_NAME']) delete result[key];
  return {
    ...result,
    CI: 'true',
    OPENBITFUN_PRODUCT_ID: 'openbitfun',
    OPENBITFUN_DATA_NAMESPACE: 'openbitfun',
    OPENBITFUN_HIDDEN_DATA_DIRECTORY: '.openbitfun',
    OPENBITFUN_PRODUCT_BINARY_NAME: 'openbitfun-data-migrator',
    OPENBITFUN_PRODUCT_DISPLAY_NAME: 'OpenBitFun Data Migrator',
  };
}

function tauriArguments(raw) {
  let offset = 0;
  while (raw[offset] === '--') offset += 1;
  return raw.slice(offset);
}

async function main() {
  await generateDataMigratorTheme();
  const forwardArgs = tauriArguments(process.argv.slice(2));
  if (forwardArgs.some((arg) => arg.startsWith('--product-config'))) {
    throw new Error('The standalone migrator has its own identity and supports OpenBitFun data only.');
  }
  const generated = prepareDataMigratorTauriConfig(
    join(APP_DIR, 'tauri.conf.json'),
  );

  const tauriBin = join(ROOT, 'node_modules', '.bin', 'tauri');
  const result = spawnSync(tauriBin, ['build', '--config', generated, ...forwardArgs], {
    cwd: APP_DIR,
    env: dataMigratorEnvironment(),
    stdio: 'inherit',
    shell: true,
    windowsHide: true,
  });
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error?.stack || error);
    process.exit(1);
  });
}
