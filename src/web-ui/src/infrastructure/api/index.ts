/**
 * OpenBitFun API unified exports.
 *
 * Follows the OpenBitFun Tauri command conventions.
 */

export * from './service-api/types';
export * from './service-api/ApiClient';
export * from './service-api/tauri-commands';
export * from './service-api/AIApi';
export * from './service-api/CronAPI';
export * from './service-api/PermissionAPI';
export * from './service-api/PageAPI';
export * from './service-api/SpeechAPI';
export * from './service-api/WorktreeAPI';
export * from './service-api/ProductControlAPI';

// Import API modules
import { workspaceAPI } from './service-api/WorkspaceAPI';
import { configAPI } from './service-api/ConfigAPI';
import { aiApi } from './service-api/AIApi';
import { toolAPI } from './service-api/ToolAPI';
import { agentAPI } from './service-api/AgentAPI';
import { systemAPI } from './service-api/SystemAPI';
import { diffAPI } from './service-api/DiffAPI';
import { snapshotAPI } from './service-api/SnapshotAPI';
import { globalAPI } from './service-api/GlobalAPI';
import { contextAPI } from './service-api/ContextAPI';
import { cronAPI } from './service-api/CronAPI';
import { permissionAPI } from './service-api/PermissionAPI';
import { pageAPI } from './service-api/PageAPI';
import { gitAPI } from './service-api/GitAPI';
import { gitAgentAPI } from './service-api/GitAgentAPI';
import { sessionAPI } from './service-api/SessionAPI';
import { i18nAPI } from './service-api/I18nAPI';
import { btwAPI } from './service-api/BtwAPI';
import { editorAiAPI } from './service-api/EditorAiAPI';
import { reviewPlatformAPI } from './service-api/ReviewPlatformAPI';
import { insightsApi } from './insightsApi';
import { tokenUsageStatisticsApi } from './tokenUsageStatisticsApi';
import { speechAPI } from './service-api/SpeechAPI';
import { worktreeAPI } from './service-api/WorktreeAPI';
import { productControlAPI } from './service-api/ProductControlAPI';

// Export API modules
export { workspaceAPI, configAPI, aiApi, toolAPI, agentAPI, systemAPI, productControlAPI, diffAPI, snapshotAPI, globalAPI, contextAPI, cronAPI, permissionAPI, pageAPI, gitAPI, gitAgentAPI, sessionAPI, i18nAPI, btwAPI, editorAiAPI, reviewPlatformAPI, insightsApi, tokenUsageStatisticsApi, speechAPI, worktreeAPI };
export { TokenUsageStatisticsUnavailableError } from './tokenUsageStatisticsApi';
export * from './service-api/ReviewPlatformAPI';
export type {
  TokenUsageStatisticsRequest,
  UsageAttributionStatus,
  UsageGranularity,
  UsageStatistics,
  UsageStatisticsEntry,
  UsageStatisticsFilterKind,
  UsageTimeRange,
  UsageTrendPoint,
} from './tokenUsageStatisticsApi';

// Export types
export type { CheckForUpdatesResponse } from './service-api/SystemAPI';
