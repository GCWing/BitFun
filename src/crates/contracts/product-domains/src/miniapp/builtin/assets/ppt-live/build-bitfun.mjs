import { build } from 'esbuild';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const appDir = path.dirname(fileURLToPath(import.meta.url));
const outfile = path.join(appDir, 'dist', 'ui.bundle.js');

await build({
  entryPoints: [path.join(appDir, 'ui.js')],
  outfile,
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
  legalComments: 'none',
});

// pdf-lib emits a template literal whose meaningful space lands at end-of-line.
// Keep the PDF xref bytes unchanged while avoiding trailing whitespace in source.
const bundledSource = await readFile(outfile, 'utf8');
await writeFile(outfile, bundledSource.replaceAll('` \n`', '" \\n"'));
