#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();

function toPosixPath(value) {
  return value.split(path.sep).join('/');
}

function sectionContent(markdown, sectionName) {
  const sectionHeading = `## ${sectionName}`;
  const lines = markdown.split(/\r?\n/);
  const startIndex = lines.findIndex((line) => line.trim() === sectionHeading);
  if (startIndex < 0) {
    return '';
  }

  const contentLines = [];
  for (let index = startIndex + 1; index < lines.length; index += 1) {
    if (/^##\s+/.test(lines[index])) {
      break;
    }
    contentLines.push(lines[index]);
  }

  return contentLines.join('\n').trim();
}

function fieldValue(content, label) {
  const escapedLabel = label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = content.match(new RegExp(`${escapedLabel}\\s*:\\s*(\\S+)`, 'i'));
  return match ? match[1].trim() : null;
}

function listEvidenceFiles() {
  const evidenceDir = path.join(root, '.agent/evidence');
  if (!fs.existsSync(evidenceDir)) {
    return [];
  }

  return fs
    .readdirSync(evidenceDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
    .map((entry) => path.join(evidenceDir, entry.name))
    .sort();
}

function evidencePathFromArgs() {
  const evidenceIndex = process.argv.indexOf('--evidence');
  if (evidenceIndex >= 0 && process.argv[evidenceIndex + 1]) {
    return path.resolve(root, process.argv[evidenceIndex + 1]);
  }

  const evidenceFiles = listEvidenceFiles();
  if (evidenceFiles.length === 1) {
    return evidenceFiles[0];
  }

  if (evidenceFiles.length > 1) {
    throw new Error('Multiple Evidence Packages found. Pass --evidence <path>.');
  }

  throw new Error('No Evidence Package found. Pass --evidence <path>.');
}

function listChangedFiles(markdown) {
  return sectionContent(markdown, 'Files Changed')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^[-*]\s+\S/.test(line))
    .map((line) => line.replace(/^[-*]\s+/, '').replace(/^`([^`]+)`.*$/, '$1').trim())
    .filter(Boolean);
}

function main() {
  const evidencePath = evidencePathFromArgs();
  const markdown = fs.readFileSync(evidencePath, 'utf8');
  const risks = sectionContent(markdown, 'Risks');
  const route = fieldValue(risks, 'Review route') ?? 'not_available';
  const trigger = fieldValue(risks, 'Review trigger') ?? 'not_available';
  const status = fieldValue(risks, 'Review escalation status') ?? 'not_available';
  const changedFiles = listChangedFiles(markdown);

  const plan = {
    evidence_package: toPosixPath(path.relative(root, evidencePath)),
    review_route: route,
    review_trigger: trigger,
    review_status: status,
    changed_files: changedFiles,
    next_action: null,
  };

  if (route === 'deep_review') {
    plan.next_action =
      'Open BitFun Deep Review for the listed changed files and record the result in Review escalation status.';
  } else if (route === 'specialist_review') {
    plan.next_action =
      'Route the listed changed files to the named specialist review path and record the result in Review escalation status.';
  } else if (route === 'manual_review') {
    plan.next_action = 'Complete manual human review and record the result in Review escalation status.';
  } else if (route === 'skipped') {
    plan.next_action = 'No review trigger should run because the route is skipped.';
  } else {
    plan.next_action = 'No supported review route was found.';
  }

  console.log(JSON.stringify(plan, null, 2));
}

try {
  main();
} catch (error) {
  console.error(`[agent:review-route] ERROR ${error.message}`);
  process.exit(1);
}
