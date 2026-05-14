type FullscreenShortcutEvent = Pick<
  KeyboardEvent,
  'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'
>;

const isMacPlatform = (platform: string): boolean =>
  platform.toUpperCase().includes('MAC');

/**
 * OS-window fullscreen shortcut detection.
 *
 * This intentionally does not use the generic "mod" shortcut mapping:
 * fullscreen follows OS conventions (`F11` on Windows/Linux and
 * `Control+Command+F` on macOS), while app shortcuts fold Ctrl into Cmd on
 * macOS. Keeping this separate prevents callers from confusing fullscreen
 * with maximize or an internal panel fullscreen action.
 */
export const isWindowFullscreenShortcut = (
  event: FullscreenShortcutEvent,
  platform = typeof navigator !== 'undefined' ? navigator.platform : ''
): boolean => {
  const key = event.key.toLowerCase();

  if (isMacPlatform(platform)) {
    return (
      key === 'f' &&
      event.ctrlKey &&
      event.metaKey &&
      !event.altKey &&
      !event.shiftKey
    );
  }

  return (
    key === 'f11' &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey &&
    !event.shiftKey
  );
};
