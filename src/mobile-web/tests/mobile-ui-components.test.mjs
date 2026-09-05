import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const sourceDirectory = path.resolve(testDirectory, '../src');
const mobileEntry = path.resolve(
  testDirectory,
  '../../../design-system/packages/ui/src/mobile.ts',
);

async function listTsxFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map(async (entry) => {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) return listTsxFiles(absolutePath);
    return entry.isFile() && entry.name.endsWith('.tsx') ? [absolutePath] : [];
  }));
  return nested.flat();
}

async function readProductSources() {
  const files = await listTsxFiles(sourceDirectory);
  const sources = await Promise.all(files.map(async (file) => ({
    file: path.relative(sourceDirectory, file),
    source: await readFile(file, 'utf8'),
  })));
  return sources;
}

test('visible mobile controls use the shared mobile component entry', async () => {
  const sources = await readProductSources();

  for (const { file, source } of sources) {
    for (const tag of ['a', 'button', 'textarea', 'select', 'details']) {
      assert.doesNotMatch(
        source,
        new RegExp(`<${tag}\\b`),
        `${file} renders a raw <${tag}> instead of an @openbitfun/ui/mobile component`,
      );
    }
    assert.doesNotMatch(
      source,
      /<(?:div|span|section)\b[^>]*\bonClick=/s,
      `${file} uses a non-interactive element as an interaction control`,
    );
    assert.doesNotMatch(
      source,
      /\brole="button"/,
      `${file} emulates a button instead of using a shared mobile control`,
    );
  }

  const nativeInputs = sources.flatMap(({ file, source }) => (
    [...source.matchAll(/<input\b[\s\S]*?\/>/g)].map((match) => ({
      file,
      markup: match[0],
    }))
  ));
  assert.equal(nativeInputs.length, 1, 'only the hidden file-input bridge may stay native');
  assert.equal(nativeInputs[0].file, 'pages/ChatPage.tsx');
  assert.match(nativeInputs[0].markup, /type="file"/);
  assert.match(nativeInputs[0].markup, /display:\s*'none'/);
});

test('every published mobile component has a real mobile-web consumer', async () => {
  const entrySource = await readFile(mobileEntry, 'utf8');
  const componentNames = [
    ...entrySource.matchAll(/^\s*(Mobile[A-Za-z]+),$/gm),
  ].map((match) => match[1]);
  const productSource = (await readProductSources())
    .map(({ source }) => source)
    .join('\n');

  assert.ok(componentNames.length > 0, 'mobile entry did not expose any components');
  for (const componentName of componentNames) {
    assert.match(
      productSource,
      new RegExp(`\\b${componentName}\\b`),
      `${componentName} is published but is not consumed by mobile-web`,
    );
  }
});

test('large mobile pages delegate stable UI regions to app components', async () => {
  const chatPage = await readFile(path.join(sourceDirectory, 'pages/ChatPage.tsx'), 'utf8');
  const chatTranscript = await readFile(path.join(sourceDirectory, 'components/ChatTranscript.tsx'), 'utf8');
  const pairingPage = await readFile(path.join(sourceDirectory, 'pages/PairingPage.tsx'), 'utf8');
  const sessionPage = await readFile(path.join(sourceDirectory, 'pages/SessionListPage.tsx'), 'utf8');

  for (const component of [
    'ChatComposerBar',
    'ChatFeedback',
    'ChatHeader',
    'ChatMessageActions',
    'ChatTranscript',
    'ModelSelectorPill',
    'ReasoningPresetPill',
  ]) {
    assert.match(chatPage, new RegExp(`\\b${component}\\b`), `ChatPage must delegate ${component}`);
  }
  assert.doesNotMatch(chatPage, /<Mobile(?:Composer|Sheet)\b/, 'ChatPage must not rebuild composer or sheet anatomy');
  assert.doesNotMatch(chatPage, /const (?:ModelSelectorPill|ReasoningPresetPill|AskQuestionCard)\b/);
  assert.doesNotMatch(chatPage, /\b(?:ReactMarkdown|SyntaxHighlighter|renderOrderedItems)\b/);
  assert.match(chatTranscript, /\bChatAskQuestionCard\b/, 'ChatTranscript must delegate the question interaction');

  assert.match(pairingPage, /\bPairingForm\b/, 'PairingPage must delegate its visual form contract');
  assert.doesNotMatch(pairingPage, /<MobileTextField\b/, 'PairingPage must keep fields inside PairingForm');

  for (const component of [
    'CompactSettingsSheet',
    'MobileChoiceSheet',
    'SessionHistoryPanel',
    'SessionLaunchPanel',
    'SessionOverlays',
  ]) {
    assert.match(sessionPage, new RegExp(`\\b${component}\\b`), `SessionListPage must delegate ${component}`);
  }
  assert.doesNotMatch(sessionPage, /<MobileSheet\b/, 'SessionListPage must not own low-level sheet anatomy');
  assert.doesNotMatch(sessionPage, /createPortal\b/, 'shared sheets own their portal lifecycle');
});
