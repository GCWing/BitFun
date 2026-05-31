#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const requireFromWebUi = createRequire(path.join(rootDir, 'src/web-ui/package.json'));
const yaml = requireFromWebUi('yaml');

const yamlFiles = [];

function addYamlFiles(dir) {
  const absoluteDir = path.join(rootDir, dir);
  if (!existsSync(absoluteDir)) {
    return;
  }

  for (const entry of readdirSync(absoluteDir, { withFileTypes: true })) {
    const relativePath = path.posix.join(dir.replace(/\\/g, '/'), entry.name);
    const absolutePath = path.join(absoluteDir, entry.name);

    if (entry.isDirectory()) {
      addYamlFiles(relativePath);
    } else if (/\.(ya?ml)$/i.test(entry.name)) {
      yamlFiles.push({ relativePath, absolutePath });
    }
  }
}

addYamlFiles('.github/workflows');
addYamlFiles('.github/ISSUE_TEMPLATE');

const errors = [];

for (const { relativePath, absolutePath } of yamlFiles) {
  const document = yaml.parseDocument(readFileSync(absolutePath, 'utf8'), {
    prettyErrors: true,
  });

  if (document.errors.length > 0) {
    for (const error of document.errors) {
      errors.push(`${relativePath}: ${error.message}`);
    }
  }
}

if (errors.length > 0) {
  console.error('GitHub YAML config check failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(`GitHub YAML config check passed (${yamlFiles.length} files parsed).`);
