import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { JSDOM } from 'jsdom';
import { describe, expect, it, vi } from 'vitest';

function readIndexHtml(): string {
  return readFileSync(fileURLToPath(new URL('../../../index.html', import.meta.url)), 'utf8');
}

describe('startup preload shell', () => {
  it('uses injected startup locale text and wires static window controls', () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    const dom = new JSDOM(readIndexHtml(), {
      url: 'http://localhost:1422/',
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.assign(window, {
          __BITFUN_BOOTSTRAP_LOCALE__: 'zh-CN',
          __BITFUN_BOOTSTRAP_MESSAGES__: {
            loadingApp: '正在启动 BitFun...',
            minimize: '最小化',
            maximize: '最大化',
            close: '关闭',
          },
          __BITFUN_SHOW_STARTUP_WINDOW_CONTROLS__: true,
          __TAURI_INTERNALS__: { invoke },
        });
      },
    });

    const hint = dom.window.document.querySelector('.splash-screen__message');
    expect(dom.window.document.documentElement.lang).toBe('zh-CN');
    expect(dom.window.document.getElementById('root')?.childElementCount).toBe(0);
    expect(dom.window.document.getElementById('bitfun-startup-overlay')).not.toBeNull();
    expect(hint?.textContent).toBe('正在启动 BitFun...');

    const controls = dom.window.document.querySelector<HTMLElement>('[data-startup-window-controls]');
    expect(controls?.hidden).toBe(false);

    const closeButton = dom.window.document.querySelector<HTMLButtonElement>('[data-startup-window-action="close"]');
    expect(closeButton?.getAttribute('aria-label')).toBe('关闭');
    closeButton?.click();

    expect(invoke).toHaveBeenCalledWith('startup_window_control', {
      request: { action: 'close' },
    });
  });

  it('shows the independent pet preload for the companion window', () => {
    const html = readIndexHtml();
    const dom = new JSDOM(html, {
      url: 'http://localhost:1422/?bitfunWindow=agent-companion',
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.assign(window, {
          __BITFUN_BOOTSTRAP_LOCALE__: 'en-US',
          __BITFUN_BOOTSTRAP_MESSAGES__: {
            petLoading: 'Loading companion...',
          },
        });
      },
    });

    expect(dom.window.document.body.classList.contains('bitfun-pet-preload-body')).toBe(true);
    expect(dom.window.document.getElementById('bitfun-startup-overlay')).toBeNull();
    expect(dom.window.document.querySelector('.bitfun-pet-preload__sprite')).not.toBeNull();
    expect(dom.window.document.querySelector('.splash-screen__logo')).toBeNull();
    expect(dom.window.document.querySelector('.bitfun-sr-only')?.textContent).toBe('Loading companion...');
    const spriteCss = html.match(/\.bitfun-pet-preload__sprite \{(?<css>[\s\S]*?)\n      \}/)?.groups?.css;
    expect(spriteCss).toBeDefined();
    expect(spriteCss).not.toContain('background:');
    expect(spriteCss).not.toContain('border:');
    expect(spriteCss).not.toContain('box-shadow:');
  });
});
