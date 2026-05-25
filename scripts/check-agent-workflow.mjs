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
  'Accepted Checks',
  'Risks',
  'Human Review Focus',
];

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
    reportError('.agent/evidence has no Evidence Packages but .agent/intents has Intent Records');
  }

  const intentSlugs = new Set();
  for (const file of intentFiles) {
    const slug = taskSlug(file, 'intent-');
    if (!slug) {
      reportError(`${rel(file)} must be named intent-*.md`);
      continue;
    }
    intentSlugs.add(slug);
    validateSections(file, requiredIntentSections);
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
  }

  for (const slug of intentSlugs) {
    if (!evidenceSlugs.has(slug)) {
      reportError(`Missing Evidence Package for intent-${slug}.md`);
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
