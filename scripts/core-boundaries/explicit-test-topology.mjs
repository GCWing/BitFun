import { readdirSync, readFileSync } from 'node:fs';
import { join, posix, relative } from 'node:path';

export const agentRuntimeIntegrationTestTargets = [
  { name: 'agent_definition_contracts', path: 'tests/agent_definition_contracts.rs' },
  { name: 'agent_interaction_contracts', path: 'tests/agent_interaction_contracts.rs' },
  { name: 'agent_long_horizon_contracts', path: 'tests/agent_long_horizon_contracts.rs' },
  { name: 'agent_session_contracts', path: 'tests/agent_session_contracts.rs' },
  { name: 'native_hook_execution_contracts', path: 'tests/native_hook_execution_contracts.rs' },
];

export const cliIntegrationTestTargets = [
  { name: 'acp_stdio_cli', path: 'tests/acp_stdio_cli.rs' },
  { name: 'cli_command_contracts', path: 'tests/cli_command_contracts.rs' },
  { name: 'terminal_process_contracts', path: 'tests/terminal_process_contracts.rs' },
];

function parseExplicitTestTargets(manifestText) {
  const targets = [];
  let current = null;
  const finishCurrent = () => {
    if (current) {
      targets.push(current);
      current = null;
    }
  };

  for (const line of manifestText.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === '[[test]]') {
      finishCurrent();
      current = {};
      continue;
    }
    if (trimmed.startsWith('[')) {
      finishCurrent();
      continue;
    }
    const field = current && trimmed.match(/^(name|path)\s*=\s*"([^"]+)"\s*$/);
    if (field) {
      current[field[1]] = field[2];
    }
  }
  finishCurrent();
  return targets;
}

function packageDisablesAutotests(manifestText) {
  let inPackage = false;
  for (const line of manifestText.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed.startsWith('[')) {
      inPackage = trimmed === '[package]';
      continue;
    }
    if (inPackage && /^autotests\s*=\s*false\s*$/.test(trimmed)) {
      return true;
    }
  }
  return false;
}

function parseFlatRootModules(root, source, errors) {
  const references = [];
  const lines = source.split(/\r?\n/);
  let valid = true;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index].trim();
    if (line === '' || line.startsWith('//!')) {
      continue;
    }
    const pathAttribute = line.match(/^#\[path\s*=\s*"([^"]+)"\]$/);
    const moduleDeclaration = lines[index + 1]?.trim().match(/^mod\s+([A-Za-z0-9_]+)\s*;$/);
    if (!pathAttribute || !moduleDeclaration) {
      errors.push(`grouped test root ${root} contains unsupported line ${index + 1}`);
      valid = false;
      continue;
    }
    references.push({ path: pathAttribute[1], moduleName: moduleDeclaration[1] });
    index += 1;
  }
  return valid ? references : [];
}

export function validateExplicitIntegrationTestTopology({
  manifestText,
  expectedTargets,
  topLevelRustFiles,
  rootSources,
  leafRustFiles,
}) {
  const errors = [];
  if (!packageDisablesAutotests(manifestText)) {
    errors.push('[package] must keep autotests = false');
  }

  const expectedTargetEntries = expectedTargets.map(({ name, path }) => `${name}=${path}`).sort();
  const actualTargetEntries = parseExplicitTestTargets(manifestText)
    .map(({ name, path }) => `${name ?? '<missing-name>'}=${path ?? '<missing-path>'}`)
    .sort();
  if (actualTargetEntries.join('\n') !== expectedTargetEntries.join('\n')) {
    errors.push(`explicit test targets must be exactly: ${expectedTargetEntries.join(', ')}`);
  }

  const expectedRoots = expectedTargets.map(({ path }) => path).sort();
  if ([...topLevelRustFiles].sort().join('\n') !== expectedRoots.join('\n')) {
    errors.push(`top-level test roots must be exactly: ${expectedRoots.join(', ')}`);
  }

  const leaves = new Set(leafRustFiles);
  const referenceCounts = new Map();
  for (const root of expectedRoots) {
    const source = rootSources.get(root);
    if (source === undefined) {
      errors.push(`missing explicit test root: ${root}`);
      continue;
    }
    const wrapperDir = `${root.slice(0, -'.rs'.length)}/`;
    const ownsLeaves = [...leaves].some((leaf) => leaf.startsWith(wrapperDir));
    if (!ownsLeaves) {
      continue;
    }
    for (const reference of parseFlatRootModules(root, source, errors)) {
      const leaf = posix.normalize(posix.join(posix.dirname(root), reference.path));
      if (!leaf.startsWith(wrapperDir)) {
        errors.push(`grouped test root ${root} may only reference leaves under ${wrapperDir}`);
        continue;
      }
      if (!leaves.has(leaf)) {
        errors.push(`test root ${root} references missing leaf: ${leaf}`);
        continue;
      }
      const expectedModuleName = posix.basename(leaf, '.rs');
      if (reference.moduleName !== expectedModuleName) {
        errors.push(`test leaf ${leaf} must use module name ${expectedModuleName}`);
      }
      referenceCounts.set(leaf, (referenceCounts.get(leaf) ?? 0) + 1);
    }
  }

  for (const leaf of [...leaves].sort()) {
    const count = referenceCounts.get(leaf) ?? 0;
    if (count !== 1) {
      errors.push(`test leaf ${leaf} must be referenced exactly once; found ${count}`);
    }
  }
  return errors;
}

function collectRustFiles(dir, testsDir, files, ignoredDirectories) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      const repoPath = `tests/${relative(testsDir, path).replaceAll('\\', '/')}`;
      if (!ignoredDirectories.has(repoPath)) {
        collectRustFiles(path, testsDir, files, ignoredDirectories);
      }
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      const repoPath = `tests/${relative(testsDir, path).replaceAll('\\', '/')}`;
      files.push(repoPath);
    }
  }
}

function checkExplicitIntegrationTestTopology(root, {
  cratePath,
  expectedTargets,
  ignoredDirectories = [],
}) {
  const crateDir = join(root, ...cratePath.split('/'));
  const testsDir = join(crateDir, 'tests');
  const manifestPath = join(crateDir, 'Cargo.toml');
  const topLevelRustFiles = [];
  const leafRustFiles = [];
  const rootSources = new Map();
  const ignoredDirectorySet = new Set(ignoredDirectories);

  for (const entry of readdirSync(testsDir, { withFileTypes: true })) {
    if (entry.isFile() && entry.name.endsWith('.rs')) {
      const repoPath = `tests/${entry.name}`;
      topLevelRustFiles.push(repoPath);
      rootSources.set(repoPath, readFileSync(join(testsDir, entry.name), 'utf8'));
    } else if (entry.isDirectory()) {
      const repoPath = `tests/${entry.name}`;
      if (!ignoredDirectorySet.has(repoPath)) {
        collectRustFiles(
          join(testsDir, entry.name),
          testsDir,
          leafRustFiles,
          ignoredDirectorySet,
        );
      }
    }
  }

  return validateExplicitIntegrationTestTopology({
    manifestText: readFileSync(manifestPath, 'utf8'),
    expectedTargets,
    topLevelRustFiles,
    rootSources,
    leafRustFiles,
  }).map((message) => ({ path: manifestPath, line: 1, message }));
}

export function checkAgentRuntimeIntegrationTestTopology(root) {
  return checkExplicitIntegrationTestTopology(root, {
    cratePath: 'src/crates/execution/agent-runtime',
    expectedTargets: agentRuntimeIntegrationTestTargets,
  });
}

export function checkCliIntegrationTestTopology(root) {
  return checkExplicitIntegrationTestTopology(root, {
    cratePath: 'src/apps/cli',
    expectedTargets: cliIntegrationTestTargets,
    ignoredDirectories: ['tests/support'],
  });
}
