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
const themePromptSnapshotOutputPath = path.join(
  repoRoot,
  'src/crates/assembly/core/src/agentic/tools/implementations/generated/theme_prompt_snapshots.json',
);
const checkOnly = process.argv.includes('--check');

function normalizeGeneratedText(content) {
  return String(content).replace(/\r\n?/g, '\n');
}

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
  const [
    { builtinThemes },
    { createStartupThemeBootstrapManifest },
    { createThemePromptSnapshotManifest },
  ] = await Promise.all([
    server.ssrLoadModule('/src/infrastructure/theme/presets/index.ts'),
    server.ssrLoadModule('/src/infrastructure/theme/presets/startupThemeBootstrap.ts'),
    server.ssrLoadModule('/src/infrastructure/theme/presets/themePromptSnapshots.ts'),
  ]);

  const generatedFiles = [
    {
      label: 'Startup theme bootstrap manifest',
      outputPath,
      content: `${JSON.stringify(createStartupThemeBootstrapManifest(builtinThemes), null, 2)}\n`,
    },
    {
      label: 'Theme prompt snapshot manifest',
      outputPath: themePromptSnapshotOutputPath,
      content: `${JSON.stringify(createThemePromptSnapshotManifest(builtinThemes), null, 2)}\n`,
    },
  ];

  for (const generatedFile of generatedFiles) {
    const currentContent = fs.existsSync(generatedFile.outputPath)
      ? fs.readFileSync(generatedFile.outputPath, 'utf8')
      : null;

    if (checkOnly) {
      const currentContentForCheck = currentContent == null
        ? null
        : normalizeGeneratedText(currentContent);
      if (currentContentForCheck !== normalizeGeneratedText(generatedFile.content)) {
        console.error(
          `${generatedFile.label} is stale. Run \`pnpm run generate-startup-theme-bootstrap\`.`,
        );
        process.exitCode = 1;
      }
    } else {
      fs.mkdirSync(path.dirname(generatedFile.outputPath), { recursive: true });
      fs.writeFileSync(generatedFile.outputPath, generatedFile.content, 'utf8');
      console.log(`Generated ${path.relative(repoRoot, generatedFile.outputPath).replace(/\\/g, '/')}`);
    }
  }
} finally {
  await server.close();
}
