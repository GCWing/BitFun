import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import viteConfig, {
  createDesignSystemSourceAliases,
  createDevServerResponseHeaders,
} from '../../../vite.config';
import type { SourceWatchPlugin } from '../../../../../design-system/tooling/vite/watch-source.mjs';

describe('design-system Vite integration', () => {
  it('resolves UI package entry points to source only while serving for HMR', () => {
    const serveAliases = createDesignSystemSourceAliases('serve');

    expect(serveAliases).toHaveLength(4);
    expect(serveAliases.map(alias => String(alias.find))).toEqual([
      '/^@bitfun\\/ui\\/flow-chat$/',
      '/^@bitfun\\/ui\\/registry$/',
      '/^@bitfun\\/ui\\/styles\\.css$/',
      '/^@bitfun\\/ui$/',
    ]);
    expect(path.normalize(serveAliases[0].replacement)).toContain(
      path.normalize('design-system/packages/ui/src/flow-chat.ts'),
    );
    expect(path.normalize(serveAliases[3].replacement)).toContain(
      path.normalize('design-system/packages/ui/src/index.ts'),
    );
    expect(createDesignSystemSourceAliases('build')).toEqual([]);
  });

  it('prevents persistent module caching in desktop development webviews', () => {
    expect(createDevServerResponseHeaders()).toEqual({
      'Cache-Control': 'no-store',
    });
  });

  it('watches aliased UI assets outside the application root during development', () => {
    const config = viteConfig({ command: 'serve', mode: 'development' });
    const watcher = config.plugins?.flat(Infinity).find(
      plugin => plugin && typeof plugin === 'object'
        && 'name' in plugin && plugin.name === 'bitfun:watch-ui-source',
    ) as SourceWatchPlugin | undefined;

    expect(watcher).toBeDefined();
    expect(watcher?.apply).toBe('serve');
    const watched: string[] = [];
    watcher?.configureServer({ watcher: { add: directory => watched.push(directory) } });
    expect(watched).toEqual([
      path.dirname(createDesignSystemSourceAliases('serve')[3].replacement),
    ]);
  });

  it('registers the layer contract before product modules can load component CSS', () => {
    const mainSource = readFileSync(
      path.resolve(__dirname, '../../main.tsx'),
      'utf8',
    );
    const indexHtml = readFileSync(
      path.resolve(__dirname, '../../../index.html'),
      'utf8',
    );
    const themeEntry = readFileSync(
      path.resolve(__dirname, '../../design-system-theme.css'),
      'utf8',
    );
    const globalStyles = readFileSync(
      path.resolve(__dirname, '../../app/styles/global.scss'),
      'utf8',
    );
    const appLayoutStyles = readFileSync(
      path.resolve(__dirname, '../../app/layout/AppLayout.scss'),
      'utf8',
    );
    const layerContract = readFileSync(
      path.resolve(
        __dirname,
        '../../../../../design-system/packages/ui/src/styles/layers.css',
      ),
      'utf8',
    );

    const layerPreludeIndex = mainSource.indexOf(
      'import "@bitfun/ui/styles.css"',
    );
    const productGraphIndex = mainSource.indexOf('import App from "./app/App"');

    expect(themeEntry).toContain('@import "@bitfun/theme-bitfun/default.css";');
    expect(mainSource).not.toContain('import "@bitfun/theme-bitfun/default.css"');
    expect(layerPreludeIndex).toBeGreaterThanOrEqual(0);
    expect(productGraphIndex).toBeGreaterThan(layerPreludeIndex);

    const bootstrapLayerOrder =
      '@layer bf.tokens.system, bf.tokens.theme, bf.reset, bf.base, bf.components, bf.overrides;';
    const bootstrapLayerOrderIndex = indexHtml.indexOf(bootstrapLayerOrder);
    const bootstrapResetIndex = indexHtml.indexOf('@layer bf.reset {');
    const moduleEntryIndex = indexHtml.indexOf(
      '<script type="module" src="/src/main.tsx"></script>',
    );
    const themeEntryIndex = indexHtml.indexOf(
      '<link rel="stylesheet" href="/src/design-system-theme.css" />',
    );

    expect(bootstrapLayerOrderIndex).toBeGreaterThanOrEqual(0);
    expect(bootstrapResetIndex).toBeGreaterThan(bootstrapLayerOrderIndex);
    expect(themeEntryIndex).toBeGreaterThanOrEqual(0);
    expect(themeEntryIndex).toBeLessThan(moduleEntryIndex);
    expect(moduleEntryIndex).toBeGreaterThan(bootstrapResetIndex);
    expect(indexHtml).toMatch(
      /@layer bf\.reset\s*\{[\s\S]*?\*\s*,\s*\*::before\s*,\s*\*::after\s*\{[\s\S]*?padding:\s*0;/,
    );
    expect(globalStyles).not.toMatch(
      /\*\s*,\s*\*::before\s*,\s*\*::after\s*\{[\s\S]*?padding:\s*0;/,
    );
    expect(globalStyles).toMatch(
      /@layer\s+bf\.base\s*\{[\s\S]*?:focus-visible\s*\{/,
    );
    expect(appLayoutStyles).not.toMatch(/^\s*\*:focus-visible\s*\{/m);
    expect(layerContract).toMatch(
      /@layer\s+bf\.tokens\.system,\s*bf\.tokens\.theme,\s*bf\.reset,\s*bf\.base,\s*bf\.components,\s*bf\.overrides\s*;/,
    );
  });
});
