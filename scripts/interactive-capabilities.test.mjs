import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  buildCapabilityCatalog,
  parseRegisteredCommands,
} from './generate-interactive-capabilities.mjs';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (relativePath) => readFile(path.join(repositoryRoot, relativePath), 'utf8');

test('the public contract is a compact feature-and-settings manual', () => {
  const { publicCatalog } = buildCapabilityCatalog();
  const featureCount = publicCatalog.capabilities.filter(({ kind }) => kind === 'feature').length;
  const settingCount = publicCatalog.capabilities.filter(({ kind }) => kind === 'setting').length;
  const documentedItemCount = publicCatalog.capabilities.reduce(
    (total, capability) => total + capability.items.length,
    0,
  );
  assert.equal(publicCatalog.counts.features, featureCount);
  assert.equal(publicCatalog.counts.settings, settingCount);
  assert.equal(publicCatalog.counts.userFacing, publicCatalog.capabilities.length);
  assert.equal(publicCatalog.counts.documentedItems, documentedItemCount);
  assert.deepEqual(new Set(publicCatalog.capabilities.map(({ kind }) => kind)), new Set(['feature', 'setting']));
  assert.equal(new Set(publicCatalog.capabilities.map(({ id }) => id)).size, publicCatalog.capabilities.length);
  assert.ok(publicCatalog.capabilities.length <= 50, 'one default list call must return the curated catalog');
  assert.equal(publicCatalog.capabilities.some(({ id }) => id === 'get_configs'), false);
  assert.equal(JSON.stringify(publicCatalog).includes('tauri::'), false);
  assert.equal(JSON.stringify(publicCatalog).includes('implementationCoverage'), false);
  assert.equal(JSON.stringify(publicCatalog).includes('tauriCommandsAudited'), false);
  assert.equal(JSON.stringify(publicCatalog).includes('evidence'), false);
  assert.equal(JSON.stringify(publicCatalog).includes('reviewedInteractionContract'), false);
});

test('the technical audit exactly covers Desktop Tauri registration without exposing it as UX', async () => {
  const registrations = parseRegisteredCommands(await read('src/apps/desktop/src/lib.rs'));
  const { publicCatalog, technicalMap } = buildCapabilityCatalog();
  assert.equal(technicalMap.commands.length, registrations.length);
  assert.deepEqual(
    new Set(technicalMap.commands.map(({ id }) => id)),
    new Set(registrations.map(({ id }) => id)),
  );
  assert.equal(technicalMap.commandCount, registrations.length);
  assert.ok(technicalMap.commands.every(({ capabilityId, visibility }) =>
    capabilityId === null ? visibility === 'internal' : visibility === 'implementation'));
});

test('every manual entry has bilingual discovery, instructions, routing, and agent recipes', () => {
  const { publicCatalog } = buildCapabilityCatalog();
  for (const capability of publicCatalog.capabilities) {
    assert.match(capability.titleZh, /[\u3400-\u9fff]/u, `${capability.id} needs a Chinese title`);
    assert.match(capability.titleEn, /[A-Za-z]/u, `${capability.id} needs an English title`);
    assert.ok(capability.searchTerms.some((term) => /[\u3400-\u9fff]/u.test(term)));
    assert.ok(capability.searchTerms.some((term) => /[A-Za-z]/u.test(term)));
    assert.ok(capability.highlightsZh.length > 0);
    assert.ok(capability.items.length >= (capability.kind === 'feature' ? 6 : 4));
    assert.equal(new Set(capability.items.map(({ id }) => id)).size, capability.items.length);
    assert.ok(capability.items.every(({ titleZh, titleEn }) =>
      /[\u3400-\u9fff]/u.test(titleZh) && /[A-Za-z]/u.test(titleEn)));
    assert.ok(capability.stepsZh.length > 0);
    assert.ok(capability.agentExamplesZh.length > 0);
    assert.ok(['settings', 'action', 'scene', 'event'].includes(capability.destination.kind));
    assert.equal(capability.docsUrl, `${publicCatalog.origin}/capabilities/${capability.id}/`);
    assert.doesNotMatch(JSON.stringify(capability), /"handler"/u);
  }
});

test('the built-in browser manual covers its element picker in both languages', () => {
  const { publicCatalog } = buildCapabilityCatalog();
  const browser = publicCatalog.capabilities.find(({ id }) => id === 'feature.browser');
  assert.ok(browser);
  assert.ok(browser.items.some(({ id, titleZh, titleEn }) =>
    id === 'element-picker' && titleZh.includes('元素选择器') && /element picker/iu.test(titleEn)));
  assert.ok(browser.items.some(({ id, titleZh, titleEn }) =>
    id === 'element-context' && titleZh.includes('CSS 路径') && /session context/iu.test(titleEn)));
  assert.ok(browser.searchTerms.includes('启动元素选择器，悬停时高亮页面元素并显示标签、ID 和 Class'));
  assert.ok(browser.searchTerms.includes('Start the element picker to highlight hovered elements and show tag, ID, and class details'));
});

test('docs, runtime, and technical views are generated projections of one semantic source', async () => {
  const source = JSON.parse(await read('src/shared/interactive-capabilities/catalog.json'));
  const publicCatalog = JSON.parse(await read('docs/interactive-capabilities/capabilities.json'));
  const runtimeCatalog = JSON.parse(await read(
    'src/web-ui/src/app/global-search/generated/interactive-capabilities.json',
  ));
  const technicalMap = JSON.parse(await read(
    'docs/interactive-capabilities/technical/tauri-command-map.json',
  ));
  const interactionMap = JSON.parse(await read(
    'docs/interactive-capabilities/technical/ui-interaction-inventory.json',
  ));

  const sourceIds = source.capabilities.map(({ id }) => id);
  assert.equal(
    source.reviewedDocumentedItemCount,
    source.capabilities.reduce((total, capability) => total + capability.items.length, 0),
  );
  assert.deepEqual(interactionMap.roots, source.reviewedInteractionContract.roots);
  assert.deepEqual(publicCatalog.capabilities.map(({ id }) => id), sourceIds);
  assert.deepEqual(runtimeCatalog.capabilities.map(({ id }) => id), sourceIds);
  assert.equal(publicCatalog.digest, runtimeCatalog.digest);
  assert.equal(publicCatalog.digest, technicalMap.catalogDigest);
  assert.equal(publicCatalog.digest, interactionMap.catalogDigest);
  assert.equal(interactionMap.digest, source.reviewedInteractionContract.digest);
  assert.ok(interactionMap.fileCount >= 300);
  assert.ok(interactionMap.interactionCount >= 4_000);
  assert.ok(interactionMap.files.some(({ sourceFile }) =>
    sourceFile.endsWith('/BrowserPanel.tsx')));
  assert.ok(interactionMap.files.some(({ sourceFile }) =>
    sourceFile.endsWith('/AssistantDefaultsPage.tsx')));
  assert.ok(interactionMap.files.some(({ sourceFile }) =>
    sourceFile.endsWith('/AppearanceSettingsPage.tsx')));
  assert.equal(publicCatalog.source, 'src/shared/interactive-capabilities/catalog.json');

  const publicById = new Map(publicCatalog.capabilities.map((capability) => [capability.id, capability]));
  for (const runtime of runtimeCatalog.capabilities) {
    const publicValue = publicById.get(runtime.id);
    assert.deepEqual(runtime.operations.map(({ handler: _handler, ...value }) => value), publicValue.operations);
    assert.deepEqual(runtime.options.map(({ handler: _handler, ...value }) => value), publicValue.options);
  }

  const docs = (await readdir(path.join(repositoryRoot, 'docs/interactive-capabilities/capabilities')))
    .filter((file) => file.endsWith('.md'));
  assert.equal(docs.length, sourceIds.length);
});

test('website, global search, and agent control consume generated semantic projections', async () => {
  const websiteBuild = await read('website/scripts/build.mjs');
  const frontendCatalog = await read('src/web-ui/src/app/global-search/interactiveCapabilityCatalog.ts');
  const searchProvider = await read(
    'src/web-ui/src/app/global-search/providers/interactiveCapabilitySearchProvider.ts',
  );
  const searchProviders = await read('src/web-ui/src/app/global-search/providers/index.ts');
  const controlBridge = await read('src/web-ui/src/app/global-search/bitfunControlBridge.ts');
  const controlTool = await read(
    'src/crates/assembly/core/src/agentic/tools/implementations/bitfun_control_tool.rs',
  );

  assert.match(websiteBuild, /docs\/interactive-capabilities\/capabilities\.json/u);
  assert.match(frontendCatalog, /generated\/interactive-capabilities\.json/u);
  assert.match(searchProvider, /INTERACTIVE_CAPABILITY_CATALOG/u);
  assert.doesNotMatch(searchProviders, /settingsSearchProvider/u);
  assert.match(controlBridge, /INTERACTIVE_CAPABILITY_CATALOG/u);
  assert.doesNotMatch(controlTool, /include_(?:str|bytes)!/u);
  assert.match(controlTool, /two-step/u);
});

test('Desktop and Web UI share the BitFunControl transport contract', async () => {
  const host = await read('src/apps/desktop/src/bitfun_control_host.rs');
  const bridge = await read('src/web-ui/src/app/global-search/bitfunControlBridge.ts');
  const desktopRegistration = await read('src/apps/desktop/src/lib.rs');

  for (const source of [host, bridge]) {
    assert.match(source, /agentic:\/\/bitfun-control-request/u);
  }
  assert.match(host, /#\[serde\(rename_all = "camelCase"\)\]/u);
  assert.match(bridge, /api\.invoke\('mark_bitfun_control_surface_ready'\)/u);
  assert.match(bridge, /api\.invoke\('report_bitfun_control_result'/u);
  assert.match(desktopRegistration, /bitfun_control_host::mark_bitfun_control_surface_ready/u);
  assert.match(desktopRegistration, /bitfun_control_host::report_bitfun_control_result/u);
});
