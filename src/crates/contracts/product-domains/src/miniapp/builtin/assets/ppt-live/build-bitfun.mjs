import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const appDir = path.dirname(fileURLToPath(import.meta.url));

await build({
  entryPoints: [path.join(appDir, 'ui.js')],
  outfile: path.join(appDir, 'dist', 'ui.bundle.js'),
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
  legalComments: 'none',
});
