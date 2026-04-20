/**
 * Icon Design System — Worker
 * Handles file I/O, icon library management, SVG optimization, and export.
 */

const fs = require('fs/promises');
const path = require('path');

const APP_DATA_DIR = process.env.BITFUN_APP_DATA || process.cwd();

const ICONS_DIR = path.join(APP_DATA_DIR, 'icons');
const LIBRARY_FILE = path.join(APP_DATA_DIR, 'library.json');
const TOKENS_FILE = path.join(APP_DATA_DIR, 'design-tokens.json');

// ── Helpers ──────────────────────────────────────────────────────────────────

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function readJsonFile(filePath, defaultValue) {
  try {
    const content = await fs.readFile(filePath, 'utf8');
    return JSON.parse(content);
  } catch {
    return defaultValue;
  }
}

async function writeJsonFile(filePath, data) {
  await fs.writeFile(filePath, JSON.stringify(data, null, 2), 'utf8');
}

function generateId() {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

// ── Icon Library ──────────────────────────────────────────────────────────────

async function loadLibrary() {
  return readJsonFile(LIBRARY_FILE, { icons: [], updatedAt: 0 });
}

async function saveLibrary(library) {
  library.updatedAt = Date.now();
  await ensureDir(APP_DATA_DIR);
  await writeJsonFile(LIBRARY_FILE, library);
}

// ── Exports ───────────────────────────────────────────────────────────────────

/**
 * Get design tokens (style definition).
 */
exports.getTokens = async () => {
  return readJsonFile(TOKENS_FILE, getDefaultTokens());
};

/**
 * Save design tokens.
 */
exports.saveTokens = async ({ tokens }) => {
  await ensureDir(APP_DATA_DIR);
  await writeJsonFile(TOKENS_FILE, { ...tokens, updatedAt: Date.now() });
  return { ok: true };
};

/**
 * List all icons in the library.
 */
exports.listIcons = async () => {
  const library = await loadLibrary();
  return library.icons || [];
};

/**
 * Get a single icon by id (includes SVG source).
 */
exports.getIcon = async ({ id }) => {
  const library = await loadLibrary();
  const meta = (library.icons || []).find(icon => icon.id === id);
  if (!meta) throw new Error(`Icon not found: ${id}`);

  const svgPath = path.join(ICONS_DIR, id, 'base.svg');
  let svg = '';
  try {
    svg = await fs.readFile(svgPath, 'utf8');
  } catch {
    svg = meta.svgSource || '';
  }
  return { ...meta, svgSource: svg };
};

/**
 * Save a new or updated icon.
 */
exports.saveIcon = async ({ id, name, tags, category, svgSource }) => {
  const iconId = id || generateId();
  await ensureDir(path.join(ICONS_DIR, iconId));

  // Save SVG file
  const svgPath = path.join(ICONS_DIR, iconId, 'base.svg');
  await fs.writeFile(svgPath, svgSource || '', 'utf8');

  // Update library index
  const library = await loadLibrary();
  const icons = library.icons || [];
  const existing = icons.findIndex(i => i.id === iconId);
  const meta = {
    id: iconId,
    name: name || 'Untitled',
    tags: tags || [],
    category: category || 'general',
    createdAt: existing >= 0 ? icons[existing].createdAt : Date.now(),
    updatedAt: Date.now(),
  };
  if (existing >= 0) {
    icons[existing] = meta;
  } else {
    icons.push(meta);
  }
  library.icons = icons;
  await saveLibrary(library);

  return meta;
};

/**
 * Delete an icon.
 */
exports.deleteIcon = async ({ id }) => {
  const library = await loadLibrary();
  library.icons = (library.icons || []).filter(i => i.id !== id);
  await saveLibrary(library);

  // Remove icon directory
  const iconDir = path.join(ICONS_DIR, id);
  try {
    await fs.rm(iconDir, { recursive: true, force: true });
  } catch { /* ignore */ }

  return { ok: true };
};

/**
 * Export all icons as a ZIP-style directory structure.
 * Emits progress events via rpcEmit.
 */
exports.exportIcons = async ({ targetDir, format }) => {
  const library = await loadLibrary();
  const icons = library.icons || [];

  await ensureDir(targetDir);

  let done = 0;
  for (const meta of icons) {
    const svgPath = path.join(ICONS_DIR, meta.id, 'base.svg');
    let svg = '';
    try {
      svg = await fs.readFile(svgPath, 'utf8');
    } catch { /* skip */ }

    if (format === 'react') {
      const componentName = toPascalCase(meta.name) + 'Icon';
      const tsx = svgToReactComponent(componentName, svg);
      await fs.writeFile(path.join(targetDir, `${componentName}.tsx`), tsx, 'utf8');
    } else {
      // Default: plain SVG files
      const safeName = meta.name.replace(/[^a-z0-9-_]/gi, '-').toLowerCase();
      await fs.writeFile(path.join(targetDir, `${safeName}.svg`), svg, 'utf8');
    }

    done++;
    if (global.rpcEmit) {
      global.rpcEmit('progress', { done, total: icons.length, name: meta.name });
    }
  }

  return { exported: done, targetDir };
};

/**
 * Export design tokens as a JSON file.
 */
exports.exportTokens = async ({ targetPath }) => {
  const tokens = await readJsonFile(TOKENS_FILE, getDefaultTokens());
  await fs.writeFile(targetPath, JSON.stringify(tokens, null, 2), 'utf8');
  return { ok: true };
};

// ── Utilities ─────────────────────────────────────────────────────────────────

function toPascalCase(str) {
  return str
    .replace(/[^a-z0-9]/gi, ' ')
    .split(' ')
    .filter(Boolean)
    .map(w => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase())
    .join('');
}

function svgToReactComponent(componentName, svgSource) {
  const svgBody = (svgSource || '')
    .replace(/<\?xml[^>]*\?>/g, '')
    .replace(/<!--.*?-->/gs, '')
    .trim();
  return `import React from 'react';

interface ${componentName}Props extends React.SVGProps<SVGSVGElement> {}

export const ${componentName}: React.FC<${componentName}Props> = (props) => (
  ${svgBody.replace(/<svg/, '<svg {...props}')}
);

export default ${componentName};
`;
}

function getDefaultTokens() {
  return {
    version: 1,
    strokeWidth: 1.5,
    cornerRadius: 2,
    gridSize: 24,
    opticalPadding: 1,
    colorMode: 'currentColor',
    sizeVariants: [16, 20, 24, 32, 48],
    styleVariants: ['outlined', 'filled'],
    updatedAt: 0,
  };
}
