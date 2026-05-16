import { agentAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { createBtwChildSession } from '../../services/BtwThreadService';
import { closeBtwSessionInAuxPane, openBtwSessionInAuxPane } from '../../services/openBtwSession';
import { FlowChatManager } from '../../services/FlowChatManager';
import { flowChatStore } from '../../store/FlowChatStore';
import { insertReviewSessionSummaryMarker } from '../../services/ReviewSessionMarkerService';
import {
  buildEffectiveReviewTeamManifest,
  buildReviewTeamPromptBlock,
  loadDefaultReviewTeam,
  loadReviewTeamProjectStrategyOverride,
  loadReviewTeamRateLimitStatus,
  prepareDefaultReviewTeamForLaunch,
  type ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';
import { classifyReviewTargetFromFiles } from '@/shared/services/reviewTargetClassifier';
import {
  getDeepReviewCommandFocus,
} from './commandParser';
import {
  buildUnknownChangeStats,
  resolveSlashCommandReviewTarget,
} from './targetResolver';
import {
  formatSessionFilesLaunchPrompt,
  formatSlashCommandLaunchPrompt,
} from './launchPrompt';
import {
  buildLaunchCleanupError,
  createDeepReviewLaunchError,
  isSessionMissingError,
  normalizeErrorMessage,
  type DeepReviewLaunchStep,
  type FailedDeepReviewCleanupResult,
} from './launchErrors';

export {
  DEEP_REVIEW_SLASH_COMMAND,
  isDeepReviewSlashCommand,
} from './commandParser';
export { getDeepReviewLaunchErrorMessage } from './launchErrors';

const log = createLogger('DeepReviewService');

interface LaunchDeepReviewSessionParams {
  parentSessionId: string;
  workspacePath?: string;
  prompt: string;
  displayMessage: string;
  childSessionName?: string;
  requestedFiles?: string[];
  runManifest?: ReviewTeamRunManifest;
}

export interface DeepReviewLaunchPrompt {
  prompt: string;
  runManifest: ReviewTeamRunManifest;
}

async function cleanupFailedDeepReviewLaunch(
  childSessionId: string,
  launchStep: DeepReviewLaunchStep,
): Promise<FailedDeepReviewCleanupResult> {
  const cleanupIssues: string[] = [];
  const childSession = flowChatStore.getState().sessions.get(childSessionId);
  const workspacePath = childSession?.workspacePath;
  const remoteConnectionId = childSession?.remoteConnectionId;
  const remoteSshHost = childSession?.remoteSshHost;

  try {
    closeBtwSessionInAuxPane(childSessionId);
  } catch (error) {
    const message = `Failed to close the deep review pane during cleanup: ${normalizeErrorMessage(error)}`;
    cleanupIssues.push(message);
    log.warn(message, { childSessionId, launchStep, error });
  }

  let backendSessionRemoved = false;
  if (!workspacePath) {
    const message = 'Workspace path is missing, so backend deep review session cleanup could not run.';
    cleanupIssues.push(message);
    log.warn(message, { childSessionId, launchStep });
  } else {
    try {
      await agentAPI.deleteSession(
        childSessionId,
        workspacePath,
        remoteConnectionId,
        remoteSshHost,
      );
      backendSessionRemoved = true;
    } catch (error) {
      if (isSessionMissingError(error)) {
        backendSessionRemoved = true;
      } else {
        const message = `Failed to delete the backend deep review session: ${normalizeErrorMessage(error)}`;
        cleanupIssues.push(message);
        log.warn(message, { childSessionId, launchStep, error });
      }
    }
  }

  if (backendSessionRemoved) {
    try {
      const flowChatManager = FlowChatManager.getInstance();
      flowChatManager.discardLocalSession(childSessionId);
    } catch (error) {
      const message = `Failed to remove the local deep review session state: ${normalizeErrorMessage(error)}`;
      cleanupIssues.push(message);
      log.warn(message, { childSessionId, launchStep, error });
    }
  }

  return {
    cleanupCompleted: cleanupIssues.length === 0,
    cleanupIssues,
  };
}

async function buildReviewTeamManifestWithRuntimeSignals(
  team: Parameters<typeof buildEffectiveReviewTeamManifest>[0],
  options: Parameters<typeof buildEffectiveReviewTeamManifest>[1],
): Promise<ReviewTeamRunManifest> {
  const manifestOptions = options ?? {};
  const [rateLimitStatus, strategyOverride] = await Promise.all([
    loadReviewTeamRateLimitStatus().catch((error) => {
      log.warn('Failed to load Deep Review rate limit status', { error });
      return null;
    }),
    manifestOptions.workspacePath
      ? loadReviewTeamProjectStrategyOverride(manifestOptions.workspacePath).catch((error) => {
        log.warn('Failed to load Deep Review project strategy override', { error });
        return undefined;
      })
      : Promise.resolve(undefined),
  ]);

  return buildEffectiveReviewTeamManifest(team, {
    ...manifestOptions,
    ...(rateLimitStatus ? { rateLimitStatus } : {}),
    ...(strategyOverride ? { strategyOverride } : {}),
  });
}

export async function buildDeepReviewLaunchFromSessionFiles(
  filePaths: string[],
  extraContext?: string,
  workspacePath?: string,
): Promise<DeepReviewLaunchPrompt> {
  const target = classifyReviewTargetFromFiles(filePaths, 'session_files');
  const changeStats = buildUnknownChangeStats(target);
  const team = await prepareDefaultReviewTeamForLaunch(workspacePath, {
    reviewTargetFilePaths: filePaths,
    target,
  });
  const manifest = await buildReviewTeamManifestWithRuntimeSignals(team, {
    workspacePath,
    target,
    changeStats,
  });
  const prompt = formatSessionFilesLaunchPrompt({
    filePaths,
    extraContext,
    reviewTeamPromptBlock: buildReviewTeamPromptBlock(team, manifest),
  });

  return { prompt, runManifest: manifest };
}

export async function buildDeepReviewPreviewFromSessionFiles(
  filePaths: string[],
  workspacePath?: string,
): Promise<ReviewTeamRunManifest> {
  const team = await loadDefaultReviewTeam(workspacePath);
  const target = classifyReviewTargetFromFiles(filePaths, 'session_files');
  const changeStats = buildUnknownChangeStats(target);
  return buildReviewTeamManifestWithRuntimeSignals(team, {
    workspacePath,
    target,
    changeStats,
  });
}

export async function buildDeepReviewPromptFromSessionFiles(
  filePaths: string[],
  extraContext?: string,
  workspacePath?: string,
): Promise<string> {
  return (await buildDeepReviewLaunchFromSessionFiles(
    filePaths,
    extraContext,
    workspacePath,
  )).prompt;
}

export async function buildDeepReviewLaunchFromSlashCommand(
  commandText: string,
  workspacePath?: string,
): Promise<DeepReviewLaunchPrompt> {
  const team = await prepareDefaultReviewTeamForLaunch(workspacePath);
  const trimmed = commandText.trim();
  const extraContext = getDeepReviewCommandFocus(trimmed);
  const { target, changeStats } = await resolveSlashCommandReviewTarget(extraContext, workspacePath);
  const manifest = await buildReviewTeamManifestWithRuntimeSignals(team, {
    workspacePath,
    target,
    changeStats,
  });
  const prompt = formatSlashCommandLaunchPrompt({
    commandText: trimmed,
    extraContext,
    reviewTeamPromptBlock: buildReviewTeamPromptBlock(team, manifest),
  });

  return { prompt, runManifest: manifest };
}

export async function buildDeepReviewPreviewFromSlashCommand(
  commandText: string,
  workspacePath?: string,
): Promise<ReviewTeamRunManifest> {
  const team = await loadDefaultReviewTeam(workspacePath);
  const trimmed = commandText.trim();
  const extraContext = getDeepReviewCommandFocus(trimmed);
  const { target, changeStats } = await resolveSlashCommandReviewTarget(extraContext, workspacePath);
  return buildReviewTeamManifestWithRuntimeSignals(team, {
    workspacePath,
    target,
    changeStats,
  });
}

export async function buildDeepReviewPromptFromSlashCommand(
  commandText: string,
  workspacePath?: string,
): Promise<string> {
  return (await buildDeepReviewLaunchFromSlashCommand(commandText, workspacePath)).prompt;
}

export async function launchDeepReviewSession({
  parentSessionId,
  workspacePath,
  prompt,
  displayMessage,
  childSessionName = 'Deep review',
  requestedFiles = [],
  runManifest,
}: LaunchDeepReviewSessionParams): Promise<{ childSessionId: string }> {
  let childSessionId: string | null = null;
  let launchStep: DeepReviewLaunchStep = 'create_child_session';

  try {
    const created = await createBtwChildSession({
      parentSessionId,
      workspacePath,
      childSessionName,
      sessionKind: 'deep_review',
      agentType: 'DeepReview',
      enableTools: true,
      safeMode: true,
      autoCompact: true,
      enableContextCompression: true,
      addMarker: false,
      deepReviewRunManifest: runManifest,
    });
    childSessionId = created.childSessionId;

    launchStep = 'open_aux_pane';
    openBtwSessionInAuxPane({
      childSessionId,
      parentSessionId,
      workspacePath,
      expand: true,
    });

    launchStep = 'send_start_message';
    const flowChatManager = FlowChatManager.getInstance();
    if (runManifest) {
      await flowChatManager.sendMessage(
        prompt,
        childSessionId,
        displayMessage,
        undefined,
        undefined,
        {
          userMessageMetadata: {
            deepReviewRunManifest: runManifest,
          },
        },
      );
    } else {
      await flowChatManager.sendMessage(
        prompt,
        childSessionId,
        displayMessage,
      );
    }

    insertReviewSessionSummaryMarker({
      parentSessionId,
      childSessionId,
      kind: 'deep_review',
      title: childSessionName,
      requestedFiles,
      parentDialogTurnId: created.parentDialogTurnId,
    });

    return { childSessionId };
  } catch (error) {
    if (!childSessionId) {
      throw createDeepReviewLaunchError(launchStep, error);
    }

    const cleanupResult = await cleanupFailedDeepReviewLaunch(childSessionId, launchStep);
    const wrappedError = buildLaunchCleanupError(
      launchStep,
      childSessionId,
      error,
      cleanupResult,
    );

    log.error('Deep review launch failed', {
      parentSessionId,
      childSessionId,
      launchStep,
      cleanupCompleted: cleanupResult.cleanupCompleted,
      cleanupIssues: cleanupResult.cleanupIssues,
      error,
    });

    if (launchStep === 'send_start_message' && cleanupResult.cleanupCompleted) {
      throw createDeepReviewLaunchError(launchStep, error, childSessionId, cleanupResult);
    }

    throw wrappedError;
  }
}
