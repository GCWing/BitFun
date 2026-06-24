let terminalOutputRendererPreload: Promise<typeof import('./TerminalOutputRenderer')> | undefined;

export function preloadTerminalOutputRenderer() {
  terminalOutputRendererPreload ??= import('./TerminalOutputRenderer');
  return terminalOutputRendererPreload;
}
