import { ALL_SHORTCUTS } from '@/shared/constants/shortcuts';
import { shortcutManager } from '@/infrastructure/services/ShortcutManager';

export const GLOBAL_SEARCH_SHORTCUT = ALL_SHORTCUTS.find(
  (definition) => definition.id === 'nav.toggleSearch',
)!;

export const subscribeGlobalSearchShortcut = (listener: () => void): (() => void) =>
  shortcutManager.subscribeRegistrationChanges(listener);

export const getGlobalSearchShortcutLabel = (): string => shortcutManager.formatShortcut(
  shortcutManager.getEffectiveConfig(GLOBAL_SEARCH_SHORTCUT.id, GLOBAL_SEARCH_SHORTCUT.config),
);

export interface GlobalSearchShortcutHint {
  key: string;
  modifier?: string;
}

/** Keeps the platform modifier and terminal key in KeyHint's compact two-part anatomy. */
export const splitGlobalSearchShortcutLabel = (label: string): GlobalSearchShortcutHint => {
  const separatorIndex = label.lastIndexOf('+');
  if (separatorIndex > 0 && separatorIndex < label.length - 1) {
    return {
      modifier: label.slice(0, separatorIndex).replace(/\+/g, ' '),
      key: label.slice(separatorIndex + 1),
    };
  }

  const macLabel = label.match(/^([⌘⇧⌥]+)(.+)$/u);
  if (macLabel) {
    return { modifier: macLabel[1], key: macLabel[2] };
  }

  return { key: label };
};
