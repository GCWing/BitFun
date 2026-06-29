import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createServer } from 'vite';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const webUiRoot = path.join(repoRoot, 'src/web-ui');
const outputPath = path.join(
  repoRoot,
  'src/apps/desktop/src/generated/startup_theme_bootstrap.json',
);
const checkOnly = process.argv.includes('--check');

const server = await createServer({
  root: webUiRoot,
  logLevel: 'error',
  appType: 'custom',
  server: { middlewareMode: true },
  optimizeDeps: {
    entries: [],
    noDiscovery: true,
  },
});

try {
  const [{ builtinThemes }, { createStartupThemeBootstrapManifest }] = await Promise.all([
    server.ssrLoadModule('/src/infrastructure/theme/presets/index.ts'),
    server.ssrLoadModule('/src/infrastructure/theme/presets/startupThemeBootstrap.ts'),
  ]);

  const manifest = createStartupThemeBootstrapManifest(builtinThemes);
  const nextContent = `${JSON.stringify(manifest, null, 2)}\n`;
  const currentContent = fs.existsSync(outputPath)
    ? fs.readFileSync(outputPath, 'utf8')
    : null;

  if (checkOnly) {
    if (currentContent !== nextContent) {
      console.error(
        'Startup theme bootstrap manifest is stale. Run `pnpm run generate-startup-theme-bootstrap`.',
      );
      process.exitCode = 1;
    }
  } else {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, nextContent, 'utf8');
    console.log(`Generated ${path.relative(repoRoot, outputPath).replace(/\\/g, '/')}`);
  }
} finally {
  await server.close();
}
