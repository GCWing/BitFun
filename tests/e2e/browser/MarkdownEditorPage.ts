import { $, $$, browser, expect } from '@wdio/globals';

export class MarkdownEditorPage {
  get richText() { return $('.ProseMirror'); }
  get source() { return $('.m-editor-textarea'); }
  get dirty() { return $('[data-testid="dirty"]'); }
  block(language: string) { return $(`[data-testid="md-embed-block"][data-language="${language}"]`); }
  async mode(index: number) { await (await $$('.openbitfun-markdown-editor__mode-toggle [role="radio"]'))[index].click(); }
  async open() {
    await browser.url('/tests/e2e/markdown-editor.html');
    await this.richText.waitForDisplayed();
    await expect(this.dirty).toHaveText('Saved');
  }
  async editBlock(language: string, value: string) {
    const block = this.block(language);
    await block.$('[data-testid="md-embed-preview"]').click();
    const source = block.$('[data-testid="md-embed-source"]');
    await source.waitForDisplayed();
    await source.setValue(value);
    return block;
  }
  async save() {
    // Chromium on macOS uses Meta; other hosts use Control.
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
    await browser.keys([modifier, 's']);
    await expect(this.dirty).toHaveText('Saved');
  }
  async savedSource() { return (await fetch('http://127.0.0.1:1450/file')).text(); }
}
