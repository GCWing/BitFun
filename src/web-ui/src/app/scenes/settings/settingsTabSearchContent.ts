/**
 * i18n keys for in-page section titles/descriptions (and related copy) per settings tab.
 * Used by SettingsNav search so queries match content inside each config page.
 *
 * Keep in sync when adding ConfigPageSection / page headers on these tabs.
 */

import type { ConfigTab } from './settingsConfig';

export interface SettingsTabSearchPhrase {
  ns: string;
  key: string;
}

/** Phrases resolved at runtime with i18n.getFixedT(lang, ns)(key). */
export const SETTINGS_TAB_SEARCH_CONTENT: Record<ConfigTab, readonly SettingsTabSearchPhrase[]> = {
  basics: [
    { ns: 'settings/basics', key: 'title' },
    { ns: 'settings/basics', key: 'subtitle' },
    { ns: 'settings/basics', key: 'logging.sections.logging' },
    { ns: 'settings/basics', key: 'logging.sections.loggingHint' },
    { ns: 'settings/basics', key: 'terminal.sections.terminal' },
    { ns: 'settings/basics', key: 'terminal.sections.terminalHint' },
    { ns: 'settings/basics', key: 'notifications.title' },
    { ns: 'settings/basics', key: 'notifications.hint' },
  ],

  appearance: [
    { ns: 'settings/appearance', key: 'title' },
    { ns: 'settings/appearance', key: 'subtitle' },
    { ns: 'settings/basics', key: 'appearance.title' },
    { ns: 'settings/basics', key: 'appearance.hint' },
    { ns: 'settings/basics', key: 'appearance.fontSize.title' },
    { ns: 'settings/basics', key: 'appearance.fontSize.hint' },
  ],

  models: [
    { ns: 'settings/ai-model', key: 'title' },
    { ns: 'settings/ai-model', key: 'subtitle' },
    { ns: 'settings/default-model', key: 'tabs.default' },
    { ns: 'settings/default-model', key: 'subtitle' },
    { ns: 'settings/default-model', key: 'tabs.models' },
    { ns: 'settings/ai-model', key: 'subtitle' },
    { ns: 'settings/default-model', key: 'tabs.proxy' },
    { ns: 'settings/ai-model', key: 'proxy.enableHint' },
  ],

  'session-personalization': [
    { ns: 'settings/session-config', key: 'personalizationPage.title' },
    { ns: 'settings/session-config', key: 'personalizationPage.subtitle' },
    { ns: 'settings/session-config', key: 'features.sessionTitle.title' },
    { ns: 'settings/session-config', key: 'features.sessionTitle.subtitle' },
    { ns: 'settings/session-config', key: 'features.agentCompanion.title' },
    { ns: 'settings/session-config', key: 'features.agentCompanion.subtitle' },
    { ns: 'settings/session-config', key: 'models.empty' },
  ],

  'session-permissions': [
    { ns: 'settings/session-config', key: 'permissionsPage.title' },
    { ns: 'settings/session-config', key: 'permissionsPage.subtitle' },
    { ns: 'settings/session-config', key: 'features.workspaceSearch.title' },
    { ns: 'settings/session-config', key: 'features.workspaceSearch.subtitle' },
    { ns: 'settings/session-config', key: 'features.workspaceSearch.enable' },
    { ns: 'settings/session-config', key: 'toolExecution.sectionTitle' },
    { ns: 'settings/session-config', key: 'toolExecution.sectionDescription' },
    { ns: 'settings/session-config', key: 'computerUse.sectionTitle' },
    { ns: 'settings/session-config', key: 'computerUse.sectionDescription' },
    { ns: 'settings/session-config', key: 'computerUse.enable' },
    { ns: 'settings/session-config', key: 'computerUse.enableDesc' },
    { ns: 'settings/session-config', key: 'browserControl.sectionTitle' },
    { ns: 'settings/session-config', key: 'browserControl.sectionDescription' },
    { ns: 'settings/agentic-tools', key: 'config.autoExecute' },
    { ns: 'settings/agentic-tools', key: 'config.autoExecuteDesc' },
    { ns: 'settings/agentic-tools', key: 'config.confirmTimeout' },
    { ns: 'settings/agentic-tools', key: 'config.confirmTimeoutDesc' },
    { ns: 'settings/agentic-tools', key: 'config.executionTimeout' },
    { ns: 'settings/agentic-tools', key: 'config.executionTimeoutDesc' },
    { ns: 'settings/debug', key: 'sections.combined' },
    { ns: 'settings/debug', key: 'sections.combinedDescription' },
    { ns: 'settings/debug', key: 'settings.logPath.label' },
    { ns: 'settings/debug', key: 'settings.logPath.description' },
    { ns: 'settings/debug', key: 'settings.ingestPort.label' },
    { ns: 'settings/debug', key: 'settings.ingestPort.description' },
    { ns: 'settings/debug', key: 'sections.templates' },
    { ns: 'settings/debug', key: 'templates.description' },
  ],

  review: [
    { ns: 'settings/review', key: 'title' },
    { ns: 'settings/review', key: 'subtitle' },
    { ns: 'settings/review', key: 'overview.title' },
    { ns: 'settings/review', key: 'overview.description' },
    { ns: 'settings/review', key: 'strategy.title' },
    { ns: 'settings/review', key: 'execution.title' },
    { ns: 'settings/review', key: 'members.title' },
    { ns: 'settings/review', key: 'extra.title' },
  ],

  'mcp-tools': [
    { ns: 'settings/mcp-tools', key: 'title' },
    { ns: 'settings/mcp-tools', key: 'subtitle' },
    { ns: 'settings/mcp', key: 'section.serverList.title' },
    { ns: 'settings/mcp', key: 'section.serverList.description' },
  ],

  'acp-agents': [
    { ns: 'settings/acp-agents', key: 'title' },
    { ns: 'settings/acp-agents', key: 'subtitle' },
    { ns: 'settings/acp-agents', key: 'registry.title' },
    { ns: 'settings/acp-agents', key: 'registry.description' },
    { ns: 'settings/acp-agents', key: 'json.title' },
  ],

  editor: [
    { ns: 'settings/editor', key: 'title' },
    { ns: 'settings/editor', key: 'subtitle' },
    { ns: 'settings/editor', key: 'sections.appearance.title' },
    { ns: 'settings/editor', key: 'sections.appearance.description' },
    { ns: 'settings/editor', key: 'sections.behavior.title' },
    { ns: 'settings/editor', key: 'sections.behavior.description' },
    { ns: 'settings/editor', key: 'sections.display.title' },
    { ns: 'settings/editor', key: 'sections.display.description' },
    { ns: 'settings/editor', key: 'sections.advanced.title' },
    { ns: 'settings/editor', key: 'sections.advanced.description' },
    { ns: 'settings/editor', key: 'actions.save' },
    { ns: 'settings/editor', key: 'actions.saveDesc' },
  ],

  keyboard: [
    { ns: 'settings', key: 'keyboard.title' },
    { ns: 'settings', key: 'keyboard.description' },
    { ns: 'settings', key: 'keyboard.scopes.app' },
    { ns: 'settings', key: 'keyboard.scopes.chat' },
    { ns: 'settings', key: 'keyboard.scopes.filetree' },
    { ns: 'settings', key: 'keyboard.scopes.git' },
    { ns: 'settings', key: 'keyboard.shortcuts.panel.toggleLeft' },
    { ns: 'settings', key: 'keyboard.shortcuts.tab.close' },
    { ns: 'settings', key: 'keyboard.shortcuts.scene.focusMerged' },
    { ns: 'settings', key: 'keyboard.shortcuts.scene.focusMergedHint' },
    { ns: 'settings', key: 'keyboard.shortcuts.tab.switchMerged' },
    { ns: 'settings', key: 'keyboard.shortcuts.tab.switchMergedHint' },
  ],

  'quick-actions': [
    { ns: 'settings/quick-actions', key: 'page.title' },
    { ns: 'settings/quick-actions', key: 'page.subtitle' },
    { ns: 'settings/quick-actions', key: 'sections.builtin.title' },
    { ns: 'settings/quick-actions', key: 'sections.custom.title' },
  ],

  // lsp: [ ... ], // nav entry temporarily hidden; omit from search index
};
