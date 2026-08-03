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

import process from 'node:process';
import {
  extractJscrashFaultlogNames,
  filterFaultlogsByBundle,
  parseFaultlogEntry,
  selectWithinMaxAge,
  sortFaultlogsByRecency,
} from './shared/jscrash-faultlogger';
import { resolveHdcOrThrow, runHdc, targetArgs } from './shared/hdc';
import { printKv, toErrorMessage } from './shared/utils';

function parseArgs(argv: string[]) {
  const map = new Map<string, string>();
  const optionalKeys = new Set(['bundle-name', 'device-id']);

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

  return {
    bundleName: map.get('bundle-name') ?? '',
    deviceId: map.get('device-id') ?? '',
    maxAgeMinutes: Number(map.get('max-age-minutes') ?? '30'),
    limit: Number(map.get('limit') ?? '10'),
  };
}

async function readFaultloggerViaHidumper(hdc: string, deviceId: string) {
  const args = [
    hdc,
    ...targetArgs(deviceId),
    'shell',
    'hidumper',
    '-s',
    '1201',
    '-a',
    '-p Faultlogger LogSuffixWithMs',
  ];

  return runHdc(args);
}

async function readFaultloggerViaLs(hdc: string, deviceId: string) {
  const remote = '/data/log/faultlog/faultlogger';
  const args = [hdc, ...targetArgs(deviceId), 'shell', 'ls', '-1', remote];

  return runHdc(args);
}

function formatCandidates(names: string[], limit: number) {
  return names.slice(0, limit).join('|');
}

function printProbeFailed(bundleName: string, nextAction: string) {
  printKv({
    status: 'probe_failed',
    bundle_name: bundleName,
    latest_faultlog: '',
    latest_timestamp: '',
    matched_count: '0',
    candidates: '',
    next_action: nextAction,
  });
}

function printProbeNotFound(bundleName: string) {
  printKv({
    status: 'not_found',
    bundle_name: bundleName,
    latest_faultlog: '',
    latest_timestamp: '',
    matched_count: '0',
    candidates: '',
    next_action: 'No recent matching faultlog was found. You may collect hilog if you still need more runtime evidence, or reproduce and retry.',
  });
}

function printProbeFound(bundleName: string, names: string[], limit: number) {
  const latest = names[0];
  const ts = parseFaultlogEntry(latest).timestampMs;
  printKv({
    status: 'found',
    bundle_name: bundleName,
    latest_faultlog: latest,
    latest_timestamp: ts === null ? '' : String(ts),
    matched_count: String(names.length),
    candidates: formatCandidates(names, limit),
    next_action: `A recent faultlog is available. Fetch it with fetch-faultlog.ts --faultlog-name "${latest}" if you want a cleaner crash anchor.`,
  });
}

async function loadCandidateNames(hdc: string, deviceId: string, bundleName: string, maxAgeMinutes: number) {
  const hidumper = await readFaultloggerViaHidumper(hdc, deviceId);
  let combined = hidumper.exitCode === 0 ? hidumper.stdout : '';

  const ls = await readFaultloggerViaLs(hdc, deviceId);
  if (ls.exitCode === 0) {
    combined = `${combined}\n${ls.stdout}`;
  }

  let names = extractJscrashFaultlogNames(combined);
  names = filterFaultlogsByBundle(names, bundleName);
  names = selectWithinMaxAge(names, maxAgeMinutes, Date.now());
  return sortFaultlogsByRecency(names);
}

async function main() {
  let bundleName = '';

  try {
    const args = parseArgs(process.argv.slice(2));
    bundleName = args.bundleName;
    const hdc = await resolveHdcOrThrow();
    const names = await loadCandidateNames(hdc, args.deviceId, args.bundleName, args.maxAgeMinutes);

    if (!names.length) {
      printProbeNotFound(args.bundleName);
      process.exit(0);
      return;
    }

    printProbeFound(args.bundleName, names, args.limit);
    process.exit(0);
  } catch (err) {
    printProbeFailed(bundleName, toErrorMessage(err));
    process.exit(1);
  }
}

await main();
