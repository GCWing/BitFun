export type SettingsCategoryId =
  | 'application'
  | 'ai'
  | 'workspace'
  | 'tools'
  | 'data';

export type SettingsPageId =
  | 'application.general'
  | 'application.appearance'
  | 'application.pet'
  | 'application.voice'
  | 'application.shortcuts'
  | 'application.terminal'
  | 'application.editor'
  | 'ai.models'
  | 'ai.memory'
  | 'workspace.session'
  | 'workspace.worktrees'
  | 'tools.execution'
  | 'tools.desktop-control'
  | 'tools.automation'
  | 'tools.webSearch'
  | 'tools.mcp'
  | 'tools.acp'
  | 'data.usage'
  | 'data.archived'
  | 'data.diagnostics';

export type SettingsViewId =
  | 'common'
  | 'advanced'
  | 'local'
  | 'ssh'
  | 'json'
  | 'quick-actions'
  | 'hooks';

export interface SettingsDestination {
  pageId: SettingsPageId;
  viewId?: SettingsViewId;
}

export interface SettingsPageProps {
  isActive?: boolean;
  viewId?: SettingsViewId;
  navigationRequestId: number;
}
