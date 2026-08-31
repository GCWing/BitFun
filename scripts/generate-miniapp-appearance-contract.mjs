#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const contractPath = path.join(repositoryRoot, 'src/shared/miniapp-appearance/contract.json');
const outputPath = path.join(
  repositoryRoot,
  'src/crates/contracts/product-domains/src/miniapp/generated/default_appearance_style.html',
);
const themeEntryPath = path.join(repositoryRoot, 'design-system/packages/theme-bitfun/dist/index.js');
const systemEntryPath = path.join(repositoryRoot, 'design-system/packages/design-tokens/dist/index.js');
const checkOnly = process.argv.includes('--check');

const contract = JSON.parse(fs.readFileSync(contractPath, 'utf8'));
const [{ themes, themeCssVariables }, { cssVariables: systemCssVariables, tokens: systemTokens }] = await Promise.all([
  import(pathToFileURL(themeEntryPath).href),
  import(pathToFileURL(systemEntryPath).href),
]);

const themeNamesByCssVariable = invertUnique(themeCssVariables, 'theme');
const systemNamesByCssVariable = invertUnique(systemCssVariables, 'system');
validateContract(contract);
const systemDeclarations = renderDeclarations('system');
const darkDeclarations = renderDeclarations('theme', 'dark');
const lightDeclarations = renderDeclarations('theme', 'light');
const generated = [
  '<style id="bitfun-appearance-default">',
  '  :root {',
  '    color-scheme: light dark;',
  '    background: transparent;',
  systemDeclarations,
  darkDeclarations,
  '  }',
  '  @media (prefers-color-scheme: light) {',
  '    :root {',
  lightDeclarations,
  '    }',
  '  }',
  '</style>',
  '',
].join('\n');

const current = fs.existsSync(outputPath) ? normalize(fs.readFileSync(outputPath, 'utf8')) : null;
if (checkOnly) {
  if (current !== normalize(generated)) {
    console.error('[miniapp-appearance] Generated first-paint projection is stale. Run `pnpm run miniapp:appearance:generate`.');
    process.exit(1);
  }
  console.log('[miniapp-appearance] Contract and generated first-paint projection are in sync.');
} else {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, generated, 'utf8');
  console.log(`[miniapp-appearance] Wrote ${relative(outputPath)}.`);
}

function validateContract(value) {
  if (value?.version !== 1 || !Array.isArray(value.variables) || value.variables.length === 0) {
    throw new Error('Unsupported or empty MiniApp appearance contract.');
  }
  const names = new Set();
  for (const variable of value.variables) {
    if (!/^--bitfun-[a-z0-9-]+$/.test(variable.name ?? '')) {
      throw new Error(`Invalid MiniApp appearance variable: ${String(variable.name)}.`);
    }
    if (names.has(variable.name)) throw new Error(`Duplicate MiniApp appearance variable: ${variable.name}.`);
    names.add(variable.name);
    if (!['theme', 'system'].includes(variable.kind)) {
      throw new Error(`Invalid MiniApp appearance variable kind for ${variable.name}.`);
    }
    const owners = variable.kind === 'theme' ? themeNamesByCssVariable : systemNamesByCssVariable;
    if (!owners.has(variable.source)) {
      throw new Error(`${variable.name} references unknown canonical ${variable.kind} variable ${variable.source}.`);
    }
  }
}

function invertUnique(values, label) {
  const result = new Map();
  for (const [name, cssVariable] of Object.entries(values)) {
    if (result.has(cssVariable)) throw new Error(`Duplicate canonical ${label} CSS variable ${cssVariable}.`);
    result.set(cssVariable, name);
  }
  return result;
}

function renderDeclarations(kind, mode) {
  return contract.variables
    .filter(variable => variable.kind === kind)
    .map(variable => {
      const canonicalName = (kind === 'theme' ? themeNamesByCssVariable : systemNamesByCssVariable).get(variable.source);
      const value = kind === 'theme' ? themes[mode][canonicalName] : systemTokens[canonicalName];
      if (!['string', 'number'].includes(typeof value)) {
        throw new Error(`Canonical value for ${variable.source} cannot be serialized into CSS.`);
      }
      return `    ${variable.name}: ${String(value)};`;
    })
    .join('\n');
}

function normalize(value) {
  return String(value).replace(/\r\n?/g, '\n');
}

function relative(value) {
  return path.relative(repositoryRoot, value).split(path.sep).join('/');
}
