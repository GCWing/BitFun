import { readFile } from 'node:fs/promises';
import { browser, $, $$, expect } from '@wdio/globals';
import { MarkdownEditorPage } from './MarkdownEditorPage';

const editor = new MarkdownEditorPage();
const fixture = await readFile(new URL('./markdown-fixture.md', import.meta.url), 'utf8');
const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';

describe('Markdown rich text browser E2E', () => {
  beforeEach(async () => {
    await browser.url('about:blank');
    await fetch('http://127.0.0.1:1450/file', { method: 'PUT', body: fixture });
    await editor.open();

  });

  it('retains the original standard preview typography and embedded appearance', async () => {
    // The math renderer adds an asynchronous wrapper. Compare the standard
    // document layout here; formula editing/rendering has separate coverage.
    await browser.url('about:blank');
    await fetch('http://127.0.0.1:1450/file', { method: 'PUT', body: fixture
      .replace('$x^2$', 'formula').replace('$$\na^2+b^2=c^2\n$$', 'Equation') });
    await editor.open();
    const styles = async (reference: boolean) => browser.execute((isReference: boolean) => {
      const root = document.querySelector(isReference ? '.m-editor-preview .markdown-renderer' : '.ProseMirror')!;
      const definitions: Record<string, [string, string[]]> = {
        body: [':scope', ['fontFamily', 'fontSize', 'lineHeight', 'color']],
        heading: ['h1', ['fontSize', 'fontWeight', 'lineHeight', 'marginTop', 'marginBottom', 'paddingLeft']],
        paragraph: ['p', ['fontSize', 'lineHeight', 'paddingTop', 'paddingLeft', 'marginTop', 'marginBottom']],
        table: ['.table-wrapper', ['borderRadius', 'borderTopWidth', 'backgroundColor']],
        cell: ['td', ['fontSize', 'lineHeight', 'paddingTop', 'paddingLeft']],
        mermaid: ['.mermaid-block', ['borderRadius', 'borderTopWidth', 'backgroundColor']],
        diagram: ['.mermaid-block__diagram', ['paddingTop', 'paddingLeft']],
        details: [isReference ? 'details' : '[data-type="details"]', ['borderRadius', 'borderTopWidth', 'paddingTop', 'backgroundColor']],
      };
      const appearance = Object.fromEntries(Object.entries(definitions).map(([key, [selector, properties]]) => {
        const element = selector === ':scope' ? root : root.querySelector(selector)!;
        if (!element) throw new Error(`Missing ${key} (${selector}): ${root.innerHTML.slice(0, 500)}`);
        const style = getComputedStyle(element);
        return [key, Object.fromEntries(properties.map(property => [property, style[property as keyof CSSStyleDeclaration]]))];
      }));
      const checkbox = root.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
      const walker = document.createTreeWalker(checkbox.closest('li')!, NodeFilter.SHOW_TEXT);
      let text = walker.nextNode();
      while (text && !text.textContent?.trim()) text = walker.nextNode();
      const range = document.createRange();
      range.selectNodeContents(text!);
      const line = range.getBoundingClientRect();
      const box = checkbox.getBoundingClientRect();
      return { ...appearance, taskOnOneLine: box.top < line.bottom && box.bottom > line.top };
    }, reference);
    await editor.block('mermaid').$('svg').waitForDisplayed();
    const rich = await styles(false);
    expect(rich.taskOnOneLine).toBe(true);
    await browser.url('/tests/e2e/markdown-editor.html?reference=1');
    await $('.m-editor-preview .mermaid-block svg').waitForDisplayed();
    const original = await styles(true);
    expect(rich).toEqual(original);
  });

  it('keeps source formatting edits dirty while undoing later rich edits exactly', async () => {
    const changedSource = fixture.replace('# Editable document', '#  Editable document') + '\n';
    await editor.mode(1);
    await editor.source.setValue(changedSource);
    await expect(editor.dirty).toHaveText('Unsaved');
    await editor.mode(0);
    await editor.editBlock('mermaid', 'graph TD\n A[Changed] --> B[Source]');
    await browser.keys([modifier, 'z']);
    await expect(editor.dirty).toHaveText('Unsaved');
    await editor.mode(1);
    await expect(editor.source).toHaveValue(changedSource, { trim: false });
    await editor.source.click();
    await editor.save();
    expect(await editor.savedSource()).toBe(changedSource);
    await editor.mode(0);
    await editor.editBlock('mermaid', 'graph TD\n A[Changed] --> B[Saved]');
    await browser.keys([modifier, 'z']);
    await expect(editor.dirty).toHaveText('Saved');
    await editor.mode(1);
    await expect(editor.source).toHaveValue(changedSource, { trim: false });
  });

  it('edits Mermaid with live rendering and saves across source switches and reload', async () => {
    await expect($$('.openbitfun-markdown-editor__mode-toggle [role="radio"]')).toBeElementsArrayOfSize(2);
    const block = await editor.editBlock('mermaid', 'graph TD\n  A[Start] --> B[Edited]');
    await expect(block.$('svg')).toHaveText(expect.stringContaining('Edited'));
    await expect(block.$('[data-testid="md-embed-source"]')).toHaveValue('graph TD\n  A[Start] --> B[Edited]');
    await expect(editor.dirty).toHaveText('Unsaved');
    await editor.save();
    expect(await editor.savedSource()).toContain('```mermaid\ngraph TD\n  A[Start] --> B[Edited]\n```');
    await block.$('[data-testid="md-embed-edit"]').click();
    await expect(block.$('[data-testid="md-embed-source"]')).not.toBeDisplayed();
    await expect(block.$('.m-editor-embed-toolbar')).not.toBeDisplayed();
    expect((await block.getCSSProperty('border-top-width')).value).toBe('0px');
    await editor.mode(1);
    await expect(editor.source).toHaveValue(expect.stringContaining('B[Edited]'));
    await editor.mode(0);
    await browser.refresh();
    await expect(editor.block('mermaid').$('svg')).toHaveText(expect.stringContaining('Edited'));
    await expect(editor.dirty).toHaveText('Saved');
  });

  it('edits an inline equation while surrounding text stays native and closes outside', async () => {
    const math = $('[data-testid="md-inline-math"]');
    await math.$('.m-editor-inline-math__preview').click();
    const source = math.$('[data-testid="md-embed-source"]');
    await source.setValue('y^3');
    await expect(math.$('.katex')).toBeDisplayed();
    await $('.ProseMirror > h1').click();
    await expect(source).not.toBeDisplayed();
    const paragraph = $('.ProseMirror > p');
    await paragraph.click();
    await browser.keys('End');
    await browser.keys(' Changed.');
    await editor.save();
    expect(await editor.savedSource()).toContain('$y^3$');
    expect(await editor.savedSource()).toContain('Changed.');
    await expect($$('.ProseMirror > p [data-testid="md-inline-math"]')).toBeElementsArrayOfSize(1);
  });

  it('keeps HTML and block equations editable and supports keyboard completion and undo', async () => {
    const html = await editor.editBlock('html', '<div data-example="embed">Updated HTML</div>');
    await expect(html.$('.markdown-body')).toHaveText(expect.stringContaining('Updated HTML'));
    await browser.keys([modifier, 'Enter']);
    await expect(html.$('[data-testid="md-embed-source"]')).not.toBeDisplayed();
    await editor.editBlock('math', 'E=mc^2');
    await expect(editor.block('math').$('.katex')).toBeDisplayed();
    await browser.keys([modifier, 'z']);
    await expect(editor.block('math').$('[data-testid="md-embed-source"]')).toHaveValue('a^2+b^2=c^2');
    await browser.keys([modifier, 'Shift', 'z']);
    await expect(editor.block('math').$('[data-testid="md-embed-source"]')).toHaveValue('E=mc^2');
    await browser.keys('Escape');
    await expect(editor.block('math').$('[data-testid="md-embed-source"]')).not.toBeDisplayed();
    await editor.save();
    expect(await editor.savedSource()).toContain('$$\nE=mc^2\n$$');
    expect(await editor.savedSource()).toContain('Updated HTML');
  });

  it('edits tasks tables and nested content and preserves unsupported syntax on save', async () => {
    await $('.ProseMirror input[type="checkbox"]').click();
    const cell = $('.ProseMirror td:last-child');
    await cell.click();
    await browser.keys('Home');
    await browser.keys('Updated ');
    const nested = $('[data-type="detailsContent"] p');
    await nested.click();
    await browser.keys('End');
    await browser.keys(' edited');
    await $('[data-type="details"] > button').click();
    await expect(nested).not.toBeDisplayed();
    await $('[data-type="details"] > button').click();
    await expect(nested).toHaveText(expect.stringContaining('edited'));
    await expect($('.ProseMirror li [data-language="mermaid"]')).toBeDisplayed();
    await editor.save();
    const source = await editor.savedSource();
    expect(source).toContain('- [x] Finish editing');
    expect(source).toContain('Updated');
    expect(source).toContain('[^test]: Preserved definition');
  });

  it('edits image properties locally with keyboard completion and persistent saves', async () => {
    const image = $('[data-testid="md-image"]');
    await image.$('img').click();
    await image.$('[data-testid="md-image-alt"]').setValue('Updated description');
    await image.$('[data-testid="md-image-title"]').setValue('Updated title');
    await browser.keys('Enter');
    await expect(image.$('[data-testid="md-image-alt"]')).not.toBeDisplayed();
    await expect(image.$('img')).toHaveAttribute('alt', 'Updated description');
    await editor.save();
    expect(await editor.savedSource()).toContain('![Updated description]');
    expect(await editor.savedSource()).toContain('"Updated title"');
    await browser.refresh();
    await expect($('[data-testid="md-image"] img')).toHaveAttribute('title', 'Updated title');
  });


  it('renders footnotes with shared numbering, anchors and live definition updates', async () => {
    const markdown = '# Notes\n\nText[^a] and again[^a].\n\nOther[^b] with $x^2$.\n\n[^a]: Definition\n\n[^b]: Another definition';
    await browser.url('about:blank');
    await fetch('http://127.0.0.1:1450/file', { method: 'PUT', body: markdown });
    await editor.open();
    await expect($$('.ProseMirror [data-footnote-ref]')).toBeElementsArrayOfSize(3);
    await expect($('.ProseMirror [data-footnote-ref]')).toHaveText('1');
    await expect($$('.ProseMirror section[data-footnotes] li')).toBeElementsArrayOfSize(2);
    const anchors = await browser.execute(() => {
      const root = document.querySelector('.ProseMirror')!;
      const ids = [...root.querySelectorAll('[id]')].map(node => node.id);
      const links = [...root.querySelectorAll<HTMLAnchorElement>('[data-footnote-ref], [data-footnote-backref]')];
      return { unique: new Set(ids).size === ids.length, resolved: links.every(link => ids.includes(link.hash.slice(1))) };
    });
    expect(anchors).toEqual({ unique: true, resolved: true });
    await expect($('.ProseMirror > h1')).toHaveText('Notes');
    const block = editor.block('footnoteDefinition');
    await block.$('[data-testid="md-embed-preview"]').click();
    await block.$('[data-testid="md-embed-source"]').setValue('[^a]: Updated definition');
    await expect(block.$('section[data-footnotes]')).toHaveText(expect.stringContaining('Updated definition'));
    await browser.keys('Escape');
    await editor.save();
    expect(await editor.savedSource()).toBe(markdown.replace('[^a]: Definition', '[^a]: Updated definition'));
    await browser.refresh();
    await expect($$('.ProseMirror [data-footnote-ref]')).toBeElementsArrayOfSize(3);
    await expect($('.ProseMirror section[data-footnotes]')).toHaveText(expect.stringContaining('Updated definition'));
  });

  it('loads edited local image addresses through the workspace file adapter', async () => {
    await browser.url('about:blank');
    await fetch('http://127.0.0.1:1450/file', { method: 'PUT', body: '# Image\n\n![test](http://127.0.0.1:1447/tests/e2e/image.svg)' });
    await editor.open();
    await browser.execute(async () => {
      // Track only the fixture filesystem boundary; use the production image loader.
      const { workspaceAPI } = await import('/src/infrastructure/api/index.ts');
      (window as any).__imageReads = [];
      const original = workspaceAPI.readFileContent;
      workspaceAPI.readFileContent = async (path: string) => {
        (window as any).__imageReads.push(path);
        if (path.endsWith('.png')) return 'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFElEQVR4nGNoIBEwjGoY1TB8NQAAJYSAEGy7FvQAAAAASUVORK5CYII=';
        return original(path);
      };
    });
    for (const name of ['new.png', 'another.png']) {
      await $('[data-testid="md-image"] img').click();
      await $('[data-testid="md-image-src"]').setValue(`./${name}`);
      await browser.keys('Enter');
      await browser.waitUntil(async () => browser.execute((path: string) =>
        (window as any).__imageReads.includes(path), `/workspace/${name}`));
      await expect($('[data-testid="md-image"] img')).toHaveAttribute('src', expect.stringContaining('data:image/png;base64,'));
      await browser.waitUntil(async () => browser.execute(() =>
        document.querySelector<HTMLImageElement>('[data-testid="md-image"] img')?.naturalWidth === 16));
    }
    await editor.save();
    expect(await editor.savedSource()).toContain('![test](./another.png)');
  });

  it('recovers from invalid Mermaid and locks all embedded editors in readonly mode', async () => {
    const block = await editor.editBlock('mermaid', 'this is not valid mermaid');
    await expect(block.$('[data-testid="md-embed-source"]')).toBeDisplayed();
    await expect(editor.richText).toHaveAttribute('contenteditable', 'true');
    await block.$('[data-testid="md-embed-source"]').setValue('graph TD\n A-->Recovered');
    await expect(block.$('svg')).toHaveText(expect.stringContaining('Recovered'));
    await $('[data-testid="readonly"]').click();
    await expect(editor.richText).toHaveAttribute('contenteditable', 'false');
    await expect(block.$('[data-testid="md-embed-edit"]')).not.toBeDisplayed();
    await expect(block.$('[data-testid="md-embed-source"]')).not.toBeDisplayed();
    await $('[data-testid="readonly"]').click();
    await expect(editor.dirty).toHaveText('Unsaved');
    await block.$('[data-testid="md-embed-preview"]').click();
    await expect(block.$('[data-testid="md-embed-source"]')).toHaveValue('graph TD\n A-->Recovered');
  });
});
