import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { allocateBrowserWebviewLabel } from './useEmbeddedBrowserWebview';

function readBrowserPanelSource(): string {
  return readFileSync(fileURLToPath(new URL('./BrowserPanel.tsx', import.meta.url)), 'utf8')
    .replace(/\r\n?/g, '\n');
}

describe('browser panel visibility contract', () => {
  it('lets the owning editor decide whether the native webview is visible', () => {
    const source = readBrowserPanelSource();

    expect(source).toContain('isVisible: isActive');
    expect(source).not.toContain("activeTabId === 'session'");
  });

  it('allocates unique labels across independent browser panel instances', () => {
    const first = allocateBrowserWebviewLabel('embedded-browser-panel-view');
    const second = allocateBrowserWebviewLabel('embedded-browser-panel-view');

    expect(second).not.toBe(first);
    expect(first).toMatch(/^embedded-browser-panel-view-\d+$/);
    expect(second).toMatch(/^embedded-browser-panel-view-\d+$/);
  });
});
