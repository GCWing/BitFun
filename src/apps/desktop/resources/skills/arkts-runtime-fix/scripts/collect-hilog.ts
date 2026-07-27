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

function cleanLines(input: string) {
  return input
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0);
}

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

  const outputDir = map.get('output-dir');
  if (!outputDir) {
    throw new Error('Required: --output-dir');
  }

  return {
    outputDir,
    deviceId: map.get('device-id') ?? '',
    lines: Number(map.get('lines') ?? '4000'),
  };
}

function printCollectFailed(nextAction: string) {
  printKv({
    status: 'collect_failed',
    source: 'device_hilog',
    log_file: '',
    log_excerpt: '',
    next_action: nextAction,
  });
}

async function collectHilogText(hdc: string, deviceId: string, lines: number) {
  const out = await runHdc([hdc, ...targetArgs(deviceId), 'shell', 'hilog', '-x']);
  if (out.exitCode !== 0) {
    throw new Error(out.stderr || out.stdout || `hdc hilog -x failed (code=${out.exitCode})`);
  }

  const all = cleanLines(out.stdout);
  return all.slice(Math.max(0, all.length - lines)).join('\n');
}

async function writeHilogSnapshot(outputDir: string, recent: string) {
  fs.mkdirSync(outputDir, { recursive: true });
  const logFile = path.join(outputDir, `hilog-${Date.now()}.txt`);
  await Bun.write(logFile, recent);
  return logFile;
}

function printCollected(logFile: string, recent: string) {
  printKv({
    status: 'collected',
    source: 'device_hilog',
    log_file: logFile,
    log_excerpt: recent.slice(0, 800),
    next_action: `A hilog snapshot is available. Parse it with parse-jscrash-log.ts --log-file "${logFile}" --source hilog if you want more crash detail.`,
  });
}

async function main() {
  try {
    const { outputDir, deviceId, lines } = parseArgs(process.argv.slice(2));
    const hdc = await resolveHdcOrThrow();
    const recent = await collectHilogText(hdc, deviceId, lines);
    const logFile = await writeHilogSnapshot(outputDir, recent);
    printCollected(logFile, recent);
    process.exit(0);
  } catch (err) {
    printCollectFailed(toErrorMessage(err));
    process.exit(1);
  }
}

await main();
