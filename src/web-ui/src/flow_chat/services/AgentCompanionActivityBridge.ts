import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import type { AgentCompanionActivityPayload } from '../utils/agentCompanionActivity';

const log = createLogger('AgentCompanionActivityBridge');

export async function emitAgentCompanionActivity(
  activity: AgentCompanionActivityPayload,
): Promise<void> {
  if (!isTauriRuntime()) return;

  try {
    const { emit } = await import('@tauri-apps/api/event');
    await emit('agent-companion://activity-updated', activity);
  } catch (error) {
    log.warn('Failed to emit Agent companion activity update', error);
  }
}
