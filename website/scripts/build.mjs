#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const buildScriptPath = fileURLToPath(import.meta.url);
const websiteRoot = path.resolve(path.dirname(buildScriptPath), '..');
const repositoryRoot = path.resolve(websiteRoot, '..');
const sourceRoot = path.join(websiteRoot, 'src');
const outputRoot = path.join(websiteRoot, 'dist');
const catalogPath = path.join(repositoryRoot, 'docs/interactive-capabilities/capabilities.json');

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function embeddedJson(value) {
  return JSON.stringify(value).replaceAll('<', '\\u003c');
}

function renderPage(template, { title, description, url, pageData, assetVersion }) {
  return template
    .replaceAll('__PAGE_TITLE__', escapeHtml(title))
    .replaceAll('__PAGE_DESCRIPTION__', escapeHtml(description))
    .replaceAll('__PAGE_URL__', escapeHtml(url))
    .replaceAll('__ASSET_VERSION__', escapeHtml(assetVersion))
    .replace('__PAGE_DATA__', embeddedJson(pageData));
}

async function main() {
  const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
  const template = await readFile(path.join(sourceRoot, 'index.html'), 'utf8');
  const appSource = await readFile(path.join(sourceRoot, 'app.js'), 'utf8');
  const stylesSource = await readFile(path.join(sourceRoot, 'styles.css'), 'utf8');
  const buildSource = await readFile(buildScriptPath);
  const logoPath = path.join(repositoryRoot, 'src/apps/desktop/icons/Logo-ICON.png');
  const logoSource = await readFile(logoPath);
  const socialImage = path.join(sourceRoot, 'og.png');
  let socialSource = Buffer.alloc(0);
  try {
    socialSource = await readFile(socialImage);
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }
  const assetVersion = createHash('sha256')
    .update(catalog.digest)
    .update(template)
    .update(appSource)
    .update(stylesSource)
    .update(buildSource)
    .update(logoSource)
    .update(socialSource)
    .digest('hex')
    .slice(0, 12);

  await rm(outputRoot, { recursive: true, force: true });
  await mkdir(path.join(outputRoot, 'assets'), { recursive: true });
  await cp(path.join(sourceRoot, 'app.js'), path.join(outputRoot, 'assets/app.js'));
  await cp(path.join(sourceRoot, 'styles.css'), path.join(outputRoot, 'assets/styles.css'));
  await cp(logoPath, path.join(outputRoot, 'assets/bitfun.png'));

  try {
    await cp(socialImage, path.join(outputRoot, 'og.png'));
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }

  await mkdir(path.join(outputRoot, 'data'), { recursive: true });
  await writeFile(
    path.join(outputRoot, 'data/capabilities.json'),
    `${JSON.stringify(catalog)}\n`,
  );
  await writeFile(
    path.join(outputRoot, 'release.json'),
    `${JSON.stringify({
      releaseId: assetVersion,
      assetVersion,
      catalogDigest: catalog.digest,
    }, null, 2)}\n`,
  );

  const homeDescription = `Learn ${catalog.counts.features} BitFun features and ${catalog.counts.settings} settings pages in Chinese or English.`;
  await writeFile(path.join(outputRoot, 'index.html'), renderPage(template, {
    title: 'BitFun Playbook — Features, settings, and practical guides',
    description: homeDescription,
    url: `${catalog.origin}/`,
    assetVersion,
    pageData: { kind: 'index', digest: catalog.digest },
  }));

  for (const capability of catalog.capabilities) {
    const directory = path.join(outputRoot, 'capabilities', capability.id);
    await mkdir(directory, { recursive: true });
    await writeFile(path.join(directory, 'index.html'), renderPage(template, {
      title: `${capability.titleEn} / ${capability.titleZh} — BitFun Playbook`,
      description: capability.summaryEn,
      url: capability.docsUrl,
      assetVersion,
      pageData: { kind: 'capability', capabilityId: capability.id, digest: catalog.digest },
    }));
  }

  const sitemap = catalog.capabilities
    .map(({ docsUrl }) => `  <url><loc>${escapeHtml(docsUrl)}</loc></url>`)
    .join('\n');
  await writeFile(
    path.join(outputRoot, 'sitemap.xml'),
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n  <url><loc>${catalog.origin}/</loc></url>\n${sitemap}\n</urlset>\n`,
  );
  await writeFile(
    path.join(outputRoot, 'robots.txt'),
    `User-agent: *\nAllow: /\nSitemap: ${catalog.origin}/sitemap.xml\n`,
  );

  process.stdout.write(
    `Built BitFun Playbook ${assetVersion} with ${catalog.counts.features} feature guides + ${catalog.counts.settings} settings guides (${catalog.digest.slice(0, 12)} catalog).\n`,
  );
}

await main();
