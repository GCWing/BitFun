export type ReloadTarget = 'all' | 'skills' | 'instructions';

export type ParsedReloadCommand =
  | { kind: 'reload'; target: ReloadTarget }
  | { kind: 'invalid' };

export function supportsLocalReloadContext(options: {
  desktopRuntime: boolean;
  acpSession: boolean;
  dispatchTransport: boolean;
}): boolean {
  return options.desktopRuntime && !options.acpSession && !options.dispatchTransport;
}

export function parseReloadCommand(input: string): ParsedReloadCommand | null {
  const command = input.trim();
  if (/^\/reload-skills$/i.test(command)) {
    return { kind: 'reload', target: 'skills' };
  }
  if (/^\/reload-skills(?:\s|$)/i.test(command)) {
    return { kind: 'invalid' };
  }
  if (!/^\/reload(?:\s|$)/i.test(command)) {
    return null;
  }

  const target = command.slice('/reload'.length).trim().toLowerCase();
  if (!target) {
    return { kind: 'reload', target: 'all' };
  }
  if (target === 'skills' || target === 'instructions') {
    return { kind: 'reload', target };
  }
  return { kind: 'invalid' };
}
