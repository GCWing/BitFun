import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';

export function shouldBlockBrowserShortcut(key: string, allowPageReload: boolean): boolean {
  const normalizedKey = key.toLowerCase();
  return normalizedKey === 'f' || normalizedKey === 'p' || (normalizedKey === 'r' && !allowPageReload);
}

/** Capture browser shortcuts before focused editors and terminals consume them. */
export function handleBrowserShortcut(
  event: KeyboardEvent,
  allowPageReload: boolean,
  adjustFontSize: (delta: -1 | 1) => void,
  platform = navigator.platform,
): void {
  if (!(event.ctrlKey || event.metaKey)) return;

  if (shouldBlockBrowserShortcut(event.key, allowPageReload)) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }

  const isMac = platform.toUpperCase().includes('MAC');
  const primary = isMac
    ? event.metaKey && !event.ctrlKey
    : event.ctrlKey && !event.metaKey;
  if (!primary || event.altKey || isImeOwnedKeyboardEvent(event)) return;

  // '+' may require Shift; '=' is the conventional unshifted zoom-in binding.
  const delta = event.key === '+' || event.key === '=' || event.code === 'NumpadAdd'
    ? 1
    : event.key === '-' || event.key === '_' || event.code === 'NumpadSubtract'
      ? -1
      : 0;
  if (!delta) return;

  event.preventDefault();
  event.stopPropagation();
  adjustFontSize(delta);
}
