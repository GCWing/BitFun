#!/usr/bin/env node

import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const sourceRoot = path.join(repositoryRoot, 'src/web-ui/src');
const sourceExtensions = new Set(['.css', '.scss', '.ts', '.tsx']);
const styleExtensions = new Set(['.css', '.scss']);
const excludedRoots = new Set([
  path.join(sourceRoot, 'generated'),
]);

function isAuditedSourceFile(file) {
  const name = path.basename(file);
  return sourceExtensions.has(path.extname(name))
    && !/\.(?:test|spec)\.(?:ts|tsx)$/.test(name);
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map(async entry => {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      return excludedRoots.has(absolutePath) ? [] : collectFiles(absolutePath);
    }
    return isAuditedSourceFile(absolutePath) ? [absolutePath] : [];
  }));
  return nested.flat();
}

function lineNumberAt(source, index) {
  return source.slice(0, index).split('\n').length;
}

function relativePath(file) {
  return path.relative(repositoryRoot, file);
}

function findMatches(source, expression) {
  return [...source.matchAll(expression)].map(match => ({
    index: match.index ?? 0,
    value: match[0],
    groups: match.slice(1),
  }));
}

function withoutComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, comment => comment.replace(/[^\n]/g, ' '))
    .replace(/(^|\s)\/\/[^\n]*/g, comment => comment.replace(/[^\n]/g, ' '));
}

function printLocations(title, locations, limit = 80) {
  console.log(`\n${title}: ${locations.length}`);
  locations.slice(0, limit).forEach(location => {
    console.log(`  ${location.file}:${location.line}${location.detail ? ` (${location.detail})` : ''}`);
  });
  if (locations.length > limit) {
    console.log(`  ... ${locations.length - limit} more`);
  }
}

const files = await collectFiles(sourceRoot);
const transitionAll = [];
const scaleZero = [];
const smoothScroll = [];
const unguardedInfinite = [];
const keyframes = new Map();
let interactiveHandlers = 0;
let motionOptIns = 0;

for (const file of files) {
  const source = await readFile(file, 'utf8');
  const searchableSource = withoutComments(source);
  const fileName = relativePath(file);
  const extension = path.extname(file);

  if (extension === '.tsx') {
    interactiveHandlers += findMatches(searchableSource, /\bon(?:Click|DoubleClick|ContextMenu|PointerDown|MouseDown|KeyDown)\s*=/g).length;
    motionOptIns += findMatches(searchableSource, /\bdata-motion\s*=/g).length;
  }

  for (const match of findMatches(searchableSource, /\btransition\s*:\s*all\b/g)) {
    transitionAll.push({ file: fileName, line: lineNumberAt(source, match.index) });
  }
  for (const match of findMatches(searchableSource, /\bscale\(\s*0(?:\.0+)?\s*\)/g)) {
    scaleZero.push({ file: fileName, line: lineNumberAt(source, match.index) });
  }
  for (const match of findMatches(searchableSource, /\bbehavior\s*:\s*['"]smooth['"]/g)) {
    smoothScroll.push({ file: fileName, line: lineNumberAt(source, match.index) });
  }

  if (!styleExtensions.has(extension)) continue;

  for (const match of findMatches(searchableSource, /@(?:-webkit-)?keyframes\s+([\w-]+)/g)) {
    const name = match.groups[0];
    const definitions = keyframes.get(name) ?? [];
    definitions.push({ file: fileName, line: lineNumberAt(source, match.index) });
    keyframes.set(name, definitions);
  }

  const hasInfiniteAnimation = /\banimation(?:-[\w-]+)?\s*:[^;{}]*\binfinite\b/.test(searchableSource);
  const hasReducedMotionRule = /@media\s*\([^)]*prefers-reduced-motion\s*:\s*reduce[^)]*\)/.test(searchableSource);
  if (hasInfiniteAnimation && !hasReducedMotionRule) {
    unguardedInfinite.push({ file: fileName, line: 1 });
  }
}

const duplicateKeyframes = [...keyframes.entries()]
  .filter(([, definitions]) => definitions.length > 1)
  .sort((left, right) => right[1].length - left[1].length);

console.log('BitFun Web UI motion inventory');
console.log(`Scanned ${files.length} source files under src/web-ui/src.`);
console.log(`Interactive handler attributes: ${interactiveHandlers}`);
console.log(`Explicit data-motion opt-ins: ${motionOptIns}`);
printLocations('transition: all', transitionAll);
printLocations('scale(0)', scaleZero);
printLocations('JavaScript smooth scrolling (verify keyboard and reduced-motion paths)', smoothScroll);
printLocations('Files with infinite animation and no local reduced-motion rule', unguardedInfinite);

console.log(`\nDuplicate global keyframe names: ${duplicateKeyframes.length}`);
duplicateKeyframes.slice(0, 40).forEach(([name, definitions]) => {
  console.log(`  ${name}: ${definitions.length}`);
  definitions.forEach(definition => console.log(`    ${definition.file}:${definition.line}`));
});
if (duplicateKeyframes.length > 40) {
  console.log(`  ... ${duplicateKeyframes.length - 40} more`);
}

console.log('\nThis command is an inventory, not a pass/fail gate. Review intent before changing layout transitions or virtualized content.');
