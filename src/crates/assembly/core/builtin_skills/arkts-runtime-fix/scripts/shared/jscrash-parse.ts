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

export type CrashReport = {
  status: 'detected' | 'no_crash_signature';
  source: string;
  device: string;
  bundle: string;
  process: string;
  errorType: string;
  errorMessage: string;
  suspectedFile: string;
  keywords: string[];
  topStack: string[];
  excerpt: string[];
};

type ScoredLine = {
  line: string;
  index: number;
  score: number;
};

const ERROR_TYPE_RE =
  /(TypeError|ReferenceError|RangeError|SyntaxError|BusinessError|ParameterError|ResourceError|SystemError|EvalError|URIError)/i;
const CRASH_SIGNAL_RE = /(jscrash|uncaught|exception|fatal|abort|crash|error)/i;
const STRONG_CRASH_SIGNAL_RE =
  /(jscrash|uncaught|fatal|abort|crash|TypeError|ReferenceError|RangeError|SyntaxError|BusinessError|ParameterError|ResourceError|SystemError|EvalError|URIError)/i;
const FILE_RE = /([A-Za-z0-9_./\\-]+\.(ets|ts|js)(?::\d+:\d+)?)/i;

function trim(input: string) {
  return input.trim();
}

function cleanLines(input: string) {
  return input
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0);
}

function unique(items: string[]) {
  return [...new Set(items)];
}

function containsIgnoreCase(input: string, token: string) {
  return input.toLowerCase().includes(token.toLowerCase());
}

function firstMatch(input: string, patterns: RegExp[]) {
  for (const pattern of patterns) {
    const match = pattern.exec(input);
    if (match?.[1]) {
      return match[1].trim();
    }
  }

  return '';
}

function errorTypeFromText(input: string) {
  const matches = input.match(new RegExp(ERROR_TYPE_RE.source, 'ig'));
  if (!matches?.length) {
    return '';
  }

  return matches[matches.length - 1];
}

function detectErrorType(lines: string[], anchor: number, focus: string) {
  const headerText = lines.slice(0, 48).join('\n');
  const fromHeader = errorTypeFromText(headerText);
  if (fromHeader) {
    return fromHeader;
  }

  for (const line of lines.slice(0, 48)) {
    const match = /^(?:Reason|Error\s+name|Error\s+type)\s*[:：]\s*([A-Za-z]+Error)\s*$/i.exec(line.trim());
    if (match?.[1] && ERROR_TYPE_RE.test(match[1])) {
      return match[1];
    }
  }

  if (anchor >= 0) {
    const aroundAnchor = errorTypeFromText(sliceWindow(lines, anchor, 12, 8).join('\n'));
    if (aroundAnchor) {
      return aroundAnchor;
    }
  }

  const fromFocus = errorTypeFromText(focus);
  if (fromFocus) {
    return fromFocus;
  }

  return 'UnknownError';
}

function detectBundle(input: string, bundleName: string) {
  if (bundleName) {
    return bundleName;
  }

  return (
    firstMatch(input, [
      /bundleName\s*[:=]\s*([A-Za-z0-9._-]+)/i,
      /bundle\s*[:=]\s*([A-Za-z0-9._-]+)/i,
      /app\s*[:=]\s*([A-Za-z0-9._-]+)/i,
    ]) || '(unknown)'
  );
}

function detectProcess(input: string, processHint: string) {
  if (processHint) {
    return processHint;
  }

  return (
    firstMatch(input, [
      /(?:process(?:Name)?)\s*[:=]\s*([A-Za-z0-9._-]+)/i,
      /pid\s*[:=]\s*([0-9]+)/i,
    ]) || '(unknown)'
  );
}

function stackLike(line: string) {
  return (
    /at\s+.+:\d+:\d+/i.test(line) ||
    /at\s+.+\(.+:\d+:\d+\)/i.test(line) ||
    /([A-Za-z0-9_./\\-]+\.(ets|ts|js)):\d+:\d+/.test(line)
  );
}

function applicationFrameScore(line: string, bundle: string) {
  let score = 0;
  if (/entry[\\/].*\.ets/i.test(line)) {
    score += 8;
  }
  if (/src[\\/].*\.(ets|ts|js)/i.test(line)) {
    score += 6;
  }
  if (bundle !== '(unknown)' && containsIgnoreCase(line, bundle)) {
    score += 4;
  }
  if (/(pages|page|feature|component|viewmodel|store|model)[\\/]/i.test(line)) {
    score += 3;
  }
  if (/(framework|runtime|node_modules|oh_modules|libarkui|ets_runtime|foundation|system)[\\/]/i.test(line)) {
    score -= 6;
  }

  return score;
}

function scoreCrashLine(line: string, bundle: string, processHint: string) {
  let score = 0;
  if (CRASH_SIGNAL_RE.test(line)) {
    score += 5;
  }
  if (STRONG_CRASH_SIGNAL_RE.test(line)) {
    score += 4;
  }
  if (ERROR_TYPE_RE.test(line)) {
    score += 4;
  }
  if (stackLike(line)) {
    score += 2;
  }
  if (bundle && bundle !== '(unknown)' && containsIgnoreCase(line, bundle)) {
    score += 2;
  }
  if (processHint && processHint !== '(unknown)' && containsIgnoreCase(line, processHint)) {
    score += 2;
  }

  return score;
}

function findCrashAnchor(lines: string[], bundle: string, processHint: string) {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const score = scoreCrashLine(lines[index], bundle, processHint);
    if (score >= 7) {
      return index;
    }
  }

  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const score = scoreCrashLine(lines[index], bundle, processHint);
    if (score >= 5) {
      return index;
    }
  }

  return -1;
}

function sliceWindow(lines: string[], anchor: number, before: number, after: number) {
  if (anchor < 0) {
    return [];
  }

  const start = Math.max(0, anchor - before);
  const end = Math.min(lines.length, anchor + after + 1);

  return lines.slice(start, end);
}

function findCrashSignal(lines: string[], start: number, end: number, step: 1 | -1) {
  for (let index = start; step > 0 ? index < end : index >= end; index += step) {
    if (CRASH_SIGNAL_RE.test(lines[index])) {
      return lines[index].trim();
    }
  }

  return '';
}

function detectErrorMessage(lines: string[], anchor: number) {
  if (anchor >= 0) {
    const forwardMatch = findCrashSignal(lines, anchor, Math.min(lines.length, anchor + 6), 1);
    if (forwardMatch) {
      return forwardMatch;
    }

    const backwardMatch = findCrashSignal(lines, anchor, Math.max(0, anchor - 3), -1);
    if (backwardMatch) {
      return backwardMatch;
    }

    return lines[anchor].trim();
  }

  for (let index = lines.length - 1; index >= 0; index -= 1) {
    if (CRASH_SIGNAL_RE.test(lines[index])) {
      return lines[index].trim();
    }
  }

  return lines[lines.length - 1] || '(not found)';
}

function detectTopStack(lines: string[], anchor: number) {
  const around = anchor >= 0 ? sliceWindow(lines, anchor, 4, 18) : lines.slice(Math.max(0, lines.length - 24));
  const frames = around.filter((line) => stackLike(line));

  return unique(frames).slice(0, 8);
}

function detectSuspectedFile(topStack: string[], bundle: string) {
  const candidates: ScoredLine[] = [];
  for (let index = 0; index < topStack.length; index += 1) {
    const line = topStack[index];
    const match = FILE_RE.exec(line);
    if (!match?.[1]) {
      continue;
    }

    candidates.push({
      line: match[1],
      index,
      score: applicationFrameScore(match[1], bundle) - index,
    });
  }

  if (!candidates.length) {
    return '(not found)';
  }

  candidates.sort((a, b) => b.score - a.score || a.index - b.index);

  return candidates[0].line;
}

function detectKeywords(input: string) {
  const candidates = [
    'jscrash',
    'uncaught',
    'exception',
    'fatal',
    'typeerror',
    'referenceerror',
    'rangeerror',
    'syntaxerror',
    'businesserror',
    'parametererror',
    'resourceerror',
    'systemerror',
    'abort',
    'crash',
  ];

  return candidates.filter((item) => containsIgnoreCase(input, item));
}

function looksLikeCrash(input: string, anchor: number, errorType: string) {
  if (errorType !== 'UnknownError') {
    return true;
  }
  if (anchor >= 0) {
    return true;
  }

  return /(jscrash|uncaught|fatal|abort|crash)/i.test(input);
}

function pickExcerpt(lines: string[], anchor: number, bundle: string, processHint: string, limit: number) {
  if (anchor >= 0) {
    const around = sliceWindow(lines, anchor, 5, 18);
    if (around.length) {
      return around.slice(0, limit);
    }
  }

  const scored = lines
    .map((line, index) => ({
      line,
      index,
      score: scoreCrashLine(line, bundle, processHint),
    }))
    .filter((item) => item.score > 0);

  if (!scored.length) {
    return lines.slice(Math.max(0, lines.length - limit));
  }

  const out: string[] = [];
  for (const item of scored.slice(Math.max(0, scored.length - limit))) {
    out.push(item.line);
  }

  return unique(out).slice(0, limit);
}

export function buildNextActionText(report: CrashReport) {
  if (report.status === 'no_crash_signature') {
    return 'Ask for a fuller crash log, a clearer repro, or optionally collect recent faultlogger or hilog evidence if needed.';
  }
  if (report.suspectedFile !== '(not found)') {
    return `Inspect ${report.suspectedFile} first and make a minimal fix. Collect additional runtime evidence only if the current anchor is still too weak.`;
  }

  return 'Inspect the top stack frames first and make a minimal fix. If the stack is too weak, optionally collect more runtime evidence before broader code reading.';
}

export function formatCrashReportText(report: CrashReport) {
  return [
    report.status === 'detected' ? 'Crash signature detected.' : 'No clear crash signature detected.',
    `source: ${report.source}`,
    `device: ${report.device}`,
    `bundle: ${report.bundle}`,
    `process: ${report.process}`,
    `error_type: ${report.errorType}`,
    `error_message: ${report.errorMessage}`,
    `suspected_file: ${report.suspectedFile}`,
    `keywords: ${report.keywords.length ? report.keywords.join(', ') : '(none)'}`,
    '',
    'Top stack:',
    ...(report.topStack.length ? report.topStack : ['(empty)']),
    '',
    'Evidence excerpt:',
    ...(report.excerpt.length ? report.excerpt : ['(empty)']),
    '',
    `next_action: ${buildNextActionText(report)}`,
  ].join('\n');
}

export function buildCrashReport(
  input: string,
  source: string,
  device: string,
  bundleName: string,
  processHint: string,
): CrashReport {
  const normalized = trim(input);
  const lines = cleanLines(normalized);
  const bundle = detectBundle(normalized, bundleName);
  const process = detectProcess(normalized, processHint);
  const anchor = findCrashAnchor(lines, bundle, process);
  const focus = anchor >= 0 ? sliceWindow(lines, anchor, 5, 18).join('\n') : normalized;
  const errorType = detectErrorType(lines, anchor, focus);
  const topStack = detectTopStack(lines, anchor);
  const excerpt = pickExcerpt(lines, anchor, bundle, process, 24);

  return {
    status: looksLikeCrash(normalized, anchor, errorType) ? 'detected' : 'no_crash_signature',
    source,
    device,
    bundle,
    process,
    errorType,
    errorMessage: detectErrorMessage(lines, anchor),
    suspectedFile: detectSuspectedFile(topStack, bundle),
    keywords: detectKeywords(focus || normalized),
    topStack,
    excerpt,
  };
}
