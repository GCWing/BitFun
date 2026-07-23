import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AgentCompanionActivityPayload } from '../utils/agentCompanionActivity';
import { emitAgentCompanionActivity } from './AgentCompanionActivityBridge';

const tauriEvent = vi.hoisted(() => ({
  emit: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  emit: tauriEvent.emit,
}));

vi.mock('@/infrastructure/runtime', () => ({
  isTauriRuntime: () => true,
}));

function hasLoneSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        return true;
      }
      index += 1;
      continue;
    }
    if (code >= 0xdc00 && code <= 0xdfff) {
      return true;
    }
  }

  return false;
}

describe('emitAgentCompanionActivity', () => {
  afterEach(() => {
    tauriEvent.emit.mockReset();
  });

  it('normalizes activity strings before crossing the desktop event boundary', async () => {
    await emitAgentCompanionActivity({
      mood: 'working',
      tasks: [{
        sessionId: 'session-\uD800',
        title: 'Broken \uD800 title',
        mood: 'working',
        state: 'running',
        labelKey: 'agentCompanion.activity.working',
        defaultLabel: 'Working \uDC00',
        latestOutput: 'Output \uD800',
        startedAt: 1000,
        updatedAt: 1200,
      }],
    });

    const emittedActivity = tauriEvent.emit.mock.calls[0]?.[1] as AgentCompanionActivityPayload;

    expect(tauriEvent.emit).toHaveBeenCalledWith('agent-companion://activity-updated', expect.any(Object));
    expect(hasLoneSurrogate(emittedActivity.tasks[0].sessionId)).toBe(false);
    expect(hasLoneSurrogate(emittedActivity.tasks[0].title)).toBe(false);
    expect(hasLoneSurrogate(emittedActivity.tasks[0].defaultLabel)).toBe(false);
    expect(hasLoneSurrogate(emittedActivity.tasks[0].latestOutput!)).toBe(false);
  });
});
