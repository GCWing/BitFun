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

import fs from 'node:fs/promises';
import process from 'node:process';
import {
  buildCrashReport,
  buildNextActionText,
  formatCrashReportText,
} from './shared/jscrash-parse.mjs';
import { resolveHdcOrThrow, runHdc, targetArgs } from './shared/hdc.mjs';
import { printKv, toErrorMessage } from './shared/utils.mjs';

function parseArgs(argv) {
  const map = new Map();
  const flags = new Set();
  const optionalKeys = new Set(['log-text', 'log-file', 'bundle-name', 'process-hint', 'device-id']);

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--include-text') {
      flags.add('include-text');
      continue;
    }
    if (!token.startsWith('--')) {
      continue;
    }
    const key = token.slice(2);
    const value = argv[i + 1];
    if (!value || value.startsWith('--')) {
      if (optionalKeys.has(key)) {
        map.set(key, '');
        continue;
      }
      throw new Error(`Missing value for --${key}`);
    }
    map.set(key, value);
    i += 1;
  }

  const logText = map.get('log-text');
  const logFile = map.get('log-file');
  if (logText && logFile) {
    throw new Error('Provide at most one of --log-text or --log-file');
  }

  const lines = Number(map.get('lines') ?? '4000');
  if (!Number.isInteger(lines) || lines < 200 || lines > 10000) {
    throw new Error('--lines must be an integer between 200 and 10000');
  }

  return {
    logText,
    logFile,
    bundleName: map.get('bundle-name') ?? '',
    processHint: map.get('process-hint') ?? '',
    deviceId: map.get('device-id') ?? '',
    lines,
    includeText: flags.has('include-text'),
  };
}

function cleanLines(input) {
  return input
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0);
}

async function collectHilogText(deviceId, lines) {
  const hdc = await resolveHdcOrThrow();
  const out = await runHdc([hdc, ...targetArgs(deviceId), 'shell', 'hilog', '-x']);
  if (out.exitCode !== 0) {
    throw new Error(out.stderr || out.stdout || `hdc hilog -x failed (code=${out.exitCode})`);
  }

  const all = cleanLines(out.stdout);
  return all.slice(Math.max(0, all.length - lines)).join('\n');
}

async function loadInput(args) {
  if (args.logText) {
    return { text: args.logText, source: 'text' };
  }
  if (args.logFile) {
    return { text: await fs.readFile(args.logFile, 'utf8'), source: 'file' };
  }
  return { text: await collectHilogText(args.deviceId, args.lines), source: 'device_hilog' };
}

function printReport(report, source, includeText) {
  printKv({
    status: report.status === 'detected' ? 'detected' : 'no_crash_signature',
    source,
    error_type: report.errorType,
    error_message: report.errorMessage,
    suspected_file: report.suspectedFile,
    top_stack: report.topStack.join('|'),
    keywords: report.keywords.join(','),
    next_action: buildNextActionText(report),
  });

  if (includeText) {
    console.log('');
    console.log(formatCrashReportText(report));
  }
}

async function main() {
  try {
    const args = parseArgs(process.argv.slice(2));
    const input = await loadInput(args);
    const report = buildCrashReport(
      input.text,
      input.source,
      args.deviceId || 'default',
      args.bundleName,
      args.processHint,
    );
    printReport(report, input.source, args.includeText);
    process.exit(0);
  } catch (err) {
    printKv({
      status: 'parse_failed',
      source: 'text',
      error_type: '',
      error_message: '',
      suspected_file: '',
      top_stack: '',
      keywords: '',
      next_action: toErrorMessage(err),
    });
    process.exit(1);
  }
}

await main();
