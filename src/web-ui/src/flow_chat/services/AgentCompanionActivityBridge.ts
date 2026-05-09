import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import type { AgentCompanionActivityPayload } from '../utils/agentCompanionActivity';

const log = createLogger('AgentCompanionActivityBridge');
let activitySequence = 0;

export async function emitAgentCompanionActivity(
  activity: AgentCompanionActivityPayload,
): Promise<void> {
  if (!isTauriRuntime()) return;

  const sequencedActivity: AgentCompanionActivityPayload = {
    ...activity,
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
