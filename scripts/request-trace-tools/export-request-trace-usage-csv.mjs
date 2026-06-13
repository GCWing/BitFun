#!/usr/bin/env node

import fs from 'node:fs/promises';
import path from 'node:path';

const USAGE_OUTPUT_FILE = 'usage-summary.csv';

function printUsage() {
  console.log(
    [
      'Usage:',
      '  node scripts/request-trace-tools/export-request-trace-usage-csv.mjs <request-trace-dir> [output.csv]',
      '',
      'Examples:',
      '  node scripts/request-trace-tools/export-request-trace-usage-csv.mjs ~/.bitfun/projects/demo/request-traces/abc123',
      '  node scripts/request-trace-tools/export-request-trace-usage-csv.mjs ./request-traces ./trace-usage.csv',
    ].join('\n'),
  );
}

function isHelpFlag(value) {
  return value === '-h' || value === '--help';
}

function csvEscape(value) {
  if (value === null || value === undefined) {
    return '';
  }

  const text = String(value);
  if (!/[",\n\r]/.test(text)) {
    return text;
  }

  return `"${text.replaceAll('"', '""')}"`;
}

function normalizeInteger(value) {
  return Number.isInteger(value) ? value : null;
}

function deriveCacheMissTokens(promptTokens, cacheHitTokens) {
  if (!Number.isInteger(promptTokens) || !Number.isInteger(cacheHitTokens)) {
    return null;
  }

  return Math.max(0, promptTokens - cacheHitTokens);
}

function buildRow(trace, fileName) {
  const usage = trace?.response?.usage ?? null;
  const promptTokens = normalizeInteger(usage?.promptTokenCount);
  const completionTokens = normalizeInteger(usage?.candidatesTokenCount);
  const reasoningTokens = normalizeInteger(usage?.reasoningTokenCount);
  const totalTokens = normalizeInteger(usage?.totalTokenCount);
  const cacheHitTokens = normalizeInteger(usage?.cachedContentTokenCount);
  const cacheMissTokens = deriveCacheMissTokens(promptTokens, cacheHitTokens);
  const cacheWriteTokens = normalizeInteger(usage?.cacheCreationTokenCount);

  return {
    file_name: fileName,
    sequence: trace?.sequence ?? null,
    recorded_at: trace?.recorded_at ?? '',
    trace_id: trace?.trace_id ?? '',
    session_id: trace?.session_id ?? '',
    turn_id: trace?.turn_id ?? '',
    round_id: trace?.round_id ?? '',
    capture_mode: trace?.capture_mode ?? '',
    provider: trace?.request?.provider ?? '',
    api_format: trace?.request?.api_format ?? '',
    model_id: trace?.request?.model_id ?? '',
    request_url: trace?.request?.request_url ?? '',
    attempt_number: trace?.request?.attempt_number ?? null,
    response_kind: trace?.response?.kind ?? '',
    prompt_tokens: promptTokens,
    completion_tokens: completionTokens,
    reasoning_tokens: reasoningTokens,
    total_tokens: totalTokens,
    cache_hit_tokens: cacheHitTokens,
    cache_miss_tokens: cacheMissTokens,
    cache_write_tokens: cacheWriteTokens,
    error: trace?.response?.error ?? '',
    partial_recovery_reason: trace?.response?.partial_recovery_reason ?? '',
  };
}

async function readTraceRows(traceDir) {
  const entries = await fs.readdir(traceDir, { withFileTypes: true });
  const traceFiles = entries
    .filter((entry) => entry.isFile() && entry.name.endsWith('.json'))
    .map((entry) => entry.name)
    .sort((left, right) => left.localeCompare(right, undefined, { numeric: true }));

  const rows = [];
  for (const fileName of traceFiles) {
    const filePath = path.join(traceDir, fileName);
    const contents = await fs.readFile(filePath, 'utf8');
    const trace = JSON.parse(contents);
    rows.push(buildRow(trace, fileName));
  }

  rows.sort((left, right) => {
    const leftSequence = Number.isInteger(left.sequence) ? left.sequence : Number.MAX_SAFE_INTEGER;
    const rightSequence = Number.isInteger(right.sequence) ? right.sequence : Number.MAX_SAFE_INTEGER;
    if (leftSequence !== rightSequence) {
      return leftSequence - rightSequence;
    }

    return left.file_name.localeCompare(right.file_name, undefined, { numeric: true });
  });

  return rows;
}

function buildCsv(rows) {
  const headers = [
    'file_name',
    'sequence',
    'prompt_tokens',
    'completion_tokens',
    'reasoning_tokens',
    'total_tokens',
    'cache_hit_tokens',
    'cache_miss_tokens',
    'cache_write_tokens',
    'recorded_at',
    'trace_id',
    'session_id',
    'turn_id',
    'round_id',
    'capture_mode',
    'provider',
    'api_format',
    'model_id',
    'request_url',
    'attempt_number',
    'response_kind',
    'error',
    'partial_recovery_reason',
  ];

  const lines = [headers.join(',')];
  for (const row of rows) {
    lines.push(headers.map((header) => csvEscape(row[header])).join(','));
  }

  return `${lines.join('\n')}\n`;
}

async function main() {
  const args = process.argv.slice(2);

  if (args.length === 0 || args.some(isHelpFlag)) {
    printUsage();
    process.exit(args.length === 0 ? 1 : 0);
  }

  const [traceDirArg, outputPathArg] = args;
  const traceDir = path.resolve(traceDirArg);
  const outputPath = outputPathArg
    ? path.resolve(outputPathArg)
    : path.join(traceDir, USAGE_OUTPUT_FILE);

  const traceDirStats = await fs.stat(traceDir).catch(() => null);
  if (!traceDirStats?.isDirectory()) {
    throw new Error(`Request trace directory not found: ${traceDir}`);
  }

  const rows = await readTraceRows(traceDir);
  const csv = buildCsv(rows);

  await fs.writeFile(outputPath, csv, 'utf8');

  console.log(
    `Wrote ${rows.length} trace row(s) to ${outputPath}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
