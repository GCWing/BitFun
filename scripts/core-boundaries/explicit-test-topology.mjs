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

export const servicesCoreIntegrationTestTargets = [
  { name: 'markdown_owner_contracts', path: 'tests/markdown_owner_contracts.rs' },
  { name: 'declarative_workspace_instruction_contracts', path: 'tests/declarative_workspace_instruction_contracts.rs' },
  { name: 'lsp_plugin_registry_contracts', path: 'tests/lsp_plugin_registry_contracts.rs' },
  { name: 'runtime_ownership_contracts', path: 'tests/runtime_ownership_contracts.rs' },
  { name: 'local_runtime_ports', path: 'tests/local_runtime_ports.rs' },
  { name: 'permission_store_contracts', path: 'tests/permission_store_contracts.rs' },
  { name: 'workspace_instruction_contracts', path: 'tests/workspace_instruction_contracts.rs' },
  { name: 'session_write_lock_contracts', path: 'tests/session_write_lock_contracts.rs' },
  { name: 'process_runtime_contracts', path: 'tests/process_runtime_contracts.rs' },
  { name: 'service_contracts', path: 'tests/service_contracts.rs' },
  { name: 'storage_owner_contracts', path: 'tests/storage_owner_contracts.rs' },
  { name: 'session_contracts', path: 'tests/session_contracts.rs' },
  { name: 'session_usage_contracts', path: 'tests/session_usage_contracts.rs' },
];

export const servicesIntegrationsIntegrationTestTargets = [
  { name: 'debug_log_owner_contracts', path: 'tests/debug_log_owner_contracts.rs' },
  { name: 'script_tool_runtime', path: 'tests/script_tool_runtime.rs' },
  { name: 'announcement_contracts', path: 'tests/announcement_contracts.rs' },
  { name: 'file_watch_contracts', path: 'tests/file_watch_contracts.rs' },
  { name: 'function_agent_contracts', path: 'tests/function_agent_contracts.rs' },
  { name: 'git_contracts', path: 'tests/git_contracts.rs' },
  { name: 'mcp_contracts', path: 'tests/mcp_contracts.rs' },
  { name: 'mcp_streamable_http_contracts', path: 'tests/mcp_streamable_http_contracts.rs' },
  { name: 'remote_connect_contracts', path: 'tests/remote_connect_contracts.rs' },
  { name: 'remote_ssh_contracts', path: 'tests/remote_ssh_contracts.rs' },
  { name: 'remote_workspace_search_disabled_contracts', path: 'tests/remote_workspace_search_disabled_contracts.rs' },
  { name: 'workspace_search_contracts', path: 'tests/workspace_search_contracts.rs' },
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
    if (
      line === ''
      || line.startsWith('//!')
      || /^#!\[cfg\(feature = "[A-Za-z0-9_-]+"\)\]$/.test(line)
    ) {
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

function skipRustTrivia(source, start) {
  let index = start;
  while (index < source.length) {
    if (/\s/.test(source[index])) {
      index += 1;
      continue;
    }
    if (source.startsWith('//', index)) {
      const lineEnd = source.indexOf('\n', index + 2);
      index = lineEnd === -1 ? source.length : lineEnd + 1;
      continue;
    }
    if (source.startsWith('/*', index)) {
      let depth = 1;
      index += 2;
      while (index < source.length && depth > 0) {
        if (source.startsWith('/*', index)) {
          depth += 1;
          index += 2;
        } else if (source.startsWith('*/', index)) {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      if (depth > 0) {
        return { index: source.length, error: 'unterminated block comment' };
      }
      continue;
    }
    break;
  }
  return { index };
}

function rustRawStringEnd(source, start) {
  let quoteIndex = start;
  if (source.startsWith('br', start) || source.startsWith('cr', start)) {
    quoteIndex += 2;
  } else if (source[start] === 'r') {
    quoteIndex += 1;
  } else {
    return null;
  }
  let hashCount = 0;
  while (source[quoteIndex] === '#') {
    hashCount += 1;
    quoteIndex += 1;
  }
  if (source[quoteIndex] !== '"') {
    return null;
  }
  const terminator = `"${'#'.repeat(hashCount)}`;
  const closingIndex = source.indexOf(terminator, quoteIndex + 1);
  return closingIndex === -1 ? -1 : closingIndex + terminator.length;
}

function rustCharLiteralEnd(source, start) {
  if (source[start] !== "'") {
    return null;
  }
  let index = start + 1;
  if (source[index] === '\\') {
    index += 1;
    if (source[index] === 'x') {
      if (!/^[0-9A-Fa-f]{2}$/.test(source.slice(index + 1, index + 3))) {
        return null;
      }
      index += 3;
    } else if (source[index] === 'u' && source[index + 1] === '{') {
      const closingBrace = source.indexOf('}', index + 2);
      if (
        closingBrace === -1
        || !/^[0-9A-Fa-f_]+$/.test(source.slice(index + 2, closingBrace))
      ) {
        return null;
      }
      index = closingBrace + 1;
    } else if (source[index] !== undefined && !/[\r\n]/.test(source[index])) {
      index += 1;
    } else {
      return null;
    }
  } else {
    const codePoint = source.codePointAt(index);
    if (codePoint === undefined || source[index] === "'" || /[\r\n]/.test(source[index])) {
      return null;
    }
    index += codePoint > 0xFFFF ? 2 : 1;
  }
  return source[index] === "'" ? index + 1 : null;
}

function rustQuotedLiteralEnd(source, start) {
  let quoteIndex = start;
  if ((source[start] === 'b' || source[start] === 'c') && source[start + 1] === '"') {
    quoteIndex += 1;
  }
  const quote = source[quoteIndex];
  if (quote !== '"') {
    return null;
  }
  let escaped = false;
  for (let index = quoteIndex + 1; index < source.length; index += 1) {
    const character = source[index];
    if (escaped) {
      escaped = false;
    } else if (character === '\\') {
      escaped = true;
    } else if (character === quote) {
      return index + 1;
    }
  }
  return -1;
}

function matchingRustAttributeBracket(source, openingIndex) {
  const closingForOpening = new Map([['[', ']'], ['(', ')'], ['{', '}']]);
  const stack = [']'];
  let index = openingIndex + 1;
  while (index < source.length) {
    if (source.startsWith('//', index) || source.startsWith('/*', index)) {
      const trivia = skipRustTrivia(source, index);
      if (trivia.error) {
        return { error: trivia.error };
      }
      index = trivia.index;
      continue;
    }
    const rawStringEnd = rustRawStringEnd(source, index);
    if (rawStringEnd !== null) {
      if (rawStringEnd === -1) {
        return { error: 'unterminated raw string in inner attribute' };
      }
      index = rawStringEnd;
      continue;
    }
    const charLiteralEnd = rustCharLiteralEnd(source, index);
    if (charLiteralEnd !== null) {
      index = charLiteralEnd;
      continue;
    }
    const quotedLiteralEnd = rustQuotedLiteralEnd(source, index);
    if (quotedLiteralEnd !== null) {
      if (quotedLiteralEnd === -1) {
        return { error: 'unterminated quoted literal in inner attribute' };
      }
      index = quotedLiteralEnd;
      continue;
    }
    const character = source[index];
    const closing = closingForOpening.get(character);
    if (closing) {
      stack.push(closing);
    } else if (character === ']' || character === ')' || character === '}') {
      if (stack.at(-1) !== character) {
        return { error: 'mismatched delimiter in inner attribute' };
      }
      stack.pop();
      if (stack.length === 0) {
        return { closingIndex: index };
      }
    }
    index += 1;
  }
  return { error: 'unterminated inner attribute' };
}

function leadingRustInnerAttributes(source) {
  const attributes = [];
  let index = source.charCodeAt(0) === 0xFEFF ? 1 : 0;
  if (source.startsWith('#!', index)) {
    const afterShebangBang = skipRustTrivia(source, index + 2);
    if (!afterShebangBang.error && source[afterShebangBang.index] !== '[') {
      const lineEnd = source.indexOf('\n', index + 2);
      index = lineEnd === -1 ? source.length : lineEnd + 1;
    }
  }
  while (index < source.length) {
    const leadingTrivia = skipRustTrivia(source, index);
    if (leadingTrivia.error) {
      return { attributes, error: leadingTrivia.error };
    }
    index = leadingTrivia.index;
    const attributeStart = index;
    if (source[index] !== '#') {
      break;
    }
    const afterHash = skipRustTrivia(source, index + 1);
    if (afterHash.error) {
      return { attributes, error: afterHash.error };
    }
    if (source[afterHash.index] !== '!') {
      break;
    }
    const afterBang = skipRustTrivia(source, afterHash.index + 1);
    if (afterBang.error) {
      return { attributes, error: afterBang.error };
    }
    if (source[afterBang.index] !== '[') {
      break;
    }
    const matched = matchingRustAttributeBracket(source, afterBang.index);
    if (matched.error) {
      return { attributes, error: matched.error };
    }
    const nameStart = skipRustTrivia(source, afterBang.index + 1);
    if (nameStart.error) {
      return { attributes, error: nameStart.error };
    }
    const nameSource = source.slice(nameStart.index, matched.closingIndex);
    const nameMatch = /^(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/.exec(nameSource);
    if (!nameMatch) {
      return { attributes, error: 'inner attribute has no supported name' };
    }
    attributes.push({
      name: nameMatch[1],
      raw: source.slice(attributeStart, matched.closingIndex + 1).trim(),
    });
    index = matched.closingIndex + 1;
  }
  return { attributes };
}

function validateGroupedLeafCfg(
  leaf,
  leafSource,
  allowedLeafCfgLines,
  errors,
) {
  const scanned = leadingRustInnerAttributes(leafSource);
  if (scanned.error) {
    errors.push(`grouped test leaf ${leaf} has an unsupported crate preamble: ${scanned.error}`);
    return;
  }
  const cfgAttributes = scanned.attributes.filter(
    (attribute) => attribute.name === 'cfg' || attribute.name === 'cfg_attr',
  );
  const allowedLine = allowedLeafCfgLines.get(leaf);
  if (
    allowedLine !== undefined
    && cfgAttributes.length === 1
    && cfgAttributes[0].raw === allowedLine
  ) {
    return;
  }
  if (cfgAttributes.length > 0 || allowedLine !== undefined) {
    errors.push(
      `grouped test leaf ${leaf} has a crate cfg that belongs in its explicit target root`,
    );
  }
}

export function validateExplicitIntegrationTestTopology({
  manifestText,
  expectedTargets,
  topLevelRustFiles,
  rootSources,
  leafRustFiles,
  leafSources,
  allowedLeafCfgLines = new Map(),
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
      const leafSource = leafSources.get(leaf);
      if (leafSource === undefined) {
        errors.push(`missing grouped test leaf source: ${leaf}`);
        continue;
      }
      validateGroupedLeafCfg(
        leaf,
        leafSource,
        allowedLeafCfgLines,
        errors,
      );
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

function collectRustFiles(dir, testsDir, files, sources, ignoredDirectories) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      const repoPath = `tests/${relative(testsDir, path).replaceAll('\\', '/')}`;
      if (!ignoredDirectories.has(repoPath)) {
        collectRustFiles(path, testsDir, files, sources, ignoredDirectories);
      }
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      const repoPath = `tests/${relative(testsDir, path).replaceAll('\\', '/')}`;
      files.push(repoPath);
      sources.set(repoPath, readFileSync(path, 'utf8'));
    }
  }
}

function checkExplicitIntegrationTestTopology(root, {
  cratePath,
  expectedTargets,
  ignoredDirectories = [],
  allowedLeafCfgLines = new Map(),
}) {
  const crateDir = join(root, ...cratePath.split('/'));
  const testsDir = join(crateDir, 'tests');
  const manifestPath = join(crateDir, 'Cargo.toml');
  const topLevelRustFiles = [];
  const leafRustFiles = [];
  const rootSources = new Map();
  const leafSources = new Map();
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
          leafSources,
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
    leafSources,
    allowedLeafCfgLines,
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

export function checkServicesCoreIntegrationTestTopology(root) {
  return checkExplicitIntegrationTestTopology(root, {
    cratePath: 'src/crates/services/services-core',
    expectedTargets: servicesCoreIntegrationTestTargets,
  });
}

export function checkServicesIntegrationsIntegrationTestTopology(root) {
  return checkExplicitIntegrationTestTopology(root, {
    cratePath: 'src/crates/services/services-integrations',
    expectedTargets: servicesIntegrationsIntegrationTestTargets,
    allowedLeafCfgLines: new Map([[
      'tests/remote_ssh_contracts/remote_ssh_disabled_contracts.rs',
      '#![cfg(not(feature = "remote-ssh-concrete"))]',
    ]]),
  });
}

export function checkServiceIntegrationTestTopologies(root) {
  return [
    ...checkServicesCoreIntegrationTestTopology(root),
    ...checkServicesIntegrationsIntegrationTestTopology(root),
  ];
}
