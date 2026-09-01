import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url));
const iconographyRoot = path.join(repositoryRoot, 'src/shared/iconography');
const svgRoot = path.join(iconographyRoot, 'svg');
const manifestPath = path.join(iconographyRoot, 'manifest.json');
const outputPath = path.join(
  repositoryRoot,
  'src/web-ui/src/app/icons/generated/iconRegistry.generated.ts',
);
const checkOnly = process.argv.includes('--check');

const fail = (message) => {
  throw new Error(`Iconography manifest: ${message}`);
};

const isRecord = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);

function validateSvgSource(prefix, source) {
  if (typeof source.path !== 'string'
    || source.path.includes('\\')
    || path.posix.normalize(source.path) !== source.path
    || !/^svg\/[a-z0-9]+(?:-[a-z0-9]+)*\.svg$/.test(source.path)) {
    fail(`${prefix}.source.path must be a normalized svg/<kebab-case>.svg path`);
  }
  if (!['bitfun-authored', 'bitfun-reference-redraw'].includes(source.origin)) {
    fail(`${prefix}.source.origin must describe an approved BitFun source`);
  }
  if (!Number.isInteger(source.revision) || source.revision < 1) {
    fail(`${prefix}.source.revision must be a positive integer`);
  }
  if (source.referenceId !== undefined
    && (typeof source.referenceId !== 'string'
      || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(source.referenceId))) {
    fail(`${prefix}.source.referenceId must be kebab-case when present`);
  }
}

function validateManifest(value) {
  if (!isRecord(value) || value.schemaVersion !== 1 || !Array.isArray(value.icons)) {
    fail('expected schemaVersion 1 and an icons array');
  }

  const ids = new Set();
  const exports = new Set();
  const semantics = new Set();
  const ownedSourcePaths = new Set();

  for (const [index, icon] of value.icons.entries()) {
    const prefix = `icons[${index}]`;
    if (!isRecord(icon)) fail(`${prefix} must be an object`);
    if (typeof icon.id !== 'string' || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(icon.id)) {
      fail(`${prefix}.id must be kebab-case`);
    }
    if (typeof icon.exportName !== 'string' || !/^[A-Z][A-Za-z0-9]*Icon$/.test(icon.exportName)) {
      fail(`${prefix}.exportName must be PascalCase and end in Icon`);
    }
    if (typeof icon.semantic !== 'string' || !/^[a-z0-9]+(?:[.-][a-z0-9]+)*$/.test(icon.semantic)) {
      fail(`${prefix}.semantic must use lowercase dotted or hyphenated segments`);
    }
    for (const key of ['category', 'displayName', 'description', 'license']) {
      if (typeof icon[key] !== 'string' || icon[key].trim() === '') {
        fail(`${prefix}.${key} must be a non-empty string`);
      }
    }
    if (!isRecord(icon.source)) {
      fail(`${prefix}.source must be an object`);
    }
    if (icon.source.type === 'lucide') {
      if (typeof icon.source.icon !== 'string' || !/^[A-Z][A-Za-z0-9]*$/.test(icon.source.icon)) {
        fail(`${prefix}.source.icon must be a Lucide PascalCase export`);
      }
    } else if (icon.source.type === 'bitfun-svg') {
      validateSvgSource(prefix, icon.source);
      if (ownedSourcePaths.has(icon.source.path)) {
        fail(`duplicate BitFun SVG source path ${icon.source.path}`);
      }
      ownedSourcePaths.add(icon.source.path);
    } else {
      fail(`${prefix}.source.type must be lucide or bitfun-svg`);
    }
    if (!Array.isArray(icon.tags) || icon.tags.some(tag => typeof tag !== 'string' || tag.trim() === '')) {
      fail(`${prefix}.tags must contain non-empty strings`);
    }
    if (ids.has(icon.id)) fail(`duplicate id ${icon.id}`);
    if (exports.has(icon.exportName)) fail(`duplicate exportName ${icon.exportName}`);
    if (semantics.has(icon.semantic)) fail(`duplicate semantic ${icon.semantic}`);
    ids.add(icon.id);
    exports.add(icon.exportName);
    semantics.add(icon.semantic);
  }

  return value.icons;
}

function parseAttributes(rawAttributes, prefix) {
  const attributes = new Map();
  const attributePattern = /([A-Za-z][A-Za-z0-9:-]*)="([^"]*)"/g;
  let match;
  while ((match = attributePattern.exec(rawAttributes)) !== null) {
    if (attributes.has(match[1])) fail(`${prefix} contains duplicate ${match[1]} attributes`);
    attributes.set(match[1], match[2]);
  }
  const unparsed = rawAttributes.replace(attributePattern, '').trim();
  if (unparsed !== '') fail(`${prefix} contains unsupported attribute syntax`);
  return attributes;
}

function parseOwnedSvg(icon, sourceText) {
  const prefix = icon.source.path;
  const normalized = sourceText.replace(/^\uFEFF/, '').trim();
  if (/<(?:script|style|foreignObject|image|use|text|filter|mask|pattern|linearGradient|radialGradient)\b/i.test(normalized)
    || /\son[a-z]+=/i.test(normalized)
    || /\s(?:href|style)=/i.test(normalized)) {
    fail(`${prefix} contains forbidden active or presentation content`);
  }

  const rootMatch = normalized.match(/^<svg\b([^>]*)>([\s\S]*)<\/svg>$/);
  if (!rootMatch) fail(`${prefix} must contain one root svg element`);
  const rootAttributes = parseAttributes(rootMatch[1], `${prefix} root`);
  const allowedRootAttributes = new Set(['xmlns', 'viewBox', 'fill']);
  for (const name of rootAttributes.keys()) {
    if (!allowedRootAttributes.has(name)) fail(`${prefix} root attribute ${name} is not allowed`);
  }
  if (rootAttributes.get('viewBox') !== '0 0 24 24') {
    fail(`${prefix} must use viewBox="0 0 24 24"`);
  }
  if (rootAttributes.get('fill') !== 'none') {
    fail(`${prefix} root must use fill="none"`);
  }

  const paths = [];
  const pathPattern = /<path\b([^>]*)\/>/g;
  let pathMatch;
  while ((pathMatch = pathPattern.exec(rootMatch[2])) !== null) {
    const attributes = parseAttributes(pathMatch[1], `${prefix} path`);
    const allowedPathAttributes = new Set([
      'd',
      'fill',
      'fill-rule',
      'clip-rule',
      'stroke',
      'stroke-width',
      'stroke-linecap',
      'stroke-linejoin',
    ]);
    for (const name of attributes.keys()) {
      if (!allowedPathAttributes.has(name)) fail(`${prefix} path attribute ${name} is not allowed`);
    }
    const d = attributes.get('d');
    if (!d || !/^[MmLlHhVvCcSsQqTtAaZz0-9eE.,+\-\s]+$/.test(d)) {
      fail(`${prefix} path data is missing or invalid`);
    }
    const fill = attributes.get('fill');
    const stroke = attributes.get('stroke');
    const strokeWidthText = attributes.get('stroke-width');
    const strokeLinecap = attributes.get('stroke-linecap');
    const strokeLinejoin = attributes.get('stroke-linejoin');
    const isFilledPath = fill === 'currentColor' && stroke === undefined;
    const isStrokedPath = fill === 'none' && stroke === 'currentColor';
    if (!isFilledPath && !isStrokedPath) {
      fail(`${prefix} paths must be currentColor fill paths or currentColor stroke paths`);
    }
    let strokeWidth;
    if (isStrokedPath) {
      if (!strokeWidthText || !/^(?:[1-2](?:\.\d+)?|3(?:\.0+)?)$/.test(strokeWidthText)) {
        fail(`${prefix} stroke paths must use a numeric stroke-width from 1 through 3`);
      }
      strokeWidth = Number(strokeWidthText);
      if (strokeLinecap !== undefined && !['butt', 'round', 'square'].includes(strokeLinecap)) {
        fail(`${prefix} has an unsupported stroke-linecap`);
      }
      if (strokeLinejoin !== undefined && !['miter', 'round', 'bevel'].includes(strokeLinejoin)) {
        fail(`${prefix} has an unsupported stroke-linejoin`);
      }
    } else if (strokeWidthText !== undefined || strokeLinecap !== undefined || strokeLinejoin !== undefined) {
      fail(`${prefix} fill paths cannot declare stroke presentation attributes`);
    }
    const fillRule = attributes.get('fill-rule');
    const clipRule = attributes.get('clip-rule');
    if (fillRule !== undefined && !['evenodd', 'nonzero'].includes(fillRule)) {
      fail(`${prefix} has an unsupported fill-rule`);
    }
    if (clipRule !== undefined && !['evenodd', 'nonzero'].includes(clipRule)) {
      fail(`${prefix} has an unsupported clip-rule`);
    }
    paths.push({
      d,
      fill,
      ...(stroke ? { stroke } : {}),
      ...(strokeWidth !== undefined ? { strokeWidth } : {}),
      ...(strokeLinecap ? { strokeLinecap } : {}),
      ...(strokeLinejoin ? { strokeLinejoin } : {}),
      ...(fillRule ? { fillRule } : {}),
      ...(clipRule ? { clipRule } : {}),
    });
  }
  if (paths.length === 0 || rootMatch[2].replace(pathPattern, '').trim() !== '') {
    fail(`${prefix} may contain path elements only`);
  }
  return paths;
}

async function loadOwnedSvgSources(icons) {
  const entries = await Promise.all(icons
    .filter(icon => icon.source.type === 'bitfun-svg')
    .map(async (icon) => {
      const absolutePath = path.resolve(iconographyRoot, ...icon.source.path.split('/'));
      const relativeToSvgRoot = path.relative(svgRoot, absolutePath);
      if (relativeToSvgRoot.startsWith('..') || path.isAbsolute(relativeToSvgRoot)) {
        fail(`${icon.source.path} escapes the BitFun SVG source root`);
      }
      let sourceText;
      try {
        sourceText = await readFile(absolutePath, 'utf8');
      } catch {
        fail(`missing BitFun SVG source ${icon.source.path}`);
      }
      return [icon.id, parseOwnedSvg(icon, sourceText)];
    }));
  return new Map(entries);
}

function renderGeneratedModule(icons, ownedSvgSources) {
  const lucideImports = [...new Set(icons
    .filter(icon => icon.source.type === 'lucide')
    .map(icon => icon.source.icon))].sort();
  const imports = [];
  if (lucideImports.length > 0) {
    imports.push(`import { ${lucideImports.join(', ')} } from 'lucide-react';`);
    imports.push("import { createBitFunIconComponent } from '../createBitFunIconComponent';");
  }
  if (ownedSvgSources.size > 0) {
    imports.push("import { createBitFunSvgIconComponent } from '../createBitFunSvgIconComponent';");
  }
  imports.push("import type { BitFunIconMetadata } from '../types';");

  const componentExports = icons.map((icon) => {
    if (icon.source.type === 'lucide') {
      return `export const ${icon.exportName} = /* @__PURE__ */ createBitFunIconComponent(\n` +
        `  '${icon.id}',\n` +
        `  ${icon.source.icon},\n` +
        `);`;
    }
    const paths = JSON.stringify(ownedSvgSources.get(icon.id), null, 2)
      .replace(/^/gm, '  ')
      .trimStart();
    return `export const ${icon.exportName} = /* @__PURE__ */ createBitFunSvgIconComponent(\n` +
      `  '${icon.id}',\n` +
      `  ${paths},\n` +
      `);`;
  }).join('\n\n');
  const componentRegistry = icons.map(icon => `  '${icon.id}': ${icon.exportName},`).join('\n');
  const names = icons.map(icon => `  '${icon.id}',`).join('\n');
  const metadata = icons.map(icon => (
    `  '${icon.id}': ${JSON.stringify(icon, null, 2).replace(/^/gm, '  ').trimStart()},`
  )).join('\n');

  return `// This file is generated by scripts/iconography/generate.mjs.\n` +
    `// Do not edit it by hand.\n\n` +
    `${imports.join('\n')}\n\n` +
    `${componentExports}\n\n` +
    `export const bitFunIconComponents = {\n${componentRegistry}\n} as const;\n\n` +
    `export type BitFunIconName = keyof typeof bitFunIconComponents;\n\n` +
    `export const bitFunIconNames = [\n${names}\n] as const satisfies readonly BitFunIconName[];\n\n` +
    `export const bitFunIconMetadata = {\n${metadata}\n} as const satisfies Record<BitFunIconName, BitFunIconMetadata>;\n`;
}

const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
const icons = validateManifest(manifest);
const ownedSvgSources = await loadOwnedSvgSources(icons);
const nextOutput = renderGeneratedModule(icons, ownedSvgSources);

if (checkOnly) {
  let currentOutput;
  try {
    currentOutput = await readFile(outputPath, 'utf8');
  } catch {
    fail(`generated output is missing: ${path.relative(repositoryRoot, outputPath)}`);
  }
  if (currentOutput.replace(/\r\n/g, '\n') !== nextOutput) {
    fail('generated output is stale; run pnpm run iconography:generate');
  }
  process.stdout.write(`Iconography check passed (${icons.length} icons).\n`);
} else {
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, nextOutput, 'utf8');
  process.stdout.write(`Generated ${icons.length} icons (${ownedSvgSources.size} BitFun-owned).\n`);
}
