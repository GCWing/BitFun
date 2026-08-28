#!/usr/bin/env node
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { stagePluginHostResources } from './cli-product.mjs';

const ROOT = path.resolve(import.meta.dirname, '..');

export function serverBuildPlan(root = ROOT, environment = process.env, platform = process.platform) {
  const targetDirectory = environment.CARGO_TARGET_DIR
    ? path.resolve(root, environment.CARGO_TARGET_DIR)
    : path.join(root, 'target');
  const target = environment.CARGO_BUILD_TARGET?.trim();
  const releaseDirectory = path.join(targetDirectory, ...(target ? [target] : []), 'release');
  const windowsTarget = target ? target.split('-').includes('windows') : platform === 'win32';

  return {
    cargoArgs: [
      'build',
      '--release',
      '--manifest-path',
      path.join(root, 'src', 'apps', 'server', 'Cargo.toml'),
    ],
    binaryPath: path.join(releaseDirectory, `bitfun-server${windowsTarget ? '.exe' : ''}`),
    pluginHostDestination: path.join(releaseDirectory, 'resources', 'ext-host'),
  };
}

function run() {
  const plan = serverBuildPlan();
  const result = spawnSync('cargo', plan.cargoArgs, {
    cwd: ROOT,
    env: process.env,
    stdio: 'inherit',
  });
  if (result.error || result.status !== 0) {
    throw result.error ?? new Error(`Server cargo build exited with status ${result.status}`);
  }
  if (!existsSync(plan.binaryPath)) {
    throw new Error(`Server binary was not produced: ${plan.binaryPath}`);
  }
  stagePluginHostResources(plan.pluginHostDestination);
  console.log(`[server] staged plugin Host: ${plan.pluginHostDestination}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    run();
  } catch (error) {
    console.error(error?.stack || error);
    process.exitCode = 1;
  }
}
