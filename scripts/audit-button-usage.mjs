#!/usr/bin/env node

import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const sourceRoot = path.join(repositoryRoot, 'src/web-ui/src');
const excludedDirectories = new Set([
  path.join(sourceRoot, 'component-library'),
  path.join(sourceRoot, 'generated'),
]);
const compoundRoles = new Set([
  'menuitem',
  'menuitemcheckbox',
  'menuitemradio',
  'option',
  'switch',
  'tab',
  'treeitem',
]);

function attributeName(attribute) {
  return ts.isJsxAttribute(attribute) ? attribute.name.text : undefined;
}

function attributeValue(attribute) {
  if (!attribute || !ts.isJsxAttribute(attribute) || !attribute.initializer) return undefined;
  if (ts.isStringLiteral(attribute.initializer)) return attribute.initializer.text;
  if (
    ts.isJsxExpression(attribute.initializer)
    && attribute.initializer.expression
  ) {
    if (ts.isStringLiteral(attribute.initializer.expression)) {
      return attribute.initializer.expression.text;
    }
    if (ts.isTemplateExpression(attribute.initializer.expression)) {
      return attribute.initializer.expression.head.text;
    }
  }
  return undefined;
}

function hasAttribute(attributes, name) {
  return attributes.properties.some(attribute => attributeName(attribute) === name);
}

function staticAttribute(attributes, name) {
  const attribute = attributes.properties.find(item => attributeName(item) === name);
  return attributeValue(attribute);
}

function hasLikelyVisibleLabel(children) {
  return children.some(child => {
    if (ts.isJsxText(child)) return child.text.trim().length > 0;
    if (!ts.isJsxExpression(child) || !child.expression) return false;
    return !ts.isJsxElement(child.expression) && !ts.isJsxSelfClosingElement(child.expression);
  });
}

function classifyButton(attributes, children) {
  const role = staticAttribute(attributes, 'role');
  const ownerPart = staticAttribute(attributes, 'data-bf-part');
  const className = staticAttribute(attributes, 'className');
  if (
    compoundRoles.has(role)
    || hasAttribute(attributes, 'aria-haspopup')
    || hasAttribute(attributes, 'aria-expanded')
    || hasAttribute(attributes, 'aria-pressed')
    || hasAttribute(attributes, 'aria-current')
    || /(?:badge|card|filter|header|item|menu|nav|option|select|summary|tab|toggle|trigger)/i.test(ownerPart ?? '')
    || /__(?:badge|card|filter|header|item|nav|option|select|summary|tab|toggle|trigger)(?:\b|[_-])/i.test(className ?? '')
    || /(?:menu|dropdown|popover|context)[_-].*item|item[_-].*(?:menu|dropdown)/i.test(className ?? '')
  ) {
    return 'compound';
  }

  const hasAccessibleName = hasAttribute(attributes, 'aria-label')
    || hasAttribute(attributes, 'aria-labelledby')
    || hasAttribute(attributes, 'title');
  if (hasAccessibleName && !hasLikelyVisibleLabel(children)) return 'icon';
  return 'action';
}

function declarationName(node) {
  if (ts.isFunctionDeclaration(node) && node.name) return node.name.text;
  if (ts.isClassDeclaration(node) && node.name) return node.name.text;
  if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)) return node.name.text;
  return undefined;
}

export function analyzeButtonSource(file, source) {
  const sourceFile = ts.createSourceFile(
    file,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  const buttons = [];
  const localButtonDefinitions = [];

  function visit(node) {
    const name = declarationName(node);
    if (name?.endsWith('Button')) {
      const location = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
      localButtonDefinitions.push({ file, line: location.line + 1, name });
    }

    if (ts.isJsxElement(node) && node.openingElement.tagName.getText(sourceFile) === 'button') {
      const location = sourceFile.getLineAndCharacterOfPosition(node.openingElement.getStart(sourceFile));
      const attributes = node.openingElement.attributes;
      buttons.push({
        file,
        line: location.line + 1,
        kind: classifyButton(attributes, node.children),
        role: staticAttribute(attributes, 'role'),
        owner: staticAttribute(attributes, 'data-bf-component'),
      });
    }

    if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(sourceFile) === 'button') {
      const location = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
      buttons.push({
        file,
        line: location.line + 1,
        kind: classifyButton(node.attributes, []),
        role: staticAttribute(node.attributes, 'role'),
        owner: staticAttribute(node.attributes, 'data-bf-component'),
      });
    }

    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
  return { buttons, localButtonDefinitions };
}

async function collectFiles(directory, includeTests) {
  if (excludedDirectories.has(directory)) return [];
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectFiles(absolutePath, includeTests));
      continue;
    }
    if (!entry.isFile() || path.extname(entry.name) !== '.tsx') continue;
    if (!includeTests && /\.(?:test|spec)\.tsx$/.test(entry.name)) continue;
    files.push(absolutePath);
  }
  return files;
}

export async function buildButtonInventory({ includeTests = false } = {}) {
  const buttons = [];
  const localButtonDefinitions = [];
  for (const absolutePath of await collectFiles(sourceRoot, includeTests)) {
    const file = path.relative(repositoryRoot, absolutePath).replaceAll(path.sep, '/');
    const source = await readFile(absolutePath, 'utf8');
    const analysis = analyzeButtonSource(file, source);
    buttons.push(...analysis.buttons);
    localButtonDefinitions.push(...analysis.localButtonDefinitions);
  }
  return { buttons, localButtonDefinitions };
}

function summarize(inventory) {
  const byKind = Object.fromEntries(
    ['action', 'icon', 'compound'].map(kind => [
      kind,
      inventory.buttons.filter(button => button.kind === kind).length,
    ]),
  );
  return {
    total: inventory.buttons.length,
    byKind,
    localButtonDefinitions: inventory.localButtonDefinitions.length,
  };
}

async function main() {
  const includeTests = process.argv.includes('--include-tests');
  const inventory = await buildButtonInventory({ includeTests });
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify({ ...summarize(inventory), ...inventory }, null, 2)}\n`);
    return;
  }

  const summary = summarize(inventory);
  console.log(`Native product buttons: ${summary.total}`);
  console.log(`  independent actions: ${summary.byKind.action}`);
  console.log(`  icon actions: ${summary.byKind.icon}`);
  console.log(`  compound-control internals: ${summary.byKind.compound}`);
  console.log(`Local *Button definitions: ${summary.localButtonDefinitions}`);

  const actionHotspots = new Map();
  for (const button of inventory.buttons.filter(item => item.kind === 'action')) {
    actionHotspots.set(button.file, (actionHotspots.get(button.file) ?? 0) + 1);
  }
  console.log('\nIndependent-action hotspots:');
  for (const [file, count] of [...actionHotspots].sort((left, right) => (
    right[1] - left[1] || left[0].localeCompare(right[0])
  )).slice(0, 30)) {
    console.log(`  ${count}\t${file}`);
  }
}

const entryPoint = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : undefined;
if (entryPoint === import.meta.url) {
  await main();
}
