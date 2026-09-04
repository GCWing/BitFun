import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-log', () => ({
  trace: vi.fn(() => Promise.resolve()),
  debug: vi.fn(() => Promise.resolve()),
  info: vi.fn(() => Promise.resolve()),
  warn: vi.fn(() => Promise.resolve()),
  error: vi.fn(() => Promise.resolve()),
}));

async function importLoggerWithBootstrapLevel(level: unknown) {
  vi.resetModules();
  if (level === undefined) {
    delete globalThis.__OPENBITFUN_BOOTSTRAP_LOG_LEVEL__;
  } else {
    globalThis.__OPENBITFUN_BOOTSTRAP_LOG_LEVEL__ = level as string;
  }
  return import('./logger');
}

describe('error formatting', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  // WebKit — what Tauri embeds on macOS — builds `stack` from frames only, so a
  // logged failure would otherwise arrive with a location and no reason.
  it('keeps the message when the stack omits it', async () => {
    const { createLogger } = await importLoggerWithBootstrapLevel('debug');
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const error = new Error('Target CLI installation failed');
    error.stack = '@tauri://localhost/assets/ChatPane.js:96:14836';

    createLogger('DispatchInstallDialog').warn('Failed to prepare SSH dispatch target', {
      connectionId: 'ssh-a',
      error,
    });

    const line = String(warn.mock.calls.at(-1)?.[0] ?? '');
    expect(line).toContain('Target CLI installation failed');
    expect(line).toContain('ChatPane.js:96:14836');
    expect(line).toContain('"connectionId":"ssh-a"');
  });

  it('does not repeat the message when the stack already carries it', async () => {
    const { createLogger } = await importLoggerWithBootstrapLevel('debug');
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const error = new Error('boom');
    error.stack = 'Error: boom\n    at somewhere';

    createLogger('Ctx').warn('failed', error);

    const line = String(warn.mock.calls.at(-1)?.[0] ?? '');
    expect(line).toBe('[Ctx] failed Error: boom\n    at somewhere');
  });
});

describe('logger bootstrap level', () => {
  afterEach(() => {
    delete globalThis.__OPENBITFUN_BOOTSTRAP_LOG_LEVEL__;
  });

  it('uses the native bootstrap log level before async config sync runs', async () => {
    const { LogLevel, logger } = await importLoggerWithBootstrapLevel('debug');

    expect(logger.getLevel()).toBe(LogLevel.DEBUG);
  });

  it('ignores invalid bootstrap levels and keeps the environment default', async () => {
    const baseline = await importLoggerWithBootstrapLevel(undefined);
    const expectedDefault = baseline.logger.getLevel();

    const { logger } = await importLoggerWithBootstrapLevel('verbose');

    expect(logger.getLevel()).toBe(expectedDefault);
  });
});
