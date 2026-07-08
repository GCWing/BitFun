#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptRootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const rootDir = process.env.BITFUN_GITHUB_CONFIG_TEST_ROOT
  ? path.resolve(process.env.BITFUN_GITHUB_CONFIG_TEST_ROOT)
  : scriptRootDir;
const requireFromWebUi = createRequire(path.join(scriptRootDir, 'src/web-ui/package.json'));
const yaml = requireFromWebUi('yaml');

const minimumNode24ActionMajors = new Map([
  ['actions/checkout', 5],
  ['actions/setup-node', 5],
  ['actions/upload-artifact', 6],
  ['actions/download-artifact', 7],
  ['pnpm/action-setup', 5],
  ['softprops/action-gh-release', 3],
]);

const minimumProjectNodeVersion = { major: 22, minor: 12, patch: 0 };
const minimumProjectNodeVersionLabel = '22.12.0';
const requiredRootNodeEngine = '>=22.12.0';

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

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(rootDir, relativePath), 'utf8'));
}

function formatPath(segments) {
  return segments.length > 0 ? segments.join('.') : '<root>';
}

function parseMajorVersion(value) {
  const match = String(value).trim().match(/^v?(\d+)/);
  return match ? Number(match[1]) : null;
}

function parseNodeVersion(value) {
  const rawValue = String(value ?? '').trim();
  const rangeMatch = rawValue.match(/^>=\s*v?(\d+)\.(\d+)\.(\d+)$/i);
  if (rangeMatch) {
    const [, major, minor, patch] = rangeMatch;
    return {
      major: Number(major),
      minor: Number(minor),
      patch: Number(patch),
      floating: false,
    };
  }

  const match = rawValue.match(/^v?(\d+)(?:\.(\d+|x))?(?:\.(\d+|x))?$/i);
  if (!match) {
    return null;
  }

  const [, major, minor, patch] = match;
  const minorIsFloating = minor === undefined || minor.toLowerCase() === 'x';
  const patchIsFloating = patch === undefined || patch.toLowerCase() === 'x';
  return {
    major: Number(major),
    minor: minorIsFloating ? null : Number(minor),
    patch: patchIsFloating ? null : Number(patch),
    floating: minorIsFloating || patchIsFloating,
  };
}

function isSupportedProjectNodeVersion(version) {
  if (version.major !== minimumProjectNodeVersion.major) {
    return version.major > minimumProjectNodeVersion.major;
  }

  if (version.minor === null) {
    return true;
  }

  if (version.minor !== minimumProjectNodeVersion.minor) {
    return version.minor > minimumProjectNodeVersion.minor;
  }

  if (version.patch === null) {
    return true;
  }

  return version.patch >= minimumProjectNodeVersion.patch;
}

function collectWorkflowSteps(value, segments = [], steps = []) {
  if (Array.isArray(value)) {
    value.forEach((entry, index) => collectWorkflowSteps(entry, [...segments, String(index)], steps));
    return steps;
  }

  if (!value || typeof value !== 'object') {
    return steps;
  }

  if (typeof value.uses === 'string') {
    steps.push({ step: value, path: formatPath(segments) });
  }

  for (const [key, entry] of Object.entries(value)) {
    collectWorkflowSteps(entry, [...segments, key], steps);
  }

  return steps;
}

function validateActionRuntime(relativePath, step, stepPath) {
  const [actionName, versionSpec] = step.uses.split('@');
  const minimumMajor = minimumNode24ActionMajors.get(actionName);
  if (!minimumMajor || !versionSpec) {
    return;
  }

  const actualMajor = parseMajorVersion(versionSpec);
  if (actualMajor === null) {
    errors.push(`${relativePath}: ${stepPath}.uses uses ${step.uses}; pin ${actionName} to v${minimumMajor} or newer.`);
    return;
  }

  if (actualMajor < minimumMajor) {
    errors.push(`${relativePath}: ${stepPath}.uses uses ${step.uses}; use ${actionName}@v${minimumMajor} or newer so the GitHub Action runs on a Node.js 24-compatible runtime.`);
  }
}

function isPathInsideRoot(absolutePath) {
  const relativePath = path.relative(rootDir, absolutePath);
  return relativePath === '' || (!relativePath.startsWith('..') && !path.isAbsolute(relativePath));
}

function readNodeVersionFromFile(absolutePath, visited = new Set()) {
  if (visited.has(absolutePath)) {
    return null;
  }
  visited.add(absolutePath);

  const contents = readFileSync(absolutePath, 'utf8');
  try {
    const manifest = JSON.parse(contents);
    if (manifest && typeof manifest === 'object') {
      if (manifest.volta?.node) {
        return String(manifest.volta.node);
      }

      if (manifest.engines?.node) {
        return String(manifest.engines.node);
      }

      if (manifest.volta?.extends) {
        const extendedPath = path.resolve(path.dirname(absolutePath), manifest.volta.extends);
        if (!isPathInsideRoot(extendedPath) || !existsSync(extendedPath)) {
          return null;
        }
        return readNodeVersionFromFile(extendedPath, visited);
      }

      return null;
    }
  } catch {
    // Non-JSON version files are parsed below, matching setup-node behavior.
  }

  const found = contents.match(/^(?:node(js)?\s+)?v?(?<version>[^\s]+)$/m);
  return found?.groups?.version ?? contents.trim();
}

function readNodeVersionFile(relativePath, nodeVersionFile, stepPath) {
  const versionFile = String(nodeVersionFile ?? '').trim();
  if (!versionFile) {
    errors.push(`${relativePath}: ${stepPath}.with.node-version-file must point to a repository file containing Node.js ${minimumProjectNodeVersionLabel} or newer.`);
    return null;
  }

  const absolutePath = path.resolve(rootDir, versionFile);
  if (!isPathInsideRoot(absolutePath)) {
    errors.push(`${relativePath}: ${stepPath}.with.node-version-file points outside the repository: ${versionFile}`);
    return null;
  }

  if (!existsSync(absolutePath)) {
    errors.push(`${relativePath}: ${stepPath}.with.node-version-file was not found: ${versionFile}`);
    return null;
  }

  const version = readNodeVersionFromFile(absolutePath);

  if (!version) {
    errors.push(`${relativePath}: ${stepPath}.with.node-version-file ${versionFile} does not declare a Node.js version; use Node.js ${minimumProjectNodeVersionLabel} or newer.`);
    return null;
  }

  return { label: `node-version-file ${versionFile}`, version };
}

function validateProjectNodeVersion(relativePath, step, stepPath) {
  if (!step.uses.startsWith('actions/setup-node@')) {
    return;
  }

  const withConfig = step.with;
  const nodeVersion = withConfig?.['node-version'];
  const nodeVersionFile = withConfig?.['node-version-file'];
  let versionSource = { label: 'node-version', version: withConfig?.['node-version'] };

  if (nodeVersion !== undefined && String(nodeVersion).trim() !== '') {
    versionSource = { label: 'node-version', version: nodeVersion };
  } else if (nodeVersionFile) {
    versionSource = readNodeVersionFile(relativePath, nodeVersionFile, stepPath);
    if (!versionSource) {
      return;
    }
  }

  const actualVersion = parseNodeVersion(versionSource.version);
  if (actualVersion === null) {
    errors.push(`${relativePath}: ${stepPath}.with.${versionSource.label} must use Node.js ${minimumProjectNodeVersionLabel} or newer. Supported forms include ${minimumProjectNodeVersion.major}, ${minimumProjectNodeVersion.major}.x, ${minimumProjectNodeVersionLabel}, or >=${minimumProjectNodeVersionLabel}.`);
    return;
  }

  if (!isSupportedProjectNodeVersion(actualVersion)) {
    errors.push(`${relativePath}: ${stepPath}.with.${versionSource.label} resolves to ${versionSource.version}; use Node.js ${minimumProjectNodeVersionLabel} or newer for CI.`);
  }
}

function validateRootNodeEngine() {
  const packageJson = readJson('package.json');
  const actualEngine = packageJson.engines?.node;
  if (actualEngine !== requiredRootNodeEngine) {
    errors.push(`package.json: engines.node is ${JSON.stringify(actualEngine)}; expected ${JSON.stringify(requiredRootNodeEngine)} to match the project Node.js baseline used by local development and CI.`);
  }
}

for (const { relativePath, absolutePath } of yamlFiles) {
  const document = yaml.parseDocument(readFileSync(absolutePath, 'utf8'), {
    prettyErrors: true,
  });

  if (document.errors.length > 0) {
    for (const error of document.errors) {
      errors.push(`${relativePath}: ${error.message}`);
    }
  }

  if (relativePath.startsWith('.github/workflows/')) {
    const workflow = document.toJS();
    for (const { step, path: stepPath } of collectWorkflowSteps(workflow)) {
      validateActionRuntime(relativePath, step, stepPath);
      validateProjectNodeVersion(relativePath, step, stepPath);
    }
  }
}

validateRootNodeEngine();

if (errors.length > 0) {
  console.error('GitHub YAML config check failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(`GitHub YAML config check passed (${yamlFiles.length} files parsed).`);
