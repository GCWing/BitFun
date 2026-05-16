#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const ROOT_DIR = path.resolve(__dirname, '..');
const DEFAULT_ASSETS_DIR = path.join(ROOT_DIR, 'dist', 'assets');
const DEFAULT_INDEX_HTML = path.join(ROOT_DIR, 'dist', 'index.html');

function parseArgs(argv) {
  const args = {
    assetsDir: DEFAULT_ASSETS_DIR,
    indexHtml: DEFAULT_INDEX_HTML,
    json: false,
    top: 30,
  };

  for (const arg of argv) {
    if (arg === '--json') {
      args.json = true;
    } else if (arg.startsWith('--assets-dir=')) {
      args.assetsDir = path.resolve(ROOT_DIR, arg.slice('--assets-dir='.length));
    } else if (arg.startsWith('--index-html=')) {
      args.indexHtml = path.resolve(ROOT_DIR, arg.slice('--index-html='.length));
    } else if (arg.startsWith('--top=')) {
      const value = Number(arg.slice('--top='.length));
      if (Number.isInteger(value) && value > 0) {
        args.top = value;
      }
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return args;
}

function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

function getEntryAssets(indexHtmlPath) {
  if (!fs.existsSync(indexHtmlPath)) {
    return new Set();
  }

  const html = fs.readFileSync(indexHtmlPath, 'utf8');
  const entries = new Set();
  const assetPattern = /["']\/assets\/([^"']+\.(?:js|css))["']/g;
  let match;
  while ((match = assetPattern.exec(html)) !== null) {
    entries.add(match[1]);
  }
  return entries;
}

function collectAssets(assetsDir, indexHtmlPath) {
  if (!fs.existsSync(assetsDir)) {
    throw new Error(`Assets directory does not exist: ${assetsDir}`);
  }

  const entryAssets = getEntryAssets(indexHtmlPath);
  return fs.readdirSync(assetsDir)
    .filter((name) => /\.(js|css)$/.test(name))
    .map((name) => {
      const filePath = path.join(assetsDir, name);
      const bytes = fs.readFileSync(filePath);
      return {
        name,
        type: path.extname(name).slice(1),
        rawBytes: bytes.length,
        gzipBytes: zlib.gzipSync(bytes).length,
        entry: entryAssets.has(name),
      };
    })
    .sort((a, b) => b.rawBytes - a.rawBytes || a.name.localeCompare(b.name));
}

function summarize(rows) {
  const js = rows.filter((row) => row.type === 'js');
  const css = rows.filter((row) => row.type === 'css');
  const totals = rows.reduce((acc, row) => {
    acc.rawBytes += row.rawBytes;
    acc.gzipBytes += row.gzipBytes;
    return acc;
  }, { rawBytes: 0, gzipBytes: 0 });

  return {
    assetCount: rows.length,
    jsCount: js.length,
    cssCount: css.length,
    rawBytes: totals.rawBytes,
    gzipBytes: totals.gzipBytes,
    largestEntry: rows.find((row) => row.entry) || null,
  };
}

function printReport(rows, args) {
  const summary = summarize(rows);

  console.log('Web bundle size report');
  console.log(`Assets dir: ${path.relative(ROOT_DIR, args.assetsDir) || '.'}`);
  console.log(`Assets: ${summary.assetCount} (${summary.jsCount} js, ${summary.cssCount} css)`);
  console.log(`Total raw: ${formatBytes(summary.rawBytes)}`);
  console.log(`Total gzip: ${formatBytes(summary.gzipBytes)}`);
  if (summary.largestEntry) {
    console.log(`Largest entry asset: ${summary.largestEntry.name} (${formatBytes(summary.largestEntry.rawBytes)}, gzip ${formatBytes(summary.largestEntry.gzipBytes)})`);
  }
  console.log('');
  console.log(`Top ${Math.min(args.top, rows.length)} JS/CSS assets by raw size:`);
  console.log('entry type raw gzip name');
  for (const row of rows.slice(0, args.top)) {
    console.log(`${row.entry ? '*' : '-'} ${row.type.padEnd(3)} ${formatBytes(row.rawBytes).padStart(10)} ${formatBytes(row.gzipBytes).padStart(10)} ${row.name}`);
  }
}

function main() {
  try {
    const args = parseArgs(process.argv.slice(2));
    const rows = collectAssets(args.assetsDir, args.indexHtml);

    if (args.json) {
      console.log(JSON.stringify({
        assetsDir: path.relative(ROOT_DIR, args.assetsDir),
        indexHtml: path.relative(ROOT_DIR, args.indexHtml),
        summary: summarize(rows),
        assets: rows,
      }, null, 2));
      return;
    }

    printReport(rows, args);
  } catch (error) {
    console.error(`[report-web-bundle-size] ${error.message}`);
    process.exitCode = 1;
  }
}

main();
