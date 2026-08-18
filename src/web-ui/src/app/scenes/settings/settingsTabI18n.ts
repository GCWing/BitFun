/**
 * settingsTabI18n — i18n namespaces every settings tab renders from.
 *
 * Only WEB_UI_BOOTSTRAP_NAMESPACES ship with the initial bundle; everything else
 * resolves through the lazy namespace backend. A panel mounted before its
 * namespace lands paints raw i18n keys for a frame (react.useSuspense is off, so
 * `t()` returns the key) and then swaps to real copy — the visible "flash" on the
 * first open of a tab. Loading these alongside the tab's lazy chunk makes the
 * first open look identical to every later one.
 *
 * Keep in sync with the `useTranslation(...)` / `useI18n(...)` namespaces used by
 * each panel and the components it renders.
 */

import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import type { I18nNamespace } from '@/infrastructure/i18n/types';
import type { ConfigTab } from './settingsConfig';

/** Namespace behind the settings shell itself: category labels, tab labels, keyboard tab copy. */
export const SETTINGS_SHELL_NAMESPACES: readonly I18nNamespace[] = ['settings'];

export const SETTINGS_TAB_I18N_NAMESPACES: Record<ConfigTab, readonly I18nNamespace[]> = {
  basics: ['settings/basics'],
  appearance: ['settings/appearance', 'settings/basics'],
  models: ['settings/ai-model', 'settings/default-model', 'components'],
  'usage-statistics': ['settings/usage-statistics'],
  'archived-sessions': ['common'],
  worktrees: ['worktrees'],
  // Both session panels live in the same module and share its sub-sections.
  'session-personalization': ['settings/session-config', 'settings/agentic-tools'],
  'session-permissions': ['settings/session-config', 'settings/agentic-tools'],
  'quick-actions': ['settings/quick-actions'],
  'voice-input': ['settings/voice-input'],
  review: ['settings/review'],
  memories: ['settings/memories'],
  'mcp-tools': ['settings/mcp-tools', 'settings/mcp', 'shared'],
  'external-sources': ['settings/external-sources', 'settings/hooks', 'shared'],
  hooks: ['settings/external-sources', 'settings/hooks', 'shared'],
  'acp-agents': ['settings/acp-agents'],
  editor: ['settings/editor'],
  keyboard: ['settings'],
};

async function preloadNamespaces(namespaces: readonly I18nNamespace[]): Promise<void> {
  await Promise.all(
    namespaces.map((namespace) =>
      i18nService.loadNamespace(namespace).catch(() => {
        // A namespace that fails to resolve must not block tab activation;
        // i18next keeps falling back the same way it does today.
      })
    )
  );
}

export function preloadSettingsShellI18n(): Promise<void> {
  return preloadNamespaces(SETTINGS_SHELL_NAMESPACES);
}

export function preloadSettingsTabI18n(tab: ConfigTab): Promise<void> {
  return preloadNamespaces(SETTINGS_TAB_I18N_NAMESPACES[tab] ?? []);
}
