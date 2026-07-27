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
  buildCrashReport,
  buildNextActionText,
  formatCrashReportText,
} from './shared/jscrash-parse';
import { printKv, toErrorMessage } from './shared/utils';

function parseArgs(argv: string[]) {
  const map = new Map<string, string>();
  const flags = new Set<string>();
  const optionalKeys = new Set(['bundle-name', 'process-hint', 'device', 'source']);

  for (let i = 0; i < argv.length; i += 1) {
    const t = argv[i];
    if (t === '--include-text') {
      flags.add('include-text');
      continue;
    }
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

  const logFile = map.get('log-file');
  const logText = map.get('log-text');
  if ((logFile && logText) || (!logFile && !logText)) {
    throw new Error('Provide exactly one of --log-file or --log-text');
  }

  return {
    logFile,
    logText,
    bundleName: map.get('bundle-name') ?? '',
    processHint: map.get('process-hint') ?? '',
    source: map.get('source') ?? (logFile ? 'file' : 'text'),
    device: map.get('device') ?? 'default',
    includeText: flags.has('include-text'),
  };
}

async function readLogText(logFile: string) {
  const file = Bun.file(logFile);
  if (!(await file.exists())) {
    throw new Error(`log file not found: ${logFile}`);
  }

  return file.text();
}

async function main() {
  try {
    const args = parseArgs(process.argv.slice(2));
    let raw = '';
    if (args.logFile) {
      raw = await readLogText(args.logFile);
    } else if (args.logText) {
      raw = args.logText;
    }

    const report = buildCrashReport(raw, args.source, args.device, args.bundleName, args.processHint);
    const nextAction = buildNextActionText(report);

    const topStackJoined = report.topStack.join('|');
    const keywordsJoined = report.keywords.join(',');

    printKv({
      status: report.status === 'detected' ? 'detected' : 'no_crash_signature',
      source: args.source,
      error_type: report.errorType,
      error_message: report.errorMessage,
      suspected_file: report.suspectedFile,
      top_stack: topStackJoined,
      keywords: keywordsJoined,
      next_action: nextAction,
    });

    if (args.includeText) {
      console.log('');
      console.log(formatCrashReportText(report));
    }

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
