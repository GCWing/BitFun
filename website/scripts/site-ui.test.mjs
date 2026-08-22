#!/usr/bin/env node

import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const websiteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(websiteRoot, '..');
const sourceRoot = path.join(websiteRoot, 'src');

await execFileAsync(process.execPath, [path.join(websiteRoot, 'scripts/build.mjs')], {
  cwd: repositoryRoot,
});

const [appSource, stylesSource, templateSource, builtIndex, releaseSource] = await Promise.all([
  readFile(path.join(sourceRoot, 'app.js'), 'utf8'),
  readFile(path.join(sourceRoot, 'styles.css'), 'utf8'),
  readFile(path.join(sourceRoot, 'index.html'), 'utf8'),
  readFile(path.join(websiteRoot, 'dist/index.html'), 'utf8'),
  readFile(path.join(websiteRoot, 'dist/release.json'), 'utf8'),
]);

function cssBlock(selector) {
  const escaped = selector.replaceAll(/[.*+?^${}()|[\]\\]/gu, '\\$&');
  const match = stylesSource.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, 'u'));
  assert.ok(match, `missing CSS block: ${selector}`);
  return match[1];
}

function customProperties(block) {
  return Object.fromEntries(
    [...block.matchAll(/(--[a-z-]+)\s*:\s*([^;]+);/gu)]
      .map(([, key, value]) => [key, value.trim()]),
  );
}

test('theme contract defaults to system and exposes all three choices', () => {
  assert.match(templateSource, /localStorage\.getItem\('bitfun-playbook-theme'\)/u);
  assert.match(templateSource, /let theme = 'system'/u);
  assert.match(templateSource, /prefers-color-scheme: dark/u);
  assert.match(appSource, /const THEME_CHOICES = \['system', 'light', 'dark'\]/u);
  assert.match(appSource, /data-theme-choice="\$\{theme\}"/u);
  assert.match(appSource, /addEventListener\('change'/u);
  assert.match(stylesSource, /html\[data-theme='dark'\]/u);
  assert.match(stylesSource, /html\[data-theme='system'\]/u);

  const explicitDark = customProperties(cssBlock("html[data-theme='dark']"));
  const systemDark = customProperties(cssBlock("html[data-theme='system']"));
  assert.deepEqual(systemDark, explicitDark, 'system-dark tokens must match explicit dark mode');
});

test('hero title, search, and statistics remain in layout flow', () => {
  assert.match(appSource, /<h1>\$\{text\([\s\S]*?<span>[\s\S]*?<em>/u);
  assert.match(appSource, /<div class="hero-actions">[\s\S]*?<form class="search-box"[\s\S]*?<div class="hero-stat"/u);

  const title = cssBlock('.hero h1');
  const actions = cssBlock('.hero-actions');
  const statistics = cssBlock('.hero-stat');
  assert.match(title, /display:\s*grid/u);
  assert.match(title, /gap:/u);
  assert.match(actions, /display:\s*grid/u);
  assert.match(actions, /grid-template-columns:\s*minmax\(420px, 1fr\) auto/u);
  assert.doesNotMatch(actions, /position:\s*absolute/u);
  assert.doesNotMatch(statistics, /position:\s*absolute/u);
});

test('live search preserves the input node and supports IME composition', () => {
  assert.match(appSource, /function updateIndexResults\(\)/u);
  assert.match(appSource, /addEventListener\('compositionstart'/u);
  assert.match(appSource, /addEventListener\('compositionend'/u);
  assert.match(appSource, /event\.isComposing/u);

  const inputBinding = appSource.match(
    /const searchInput = document\.querySelector\('#capability-search'\);([\s\S]*?)document\.querySelectorAll\('\[data-kind\]'/u,
  );
  assert.ok(inputBinding, 'missing index search input binding');
  assert.match(inputBinding[1], /updateIndexResults\(\)/u);
  assert.doesNotMatch(inputBinding[1], /renderIndex\(\)/u);
});

test('sidebar navigation preserves its scroll position across page loads', () => {
  assert.match(appSource, /SIDEBAR_SCROLL_STORAGE_KEY/u);
  assert.match(appSource, /sessionStorage\.setItem/u);
  assert.match(appSource, /data-sidebar-scroll/u);
  assert.match(appSource, /addEventListener\('scroll', saveSidebarScroll/u);
  assert.match(appSource, /link\.addEventListener\('click', saveSidebarScroll\)/u);
  assert.match(appSource, /sidebarNav\.scrollTop = savedScrollTop/u);
  assert.match(appSource, /sidebarNav\.querySelector\('\[aria-current="page"\]'\)/u);
});

test('dark mode gives the agent callout its own subdued surface tokens', () => {
  const light = customProperties(cssBlock(':root'));
  const dark = customProperties(cssBlock("html[data-theme='dark']"));
  assert.notEqual(dark['--agent-panel'], light['--agent-panel']);
  assert.equal(dark['--agent-panel'], '#20271a');
  assert.match(cssBlock('.detail-aside'), /background:\s*var\(--agent-panel\)/u);
  assert.match(cssBlock('.detail-aside'), /color:\s*var\(--agent-panel-ink\)/u);
  assert.doesNotMatch(stylesSource, /var\(--lime(?:-ink)?\)/u);
});

test('build emits a source-sensitive immutable release id', () => {
  const release = JSON.parse(releaseSource);
  assert.match(release.releaseId, /^[0-9a-f]{12}$/u);
  assert.equal(release.releaseId, release.assetVersion);
  assert.match(release.catalogDigest, /^[0-9a-f]{64}$/u);
  assert.match(builtIndex, new RegExp(`/assets/styles\\.css\\?v=${release.releaseId}`, 'u'));
  assert.match(builtIndex, new RegExp(`/assets/app\\.js\\?v=${release.releaseId}`, 'u'));
});
