#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();

function sectionContent(markdown, sectionName) {
  const sectionHeading = `## ${sectionName}`;
  const lines = markdown.split(/\r?\n/);
  const startIndex = lines.findIndex((line) => line.trim() === sectionHeading);
  if (startIndex < 0) {
    return '';
  }

  const contentLines = [];
  for (let index = startIndex + 1; index < lines.length; index += 1) {
    if (/^##(?!#)\s+/.test(lines[index])) {
      break;
    }
    contentLines.push(lines[index]);
  }

  return contentLines.join('\n').trim();
}

const SAFE_ID_PATTERN = /^[A-Za-z0-9_.-]+$/;

function assertSafeId(label, value) {
  if (!SAFE_ID_PATTERN.test(value)) {
    throw new Error(
      `${label} must match ${SAFE_ID_PATTERN}; got ${JSON.stringify(value)}`,
    );
  }
}

function assertInsideSessionStore(resolvedPath) {
  const base = path.resolve(root, '.bitfun', 'sessions');
  const baseWithSep = base.endsWith(path.sep) ? base : base + path.sep;
  if (resolvedPath !== base && !resolvedPath.startsWith(baseWithSep)) {
    throw new Error(`Resolved path ${resolvedPath} escapes ${base}`);
  }
}

function fieldValue(content, label) {
  const escapedLabel = label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = content.match(new RegExp(`${escapedLabel}\\s*:\\s*(\\S+)`, 'i'));
  return match ? match[1].trim() : null;
}

function argValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : null;
}

function listItems(content) {
  return content
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^[-*]\s+\S/.test(line))
    .map((line) => line.replace(/^[-*]\s+/, '').trim());
}

function main() {
  const evidenceArg = argValue('--evidence');
  if (!evidenceArg) {
    throw new Error('Pass --evidence <path>');
  }

  const evidencePath = path.resolve(root, evidenceArg);
  const markdown = fs.readFileSync(evidencePath, 'utf8');
  const provenance = sectionContent(markdown, 'Provenance Chain');
  const sessionId = argValue('--session-id') ?? fieldValue(provenance, 'Session id');
  const turnId = argValue('--turn-id') ?? fieldValue(provenance, 'Turn id');

  if (!sessionId || sessionId === 'not_available') {
    throw new Error('A concrete session id is required. Pass --session-id <id>.');
  }
  if (!turnId || turnId === 'not_available') {
    throw new Error('A concrete turn id is required. Pass --turn-id <id>.');
  }
  assertSafeId('Session id', sessionId);
  assertSafeId('Turn id', turnId);

  const recordPath = path.resolve(
    root,
    '.bitfun',
    'sessions',
    sessionId,
    'intent-coding',
    `provenance-${turnId}.json`,
  );
  assertInsideSessionStore(recordPath);

  const record = {
    schema_version: 1,
    session_id: sessionId,
    turn_id: turnId,
    evidence_package: path.relative(root, evidencePath).split(path.sep).join('/'),
    intent_record: fieldValue(provenance, 'Intent Record'),
    context_inputs: listItems(sectionContent(markdown, 'Context Inputs')),
    files_changed: listItems(sectionContent(markdown, 'Files Changed')),
    accepted_checks: listItems(sectionContent(markdown, 'Accepted Checks')),
    policy_gates: listItems(sectionContent(markdown, 'Policy Gates')),
    verification: listItems(sectionContent(markdown, 'Verification')),
    risks: sectionContent(markdown, 'Risks'),
    human_review_focus: listItems(sectionContent(markdown, 'Human Review Focus')),
  };

  fs.mkdirSync(path.dirname(recordPath), { recursive: true });
  fs.writeFileSync(recordPath, `${JSON.stringify(record, null, 2)}\n`);
  console.log(path.relative(root, recordPath).split(path.sep).join('/'));
}

try {
  main();
} catch (error) {
  console.error(`[agent:provenance-record] ERROR ${error.message}`);
  process.exit(1);
}
