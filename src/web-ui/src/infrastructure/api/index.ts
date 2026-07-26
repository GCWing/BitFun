/**
 * BitFun API unified exports.
 *
 * Follows the BitFun Tauri command conventions.
 */

export * from './service-api/types';
export * from './service-api/ApiClient';
export * from './service-api/tauri-commands';
export * from './service-api/AIApi';
export * from './service-api/CronAPI';
export * from './service-api/PermissionAPI';
export * from './service-api/PageAPI';
export * from './service-api/SpeechAPI';

// Import API modules
import { workspaceAPI } from './service-api/WorkspaceAPI';
import { configAPI } from './service-api/ConfigAPI';
import { aiApi } from './service-api/AIApi';
import { toolAPI } from './service-api/ToolAPI';
import { agentAPI } from './service-api/AgentAPI';
import { systemAPI } from './service-api/SystemAPI';
import { projectAPI } from './service-api/ProjectAPI';
import { diffAPI } from './service-api/DiffAPI';
import { snapshotAPI } from './service-api/SnapshotAPI';
import { globalAPI } from './service-api/GlobalAPI';
import { contextAPI } from './service-api/ContextAPI';
import { cronAPI } from './service-api/CronAPI';
import { permissionAPI } from './service-api/PermissionAPI';
import { pageAPI } from './service-api/PageAPI';
import { gitAPI } from './service-api/GitAPI';
import { gitAgentAPI } from './service-api/GitAgentAPI';
import { gitRepoHistoryAPI, type GitRepoHistory } from './service-api/GitRepoHistoryAPI';
import { startchatAgentAPI } from './service-api/StartchatAgentAPI';
import { sessionAPI } from './service-api/SessionAPI';
import { i18nAPI } from './service-api/I18nAPI';
import { btwAPI } from './service-api/BtwAPI';
import { editorAiAPI } from './service-api/EditorAiAPI';
import { reviewPlatformAPI } from './service-api/ReviewPlatformAPI';
import { insightsApi } from './insightsApi';
import { speechAPI } from './service-api/SpeechAPI';

// Export API modules
export { workspaceAPI, configAPI, aiApi, toolAPI, agentAPI, systemAPI, projectAPI, diffAPI, snapshotAPI, globalAPI, contextAPI, cronAPI, permissionAPI, pageAPI, gitAPI, gitAgentAPI, gitRepoHistoryAPI, startchatAgentAPI, sessionAPI, i18nAPI, btwAPI, editorAiAPI, reviewPlatformAPI, insightsApi, speechAPI };
export * from './service-api/ReviewPlatformAPI';

// Export types
export type { GitRepoHistory };
export type { CheckForUpdatesResponse } from './service-api/SystemAPI';

// BitFun API collection: a single access point for all API modules.
export const bitfunAPI = {
  workspace: workspaceAPI,
  config: configAPI,
  ai: aiApi,
  tool: toolAPI,
  agent: agentAPI,
  system: systemAPI,
  project: projectAPI,
  diff: diffAPI,
  snapshot: snapshotAPI,
  global: globalAPI,
  context: contextAPI,
  cron: cronAPI,
  permission: permissionAPI,
  pages: pageAPI,
  git: gitAPI,
  gitAgent: gitAgentAPI,
  gitRepoHistory: gitRepoHistoryAPI,
  startchatAgent: startchatAgentAPI,
  session: sessionAPI,
  i18n: i18nAPI,
  btw: btwAPI,
  editorAi: editorAiAPI,
  reviewPlatform: reviewPlatformAPI,
  insights: insightsApi,
  speech: speechAPI,
};

// Default export
export default bitfunAPI;
