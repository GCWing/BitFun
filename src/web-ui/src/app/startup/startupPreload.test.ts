import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { JSDOM } from 'jsdom';
import { describe, expect, it, vi } from 'vitest';

function readIndexHtml(): string {
  return readFileSync(fileURLToPath(new URL('../../../index.html', import.meta.url)), 'utf8');
}

function readWindowControlsSource(): string {
  return readFileSync(
    fileURLToPath(new URL('../components/WindowControls/WindowControls.tsx', import.meta.url)),
    'utf8',
  );
}

function readLoadingMessage(locale: string): string {
  const common = JSON.parse(readFileSync(
    fileURLToPath(new URL(`../../locales/${locale}/common.json`, import.meta.url)),
    'utf8',
  ));
  return common.loading.app;
}

describe('startup preload shell', () => {
  it.each([
    ['zh-CN', 'en-US', 'zh-CN'],
    ['en-US', 'zh-CN', 'en-US'],
    ['zh-TW', 'en-US', 'zh-TW'],
    ['  ZH-hant-TW  ', 'en-US', 'zh-TW'],
    ['zh-HK', 'en-US', 'zh-TW'],
    ['en-GB', 'zh-CN', 'en-US'],
    [undefined, 'zh-Hant', 'zh-TW'],
    ['fr-FR', 'en-US', 'zh-CN'],
  ])('localizes first-paint text for saved locale %s and system language %s', (saved, system, expected) => {
    const dom = new JSDOM(readIndexHtml(), {
      url: 'http://localhost:1422/',
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.defineProperty(window.navigator, 'language', { value: system });
        Object.assign(window, {
          __OPENBITFUN_BOOTSTRAP_LOCALE__: saved,
          // Older hosts may still inject their own copy; the surface owns it now.
          __OPENBITFUN_BOOTSTRAP_MESSAGES__: { loadingApp: 'Legacy startup copy' },
        });
      },
    });

    expect(dom.window.document.documentElement.lang).toBe(expected);
    expect(dom.window.document.querySelector('.splash-screen__message')?.textContent)
      .toBe(readLoadingMessage(expected!));
    dom.window.close();
  });

  it('uses injected startup locale text and mirrors native window-control state', async () => {
    let isMaximized = true;
    const invoke = vi.fn().mockImplementation((_command, payload) => {
      const action = (payload as { request: { action: string } }).request.action;
      if (action === 'toggle_maximize') isMaximized = !isMaximized;
      return Promise.resolve({ isMaximized });
    });
    const dom = new JSDOM(readIndexHtml(), {
      url: 'http://localhost:1422/',
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.defineProperty(window.navigator, 'platform', { value: 'Win32' });
        Object.assign(window, {
          __OPENBITFUN_BOOTSTRAP_LOCALE__: 'zh-CN',
          __OPENBITFUN_SHOW_STARTUP_WINDOW_CONTROLS__: true,
          __TAURI_INTERNALS__: { invoke },
        });
      },
    });

    const hint = dom.window.document.querySelector('.splash-screen__message');
    expect(dom.window.document.documentElement.lang).toBe('zh-CN');
    expect(dom.window.document.getElementById('root')?.childElementCount).toBe(0);
    expect(dom.window.document.getElementById('openbitfun-startup-overlay')).not.toBeNull();
    expect(hint?.textContent).toBe('正在启动 OpenBitFun...');

    const controls = dom.window.document.querySelector<HTMLElement>('[data-startup-window-controls]');
    expect(controls?.hidden).toBe(false);
    expect(controls?.classList.contains('window-controls')).toBe(true);
    expect(controls?.classList.contains('window-controls--windows')).toBe(true);
    expect(controls?.getAttribute('data-openbitfun-component')).toBe('window-controls');
    expect(dom.window.document.querySelector('.splash-screen')?.hasAttribute('aria-hidden')).toBe(false);
    await Promise.resolve();

    const minimizeButton = dom.window.document.querySelector<HTMLButtonElement>('[data-startup-window-action="minimize"]');
    expect(minimizeButton?.className).toBe('window-controls__btn window-controls__btn--minimize');
    expect(minimizeButton?.querySelector('path')?.getAttribute('d')).toBe('M1 6.5h10');
    expect(minimizeButton?.querySelectorAll('svg')).toHaveLength(1);

    const maximizeButton = dom.window.document.querySelector<HTMLButtonElement>('[data-startup-window-action="toggle_maximize"]');
    const visibleMaximizePath = () => Array.from(maximizeButton?.querySelectorAll('svg') ?? [])
      .find(glyph => glyph.style.display !== 'none')
      ?.querySelector('path')
      ?.getAttribute('d');
    expect(controls?.getAttribute('data-openbitfun-state')).toBe('maximized');
    expect(maximizeButton?.getAttribute('aria-label')).toBe('还原');
    expect(visibleMaximizePath()).toBe('M3.5 3.5v-2h7v7h-2 M1.5 3.5h7v7h-7z');

    dom.window.document.documentElement.lang = 'zh-TW';
    await Promise.resolve();
    expect(hint?.textContent).toBe(readLoadingMessage('zh-TW'));
    expect(maximizeButton?.getAttribute('aria-label')).toBe('還原');
    dom.window.document.documentElement.lang = 'zh-CN';
    await Promise.resolve();

    maximizeButton?.click();
    await Promise.resolve();
    expect(controls?.hasAttribute('data-openbitfun-state')).toBe(false);
    expect(maximizeButton?.getAttribute('aria-label')).toBe('最大化');
    expect(visibleMaximizePath()).toBe('M1.5 1.5h9v9h-9z');

    const closeButton = dom.window.document.querySelector<HTMLButtonElement>('[data-startup-window-action="close"]');
    expect(closeButton?.getAttribute('aria-label')).toBe('关闭');
    closeButton?.click();

    expect(invoke).toHaveBeenCalledWith('startup_window_control', {
      request: { action: 'close' },
    });
    expect(invoke).toHaveBeenCalledWith('startup_window_control', {
      request: { action: 'get_state' },
    });

    const html = readIndexHtml();
    expect(html).toContain('href="/src/app/components/WindowControls/WindowControls.scss"');
    expect(html).toContain('.window-controls.openbitfun-startup-window-controls');
    expect(html).not.toContain('openbitfun-startup-window-controls__btn');
    expect(readWindowControlsSource()).not.toContain("import './WindowControls.scss'");
    dom.window.close();
  });

  it('updates a system-language hint when the app resolves its saved language', async () => {
    const dom = new JSDOM(readIndexHtml(), {
      url: 'http://localhost:1422/',
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.defineProperty(window.navigator, 'language', { value: 'en-US' });
      },
    });
    const hint = dom.window.document.querySelector('.splash-screen__message');
    expect(hint?.textContent).toBe(readLoadingMessage('en-US'));

    dom.window.document.documentElement.lang = 'zh-CN';
    await Promise.resolve();
    expect(hint?.textContent).toBe(readLoadingMessage('zh-CN'));

    dom.window.dispatchEvent(new dom.window.Event('openbitfun:startup-overlay-hidden'));
    dom.window.document.documentElement.lang = 'en-US';
    await Promise.resolve();
    expect(hint?.textContent).toBe(readLoadingMessage('zh-CN'));
    dom.window.close();
  });

  it('shows the independent pet preload for the companion window', () => {
    const html = readIndexHtml();
    const dom = new JSDOM(html, {
      url: 'http://localhost:1422/?openbitfunWindow=agent-companion',
      runScripts: 'dangerously',
      beforeParse(window) {
        Object.assign(window, {
          __OPENBITFUN_BOOTSTRAP_LOCALE__: 'en-US',
        });
      },
    });

    expect(dom.window.document.body.classList.contains('openbitfun-pet-preload-body')).toBe(true);
    expect(dom.window.document.getElementById('openbitfun-startup-overlay')).toBeNull();
    expect(dom.window.document.querySelector('.openbitfun-pet-preload__sprite')).not.toBeNull();
    expect(dom.window.document.querySelector('.splash-screen__logo')).toBeNull();
    expect(dom.window.document.querySelector('.openbitfun-sr-only')?.textContent).toBe('Loading companion...');
    const spriteCss = html.match(/\.openbitfun-pet-preload__sprite \{(?<css>[\s\S]*?)\n      \}/)?.groups?.css;
    expect(spriteCss).toBeDefined();
    expect(spriteCss).not.toContain('background:');
    expect(spriteCss).not.toContain('border:');
    expect(spriteCss).not.toContain('box-shadow:');
    dom.window.close();
  });
});
