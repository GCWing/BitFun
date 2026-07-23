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
    delete globalThis.__BITFUN_BOOTSTRAP_LOG_LEVEL__;
  } else {
    globalThis.__BITFUN_BOOTSTRAP_LOG_LEVEL__ = level as string;
  }
  return import('./logger');
}

describe('logger bootstrap level', () => {
  afterEach(() => {
    delete globalThis.__BITFUN_BOOTSTRAP_LOG_LEVEL__;
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
