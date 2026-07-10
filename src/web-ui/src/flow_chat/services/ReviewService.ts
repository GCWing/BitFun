import { agentAPI } from '@/infrastructure/api';
import type {
  ReviewIntent,
  ReviewQualityDecision,
  ReviewQualityDecisionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';
import {
  buildReviewRiskFactors,
  loadReviewTeamProjectStrategyOverride,
  type ReviewTeamChangeStats,
  type ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';
import {
  classifyReviewTargetFromFiles,
  type ReviewTargetClassification,
} from '@/shared/services/reviewTargetClassifier';
import { createLogger } from '@/shared/utils/logger';
import {
  buildDeepReviewLaunchFromSessionFiles,
  buildDeepReviewLaunchFromSlashCommand,
  launchDeepReviewSession,
} from './DeepReviewService';
import { createBtwChildSession } from './BtwThreadService';
import { FlowChatManager } from './FlowChatManager';
import { flowChatStore } from '../store/FlowChatStore';
import { insertReviewSessionSummaryMarker } from './ReviewSessionMarkerService';
import { closeBtwSessionInAuxPane, openBtwSessionInAuxPane } from './btwSessionPane';
import {
  getDeepReviewCommandFocus,
  getReviewSlashCommandIntent,
} from '../deep-review/launch/commandParser';
import {
  buildUnknownChangeStats,
  resolveCurrentFileReviewChangeStats,
  resolveSlashCommandReviewTarget,
} from '../deep-review/launch/targetResolver';

const log = createLogger('ReviewService');

interface PreparedReviewBase {
  target: ReviewTargetClassification;
  requestedFiles: string[];
  prompt: string;
  decision: ReviewQualityDecision;
  requiresConsent: boolean;
}

export interface PreparedStandardReviewLaunch extends PreparedReviewBase {
  mode: 'standard';
  level: 'l1';
  strategyLevel: 'quick';
}

export interface PreparedStrictReviewLaunch extends PreparedReviewBase {
  mode: 'strict';
  level: 'l2' | 'l3';
  strategyLevel: 'normal' | 'deep';
  runManifest: ReviewTeamRunManifest;
}

export type PreparedReviewLaunch =
  | PreparedStandardReviewLaunch
  | PreparedStrictReviewLaunch;

export interface PrepareReviewLaunchOptions {
  workspacePath?: string;
  remoteConnectionId?: string;
  extraContext?: string;
  changeStats?: ReviewTeamChangeStats;
  intent?: 'adaptive' | 'strict';
}

function includedTargetFiles(target: ReviewTargetClassification): string[] {
  return target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath);
}

function buildDecisionRequest(params: {
  intent: ReviewIntent;
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  projectStrategyOverride?: ReviewQualityDecisionRequest['projectStrategyOverride'];
}): ReviewQualityDecisionRequest {
  const factors = buildReviewRiskFactors(params.target, params.changeStats);
  return {
    intent: params.intent,
    target: {
      resolution: params.target.resolution,
      fileCount: factors.fileCount,
      ...(factors.totalLinesChanged !== undefined
        ? { totalLinesChanged: factors.totalLinesChanged }
        : {}),
      securitySensitiveFileCount: factors.securityFileCount,
      workspaceAreaCount: factors.workspaceAreaCount,
      contractSurfaceChanged: factors.contractSurfaceChanged,
    },
    ...(params.projectStrategyOverride
      ? { projectStrategyOverride: params.projectStrategyOverride }
      : {}),
  };
}

async function decideReview(params: {
  workspacePath?: string;
  intent: ReviewIntent;
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
}): Promise<ReviewQualityDecision> {
  let projectStrategyOverride: ReviewQualityDecisionRequest['projectStrategyOverride'];
  if (params.workspacePath) {
    try {
      projectStrategyOverride = await loadReviewTeamProjectStrategyOverride(
        params.workspacePath,
      );
    } catch (error) {
      log.warn('Failed to load Review project strategy override', { error });
    }
  }

  return agentAPI.decideReviewQuality(buildDecisionRequest({
    ...params,
    projectStrategyOverride,
  }));
}

function buildStandardReviewPrompt(params: {
  target: ReviewTargetClassification;
  extraContext?: string;
}): string {
  const files = includedTargetFiles(params.target);
  const targetBlock = files.length > 0
    ? files.map((file) => `- ${file}`).join('\n')
    : '- Resolve and inspect the current workspace changes without modifying them.';
  const focusBlock = params.extraContext?.trim()
    ? `\nUser focus:\n${params.extraContext.trim()}\n`
    : '';

  return `Perform an independent adversarial review of the requested change.\n\n` +
    `Treat the implementation as untrusted until the repository evidence supports it. ` +
    `Look for concrete correctness, regression, security, architecture, and test-coverage issues. ` +
    `Remain read-only: report findings and do not edit files or run mutating commands.\n\n` +
    `Review target:\n${targetBlock}\n${focusBlock}\n` +
    `Return findings first, ordered by severity, with precise file and line references. ` +
    `If there are no actionable findings, say so and identify residual verification gaps.`;
}

async function prepareFromResolvedTarget(params: {
  target: ReviewTargetClassification;
  changeStats: ReviewTeamChangeStats;
  requestedFiles: string[];
  workspacePath?: string;
  extraContext?: string;
  commandText?: string;
  intent: ReviewIntent;
}): Promise<PreparedReviewLaunch> {
  const decision = await decideReview(params);

  if (decision.executionMode === 'standard' && decision.level === 'l1') {
    return {
      mode: 'standard',
      level: 'l1',
      strategyLevel: 'quick',
      target: params.target,
      requestedFiles: params.requestedFiles,
      prompt: buildStandardReviewPrompt(params),
      decision,
      requiresConsent: decision.requiresConsent,
    };
  }

  if (
    decision.executionMode !== 'strict' ||
    (decision.level !== 'l2' && decision.level !== 'l3') ||
    (decision.strategyLevel !== 'normal' && decision.strategyLevel !== 'deep')
  ) {
    throw new Error(`Unsupported explicit Review decision: ${decision.level}/${decision.executionMode}`);
  }
  const qualityDecision = {
    level: decision.level,
    executionMode: decision.executionMode,
    strategyLevel: decision.strategyLevel,
    reason: decision.reason,
    score: decision.score,
    requiresConsent: decision.requiresConsent,
  };

  const launch = params.commandText
    ? await buildDeepReviewLaunchFromSlashCommand(
      params.commandText,
      params.workspacePath,
      {
        qualityDecision,
        ...(decision.level === 'l2'
          ? { maxCoreReviewers: 3, maxExtraReviewers: 0, includeQualityGate: false }
          : { includeQualityGate: true }),
        resolvedTarget: {
          target: params.target,
          changeStats: params.changeStats,
        },
      },
    )
    : await buildDeepReviewLaunchFromSessionFiles(
      params.requestedFiles,
      params.extraContext,
      params.workspacePath,
      {
        qualityDecision,
        changeStats: params.changeStats,
        ...(decision.level === 'l2'
          ? { maxCoreReviewers: 3, maxExtraReviewers: 0, includeQualityGate: false }
          : { includeQualityGate: true }),
      },
    );

  return {
    mode: 'strict',
    level: decision.level,
    strategyLevel: decision.strategyLevel,
    target: params.target,
    requestedFiles: params.requestedFiles,
    prompt: launch.prompt,
    runManifest: launch.runManifest,
    decision,
    requiresConsent: decision.requiresConsent,
  };
}

export async function prepareReviewLaunchFromSessionFiles(
  filePaths: string[],
  options: PrepareReviewLaunchOptions = {},
): Promise<PreparedReviewLaunch> {
  const target = classifyReviewTargetFromFiles(filePaths, 'session_files');
  let changeStats = options.changeStats;
  if (!changeStats && options.workspacePath) {
    changeStats = options.remoteConnectionId
      ? await resolveCurrentFileReviewChangeStats(
          options.workspacePath,
          target,
          undefined,
          options.remoteConnectionId,
        )
      : await resolveCurrentFileReviewChangeStats(options.workspacePath, target);
  }
  changeStats ??= buildUnknownChangeStats(target);
  return prepareFromResolvedTarget({
    target,
    changeStats,
    requestedFiles: includedTargetFiles(target),
    workspacePath: options.workspacePath,
    extraContext: options.extraContext,
    intent: options.intent === 'strict' ? 'strict' : 'review',
  });
}

export async function prepareReviewLaunchFromSlashCommand(
  commandText: string,
  workspacePath?: string,
  remoteConnectionId?: string,
): Promise<PreparedReviewLaunch> {
  const extraContext = getDeepReviewCommandFocus(commandText);
  const { target, changeStats } = await resolveSlashCommandReviewTarget(
    extraContext,
    workspacePath,
    remoteConnectionId,
  );
  return prepareFromResolvedTarget({
    target,
    changeStats,
    requestedFiles: includedTargetFiles(target),
    workspacePath,
    extraContext,
    commandText,
    intent: getReviewSlashCommandIntent(commandText) === 'strict' ? 'strict' : 'review',
  });
}

export async function launchPreparedReviewSession(params: {
  parentSessionId: string;
  workspacePath?: string;
  displayMessage: string;
  prepared: PreparedReviewLaunch;
  childSessionName?: string;
  requestId?: string;
}): Promise<{ childSessionId: string }> {
  const childSessionName = params.childSessionName ?? 'Review';
  if (params.prepared.mode === 'strict') {
    const result = await launchDeepReviewSession({
      parentSessionId: params.parentSessionId,
      workspacePath: params.workspacePath,
      prompt: params.prepared.prompt,
      displayMessage: params.displayMessage,
      childSessionName,
      requestedFiles: params.prepared.requestedFiles,
      runManifest: params.prepared.runManifest,
      requestId: params.requestId,
    });
    openBtwSessionInAuxPane({
      childSessionId: result.childSessionId,
      parentSessionId: params.parentSessionId,
      workspacePath: params.workspacePath,
      expand: true,
      sessionKind: 'deep_review',
      sessionTitle: childSessionName,
      agentType: 'DeepReview',
    });
    return result;
  }

  const created = await createBtwChildSession({
    parentSessionId: params.parentSessionId,
    workspacePath: params.workspacePath,
    childSessionName,
    sessionKind: 'review',
    agentType: 'CodeReview',
    enableTools: true,
    safeMode: true,
    autoCompact: true,
    enableContextCompression: true,
    addMarker: false,
    reviewTargetFilePaths: params.prepared.requestedFiles,
    requestId: params.requestId,
  });
  try {
    await FlowChatManager.getInstance().sendMessage(
      params.prepared.prompt,
      created.childSessionId,
      params.displayMessage,
    );
  } catch (error) {
    const childSession = flowChatStore.getState().sessions.get(created.childSessionId);
    const cleanupWorkspacePath = childSession?.workspacePath ?? params.workspacePath;
    try {
      closeBtwSessionInAuxPane(created.childSessionId);
    } catch (cleanupError) {
      log.warn('Failed to close standard Review pane during cleanup', {
        childSessionId: created.childSessionId,
        cleanupError,
      });
    }
    if (cleanupWorkspacePath) {
      try {
        await agentAPI.deleteSession(
          created.childSessionId,
          cleanupWorkspacePath,
          childSession?.remoteConnectionId,
          childSession?.remoteSshHost,
        );
        FlowChatManager.getInstance().discardLocalSession(created.childSessionId);
      } catch (cleanupError) {
        log.warn('Failed to clean up standard Review launch', {
          childSessionId: created.childSessionId,
          cleanupError,
        });
      }
    }
    throw error;
  }
  insertReviewSessionSummaryMarker({
    parentSessionId: params.parentSessionId,
    childSessionId: created.childSessionId,
    kind: 'review',
    title: childSessionName,
    requestedFiles: params.prepared.requestedFiles,
    parentDialogTurnId: created.parentDialogTurnId,
  });
  openBtwSessionInAuxPane({
    childSessionId: created.childSessionId,
    parentSessionId: params.parentSessionId,
    workspacePath: params.workspacePath,
    expand: true,
    sessionKind: 'review',
    sessionTitle: childSessionName,
    agentType: 'CodeReview',
  });
  return { childSessionId: created.childSessionId };
}
