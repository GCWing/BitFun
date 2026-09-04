import { readFileSync, realpathSync, statSync } from 'node:fs';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';

import { canonicalJson, sha256 } from './canonical-json.mjs';

const BINARY_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const BUNDLE_ID = /^[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$/;
const STABLE_ID = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const WINDOWS_RESERVED_BASE = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i;
const ROOT_FIELDS = new Set([
  '$schema',
  'schemaVersion',
  'productId',
  'dataNamespace',
  'localeRoot',
  'members',
]);
const MEMBERS_FIELDS = new Set(['desktop', 'cli']);
const COMMON_MEMBER_FIELDS = new Set(['displayNameKey', 'binaryName']);
const DESKTOP_MEMBER_FIELDS = new Set([...COMMON_MEMBER_FIELDS, 'bundleId']);

export class ProductDefinitionError extends Error {
  constructor(code, message, action) {
    super(`${code}: ${message}\nAction: ${action}`);
    this.name = 'ProductDefinitionError';
    this.code = code;
    this.action = action;
  }
}

function fail(code, message, action) {
  throw new ProductDefinitionError(code, message, action);
}

export function parseJsonc(source, sourceName = 'product.jsonc') {
  let output = '';
  let inString = false;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;
  for (let index = 0; index < source.length; index += 1) {
    const current = source[index];
    const next = source[index + 1];
    if (lineComment) {
      output += current === '\n' || current === '\r' ? current : ' ';
      if (current === '\n' || current === '\r') lineComment = false;
    } else if (blockComment) {
      if (current === '*' && next === '/') {
        output += '  ';
        index += 1;
        blockComment = false;
      } else {
        output += current === '\n' || current === '\r' ? current : ' ';
      }
    } else if (inString) {
      output += current;
      if (escaped) escaped = false;
      else if (current === '\\') escaped = true;
      else if (current === '"') inString = false;
    } else if (current === '"') {
      inString = true;
      output += current;
    } else if (current === '/' && next === '/') {
      output += '  ';
      index += 1;
      lineComment = true;
    } else if (current === '/' && next === '*') {
      output += '  ';
      index += 1;
      blockComment = true;
    } else {
      output += current;
    }
  }
  if (blockComment) fail('invalid_jsonc', `${sourceName} has an unterminated block comment.`, 'Close the block comment.');
  let withoutTrailingCommas = '';
  inString = false;
  escaped = false;
  for (let index = 0; index < output.length; index += 1) {
    const current = output[index];
    if (inString) {
      withoutTrailingCommas += current;
      if (escaped) escaped = false;
      else if (current === '\\') escaped = true;
      else if (current === '"') inString = false;
      continue;
    }
    if (current === '"') {
      inString = true;
      withoutTrailingCommas += current;
      continue;
    }
    if (current === ',') {
      let next = index + 1;
      while (next < output.length && /\s/.test(output[next])) next += 1;
      if (output[next] === '}' || output[next] === ']') continue;
    }
    withoutTrailingCommas += current;
  }
  try {
    return JSON.parse(withoutTrailingCommas);
  } catch (error) {
    fail('invalid_jsonc', `${sourceName} is not valid JSONC: ${error.message}`, 'Correct the JSON syntax.');
  }
}

function requireObject(value, owner) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    fail('invalid_field_type', `${owner} must be an object.`, `Set ${owner} to a JSON object.`);
  }
  return value;
}

function rejectUnknownFields(value, allowed, owner) {
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) fail('unknown_field', `${owner}.${key} is not supported in schema version 1.`, `Remove ${owner}.${key}.`);
  }
}

function requiredString(value, owner) {
  if (typeof value !== 'string' || value.trim() === '') {
    fail('missing_required_field', `${owner} must be a non-empty string.`, `Set ${owner}.`);
  }
  return value;
}

function binaryName(value, owner) {
  const result = requiredString(value, owner);
  if (!BINARY_NAME.test(result) || result.endsWith('.') || WINDOWS_RESERVED_BASE.test(result)) {
    fail('invalid_binary_name', `${owner} is not a safe executable name.`, `Use letters, digits, dot, dash, or underscore for ${owner}.`);
  }
  return result;
}

function bundleId(value, owner) {
  const result = requiredString(value, owner);
  if (!BUNDLE_ID.test(result)) {
    fail('invalid_bundle_id', `${owner} is not a reverse-domain identifier.`, `Set ${owner} to a value such as com.example.product.`);
  }
  return result;
}

function stableId(value, owner) {
  const result = requiredString(value, owner);
  if (result.length > 63 || !STABLE_ID.test(result) || WINDOWS_RESERVED_BASE.test(result)) {
    fail(
      'invalid_stable_id',
      `${owner} is not a stable lowercase identifier.`,
      `Use lowercase letters, digits, and single dashes for ${owner}.`,
    );
  }
  return result;
}

function inside(root, candidate) {
  const path = relative(root, candidate);
  return path === '' || (!path.startsWith(`..${sep}`) && path !== '..' && !isAbsolute(path));
}

function ownedDirectory(definitionDir, declaredPath) {
  const lexical = resolve(definitionDir, requiredString(declaredPath, 'localeRoot'));
  if (!inside(definitionDir, lexical)) {
    fail('resource_path_escape', 'localeRoot escapes the product definition directory.', 'Keep product locales beside the definition.');
  }
  let canonical;
  try {
    canonical = realpathSync.native(lexical);
  } catch {
    fail('resource_not_found', `Locale directory does not exist: ${lexical}`, 'Create the locale directory or correct localeRoot.');
  }
  if (!inside(definitionDir, canonical)) {
    fail('resource_path_escape', 'localeRoot resolves outside the product definition directory.', 'Remove the escaping link.');
  }
  if (!statSync(canonical).isDirectory()) fail('invalid_resource_type', 'localeRoot is not a directory.', 'Point it to a directory.');
  return canonical;
}

function ownedLocaleFile(localeRoot, locale) {
  const lexical = join(localeRoot, `${locale}.json`);
  let canonical;
  try {
    canonical = realpathSync.native(lexical);
  } catch {
    fail('invalid_locale_resource', `Cannot read ${lexical}.`, 'Add valid JSON for every supported locale.');
  }
  if (!inside(localeRoot, canonical) || !statSync(canonical).isFile()) {
    fail('resource_path_escape', `${locale}.json is not an owned locale file.`, 'Remove the escaping link.');
  }
  return canonical;
}

function validateMember(raw, member) {
  const owner = `members.${member}`;
  const value = requireObject(raw, owner);
  rejectUnknownFields(value, member === 'desktop' ? DESKTOP_MEMBER_FIELDS : COMMON_MEMBER_FIELDS, owner);
  const result = {
    displayNameKey: requiredString(value.displayNameKey, `${owner}.displayNameKey`),
    binaryName: binaryName(value.binaryName, `${owner}.binaryName`),
  };
  if (member === 'desktop') result.bundleId = bundleId(value.bundleId, `${owner}.bundleId`);
  return result;
}

function loadProductNames(rootDir, localeRoot, displayNameKeys) {
  const contract = JSON.parse(readFileSync(join(rootDir, 'src', 'shared', 'i18n', 'contract', 'locales.json'), 'utf8'));
  const resources = {};
  let expectedKeys;
  for (const locale of contract.locales.map(({ id }) => id)) {
    const path = ownedLocaleFile(localeRoot, locale);
    let terms;
    try {
      terms = requireObject(JSON.parse(readFileSync(path, 'utf8')), `locale ${locale}`);
    } catch (error) {
      if (error instanceof ProductDefinitionError) throw error;
      fail('invalid_locale_resource', `Cannot read ${path}: ${error.message}`, 'Add valid JSON for every supported locale.');
    }
    const keys = Object.keys(terms).sort();
    if (!expectedKeys) expectedKeys = keys;
    else if (canonicalJson(keys) !== canonicalJson(expectedKeys)) {
      fail('locale_key_mismatch', `${locale} has a different product-name key set.`, 'Use the same keys in every product locale.');
    }
    for (const [key, value] of Object.entries(terms)) requiredString(value, `locale ${locale}.${key}`);
    resources[locale] = terms;
  }
  for (const key of displayNameKeys) {
    if (!expectedKeys?.includes(key)) fail('missing_product_name_key', `${key} is absent from product locales.`, `Add ${key} to every locale.`);
  }
  return {
    resources,
    digest: sha256(canonicalJson(resources)),
    defaultLocale: contract.defaultLocale,
    fallbackLocale: contract.fallbackLocale,
  };
}

export function resolveProductDefinition({ rootDir, productConfig, member }) {
  if (!['desktop', 'cli'].includes(member)) fail('invalid_member', `Unsupported product member: ${member}`, 'Use desktop or cli.');
  const canonicalRoot = realpathSync.native(resolve(rootDir));
  const defaultPath = realpathSync.native(join(canonicalRoot, 'products', 'openbitfun', 'product.jsonc'));
  const selectedPath = resolve(productConfig || defaultPath);
  let sourcePath;
  try {
    sourcePath = realpathSync.native(selectedPath);
  } catch {
    fail('definition_not_found', `Product definition does not exist: ${selectedPath}`, 'Pass an existing JSONC file to --product-config.');
  }
  const sourceBytes = readFileSync(sourcePath);
  const raw = requireObject(parseJsonc(sourceBytes.toString('utf8'), sourcePath), 'product definition');
  rejectUnknownFields(raw, ROOT_FIELDS, 'product definition');
  if (raw.schemaVersion !== 1) fail('unsupported_schema_version', `schemaVersion ${raw.schemaVersion} is unsupported.`, 'Set schemaVersion to 1.');

  const productId = stableId(raw.productId, 'productId');
  const dataNamespace = stableId(raw.dataNamespace, 'dataNamespace');

  const definitionDir = realpathSync.native(resolve(sourcePath, '..'));
  const localeRoot = ownedDirectory(definitionDir, raw.localeRoot);

  const members = requireObject(raw.members, 'members');
  rejectUnknownFields(members, MEMBERS_FIELDS, 'members');
  const normalizedMembers = {
    desktop: validateMember(members.desktop, 'desktop'),
    cli: validateMember(members.cli, 'cli'),
  };
  const locales = loadProductNames(
    canonicalRoot,
    localeRoot,
    [normalizedMembers.desktop.displayNameKey, normalizedMembers.cli.displayNameKey],
  );
  const selected = normalizedMembers[member];
  const assemblyContent = {
    schemaVersion: 1,
    sourceDigest: sha256(sourceBytes),
    productId,
    dataNamespace,
    member,
    displayNameKey: selected.displayNameKey,
    binaryName: selected.binaryName,
    localeDigest: locales.digest,
    defaultLocale: locales.defaultLocale,
    fallbackLocale: locales.fallbackLocale,
  };
  if (selected.bundleId) assemblyContent.bundleId = selected.bundleId;
  const assembly = { ...assemblyContent, assemblyDigest: sha256(canonicalJson(assemblyContent)) };
  return {
    rootDir: canonicalRoot,
    sourcePath,
    productNames: Object.fromEntries(
      Object.entries(locales.resources).map(([locale, terms]) => [locale, terms[selected.displayNameKey]]),
    ),
    assembly,
    outputDir: join(canonicalRoot, 'target', 'product-assembly', assembly.assemblyDigest, member),
    isDefaultProduct: sourcePath === defaultPath,
  };
}
