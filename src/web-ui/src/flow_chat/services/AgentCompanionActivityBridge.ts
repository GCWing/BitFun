import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import { toWellFormedText } from '@/shared/utils/wellFormedText';
import type { AgentCompanionActivityPayload, AgentCompanionTaskStatus } from '../utils/agentCompanionActivity';

const log = createLogger('AgentCompanionActivityBridge');
let activitySequence = 0;

function sanitizeTaskForEmit(task: AgentCompanionTaskStatus): AgentCompanionTaskStatus {
  return {
    ...task,
    sessionId: toWellFormedText(task.sessionId),
    title: toWellFormedText(task.title),
    labelKey: toWellFormedText(task.labelKey),
    defaultLabel: toWellFormedText(task.defaultLabel),
    latestOutput: task.latestOutput === undefined ? undefined : toWellFormedText(task.latestOutput),
  };
}

function sanitizeActivityForEmit(activity: AgentCompanionActivityPayload): AgentCompanionActivityPayload {
  return {
    ...activity,
    tasks: activity.tasks.map(sanitizeTaskForEmit),
  };
}

export async function emitAgentCompanionActivity(
  activity: AgentCompanionActivityPayload,
): Promise<void> {
  if (!isTauriRuntime()) return;

  const sequencedActivity: AgentCompanionActivityPayload = {
    ...sanitizeActivityForEmit(activity),
    sequence: activitySequence += 1,
    emittedAt: Date.now(),
  };

  try {
    const { emit } = await import('@tauri-apps/api/event');
    await emit('agent-companion://activity-updated', sequencedActivity);
  } catch (error) {
    log.warn('Failed to emit Agent companion activity update', error);
  }
}
