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
  | 'application.input'
  | 'application.development'
  | 'ai.models'
  | 'ai.memory'
  | 'workspace.session'
  | 'workspace.worktrees'
  | 'tools.execution'
  | 'tools.automation'
  | 'tools.mcp'
  | 'tools.acp'
  | 'data.usage'
  | 'data.archived'
  | 'data.diagnostics';

export type SettingsViewId =
  | 'voice'
  | 'shortcuts'
  | 'editor'
  | 'terminal'
  | 'quick-actions'
  | 'hooks';

export interface SettingsDestination {
  pageId: SettingsPageId;
  viewId?: SettingsViewId;
}

export interface SettingsPageProps {
  viewId?: SettingsViewId;
  navigationRequestId: number;
}
