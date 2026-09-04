import react from '@vitejs/plugin-react';
import path from 'path';
import fs from 'fs';
import { build, type Plugin } from 'vite';

const VIRTUAL_MODULE_ID = 'virtual:openbitfun-canvas-runtime-bundle';
const RESOLVED_VIRTUAL_MODULE_ID = `\0${VIRTUAL_MODULE_ID}`;

interface RuntimeBundleCache {
  key: string;
  code: string;
}

export function openbitfunCanvasRuntimeBundlePlugin(): Plugin {
  let cache: RuntimeBundleCache | null = null;
  const webUiRoot = path.resolve(__dirname);
  const runtimeRoot = path.resolve(webUiRoot, 'src/tools/openbitfun-canvas/runtime');
  const entry = path.resolve(runtimeRoot, 'entry.tsx');

  function collectRuntimeFiles(dir: string): string[] {
    return fs
      .readdirSync(dir, { withFileTypes: true })
      .flatMap(item => {
        const itemPath = path.resolve(dir, item.name);
        return item.isDirectory() ? collectRuntimeFiles(itemPath) : [itemPath];
      });
  }

  function runtimeBundleKey(): string {
    const files = collectRuntimeFiles(runtimeRoot)
      .map(filePath => {
        const stat = fs.statSync(filePath);
        return `${filePath}:${stat.mtimeMs}:${stat.size}`;
      })
      .sort()
      .join('|');
    return `${entry}:${process.env.NODE_ENV ?? ''}:${files}`;
  }

  async function loadBundle(): Promise<string> {
    const key = runtimeBundleKey();
    if (cache?.key === key) return cache.code;

    const output = await build({
      configFile: false,
      root: webUiRoot,
      publicDir: false,
      logLevel: 'silent',
      plugins: [react({ jsxRuntime: 'classic' })],
      define: {
        'process.env.NODE_ENV': JSON.stringify('production'),
      },
      build: {
        write: false,
        cssCodeSplit: false,
        minify: true,
        sourcemap: 'inline',
        lib: {
          entry,
          name: 'OpenBitFunCanvasRuntimeAdapters',
          formats: ['iife'],
        },
        rollupOptions: {
          external: ['react', 'react-dom'],
          output: {
            globals: {
              react: 'React',
              'react-dom': 'ReactDOM',
            },
          },
        },
      },
    });

    const outputs = Array.isArray(output) ? output.flatMap(item => item.output) : output.output;
    const js = outputs.find(item => item.type === 'chunk')?.code ?? '';
    const css = outputs
      .filter(item => item.type === 'asset' && item.fileName.endsWith('.css'))
      .map(item => String(item.source))
      .join('\n');

    const code = [
      `export const openbitfunCanvasRuntimeBundle = ${JSON.stringify({ js, css })};`,
      'export default openbitfunCanvasRuntimeBundle;',
    ].join('\n');
    cache = { key, code };
    return code;
  }

  return {
    name: 'openbitfun-canvas-runtime-bundle',
    enforce: 'pre',
    watchChange(id) {
      if (path.resolve(id).startsWith(runtimeRoot)) {
        cache = null;
      }
    },
    handleHotUpdate(context) {
      if (!path.resolve(context.file).startsWith(runtimeRoot)) return undefined;
      cache = null;
      const module = context.server.moduleGraph.getModuleById(RESOLVED_VIRTUAL_MODULE_ID);
      if (module) {
        context.server.moduleGraph.invalidateModule(module);
        return [module];
      }
      return undefined;
    },
    resolveId(id) {
      if (id === VIRTUAL_MODULE_ID) return RESOLVED_VIRTUAL_MODULE_ID;
      return null;
    },
    async load(id) {
      if (id !== RESOLVED_VIRTUAL_MODULE_ID) return null;
      return loadBundle();
    },
  };
}
