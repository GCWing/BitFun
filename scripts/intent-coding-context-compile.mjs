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
    if (/^##\s+/.test(lines[index])) {
      break;
    }
    contentLines.push(lines[index]);
  }

  return contentLines.join('\n').trim();
}

function argValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : null;
}

function changedFiles(markdown) {
  return sectionContent(markdown, 'Files Changed')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^[-*]\s+\S/.test(line))
    .map((line) => line.replace(/^[-*]\s+/, '').replace(/^`([^`]+)`.*$/, '$1').trim())
    .filter(Boolean);
}

function nearestAgentDocs(filePath) {
  const docs = [];
  let currentDir = path.dirname(path.resolve(root, filePath));
  while (currentDir.startsWith(root)) {
    for (const name of ['AGENTS.md', 'AGENTS-CN.md']) {
      const candidate = path.join(currentDir, name);
      if (fs.existsSync(candidate)) {
        docs.push(path.relative(root, candidate).split(path.sep).join('/'));
      }
    }
    const nextDir = path.dirname(currentDir);
    if (nextDir === currentDir) {
      break;
    }
    currentDir = nextDir;
  }
  return docs;
}

function addLine(lines, type, reference, reason) {
  lines.add(`- [${type}] ${reference}: ${reason}`);
}

function main() {
  const evidenceArg = argValue('--evidence');
  if (!evidenceArg) {
    throw new Error('Pass --evidence <path>');
  }

  const evidencePath = path.resolve(root, evidenceArg);
  const markdown = fs.readFileSync(evidencePath, 'utf8');
  const files = changedFiles(markdown);
  const lines = new Set();

  addLine(lines, 'builtin_rule', 'intent_coding_rules/context-compiler.md', 'context input generation');
  addLine(lines, 'builtin_rule', 'intent_coding_rules/risk-classification.md', 'risk-sensitive context selection');

  if (fs.existsSync(path.join(root, 'AGENTS.md'))) {
    addLine(lines, 'workspace_instruction', 'AGENTS.md', 'repository workflow guidance');
  }

  for (const file of files) {
    addLine(lines, 'source_file', file, 'changed file');
    for (const doc of nearestAgentDocs(file)) {
      addLine(lines, doc.endsWith('/AGENTS.md') || doc.endsWith('/AGENTS-CN.md') ? 'module_doc' : 'workspace_instruction', doc, 'nearest instruction for changed file');
    }
  }

  if (lines.size === 0) {
    addLine(lines, 'not_available', 'context_inputs', 'reason: no changed files or workspace instructions found');
  }

  console.log(Array.from(lines).join('\n'));
}

try {
  main();
} catch (error) {
  console.error(`[agent:context-compile] ERROR ${error.message}`);
  process.exit(1);
}
