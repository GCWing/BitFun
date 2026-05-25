#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const agentDir = path.join(root, '.agent');

const requiredIntentSections = [
  'Metadata',
  'Original User Request',
  'Agent Understanding',
  'In Scope',
  'Out of Scope',
  'Acceptance Criteria',
  'Accepted Checks',
  'Execution Contract',
  'Metrics',
];

const requiredEvidenceSections = [
  'Metadata',
  'Intent Record',
  'Summary',
  'Files Changed',
  'Verification',
  'Repair Loop',
  'Accepted Checks',
  'Risks',
  'Human Review Focus',
];

const validRepairStatuses = new Set(['not_needed', 'repaired', 'blocked', 'deferred']);
const validRiskLevels = new Set(['L0', 'L1', 'L2', 'L3', 'L4']);

let errorCount = 0;

function toPosixPath(value) {
  return value.split(path.sep).join('/');
}

function rel(filePath) {
  return toPosixPath(path.relative(root, filePath));
}

function reportError(message) {
  errorCount += 1;
  console.error(`[agent:check] ERROR ${message}`);
}

function reportWarn(message) {
  console.warn(`[agent:check] WARN  ${message}`);
}

function reportInfo(message) {
  console.log(`[agent:check] ${message}`);
}

function readMarkdown(filePath) {
  try {
    return fs.readFileSync(filePath, 'utf8');
  } catch (error) {
    reportError(`Failed to read ${rel(filePath)}: ${error.message}`);
    return '';
  }
}

function listMarkdownFiles(dir) {
  if (!fs.existsSync(dir)) {
    return [];
  }

  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
    .map((entry) => path.join(dir, entry.name))
    .sort();
}

function hasSection(markdown, sectionName) {
  const escaped = sectionName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`^## ${escaped}\\s*$`, 'm').test(markdown);
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

function validateSections(filePath, requiredSections) {
  const markdown = readMarkdown(filePath);
  for (const section of requiredSections) {
    if (!hasSection(markdown, section)) {
      reportError(`${rel(filePath)} is missing "## ${section}"`);
    }
  }
  return markdown;
}

function taskSlug(filePath, prefix) {
  const basename = path.basename(filePath, '.md');
  return basename.startsWith(prefix) ? basename.slice(prefix.length) : null;
}

function validateEvidenceIntentReference(filePath, markdown) {
  const match = markdown.match(/\.agent\/intents\/intent-[^\s`)]+\.md/);
  if (!match) {
    reportError(`${rel(filePath)} does not reference an Intent Record path`);
    return;
  }

  const intentPath = path.join(root, match[0]);
  if (!fs.existsSync(intentPath)) {
    reportError(`${rel(filePath)} references missing Intent Record ${match[0]}`);
  }
}

function acceptedCheckLineHasStatus(line) {
  return /^\s*[-*]\s+(?:\[[ xX~-]\]|\[(?:passed|failed|skipped|blocked|not run|partial)\])\s+\S/i.test(
    line,
  );
}

function validateEvidenceAcceptedCheckStatuses(filePath, markdown) {
  const content = sectionContent(markdown, 'Accepted Checks');
  if (!content) {
    return;
  }

  const checkLines = content
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => /^\s*[-*]\s+/.test(line));

  if (checkLines.length === 0) {
    reportError(
      `${rel(filePath)} "## Accepted Checks" must list at least one check with an explicit status`,
    );
    return;
  }

  for (const line of checkLines) {
    if (!acceptedCheckLineHasStatus(line)) {
      reportError(
        `${rel(filePath)} Accepted Check must start with a status marker: ${line.trim()}`,
      );
    }
  }
}

function validateEvidenceRepairLoop(filePath, markdown) {
  const content = sectionContent(markdown, 'Repair Loop');
  if (!content) {
    return;
  }

  const attemptsMatch = content.match(/Repair attempts\s*:\s*(\d+)/i);
  if (!attemptsMatch) {
    reportError(`${rel(filePath)} "## Repair Loop" must include "Repair attempts: <number>"`);
  }

  const statusMatch = content.match(/Final repair status\s*:\s*([a-z_]+)/i);
  if (!statusMatch) {
    reportError(`${rel(filePath)} "## Repair Loop" must include "Final repair status: <status>"`);
    return;
  }

  const status = statusMatch[1].toLowerCase();
  if (!validRepairStatuses.has(status)) {
    reportError(
      `${rel(filePath)} has invalid Final repair status "${status}". Expected one of: ${Array.from(validRepairStatuses).join(', ')}`,
    );
  }
}

function validateRiskLevelLine(filePath, markdown, sectionName, label) {
  const content = sectionContent(markdown, sectionName);
  if (!content) {
    return null;
  }

  const escapedLabel = label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = content.match(new RegExp(`${escapedLabel}\\s*:\\s*(L[0-4])\\b`, 'i'));
  if (!match) {
    reportError(`${rel(filePath)} "## ${sectionName}" must include "${label}: L0|L1|L2|L3|L4"`);
    return null;
  }

  const riskLevel = match[1].toUpperCase();
  if (!validRiskLevels.has(riskLevel)) {
    reportError(`${rel(filePath)} has invalid ${label} "${riskLevel}"`);
    return null;
  }

  return riskLevel;
}

function isHighRiskLevel(riskLevel) {
  return riskLevel === 'L3' || riskLevel === 'L4';
}

function validateHighRiskIntentReviewEscalation(filePath, markdown, riskLevel) {
  if (!isHighRiskLevel(riskLevel)) {
    return;
  }

  const metadata = sectionContent(markdown, 'Metadata');
  if (!/Review escalation\s*:\s*\S/i.test(metadata)) {
    reportError(
      `${rel(filePath)} L3/L4 Intent Record must include "Review escalation: <plan>" in "## Metadata"`,
    );
  }
}

function validateHighRiskEvidenceReviewEscalation(filePath, markdown, riskLevel) {
  if (!isHighRiskLevel(riskLevel)) {
    return;
  }

  const risks = sectionContent(markdown, 'Risks');
  if (!/Review escalation status\s*:\s*\S/i.test(risks)) {
    reportError(
      `${rel(filePath)} L3/L4 Evidence Package must include "Review escalation status: <completed|skipped|blocked>" in "## Risks"`,
    );
  }
}

function main() {
  // .agent is a runtime artifact directory created by the IntentCoding agent.
  // Its absence is not an error — just means no active Intent Coding task.
  if (!fs.existsSync(agentDir)) {
    reportInfo('.agent directory not found — no active Intent Coding task.');
    process.exit(0);
  }

  const intentFiles = listMarkdownFiles(path.join(agentDir, 'intents'));
  const evidenceFiles = listMarkdownFiles(path.join(agentDir, 'evidence'));

  if (intentFiles.length === 0 && evidenceFiles.length === 0) {
    reportInfo('No active Intent Records or Evidence Packages.');
    process.exit(0);
  }

  if (intentFiles.length === 0) {
    reportError('.agent/intents has no Intent Records but .agent/evidence has Evidence Packages');
  }
  if (evidenceFiles.length === 0) {
    // Intent Record exists without Evidence Package — normal during active work.
    reportWarn('.agent/evidence has no Evidence Packages yet — task may still be in progress');
  }

  const intentSlugs = new Set();
  for (const file of intentFiles) {
    const slug = taskSlug(file, 'intent-');
    if (!slug) {
      reportError(`${rel(file)} must be named intent-*.md`);
      continue;
    }
    intentSlugs.add(slug);
    const markdown = validateSections(file, requiredIntentSections);
    const riskLevel = validateRiskLevelLine(file, markdown, 'Metadata', 'Risk level');
    validateHighRiskIntentReviewEscalation(file, markdown, riskLevel);
  }

  const evidenceSlugs = new Set();
  for (const file of evidenceFiles) {
    const slug = taskSlug(file, 'evidence-');
    if (!slug) {
      reportError(`${rel(file)} must be named evidence-*.md`);
      continue;
    }
    evidenceSlugs.add(slug);
    const markdown = validateSections(file, requiredEvidenceSections);
    validateEvidenceIntentReference(file, markdown);
    validateEvidenceAcceptedCheckStatuses(file, markdown);
    validateEvidenceRepairLoop(file, markdown);
    const riskLevel = validateRiskLevelLine(file, markdown, 'Risks', 'Final risk level');
    validateHighRiskEvidenceReviewEscalation(file, markdown, riskLevel);
  }

  for (const slug of intentSlugs) {
    if (!evidenceSlugs.has(slug)) {
      // Intent without matching Evidence is expected during active work.
      reportWarn(`Evidence Package not yet written for intent-${slug}.md`);
    }
  }

  for (const slug of evidenceSlugs) {
    if (!intentSlugs.has(slug)) {
      reportError(`Missing Intent Record for evidence-${slug}.md`);
    }
  }

  if (errorCount > 0) {
    console.error(`[agent:check] Failed with ${errorCount} error(s).`);
    process.exit(1);
  }

  reportInfo(
    `Passed: ${intentFiles.length} Intent Record(s), ${evidenceFiles.length} Evidence Package(s).`,
  );
}

main();
