import { describe, expect, it, vi } from 'vitest';

import { shouldScheduleDeferredStartupSystems } from './deferredStartupGate';
import { scheduleDeferredStartupSystems } from './deferredStartupSystems';

describe('shouldScheduleDeferredStartupSystems', () => {
  it('waits for both interactive readiness and startup overlay handoff', () => {
    expect(shouldScheduleDeferredStartupSystems({
      interactiveShellReady: false,
      startupOverlayVisible: true,
    })).toBe(false);
    expect(shouldScheduleDeferredStartupSystems({
      interactiveShellReady: true,
      startupOverlayVisible: true,
    })).toBe(false);
    expect(shouldScheduleDeferredStartupSystems({
      interactiveShellReady: false,
      startupOverlayVisible: false,
    })).toBe(false);
    expect(shouldScheduleDeferredStartupSystems({
      interactiveShellReady: true,
      startupOverlayVisible: false,
    })).toBe(true);
  });
});

describe('scheduleDeferredStartupSystems', () => {
  it('schedules MCP, ACP, and IDE startup as deferred idle work', async () => {
    let scheduledTask: ((signal: AbortSignal) => Promise<void>) | null = null;
    const schedule = vi.fn((task: (signal: AbortSignal) => Promise<void>, options) => {
      scheduledTask = task;
      return {
        promise: Promise.resolve(),
        cancel: vi.fn(),
      };
    });
    const order: string[] = [];

    scheduleDeferredStartupSystems({
      scheduler: { schedule },
      log: {
        debug: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
      },
      trace: {
        markPhase: vi.fn(),
      },
      initializeIdeControl: async () => {
        order.push('ide');
      },
      initializeMcpServers: async () => {
        order.push('mcp');
      },
      initializeAcpClients: async () => {
        order.push('acp');
      },
      probeAcpClientRequirements: async () => {
        order.push('acp-probe');
      },
    });

    expect(schedule).toHaveBeenCalledTimes(1);
    expect(schedule.mock.calls[0][1]).toMatchObject({
      idle: true,
      priority: 'low',
      inFlightKey: 'startup:deferred-systems',
    });
    expect(order).toEqual([]);

    await scheduledTask?.(new AbortController().signal);

    expect(order).toEqual(['ide', 'mcp', 'acp', 'acp-probe']);
  });

  it('skips deferred startup systems when cancelled before execution', async () => {
    let scheduledTask: ((signal: AbortSignal) => Promise<void>) | null = null;
    const schedule = vi.fn((task: (signal: AbortSignal) => Promise<void>) => {
      scheduledTask = task;
      return {
        promise: Promise.resolve(),
        cancel: vi.fn(),
      };
    });
    const initializeIdeControl = vi.fn();
    const controller = new AbortController();

    scheduleDeferredStartupSystems({
      scheduler: { schedule },
      log: {
        debug: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
      },
      trace: {
        markPhase: vi.fn(),
      },
      initializeIdeControl,
      initializeMcpServers: vi.fn(),
      initializeAcpClients: vi.fn(),
      probeAcpClientRequirements: vi.fn(),
    });

    controller.abort();
    await scheduledTask?.(controller.signal);

    expect(initializeIdeControl).not.toHaveBeenCalled();
  });
});
