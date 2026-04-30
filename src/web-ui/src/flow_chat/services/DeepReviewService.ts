import { agentAPI, gitAPI } from '@/infrastructure/api';
import type {
  GitChangedFile,
  GitChangedFilesParams,
  GitDiffParams,
  GitStatus,
} from '@/infrastructure/api/service-api/GitAPI';
import { createLogger } from '@/shared/utils/logger';
import { createBtwChildSession } from './BtwThreadService';
import { closeBtwSessionInAuxPane, openBtwSessionInAuxPane } from './openBtwSession';
import { FlowChatManager } from './FlowChatManager';
import { flowChatStore } from '../store/FlowChatStore';
import { insertReviewSessionSummaryMarker } from './ReviewSessionMarkerService';
import {
  buildEffectiveReviewTeamManifest,
  buildReviewTeamPromptBlock,
  loadDefaultReviewTeam,
  loadReviewTeamProjectStrategyOverride,
  loadReviewTeamRateLimitStatus,
  prepareDefaultReviewTeamForLaunch,
  type ReviewTeamChangeStats,
  type ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';
import {
  classifyReviewTargetFromFiles,
  createUnknownReviewTargetClassification,
  normalizeReviewPath,
  type ReviewTargetClassification,
} from '@/shared/services/reviewTargetClassifier';
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
  runManifest?: ReviewTeamRunManifest;
}

export interface DeepReviewLaunchPrompt {
  prompt: string;
  runManifest: ReviewTeamRunManifest;
}

interface ResolvedDeepReviewTarget {
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
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

const EXPLICIT_REVIEW_FILE_EXTENSIONS = new Set([
  '.ts',
  '.tsx',
  '.js',
  '.jsx',
  '.rs',
  '.json',
  '.scss',
  '.css',
  '.md',
  '.toml',
  '.yaml',
  '.yml',
]);

function cleanPotentialFileToken(token: string): string {
  return token
    .trim()
    .replace(/^[`"']+/, '')
    .replace(/[`"',;:]+$/, '');
}

function getPathExtension(path: string): string {
  const lastSlash = path.lastIndexOf('/');
  const lastDot = path.lastIndexOf('.');
  if (lastDot <= lastSlash) {
    return '';
  }
  return path.slice(lastDot);
}

function looksLikeExplicitReviewPath(token: string): boolean {
  const normalizedPath = normalizeReviewPath(token);
  return (
    normalizedPath.includes('/') &&
    !normalizedPath.startsWith('-') &&
    EXPLICIT_REVIEW_FILE_EXTENSIONS.has(getPathExtension(normalizedPath))
  );
}

function extractExplicitReviewFilePaths(commandFocus: string): string[] {
  const paths = commandFocus
    .split(/\s+/)
    .map(cleanPotentialFileToken)
    .filter(Boolean)
    .filter(looksLikeExplicitReviewPath);

  return Array.from(new Set(paths));
}

function parseSlashCommandGitTarget(commandFocus: string): GitChangedFilesParams | null {
  const tokens = commandFocus
    .split(/\s+/)
    .map(cleanPotentialFileToken)
    .filter(Boolean);

  const commitKeywordIndex = tokens.findIndex((token) => token.toLowerCase() === 'commit');
  const commitRef = commitKeywordIndex >= 0 ? tokens[commitKeywordIndex + 1] : undefined;
  if (commitRef && !commitRef.startsWith('-')) {
    return {
      source: `${commitRef}^`,
      target: commitRef,
    };
  }

  const rangeToken = tokens.find((token) => {
    if (token.startsWith('-') || !token.includes('..')) {
      return false;
    }

    const parts = token.split('..');
    return parts.length === 2 && Boolean(parts[0]) && Boolean(parts[1]);
  });

  if (!rangeToken) {
    return null;
  }

  const [source, target] = rangeToken.split('..');
  return { source, target };
}

function collectChangedFilePaths(changedFiles: GitChangedFile[]): string[] {
  return Array.from(
    new Set(
      changedFiles
        .flatMap((file) => [file.path, file.old_path])
        .filter((path): path is string => Boolean(path)),
    ),
  );
}

function collectWorkspaceDiffFilePaths(status: GitStatus): string[] {
  return Array.from(
    new Set([
      ...status.staged.map((file) => file.path),
      ...status.unstaged.map((file) => file.path),
      ...status.untracked,
      ...status.conflicts,
    ].filter(Boolean)),
  );
}

function countReviewTargetFiles(target: ReviewTargetClassification): number {
  return target.files.filter((file) => !file.excluded).length;
}

function buildUnknownChangeStats(target: ReviewTargetClassification): ReviewTeamChangeStats {
  return {
    fileCount: countReviewTargetFiles(target),
    lineCountSource: 'unknown',
  };
}

function countChangedLinesFromUnifiedDiff(diff: string): number | undefined {
  if (!diff.trim()) {
    return undefined;
  }

  let changedLines = 0;
  for (const line of diff.split(/\r?\n/)) {
    if (
      (line.startsWith('+') && !/^\+\+\+\s/.test(line)) ||
      (line.startsWith('-') && !/^---\s/.test(line))
    ) {
      changedLines += 1;
    }
  }

  return changedLines;
}

function buildDiffChangeStats(
  target: ReviewTargetClassification,
  totalLinesChanged: number | undefined,
): ReviewTeamChangeStats {
  if (totalLinesChanged === undefined) {
    return buildUnknownChangeStats(target);
  }

  return {
    fileCount: countReviewTargetFiles(target),
    totalLinesChanged,
    lineCountSource: 'diff_stat',
  };
}

async function resolveGitDiffChangeStats(
  workspacePath: string,
  params: GitDiffParams,
  target: ReviewTargetClassification,
): Promise<ReviewTeamChangeStats> {
  try {
    const diff = await gitAPI.getDiff(workspacePath, params);
    return buildDiffChangeStats(target, countChangedLinesFromUnifiedDiff(diff));
  } catch (error) {
    log.warn('Failed to resolve Git diff stats for Deep Review target', {
      workspacePath,
      params,
      error,
    });
    return buildUnknownChangeStats(target);
  }
}

async function resolveWorkspaceDiffChangeStats(
  workspacePath: string,
  target: ReviewTargetClassification,
): Promise<ReviewTeamChangeStats> {
  return resolveGitDiffChangeStats(workspacePath, { source: 'HEAD' }, target);
}

async function resolveSlashCommandReviewTarget(
  commandFocus: string,
  workspacePath?: string,
): Promise<ResolvedDeepReviewTarget> {
  const explicitFilePaths = extractExplicitReviewFilePaths(commandFocus);
  if (explicitFilePaths.length > 0) {
    const target = classifyReviewTargetFromFiles(
      explicitFilePaths,
      'slash_command_explicit_files',
    );
    return { target, changeStats: buildUnknownChangeStats(target) };
  }

  const gitTarget = parseSlashCommandGitTarget(commandFocus);
  if (gitTarget) {
    if (!workspacePath) {
      const target = createUnknownReviewTargetClassification('slash_command_git_ref');
      return { target, changeStats: buildUnknownChangeStats(target) };
    }

    try {
      const changedFiles = await gitAPI.getChangedFiles(workspacePath, gitTarget);
      const target = classifyReviewTargetFromFiles(
        collectChangedFilePaths(changedFiles),
        'slash_command_git_ref',
      );
      const changeStats = await resolveGitDiffChangeStats(
        workspacePath,
        gitTarget,
        target,
      );
      return { target, changeStats };
    } catch (error) {
      log.warn('Failed to resolve Git target for Deep Review target', {
        workspacePath,
        gitTarget,
        error,
      });
      const target = createUnknownReviewTargetClassification('slash_command_git_ref');
      return { target, changeStats: buildUnknownChangeStats(target) };
    }
  }

  if (!commandFocus && workspacePath) {
    try {
      const status = await gitAPI.getStatus(workspacePath);
      const target = classifyReviewTargetFromFiles(
        collectWorkspaceDiffFilePaths(status),
        'workspace_diff',
      );
      const changeStats = await resolveWorkspaceDiffChangeStats(
        workspacePath,
        target,
      );
      return { target, changeStats };
    } catch (error) {
      log.warn('Failed to resolve workspace diff for Deep Review target', {
        workspacePath,
        error,
      });
    }
  }

  const target = createUnknownReviewTargetClassification(
    commandFocus ? 'manual_prompt' : 'unknown',
  );
  return { target, changeStats: buildUnknownChangeStats(target) };
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
  const team = await prepareDefaultReviewTeamForLaunch(workspacePath);
  const target = classifyReviewTargetFromFiles(filePaths, 'session_files');
  const changeStats = buildUnknownChangeStats(target);
  const manifest = await buildReviewTeamManifestWithRuntimeSignals(team, {
    workspacePath,
    target,
    changeStats,
  });
  const fileList = formatFileList(filePaths);
  const contextBlock = extraContext?.trim()
    ? `User-provided focus:\n${extraContext.trim()}`
    : 'User-provided focus:\nNone.';

  const prompt = [
    'Run a deep code review using the parallel Code Review Team.',
    'Review scope: ONLY inspect the following files modified in this session.',
    fileList,
    contextBlock,
    buildReviewTeamPromptBlock(team, manifest),
    'Keep the scope tight to the listed files unless a directly-related dependency must be read to confirm a finding.',
  ].join('\n\n');

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
  const contextBlock = extraContext
    ? `User-provided focus or target:\n${extraContext}`
    : 'User-provided focus or target:\nNone. If no explicit target is given, review the current workspace changes relative to HEAD.';

  const prompt = [
    'Run a deep code review using the parallel Code Review Team.',
    'Interpret the user command below to determine the review target.',
    'If the user mentions a commit, ref, branch, or explicit file set, review that target.',
    'Otherwise, review the current workspace changes relative to HEAD.',
    `Original command:\n${trimmed}`,
    contextBlock,
    buildReviewTeamPromptBlock(team, manifest),
  ].join('\n\n');

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
