/*
 * Copyright (c) 2026 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import fsp from 'node:fs/promises';
import { resolveHdcOrThrow, runHdc, targetArgs } from './shared/hdc.mjs';
import { printKv, toErrorMessage } from './shared/utils.mjs';

function parseArgs(argv) {
  const map = new Map();
  const optionalKeys = new Set(['device-id']);

  for (let i = 0; i < argv.length; i += 1) {
    const t = argv[i];
    if (!t.startsWith('--')) {
      continue;
    }
    const key = t.slice(2);
    const val = argv[i + 1];
    if (!val || val.startsWith('--')) {
      if (optionalKeys.has(key)) {
        map.set(key, '');
        continue;
      }

      throw new Error(`Missing value for --${key}`);
    }
    map.set(key, val);
    i += 1;
  }

  const faultlogName = map.get('faultlog-name');
  const outputDir = map.get('output-dir');
  if (!faultlogName || !outputDir) {
    throw new Error('Required: --faultlog-name and --output-dir');
  }

  return {
    faultlogName,
    outputDir,
    deviceId: map.get('device-id') ?? '',
  };
}

function normalizeRemoteName(name) {
  return name.endsWith('.log') ? name : `${name}.log`;
}

function printFetchFailed(faultlogName, remotePath, nextAction) {
  printKv({
    status: 'fetch_failed',
    faultlog_name: faultlogName,
    remote_path: remotePath,
    local_path: '',
    next_action: nextAction,
  });
}

async function recvFaultlog(hdc, deviceId, remotePath, localPath) {
  const args = [hdc, ...targetArgs(deviceId), 'file', 'recv', remotePath, localPath];
  const out = await runHdc(args);
  if (out.exitCode !== 0) {
    throw new Error(out.stderr || out.stdout || `hdc file recv failed (code=${out.exitCode})`);
  }

  await fsp.access(localPath).catch(() => {
    throw new Error('Local file missing after recv.');
  });
}

function printFetched(faultlogName, remotePath, localPath) {
  printKv({
    status: 'fetched',
    faultlog_name: faultlogName,
    remote_path: remotePath,
    local_path: localPath,
    next_action: `parse-jscrash-log.mjs --log-file "${localPath}"`,
  });
}

async function main() {
  let faultlogName = '';
  let remotePath = '';

  try {
    const args = parseArgs(process.argv.slice(2));
    const base = normalizeRemoteName(path.basename(args.faultlogName.trim()));
    faultlogName = base;
    remotePath = `/data/log/faultlog/faultlogger/${base}`;
    const hdc = await resolveHdcOrThrow();

    fs.mkdirSync(args.outputDir, { recursive: true });
    const localPath = path.join(args.outputDir, base);
    await recvFaultlog(hdc, args.deviceId, remotePath, localPath);
    printFetched(base, remotePath, localPath);
    process.exit(0);
  } catch (err) {
    printFetchFailed(faultlogName, remotePath, toErrorMessage(err));
    process.exit(1);
  }
}

await main();
