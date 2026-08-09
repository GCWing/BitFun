import { describe, expect, it } from 'vitest';
import { parseReloadCommand, supportsLocalReloadContext } from './reloadCommand';

describe('parseReloadCommand', () => {
  it.each([
    ['/reload', 'all'],
    ['  /RELOAD  ', 'all'],
    ['/reload skills', 'skills'],
    ['/reload Instructions', 'instructions'],
    ['/reload-skills', 'skills'],
  ] as const)('parses %s as %s', (input, target) => {
    expect(parseReloadCommand(input)).toEqual({ kind: 'reload', target });
  });

  it('keeps unrelated input outside the local command path', () => {
    expect(parseReloadCommand('/reloader')).toBeNull();
    expect(parseReloadCommand('please /reload')).toBeNull();
  });

  it.each([
    '/reload mcp',
    '/reload skills instructions',
    '/reload\nskills\ninstructions',
    '/reload-skills instructions',
  ])('rejects unsupported form %s', input => {
    expect(parseReloadCommand(input)).toEqual({ kind: 'invalid' });
  });
});

describe('supportsLocalReloadContext', () => {
  it('allows only local Desktop sessions', () => {
    expect(supportsLocalReloadContext({
      desktopRuntime: true,
      acpSession: false,
      dispatchTransport: false,
    })).toBe(true);
    expect(supportsLocalReloadContext({
      desktopRuntime: false,
      acpSession: false,
      dispatchTransport: false,
    })).toBe(false);
    expect(supportsLocalReloadContext({
      desktopRuntime: true,
      acpSession: true,
      dispatchTransport: false,
    })).toBe(false);
    expect(supportsLocalReloadContext({
      desktopRuntime: true,
      acpSession: false,
      dispatchTransport: true,
    })).toBe(false);
  });
});
