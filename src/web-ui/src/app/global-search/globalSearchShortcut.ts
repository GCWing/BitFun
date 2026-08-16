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
