/**
 * BitFun API unified exports.
 *
 * Follows the BitFun Tauri command conventions.
 */

export * from './service-api/types';
export * from './service-api/ApiClient';
export * from './service-api/tauri-commands';
export * from './service-api/AIApi';

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
import { gitAPI } from './service-api/GitAPI';
import { gitAgentAPI } from './service-api/GitAgentAPI';
import { gitRepoHistoryAPI, type GitRepoHistory } from './service-api/GitRepoHistoryAPI';
import { startchatAgentAPI } from './service-api/StartchatAgentAPI';
import { conversationAPI } from './service-api/ConversationAPI';
import { i18nAPI } from './service-api/I18nAPI';
import { pluginAPI } from './service-api/PluginAPI';

// Export API modules
export { workspaceAPI, configAPI, aiApi, toolAPI, agentAPI, systemAPI, projectAPI, diffAPI, snapshotAPI, globalAPI, contextAPI, gitAPI, gitAgentAPI, gitRepoHistoryAPI, startchatAgentAPI, conversationAPI, i18nAPI, pluginAPI };

// Export types
export type { GitRepoHistory };

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
  git: gitAPI,
  gitAgent: gitAgentAPI,
  gitRepoHistory: gitRepoHistoryAPI,
  startchatAgent: startchatAgentAPI,
  conversation: conversationAPI,
  i18n: i18nAPI,
  plugin: pluginAPI,
};

// Default export
export default bitfunAPI;
