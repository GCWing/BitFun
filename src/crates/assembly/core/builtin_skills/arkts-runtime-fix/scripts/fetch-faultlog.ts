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
import { resolveHdcOrThrow, runHdc, targetArgs } from './shared/hdc';
import { printKv, toErrorMessage } from './shared/utils';

function parseArgs(argv: string[]) {
  const map = new Map<string, string>();
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

function normalizeRemoteName(name: string) {
  return name.endsWith('.log') ? name : `${name}.log`;
}

function printFetchFailed(faultlogName: string, remotePath: string, nextAction: string) {
  printKv({
    status: 'fetch_failed',
    faultlog_name: faultlogName,
    remote_path: remotePath,
    local_path: '',
    next_action: nextAction,
  });
}

async function recvFaultlog(hdc: string, deviceId: string, remotePath: string, localPath: string) {
  const args = [hdc, ...targetArgs(deviceId), 'file', 'recv', remotePath, localPath];
  const out = await runHdc(args);
  if (out.exitCode !== 0) {
    throw new Error(out.stderr || out.stdout || `hdc file recv failed (code=${out.exitCode})`);
  }

  if (!(await Bun.file(localPath).exists())) {
    throw new Error('Local file missing after recv.');
  }
}

function printFetched(faultlogName: string, remotePath: string, localPath: string) {
  printKv({
    status: 'fetched',
    faultlog_name: faultlogName,
    remote_path: remotePath,
    local_path: localPath,
    next_action: `parse-jscrash-log.ts --log-file "${localPath}"`,
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
