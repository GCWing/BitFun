#!/usr/bin/env node

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  existsSync,
  readFileSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ASSET_DIR = join(
  ROOT,
  'src',
  'crates',
  'services',
  'services-integrations',
  'assets',
);
const SNAPSHOT_PATH = join(ASSET_DIR, 'models-dev.json');
const PROVENANCE_PATH = join(ASSET_DIR, 'models-dev.provenance.json');
const LICENSE_PATH = join(ASSET_DIR, 'models-dev.LICENSE.txt');
const PROVIDER_OVERLAY_PATH = join(
  ROOT,
  'src',
  'shared',
  'ai-provider-catalog',
  'providers.json',
);
const NOTICE_PATH = join(ROOT, 'THIRD_PARTY_NOTICES.md');
const API_URL = 'https://models.dev/api.json';
const REPOSITORY = 'https://github.com/anomalyco/models.dev';
const REPOSITORY_BRANCH = 'dev';
const EXPECTED_ARTIFACT_PATH =
  'src/crates/services/services-integrations/assets/models-dev.json';
const EXPECTED_LICENSE_PATH =
  'src/crates/services/services-integrations/assets/models-dev.LICENSE.txt';
const EXPECTED_PROVIDER_OVERLAY_PATH =
  'src/shared/ai-provider-catalog/providers.json';

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function serializedJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function optionValue(args, name) {
  const inlinePrefix = `${name}=`;
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === name) {
      return args[index + 1];
    }
    if (args[index].startsWith(inlinePrefix)) {
      return args[index].slice(inlinePrefix.length);
    }
  }
  return undefined;
}

function assertString(value, message) {
  assert.equal(typeof value, 'string', message);
  assert.notEqual(value.trim(), '', message);
}

function normalizedOptions(value) {
  if (Array.isArray(value)) {
    return value;
  }
  if (value && typeof value === 'object') {
    return [value];
  }
  return [];
}

function readProviderOverlay() {
  const overlay = readJson(PROVIDER_OVERLAY_PATH);
  assert.equal(overlay.schema_version, 1,
    'unsupported built-in provider overlay schema');
  assert(Array.isArray(overlay.providers) && overlay.providers.length > 0,
    'built-in provider overlay must define providers');
  return overlay;
}

function boundModelsDevProviderIds(overlay) {
  const providerIds = new Set();
  for (const provider of overlay.providers) {
    for (const providerId of provider.catalog_provider_ids || []) {
      assertString(providerId,
        `provider ${provider.id} has an invalid models.dev binding`);
      providerIds.add(providerId);
    }
    for (const endpoint of provider.endpoints || []) {
      for (const providerId of endpoint.catalog_provider_ids || []) {
        assertString(providerId,
          `provider ${provider.id} endpoint ${endpoint.id} has an invalid models.dev binding`);
        providerIds.add(providerId);
      }
      for (const binding of endpoint.reasoning_catalog_bindings || []) {
        assertString(binding.model_id,
          `provider ${provider.id} endpoint ${endpoint.id} has an invalid reasoning model binding`);
        assertString(binding.source_provider_id,
          `provider ${provider.id} endpoint ${endpoint.id} has an invalid reasoning source provider`);
        assertString(binding.source_model_id,
          `provider ${provider.id} endpoint ${endpoint.id} has an invalid reasoning source model`);
        providerIds.add(binding.source_provider_id);
      }
    }
  }
  return [...providerIds].sort();
}

function bundledModelsDevProviderIds(overlay, provenance) {
  const providerIds = new Set(boundModelsDevProviderIds(overlay));
  const supplementalProviderIds =
    provenance.transformation?.supplemental_reasoning_provider_ids || [];
  assert(Array.isArray(supplementalProviderIds),
    'supplemental reasoning provider IDs must be an array');
  for (const providerId of supplementalProviderIds) {
    assertString(providerId, 'supplemental reasoning provider ID is invalid');
    providerIds.add(providerId);
  }
  return [...providerIds].sort();
}

function copyDefinedFields(source, fields) {
  return Object.fromEntries(fields
    .filter(field => source[field] !== undefined && source[field] !== null)
    .map(field => [field, source[field]]));
}

function copyModel(sourceModel, modelId) {
  const model = {
    id: sourceModel.id || modelId,
    ...copyDefinedFields(sourceModel, [
      'name',
      'description',
      'family',
      'status',
      'release_date',
      'last_updated',
      'knowledge',
      'open_weights',
      'attachment',
      'reasoning',
      'tool_call',
      'structured_output',
    ]),
  };
  if (Object.hasOwn(sourceModel, 'reasoning_options')) {
    model.reasoning_options = normalizedOptions(sourceModel.reasoning_options);
  }
  if (sourceModel.modalities && typeof sourceModel.modalities === 'object') {
    model.modalities = copyDefinedFields(sourceModel.modalities, ['input', 'output']);
  }
  if (sourceModel.limit && typeof sourceModel.limit === 'object') {
    model.limit = copyDefinedFields(sourceModel.limit, ['context', 'input', 'output']);
  }
  if (sourceModel.cost && typeof sourceModel.cost === 'object') {
    model.cost = copyDefinedFields(
      sourceModel.cost,
      ['input', 'output', 'cache_read', 'cache_write'],
    );
  }
  return model;
}

function buildBundledSnapshot(source, overlay, provenance) {
  assert(source && typeof source === 'object' && !Array.isArray(source),
    'models.dev source must be a provider object');

  const snapshot = {};
  for (const providerId of bundledModelsDevProviderIds(overlay, provenance)) {
    const sourceProvider = source[providerId];
    assert(sourceProvider, `models.dev source is missing provider ${providerId}`);
    const sourceModels = sourceProvider.models;
    assert(sourceModels && typeof sourceModels === 'object' && !Array.isArray(sourceModels),
      `models.dev provider ${providerId} has no model catalog`);
    const modelEntries = Object.entries(sourceModels).sort(([left], [right]) =>
      (left < right ? -1 : left > right ? 1 : 0));
    assert(modelEntries.length > 0,
      `models.dev provider ${providerId} has no models`);

    const provider = {
      id: sourceProvider.id || providerId,
      name: sourceProvider.name || providerId,
      ...copyDefinedFields(sourceProvider, ['api', 'doc', 'env']),
      models: {},
    };
    for (const [modelId, sourceModel] of modelEntries) {
      assert(sourceModel && typeof sourceModel === 'object' && !Array.isArray(sourceModel),
        `models.dev model ${providerId}/${modelId} is invalid`);
      provider.models[modelId] = copyModel(sourceModel, modelId);
    }
    snapshot[providerId] = provider;
  }
  return snapshot;
}

function snapshotModelCounts(snapshot) {
  return Object.fromEntries(Object.entries(snapshot).map(([providerId, provider]) => [
    providerId,
    Object.keys(provider.models || {}).length,
  ]));
}

function validateProvenance(provenance, snapshotText, licenseText) {
  assert.equal(provenance.schema_version, 1, 'unsupported provenance schema');
  assert.equal(provenance.artifact?.path, EXPECTED_ARTIFACT_PATH,
    'unexpected bundled artifact path');
  assert.match(provenance.artifact?.sha256 || '', /^[a-f0-9]{64}$/,
    'artifact SHA-256 is missing or invalid');
  assert.equal(provenance.artifact.sha256, sha256(snapshotText),
    'bundled models.dev snapshot hash does not match provenance');
  assert.equal(provenance.artifact.bytes, Buffer.byteLength(snapshotText),
    'bundled models.dev snapshot byte count does not match provenance');

  assert.equal(provenance.source?.project, 'models.dev');
  assert.equal(provenance.source?.api_url, API_URL);
  assert.equal(provenance.source?.repository, REPOSITORY);
  assert.equal(provenance.source?.repository_branch, REPOSITORY_BRANCH);
  assert.match(provenance.source?.repository_revision || '', /^[a-f0-9]{40}$/,
    'upstream repository revision is missing or invalid');
  assert.match(provenance.source?.api_sha256 || '', /^[a-f0-9]{64}$/,
    'upstream API SHA-256 is missing or invalid');
  assert(Number.isSafeInteger(provenance.source?.api_bytes) && provenance.source.api_bytes > 0,
    'upstream API byte count is missing or invalid');
  assertString(provenance.source?.revision_role,
    'provenance must explain the role of the repository revision');
  assert(!Number.isNaN(Date.parse(provenance.source?.retrieved_at)),
    'upstream retrieval time is missing or invalid');

  assert.equal(provenance.license?.spdx, 'MIT');
  assert.equal(provenance.license?.copyright, 'Copyright (c) 2025 models.dev');
  assert.equal(provenance.license?.source_path, EXPECTED_LICENSE_PATH);
  assert.equal(provenance.license?.release_path, 'third-party/models.dev/LICENSE.txt');
  assert.equal(provenance.license?.sha256, sha256(licenseText),
    'models.dev license hash does not match provenance');
  assert.match(licenseText, /^MIT License\r?\n/);
  assert.match(licenseText, /Copyright \(c\) 2025 models\.dev/);
  assert.match(licenseText,
    /The above copyright notice and this permission notice shall be included/);
}

function validateBundledSnapshot(snapshot, provenance, overlay) {
  const requiredProviderIds = boundModelsDevProviderIds(overlay);
  const expectedProviderIds = bundledModelsDevProviderIds(overlay, provenance);
  assert.equal(
    provenance.transformation?.kind,
    'builtin_provider_catalog_fallback',
    'bundled snapshot must declare its provider catalog fallback role',
  );
  assert.equal(
    provenance.transformation?.provider_overlay_path,
    EXPECTED_PROVIDER_OVERLAY_PATH,
    'bundled snapshot must declare the provider overlay that owns its coverage',
  );
  assert.deepEqual(
    provenance.transformation?.included_provider_ids,
    expectedProviderIds,
    'provenance provider coverage must match the provider overlay and supplemental reasoning providers',
  );
  for (const providerId of requiredProviderIds) {
    assert(Object.hasOwn(snapshot, providerId),
      `bundled snapshot is missing provider overlay binding ${providerId}`);
  }
  for (const provider of overlay.providers) {
    for (const endpoint of provider.endpoints || []) {
      for (const binding of endpoint.reasoning_catalog_bindings || []) {
        assert(
          Object.hasOwn(
            snapshot[binding.source_provider_id]?.models || {},
            binding.source_model_id,
          ),
          `bundled snapshot is missing reasoning binding ${binding.source_provider_id}/${binding.source_model_id}`,
        );
      }
    }
  }
  assert.deepEqual(
    Object.keys(snapshot).sort(),
    expectedProviderIds,
    'bundled snapshot must cover every models.dev binding in the provider overlay',
  );
  assert.deepEqual(
    provenance.transformation?.included_provider_model_counts,
    snapshotModelCounts(snapshot),
    'provenance model counts must match the bundled snapshot',
  );
  const rebuilt = buildBundledSnapshot(snapshot, overlay, provenance);
  assert.deepEqual(rebuilt, snapshot,
    'bundled models.dev snapshot contains fields or entries outside the declared transformation');
}

function validateReleaseDelivery() {
  const notice = readFileSync(NOTICE_PATH, 'utf8');
  assert.match(notice, /Copyright \(c\) 2025 models\.dev/);
  assert.match(notice, /models-dev\.LICENSE\.txt/);
  assert.match(notice, /models-dev\.provenance\.json/);

  for (const configName of ['tauri.conf.json', 'tauri.dev.conf.json']) {
    const tauriConfig = readJson(join(ROOT, 'src', 'apps', 'desktop', configName));
    const resources = tauriConfig.bundle?.resources || {};
    assert.equal(resources['../../../THIRD_PARTY_NOTICES.md'], 'THIRD_PARTY_NOTICES.md',
      `${configName} must bundle THIRD_PARTY_NOTICES.md`);
    assert.equal(
      resources['../../crates/services/services-integrations/assets/models-dev.LICENSE.txt'],
      'third-party/models.dev/LICENSE.txt',
      `${configName} must bundle the models.dev license`,
    );
    assert.equal(
      resources['../../crates/services/services-integrations/assets/models-dev.provenance.json'],
      'third-party/models.dev/provenance.json',
      `${configName} must bundle models.dev provenance`,
    );
  }

  for (const packageScript of [
    join(ROOT, 'scripts', 'cli', 'package-unix.sh'),
    join(ROOT, 'scripts', 'cli', 'package-windows.ps1'),
  ]) {
    const content = readFileSync(packageScript, 'utf8');
    assert.match(content, /THIRD_PARTY_NOTICES\.md/,
      `${packageScript} must package third-party notices`);
    assert.match(content, /models-dev\.LICENSE\.txt/,
      `${packageScript} must package the models.dev license`);
    assert.match(content, /models-dev\.provenance\.json/,
      `${packageScript} must package models.dev provenance`);
  }
}

function checkAssets() {
  for (const path of [
    SNAPSHOT_PATH,
    PROVENANCE_PATH,
    LICENSE_PATH,
    PROVIDER_OVERLAY_PATH,
    NOTICE_PATH,
  ]) {
    assert(existsSync(path), `required models.dev release asset is missing: ${path}`);
  }
  const snapshotText = readFileSync(SNAPSHOT_PATH, 'utf8');
  const licenseText = readFileSync(LICENSE_PATH, 'utf8');
  const snapshot = JSON.parse(snapshotText);
  const provenance = readJson(PROVENANCE_PATH);
  const overlay = readProviderOverlay();
  validateProvenance(provenance, snapshotText, licenseText);
  validateBundledSnapshot(snapshot, provenance, overlay);
  validateReleaseDelivery();
  console.log('models.dev snapshot, provenance, license, and release delivery are valid.');
}

async function fetchBytes(url, headers = {}) {
  let response;
  try {
    response = await fetch(url, { headers });
  } catch (error) {
    throw new Error(`Failed to fetch ${url}: ${error.message || String(error)}`);
  }
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: HTTP ${response.status}`);
  }
  return {
    bytes: Buffer.from(await response.arrayBuffer()),
    etag: response.headers.get('etag'),
  };
}

async function resolveRepositoryRevision(explicitRevision) {
  if (explicitRevision) {
    assert.match(explicitRevision, /^[a-f0-9]{40}$/,
      '--repository-revision must be a full lowercase commit SHA');
    return explicitRevision;
  }
  const response = await fetch(
    `https://api.github.com/repos/anomalyco/models.dev/commits/${REPOSITORY_BRANCH}`,
    {
      headers: {
        Accept: 'application/vnd.github+json',
        'User-Agent': 'OpenBitFun-models-dev-updater',
      },
    },
  );
  if (!response.ok) {
    throw new Error(`Failed to resolve models.dev revision: HTTP ${response.status}`);
  }
  const document = await response.json();
  assert.match(document.sha || '', /^[a-f0-9]{40}$/,
    'GitHub returned an invalid models.dev revision');
  return document.sha;
}

async function updateAssets(args) {
  const provenance = readJson(PROVENANCE_PATH);
  const sourceFile = optionValue(args, '--source-file');
  const explicitRevision = optionValue(args, '--repository-revision');
  const revision = await resolveRepositoryRevision(explicitRevision);
  let sourceBytes;
  let etag = optionValue(args, '--etag') || null;
  if (sourceFile) {
    sourceBytes = readFileSync(resolve(sourceFile));
  } else {
    const fetched = await fetchBytes(API_URL);
    sourceBytes = fetched.bytes;
    etag = fetched.etag;
  }

  const licenseFile = optionValue(args, '--license-file');
  const upstreamLicense = licenseFile
    ? { bytes: readFileSync(resolve(licenseFile)) }
    : await fetchBytes(
      `https://raw.githubusercontent.com/anomalyco/models.dev/${revision}/LICENSE`,
    );
  const localLicense = readFileSync(LICENSE_PATH);
  assert.deepEqual(upstreamLicense.bytes, localLicense,
    'upstream models.dev license changed; review and update the preserved license before regenerating');

  const source = JSON.parse(sourceBytes.toString('utf8'));
  const overlay = readProviderOverlay();
  const snapshot = buildBundledSnapshot(source, overlay, provenance);
  const snapshotText = serializedJson(snapshot);
  provenance.artifact.sha256 = sha256(snapshotText);
  provenance.artifact.bytes = Buffer.byteLength(snapshotText);
  provenance.source.api_sha256 = sha256(sourceBytes);
  provenance.source.api_bytes = sourceBytes.length;
  provenance.source.api_etag = etag;
  provenance.source.retrieved_at = new Date().toISOString();
  provenance.source.repository_revision = revision;
  provenance.license.sha256 = sha256(localLicense);
  provenance.transformation.included_provider_ids =
    bundledModelsDevProviderIds(overlay, provenance);
  provenance.transformation.included_provider_model_counts = snapshotModelCounts(snapshot);

  writeFileSync(SNAPSHOT_PATH, snapshotText, 'utf8');
  writeFileSync(PROVENANCE_PATH, serializedJson(provenance), 'utf8');
  checkAssets();
  console.log(`Updated bundled models.dev snapshot from revision ${revision}.`);
}

const args = process.argv.slice(2);
if (args.includes('--help')) {
  console.log(`Usage:
  pnpm run models-dev:update
  pnpm run models-dev:update -- --source-file <api.json> --license-file <LICENSE> --repository-revision <sha> [--etag <etag>]
  pnpm run models-dev:check`);
} else if (args.includes('--check')) {
  checkAssets();
} else {
  updateAssets(args).catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
