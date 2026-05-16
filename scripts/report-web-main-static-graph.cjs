#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const ROOT_DIR = path.resolve(__dirname, '..');
const WEB_SRC_DIR = path.join(ROOT_DIR, 'src', 'web-ui', 'src');
const DEFAULT_ENTRY = path.join(WEB_SRC_DIR, 'main.tsx');
const SOURCE_EXTENSIONS = ['.ts', '.tsx', '.js', '.jsx'];
const HEAVY_PACKAGES = [
  'monaco-editor',
  '@monaco-editor/react',
  '@xterm/xterm',
  '@xterm/addon-fit',
  '@xterm/addon-web-links',
  '@xterm/addon-webgl',
  'mermaid',
  'react-syntax-highlighter',
  'react-markdown',
  'remark-gfm',
  'remark-math',
  'rehype-katex',
  'rehype-raw',
  'rehype-sanitize',
  'katex',
  '@tiptap/core',
  '@tiptap/react',
  '@tiptap/starter-kit',
  '@tiptap/pm',
  'lucide-react',
];

function parseArgs(argv) {
  const args = {
    entry: DEFAULT_ENTRY,
    json: false,
    top: 40,
    assertNoDirectImports: [],
    assertExternalUnreachable: [],
    assertLocalPrefixUnreachable: [],
  };

  for (const arg of argv) {
    if (arg === '--json') {
      args.json = true;
    } else if (arg.startsWith('--entry=')) {
      args.entry = resolveRepoPath(arg.slice('--entry='.length));
    } else if (arg.startsWith('--top=')) {
      const value = Number(arg.slice('--top='.length));
      if (Number.isInteger(value) && value > 0) {
        args.top = value;
      }
    } else if (arg.startsWith('--assert-no-direct-import=')) {
      const value = arg.slice('--assert-no-direct-import='.length);
      const separatorIndex = value.lastIndexOf(':');
      if (separatorIndex <= 0 || separatorIndex === value.length - 1) {
        throw new Error(`Invalid --assert-no-direct-import value: ${value}`);
      }
      args.assertNoDirectImports.push({
        file: resolveRepoPath(value.slice(0, separatorIndex)),
        specifier: value.slice(separatorIndex + 1),
      });
    } else if (arg.startsWith('--assert-external-unreachable=')) {
      args.assertExternalUnreachable.push(arg.slice('--assert-external-unreachable='.length));
    } else if (arg.startsWith('--assert-local-prefix-unreachable=')) {
      args.assertLocalPrefixUnreachable.push(resolveRepoPath(arg.slice('--assert-local-prefix-unreachable='.length)));
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return args;
}

function resolveRepoPath(input) {
  return path.isAbsolute(input) ? path.resolve(input) : path.resolve(ROOT_DIR, input);
}

function toRepoPath(filePath) {
  return path.relative(ROOT_DIR, filePath).replace(/\\/g, '/');
}

function packageName(specifier) {
  if (specifier.startsWith('@')) {
    const [scope, name] = specifier.split('/');
    return name ? `${scope}/${name}` : specifier;
  }
  return specifier.split('/')[0];
}

function importsOf(filePath) {
  const source = fs.readFileSync(filePath, 'utf8');
  const imports = [];
  const staticImportPattern = /(?:^|\n)\s*import\s+(?!type\b)(?:[^'";]*?\s+from\s+)?['"]([^'"]+)['"]/g;
  const exportFromPattern = /(?:^|\n)\s*export\s+(?!type\b)(?:\*|\{[^}]*\})\s+from\s+['"]([^'"]+)['"]/g;
  let match;

  while ((match = staticImportPattern.exec(source)) !== null) {
    imports.push({ specifier: match[1], kind: 'import' });
  }

  while ((match = exportFromPattern.exec(source)) !== null) {
    imports.push({ specifier: match[1], kind: 'export' });
  }

  return imports;
}

function resolveSpecifier(fromFile, specifier) {
  let candidate;

  if (specifier.startsWith('@/')) {
    candidate = path.join(WEB_SRC_DIR, specifier.slice(2));
  } else if (specifier === '@components') {
    candidate = path.join(WEB_SRC_DIR, 'component-library', 'components');
  } else if (specifier.startsWith('@components/')) {
    candidate = path.join(WEB_SRC_DIR, 'component-library', 'components', specifier.slice('@components/'.length));
  } else if (specifier.startsWith('.')) {
    candidate = path.resolve(path.dirname(fromFile), specifier);
  } else {
    return { external: packageName(specifier) };
  }

  const candidates = [candidate];
  for (const extension of SOURCE_EXTENSIONS) {
    candidates.push(`${candidate}${extension}`);
  }
  for (const extension of SOURCE_EXTENSIONS) {
    candidates.push(path.join(candidate, `index${extension}`));
  }

  for (const item of candidates) {
    if (fs.existsSync(item) && fs.statSync(item).isFile()) {
      return { file: path.resolve(item) };
    }
  }

  return { unresolved: candidate };
}

function buildGraph(entry) {
  if (!fs.existsSync(entry)) {
    throw new Error(`Entry file does not exist: ${entry}`);
  }

  const seen = new Set();
  const externalImporters = new Map();
  const unresolved = [];
  const edges = new Map();

  function walk(filePath) {
    const resolvedFile = path.resolve(filePath);
    if (seen.has(resolvedFile)) {
      return;
    }
    seen.add(resolvedFile);

    const imports = importsOf(resolvedFile);
    for (const item of imports) {
      const resolved = resolveSpecifier(resolvedFile, item.specifier);

      if (resolved.file) {
        const list = edges.get(resolvedFile) || [];
        list.push(resolved.file);
        edges.set(resolvedFile, list);
        walk(resolved.file);
      } else if (resolved.external) {
        const list = externalImporters.get(resolved.external) || [];
        list.push(resolvedFile);
        externalImporters.set(resolved.external, list);
      } else {
        unresolved.push({
          importer: resolvedFile,
          specifier: item.specifier,
          resolvedTo: resolved.unresolved,
        });
      }
    }
  }

  walk(entry);

  const localModules = Array.from(seen).sort();
  const largestLocalModules = localModules
    .map((file) => ({
      file,
      bytes: fs.statSync(file).size,
    }))
    .sort((a, b) => b.bytes - a.bytes || a.file.localeCompare(b.file));

  return {
    entry,
    localModules,
    largestLocalModules,
    externalImporters,
    unresolved,
    edges,
  };
}

function runAssertions(args, graph) {
  const failures = [];

  for (const assertion of args.assertNoDirectImports) {
    const imports = importsOf(assertion.file);
    if (imports.some((item) => item.specifier === assertion.specifier)) {
      failures.push(`${toRepoPath(assertion.file)} still directly imports ${assertion.specifier}`);
    }
  }

  for (const pkg of args.assertExternalUnreachable) {
    if (graph.externalImporters.has(pkg)) {
      failures.push(`external package is still reachable from entry: ${pkg}`);
    }
  }

  for (const prefix of args.assertLocalPrefixUnreachable) {
    const normalizedPrefix = `${path.resolve(prefix)}${path.sep}`;
    const matched = graph.localModules.filter((file) => file === path.resolve(prefix) || file.startsWith(normalizedPrefix));
    if (matched.length > 0) {
      failures.push(`local prefix is still reachable from entry: ${toRepoPath(prefix)} (${matched.length} modules)`);
    }
  }

  if (failures.length > 0) {
    for (const failure of failures) {
      console.error(`[report-web-main-static-graph] assertion failed: ${failure}`);
    }
    process.exitCode = 1;
  }
}

function serializableGraph(graph, top) {
  const heavyExternals = {};
  for (const pkg of HEAVY_PACKAGES) {
    const importers = graph.externalImporters.get(pkg);
    if (!importers) {
      continue;
    }
    heavyExternals[pkg] = Array.from(new Set(importers)).map(toRepoPath);
  }

  return {
    entry: toRepoPath(graph.entry),
    localModuleCount: graph.localModules.length,
    unresolved: graph.unresolved.map((item) => ({
      importer: toRepoPath(item.importer),
      specifier: item.specifier,
      resolvedTo: toRepoPath(item.resolvedTo),
    })),
    heavyExternals,
    largestLocalModules: graph.largestLocalModules.slice(0, top).map((item) => ({
      file: toRepoPath(item.file),
      bytes: item.bytes,
    })),
  };
}

function formatBytes(bytes) {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KiB`;
}

function printReport(graph, args) {
  const data = serializableGraph(graph, args.top);

  console.log('Web main static graph report');
  console.log(`Entry: ${data.entry}`);
  console.log(`Local modules reachable: ${data.localModuleCount}`);
  console.log(`Unresolved relative/alias imports: ${data.unresolved.length}`);
  console.log('');
  console.log('Heavy externals reachable:');

  const heavyEntries = Object.entries(data.heavyExternals);
  if (heavyEntries.length === 0) {
    console.log('- none');
  } else {
    for (const [pkg, importers] of heavyEntries) {
      console.log(`- ${pkg}: ${importers.length} import site(s)`);
      for (const importer of importers.slice(0, 8)) {
        console.log(`  ${importer}`);
      }
    }
  }

  console.log('');
  console.log(`Largest ${Math.min(args.top, data.largestLocalModules.length)} local modules in static graph:`);
  for (const item of data.largestLocalModules) {
    console.log(`${formatBytes(item.bytes).padStart(10)} ${item.file}`);
  }
}

function main() {
  try {
    const args = parseArgs(process.argv.slice(2));
    const graph = buildGraph(args.entry);

    if (args.json) {
      console.log(JSON.stringify(serializableGraph(graph, args.top), null, 2));
    } else {
      printReport(graph, args);
    }

    runAssertions(args, graph);
  } catch (error) {
    console.error(`[report-web-main-static-graph] ${error.message}`);
    process.exitCode = 1;
  }
}

main();
