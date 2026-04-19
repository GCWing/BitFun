
import type { ShortcutConfig, ShortcutScope } from '@/shared/types/shortcut';

/**
 * A complete shortcut definition: default key binding + metadata.
 * The `descriptionKey` is an i18n key under the 'settings' namespace.
 */
export interface ShortcutDef {
  id: string;
  config: ShortcutConfig;
  /** i18n key for the human-readable shortcut description */
  descriptionKey: string;
}

/**
 * These bindings always use catalog defaults; user overrides in config are ignored
 * and the keyboard settings UI does not allow remapping them.
 */
export const NON_USER_CUSTOMIZABLE_SHORTCUT_IDS = new Set<string>();

// ─── Helpers ───────────────────────────────────────────────────────────────

const isMac = typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');

/** Build a ShortcutConfig using Ctrl (Win/Linux) or Meta (Mac) as the primary modifier. */
function mod(
  key: string,
  extras: Omit<ShortcutConfig, 'key' | 'ctrl' | 'meta'> = {}
): ShortcutConfig {
  return isMac ? { key, meta: true, ...extras } : { key, ctrl: true, ...extras };
}

// ─── Global shortcuts (scope: 'app') ──────────────────────────────────────
// These fire regardless of where focus is, including inside input elements
// (where allowInInput: true) or from any scene. Use sparingly — only for
// navigation and layout operations that must always be reachable.

export const APP_SHORTCUTS: ShortcutDef[] = [
  // Settings list order: 搜索 → 返回 Agentic OS → 其余（见 compareAppShortcutIdsForSettings）
  {
    id: 'nav.toggleSearch',
    config: { key: 'f', alt: true, scope: 'app', allowInInput: true },
    descriptionKey: 'keyboard.shortcuts.nav.toggleSearch',
  },
  {
    id: 'scene.escapeToAgenticBase',
    config: { key: 'Escape', scope: 'app', allowInInput: true },
    descriptionKey: 'keyboard.shortcuts.scene.escapeToAgenticBase',
  },
  {
    id: 'scene.openSettings',
    config: mod(',', { scope: 'app', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.scene.openSettings',
  },
  {
    id: 'scene.openTerminal',
    config: mod('`', { shift: true, scope: 'app', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.scene.openTerminal',
  },
];

// ─── Editor canvas shortcuts (scope: 'canvas') ────────────────────────────
// Active only when focus is inside the editor canvas area
// (data-shortcut-scope="canvas"). Tab-switch shortcuts retain allowInInput
// so they continue to work while a Monaco editor is focused.

export const CANVAS_SHORTCUTS: ShortcutDef[] = [
  // View
  {
    id: 'canvas.missionControl',
    config: mod('Tab', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.canvas.missionControl',
  },
  {
    id: 'canvas.splitHorizontal',
    config: mod('\\', { scope: 'canvas' }),
    descriptionKey: 'keyboard.shortcuts.canvas.splitHorizontal',
  },
  {
    id: 'canvas.splitVertical',
    config: mod('\\', { shift: true, scope: 'canvas' }),
    descriptionKey: 'keyboard.shortcuts.canvas.splitVertical',
  },
  {
    id: 'canvas.anchorZone',
    config: mod('`', { scope: 'canvas' }),
    descriptionKey: 'keyboard.shortcuts.canvas.anchorZone',
  },
  {
    id: 'canvas.maximize',
    config: mod('M', { shift: true, scope: 'canvas' }),
    descriptionKey: 'keyboard.shortcuts.canvas.maximize',
  },
  {
    id: 'canvas.closePreview',
    config: { key: 'Escape', scope: 'canvas', allowInInput: true },
    descriptionKey: 'keyboard.shortcuts.canvas.closePreview',
  },

  // Tab management
  {
    id: 'tab.close',
    config: mod('W', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.close',
  },
  {
    id: 'tab.reopenClosed',
    config: mod('T', { shift: true, scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.reopenClosed',
  },
  {
    id: 'tab.switch1',
    config: mod('1', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch2',
    config: mod('2', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch3',
    config: mod('3', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch4',
    config: mod('4', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch5',
    config: mod('5', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch6',
    config: mod('6', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch7',
    config: mod('7', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switch8',
    config: mod('8', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
  {
    id: 'tab.switchLast',
    config: mod('9', { scope: 'canvas', allowInInput: true }),
    descriptionKey: 'keyboard.shortcuts.tab.switchMerged',
  },
];

// ─── Chat shortcuts (scope: 'chat') ───────────────────────────────────────

export const CHAT_SHORTCUTS: ShortcutDef[] = [
  {
    id: 'btw-fill',
    config: { key: 'B', ctrl: true, alt: true, scope: 'chat', allowInInput: true },
    descriptionKey: 'keyboard.shortcuts.chat.btwFill',
  },
  {
    id: 'chat.stopGeneration',
    config: { key: 'Escape', scope: 'chat', allowInInput: true },
    descriptionKey: 'keyboard.shortcuts.chat.stopGeneration',
  },
  {
    id: 'chat.newSession',
    config: mod('N', { scope: 'chat' }),
    descriptionKey: 'keyboard.shortcuts.chat.newSession',
  },
];

// ─── File tree shortcuts (scope: 'filetree') ──────────────────────────────

export const FILETREE_SHORTCUTS: ShortcutDef[] = [
  {
    id: 'filetree.rename',
    config: { key: 'F2', scope: 'filetree' },
    descriptionKey: 'keyboard.shortcuts.filetree.rename',
  },
  {
    id: 'filetree.delete',
    config: { key: 'Delete', scope: 'filetree' },
    descriptionKey: 'keyboard.shortcuts.filetree.delete',
  },
  {
    id: 'filetree.refresh',
    config: { key: 'F5', scope: 'filetree' },
    descriptionKey: 'keyboard.shortcuts.filetree.refresh',
  },
  {
    id: 'filetree.newFile',
    config: mod('N', { scope: 'filetree' }),
    descriptionKey: 'keyboard.shortcuts.filetree.newFile',
  },
  {
    id: 'filetree.newFolder',
    config: mod('N', { shift: true, scope: 'filetree' }),
    descriptionKey: 'keyboard.shortcuts.filetree.newFolder',
  },
  {
    id: 'filetree.collapseAll',
    config: mod('[', { shift: true, scope: 'filetree' }),
    descriptionKey: 'keyboard.shortcuts.filetree.collapseAll',
  },
];

// ─── All shortcuts combined ────────────────────────────────────────────────

export const ALL_SHORTCUTS: ShortcutDef[] = [
  ...APP_SHORTCUTS,
  ...CANVAS_SHORTCUTS,
  ...CHAT_SHORTCUTS,
  ...FILETREE_SHORTCUTS,
];

/** App-scope rows listed first in Settings → Keyboard; others follow `ALL_SHORTCUTS` order. */
const APP_SHORTCUT_SETTINGS_LEADERS = ['nav.toggleSearch', 'scene.escapeToAgenticBase'] as const;

export function compareAppShortcutIdsForSettings(aId: string, bId: string): number {
  const rank = (id: string): number => {
    const lead = (APP_SHORTCUT_SETTINGS_LEADERS as readonly string[]).indexOf(id);
    if (lead !== -1) return lead;
    const cat = ALL_SHORTCUTS.findIndex((d) => d.id === id);
    return 1000 + (cat < 0 ? 9999 : cat);
  };
  return rank(aId) - rank(bId);
}

/** Shortcuts registered in code but not listed in ALL_SHORTCUTS (e.g. legacy ids). */
const EXTRA_SHORTCUT_DESCRIPTION_KEYS: Record<string, string> = {};

/**
 * Resolve the i18n key (settings namespace) for a shortcut id.
 * Used by the keyboard settings UI so labels always match known definitions.
 */
export function getShortcutDescriptionI18nKey(id: string): string | undefined {
  const fromCatalog = ALL_SHORTCUTS.find((d) => d.id === id)?.descriptionKey;
  if (fromCatalog) return fromCatalog;
  return EXTRA_SHORTCUT_DESCRIPTION_KEYS[id];
}

/** Scope display order for the settings UI. */
export const SCOPE_ORDER: ShortcutScope[] = ['app', 'chat', 'canvas', 'filetree'];

/** i18n keys for scope group labels in the settings UI. */
export const SCOPE_LABEL_KEYS: Record<ShortcutScope, string> = {
  app:      'keyboard.scopes.app',
  chat:     'keyboard.scopes.chat',
  canvas:   'keyboard.scopes.canvas',
  filetree: 'keyboard.scopes.filetree',
};
