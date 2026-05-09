import { agentAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { createBtwChildSession } from './BtwThreadService';
import { closeBtwSessionInAuxPane, openBtwSessionInAuxPane } from './openBtwSession';
import { FlowChatManager } from './FlowChatManager';
import { flowChatStore } from '../store/FlowChatStore';
import { insertReviewSessionSummaryMarker } from './ReviewSessionMarkerService';
import {
  buildEffectiveReviewTeamManifest,
  buildReviewTeamPromptBlock,
  prepareDefaultReviewTeamForLaunch,
} from '@/shared/services/reviewTeamService';
import { DEEP_REVIEW_COMMAND_RE } from '../utils/deepReviewConstants';
import { classifyLaunchError } from '../utils/deepReviewExperience';

const log = createLogger('DeepReviewService');

export const DEEP_REVIEW_SLASH_COMMAND = '/DeepReview';

interface LaunchDeepReviewSessionParams {
  parentSessionId: string;
  workspacePath?: string;
  prompt: string;
  displayMessage: string;
  childSessionName?: string;
  requestedFiles?: string[];
}

type DeepReviewLaunchStep =
  | 'create_child_session'
  | 'open_aux_pane'
  | 'send_start_message';

interface FailedDeepReviewCleanupResult {
  cleanupCompleted: boolean;
  cleanupIssues: string[];
}

interface DeepReviewLaunchError extends Error {
  launchErrorCategory?: string;
  launchErrorActions?: string[];
  launchErrorMessageKey?: string;
  launchErrorStep?: string;
  originalMessage?: string;
  childSessionId?: string;
  cleanupCompleted?: boolean;
  cleanupIssues?: string[];
}

const LAUNCH_ERROR_DEFAULT_MESSAGES: Record<string, string> = {
  'deepReviewActionBar.launchError.modelConfig': 'Deep review could not create a review session. Check the model configuration.',
  'deepReviewActionBar.launchError.network': 'Network connection was interrupted before Deep Review could start.',
  'deepReviewActionBar.launchError.unknown': 'Deep review failed to start. Please try again.',
};

function normalizeErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }

  if (typeof error === 'string' && error.trim()) {
    return error.trim();
  }

  return 'Deep review failed to start';
}

function isSessionMissingError(error: unknown): boolean {
  const message = normalizeErrorMessage(error).toLowerCase();
  return message.includes('session does not exist') || message.includes('not found');
}

function describeLaunchStep(step: DeepReviewLaunchStep): string {
  switch (step) {
    case 'create_child_session':
      return 'creating the deep review session';
    case 'open_aux_pane':
      return 'opening the deep review pane';
    case 'send_start_message':
      return 'starting the deep review run';
    default:
      return 'launching deep review';
  }
}

function createDeepReviewLaunchError(
  launchStep: DeepReviewLaunchStep,
  originalError: unknown,
  childSessionId?: string,
  cleanupResult?: FailedDeepReviewCleanupResult,
): DeepReviewLaunchError {
  const classified = classifyLaunchError(launchStep, originalError);
  const friendlyError = new Error(
    LAUNCH_ERROR_DEFAULT_MESSAGES[classified.messageKey] ??
      LAUNCH_ERROR_DEFAULT_MESSAGES['deepReviewActionBar.launchError.unknown'],
  ) as DeepReviewLaunchError;

  friendlyError.launchErrorCategory = classified.category;
  friendlyError.launchErrorActions = classified.actions;
  friendlyError.launchErrorMessageKey = classified.messageKey;
  friendlyError.launchErrorStep = classified.step;
  friendlyError.originalMessage = normalizeErrorMessage(originalError);
  if (childSessionId) {
    friendlyError.childSessionId = childSessionId;
  }
  if (cleanupResult) {
    friendlyError.cleanupCompleted = cleanupResult.cleanupCompleted;
    friendlyError.cleanupIssues = cleanupResult.cleanupIssues;
  }

  return friendlyError;
}

export function getDeepReviewLaunchErrorMessage(
  error: unknown,
  translate: (key: string, options?: { defaultValue?: string }) => string,
  fallback = LAUNCH_ERROR_DEFAULT_MESSAGES['deepReviewActionBar.launchError.unknown'],
): string {
  const launchError = error as DeepReviewLaunchError | null | undefined;
  if (launchError?.launchErrorMessageKey) {
    return translate(launchError.launchErrorMessageKey, {
      defaultValue: launchError.message || fallback,
    });
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message.trim();
  }

  return fallback;
}

function buildLaunchCleanupError(
  launchStep: DeepReviewLaunchStep,
  childSessionId: string,
  originalError: unknown,
  cleanupResult: FailedDeepReviewCleanupResult,
): Error {
  const originalMessage = normalizeErrorMessage(originalError);
  if (cleanupResult.cleanupCompleted) {
    return originalError instanceof Error ? originalError : new Error(originalMessage);
  }

  const cleanupSummary = cleanupResult.cleanupIssues.join(' ');
  return new Error(
    `${originalMessage} Cleanup was incomplete after failure while ${describeLaunchStep(launchStep)}. ` +
      `The partially created deep review session (${childSessionId}) may need manual cleanup. ${cleanupSummary}`.trim(),
  );
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

function formatFileList(filePaths: string[]): string {
  return filePaths.map(filePath => `- ${filePath}`).join('\n');
}

export function isDeepReviewSlashCommand(commandText: string): boolean {
  return DEEP_REVIEW_COMMAND_RE.test(commandText.trim());
}

function getDeepReviewCommandFocus(commandText: string): string {
  return commandText.trim().replace(/^\/DeepReview\b/, '').trim();
}

export async function buildDeepReviewPromptFromSessionFiles(
  filePaths: string[],
  extraContext?: string,
  workspacePath?: string,
): Promise<string> {
  const team = await prepareDefaultReviewTeamForLaunch(workspacePath, {
    reviewTargetFilePaths: filePaths,
  });
  const manifest = buildEffectiveReviewTeamManifest(team, {
    workspacePath,
    reviewTargetFilePaths: filePaths,
  });
  const fileList = formatFileList(filePaths);
  const contextBlock = extraContext?.trim()
    ? `User-provided focus:\n${extraContext.trim()}`
    : 'User-provided focus:\nNone.';

  return [
    'Run a deep code review using the parallel Code Review Team.',
    'Review scope: ONLY inspect the following files modified in this session.',
    fileList,
    contextBlock,
    buildReviewTeamPromptBlock(team, manifest),
    'Keep the scope tight to the listed files unless a directly-related dependency must be read to confirm a finding.',
  ].join('\n\n');
}

export async function buildDeepReviewPromptFromSlashCommand(
  commandText: string,
  workspacePath?: string,
): Promise<string> {
  const team = await prepareDefaultReviewTeamForLaunch(workspacePath);
  const manifest = buildEffectiveReviewTeamManifest(team, { workspacePath });
  const trimmed = commandText.trim();
  const extraContext = getDeepReviewCommandFocus(trimmed);
  const contextBlock = extraContext
    ? `User-provided focus or target:\n${extraContext}`
    : 'User-provided focus or target:\nNone. If no explicit target is given, review the current workspace changes relative to HEAD.';

  return [
    'Run a deep code review using the parallel Code Review Team.',
    'Interpret the user command below to determine the review target.',
    'If the user mentions a commit, ref, branch, or explicit file set, review that target.',
    'Otherwise, review the current workspace changes relative to HEAD.',
    `Original command:\n${trimmed}`,
    contextBlock,
    buildReviewTeamPromptBlock(team, manifest),
  ].join('\n\n');
}

export async function launchDeepReviewSession({
  parentSessionId,
  workspacePath,
  prompt,
  displayMessage,
  childSessionName = 'Deep review',
  requestedFiles = [],
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
    await flowChatManager.sendMessage(
      prompt,
      childSessionId,
      displayMessage,
    );

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
