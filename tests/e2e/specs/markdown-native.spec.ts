import { mkdtemp, readFile, writeFile, copyFile, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveScreenshot } from '../helpers/screenshot-utils';

/** Real desktop WebView, production transport and a temporary on-disk workspace. */
class NativeMarkdownPage {
  get richText() { return $('.ProseMirror'); }
  get source() { return $('.m-editor-textarea'); }
  get tab() { return $('[data-tab-title="editor-test.md"]'); }
  block(kind: string) { return $(`[data-testid="md-embed-block"][data-language="${kind}"]`); }
  async waitForDiagram() {
    const block = this.block('mermaid');
    await expect(block.$('svg')).toHaveProperty('textContent', expect.stringContaining('Desktop'));
    await browser.waitUntil(async () => (await block.$('.mermaid-block__diagram').getCSSProperty('opacity')).value === 1
      || (await block.$('.mermaid-block__diagram').getCSSProperty('opacity')).value === '1', { timeout: 10000 });
  }
  async editBlock(kind: string, source: string) {
    const block = this.block(kind);
    await block.$('[data-testid="md-embed-preview"]').click();
    await block.$('[data-testid="md-embed-source"]').setValue(source);
    return block;
  }
  async typeAtStart(selector: string, text: string) {
    await $(selector).click();
    // The embedded driver dispatches synthetic clicks, which do not place a
    // native WebKit caret. Set only the selection, then type through WebDriver.
    await browser.execute((query: string) => {
      const element = document.querySelector(query)!;
      (element.closest('[contenteditable="true"]') as HTMLElement).focus();
      const range = document.createRange();
      range.selectNodeContents(element);
      range.collapse(true);
      const selection = window.getSelection()!;
      selection.removeAllRanges();
      selection.addRange(range);
      document.dispatchEvent(new Event('selectionchange'));
    }, selector);
    await browser.keys(text);
    await expect($(selector)).toHaveText(expect.stringContaining(text.trim()));
  }
  async mode(index: number) {
    await (await $$('.openbitfun-markdown-editor__mode-toggle [role="radio"]'))[index].click();
  }
  async openFile(file: string) {
    const row = $(`.openbitfun-file-viewer-nav [data-file-path="${file}"]`);
    await row.waitForDisplayed({ timeout: 20000 });
    await row.click();
    try {
      await this.richText.waitForDisplayed({ timeout: 20000 });
    } catch (error) {
      await saveScreenshot('markdown-native-open-failure', { includeTimestamp: false });
      console.log('Desktop open state:', await browser.execute(() => document.body.innerText.slice(-6000)));
      throw error;
    }
  }
}

const page = new NativeMarkdownPage();
const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
let workspace: string;
let file: string;

describe('Native desktop Markdown editing', () => {
  before(async () => {
    workspace = await realpath(await mkdtemp(join(tmpdir(), 'openbitfun-markdown-native-')));
    file = join(workspace, 'editor-test.md');
    const fixture = (await readFile(new URL('../browser/markdown-fixture.md', import.meta.url), 'utf8'))
      .replace('# Editable document', '#  Editable document')
      .replace('http://127.0.0.1:1447/tests/e2e/image.svg', './image.svg');
    await writeFile(file, fixture);
    await copyFile(new URL('../../../src/web-ui/tests/e2e/image.svg', import.meta.url), join(workspace, 'image.svg'));
    expect(await openWorkspace(workspace)).toBe(true);
    await browser.execute(() => window.dispatchEvent(new CustomEvent('scene:open', {
      detail: { sceneId: 'file-viewer' },
    })));
    await page.openFile(file);
  });

  after(async () => { if (workspace) await rm(workspace, { recursive: true, force: true }); });

  it('restores original source and dirty state through undo, redo, and saving', async () => {
    const original = await readFile(file, 'utf8');
    await page.typeAtStart('.ProseMirror h1', 'Changed ');
    await expect(page.tab).toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    await browser.keys([modifier, 'z']);
    await expect($('.ProseMirror h1')).toHaveText('Editable document');
    await expect(page.tab).not.toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    await browser.keys([modifier, 'Shift', 'z']);
    await expect(page.tab).toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    await browser.keys([modifier, 's']);
    await expect(page.tab).not.toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    const saved = await readFile(file, 'utf8');
    expect(saved).toContain('Changed Editable document');
    await browser.keys([modifier, 'z']);
    await expect(page.tab).toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    await browser.keys([modifier, 'Shift', 'z']);
    await expect(page.tab).not.toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    await browser.keys([modifier, 'z']);
    await page.mode(1);
    await expect(page.source).toHaveValue(original, { trim: false });
    await page.source.click();
    await browser.keys([modifier, 's']);
    await expect(page.tab).not.toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    expect(await readFile(file, 'utf8')).toBe(original);
    await page.mode(0);
  });

  it('edits native blocks and embedded source, saves through Tauri, and reopens the same file', async () => {
    expect(await browser.execute(() => Boolean(window.__TAURI__?.core?.invoke))).toBe(true);
    await expect($$('.openbitfun-markdown-editor__mode-toggle [role="radio"]')).toBeElementsArrayOfSize(2);
    await expect(page.richText).toHaveAttribute('contenteditable', 'true');

    const mermaid = await page.editBlock('mermaid', 'graph TD\n A[Desktop] --> B[Saved]');
    await expect(mermaid.$('svg')).toHaveProperty('textContent', expect.stringContaining('Desktop'));
    await browser.keys([modifier, 'Enter']);
    await expect(mermaid.$('[data-testid="md-embed-source"]')).not.toBeDisplayed();
    await expect(mermaid.$('.m-editor-embed-toolbar')).not.toBeDisplayed();
    expect((await mermaid.getCSSProperty('border-top-width')).value).toBe('0px');

    const html = await page.editBlock('html', '<div>Native HTML edited</div>');
    await expect(html.$('.markdown-body')).toHaveText(expect.stringContaining('Native HTML edited'));
    await browser.keys([modifier, 'Enter']);
    const math = await page.editBlock('math', 'E=mc^2');
    await expect(math.$('.katex')).toBeDisplayed();
    await browser.keys([modifier, 'z']);
    await expect(math.$('[data-testid="md-embed-source"]')).toHaveValue('a^2+b^2=c^2');
    await browser.keys([modifier, 'Shift', 'z']);
    await expect(math.$('[data-testid="md-embed-source"]')).toHaveValue('E=mc^2');
    await browser.keys('Escape');

    const inlineMath = $('[data-testid="md-inline-math"]');
    await inlineMath.$('.m-editor-inline-math__preview').click();
    await inlineMath.$('[data-testid="md-embed-source"]').setValue('y^3');
    await $('.ProseMirror > h1').click();
    await expect(inlineMath.$('[data-testid="md-embed-source"]')).not.toBeDisplayed();

    const image = $('[data-testid="md-image"]');
    await image.$('img').click();
    await image.$('[data-testid="md-image-alt"]').setValue('Desktop image description');
    await image.$('[data-testid="md-image-title"]').setValue('Desktop title');
    await browser.keys('Enter');
    await expect(image.$('img')).toHaveAttribute('alt', 'Desktop image description');
    await expect(image.$('img')).toHaveAttribute('src', expect.stringContaining('data:image/'));

    await $('.ProseMirror input[type="checkbox"]').click();
    await page.typeAtStart('.ProseMirror td:last-child', 'Desktop cell ');
    await page.typeAtStart('[data-type="detailsContent"] p', 'Desktop nested ');
    await expect($('.ProseMirror li [data-language="mermaid"]')).toBeDisplayed();

    await page.mode(1);
    await expect(page.source).toHaveValue(expect.stringContaining('Native HTML edited'));
    await page.mode(0);
    await page.richText.click();
    await browser.keys([modifier, 's']);
    await browser.waitUntil(async () => (await readFile(file, 'utf8')).includes('Desktop image description'), {
      timeout: 15000, timeoutMsg: 'Desktop save did not reach the actual file',
    });
    const saved = await readFile(file, 'utf8');
    for (const value of ['A[Desktop]', 'Native HTML edited', '$y^3$', 'E=mc^2', '- [x] Finish editing', 'Desktop cell', 'Desktop nested', '[^test]: Preserved definition']) {
      expect(saved).toContain(value);
    }
    await expect(page.tab).not.toHaveAttribute('data-openbitfun-state', expect.stringContaining('dirty'));
    await page.waitForDiagram();
    await saveScreenshot('markdown-native-edited', { includeTimestamp: false });

    await page.tab.$('.canvas-tab__close-btn').click();
    await expect(page.richText).not.toBeExisting();
    await page.openFile(file);
    await expect(page.block('mermaid').$('svg')).toHaveProperty('textContent', expect.stringContaining('Desktop'));
    await expect($('[data-testid="md-image"] img')).toHaveAttribute('title', 'Desktop title');
    await page.mode(1);
    await expect(page.source).toHaveValue(saved, { trim: false });
    await page.mode(0);
    await page.waitForDiagram();
    await page.block('mermaid').scrollIntoView({ block: 'center' });
    await saveScreenshot('markdown-native-reopened', { includeTimestamp: false });
  });
});
