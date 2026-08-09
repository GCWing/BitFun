// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const transportMocks = vi.hoisted(() => ({
  append: vi.fn(async () => undefined),
}));

vi.mock('./flowChatDiagnosticsTransport', () => ({
  appendFlowChatDiagnosticEntries: transportMocks.append,
}));

vi.mock('@/infrastructure/runtime', () => ({
  isTauriRuntime: () => true,
}));

import { flowChatDiagnostics } from './flowChatDiagnostics';
import type { FlowChatDiagnosticTransportEntry } from './flowChatDiagnosticsTransport';

describe('flowChatDiagnostics', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    transportMocks.append.mockClear();
    flowChatDiagnostics.resetForTests();
  });

  afterEach(() => {
    flowChatDiagnostics.resetForTests();
    vi.useRealTimers();
  });

  it('does not evaluate diagnostic payloads while disabled', () => {
    const data = vi.fn(() => ({ scrollTop: 120 }));

    flowChatDiagnostics.trace({
      hypothesis: 'A',
      location: 'test.disabled',
      message: 'Disabled probe',
      data,
    });
    vi.runAllTimers();

    expect(data).not.toHaveBeenCalled();
    expect(transportMocks.append).not.toHaveBeenCalled();
  });

  it('batches enabled diagnostics with ordered structured metadata', async () => {
    flowChatDiagnostics.setEnabled(true);
    flowChatDiagnostics.trace({
      hypothesis: 'A',
      location: 'test.enabled',
      message: 'Enabled probe',
      data: () => ({ scrollTop: 240 }),
    });

    await vi.advanceTimersByTimeAsync(150);

    expect(transportMocks.append).toHaveBeenCalledTimes(1);
    const entries = transportMocks.append.mock.calls[0]?.[0];
    expect(entries).toHaveLength(2);
    expect(entries[0]).toMatchObject({
      sequence: 1,
      hypothesis: 'I',
      location: 'FlowChatDiagnosticsRecorder.setEnabled',
    });
    expect(entries[1]).toMatchObject({
      sequence: 2,
      hypothesis: 'A',
      location: 'test.enabled',
      message: 'Enabled probe',
      data: { scrollTop: 240 },
    });
  });

  it('flushes queued diagnostics when disabled', async () => {
    flowChatDiagnostics.setEnabled(true);
    flowChatDiagnostics.trace({
      hypothesis: 'B',
      location: 'test.disable-flush',
      message: 'Queued probe',
    });

    flowChatDiagnostics.setEnabled(false);
    await Promise.resolve();
    await Promise.resolve();

    expect(transportMocks.append).toHaveBeenCalledTimes(1);
    expect(flowChatDiagnostics.isEnabled()).toBe(false);
  });

  it('coalesces flush requests and preserves sequence order after queue overflow', async () => {
    let resolveFirstWrite!: () => void;
    transportMocks.append.mockImplementationOnce(() => new Promise<void>((resolve) => {
      resolveFirstWrite = resolve;
    }));

    flowChatDiagnostics.setEnabled(true);
    for (let index = 0; index < 127; index += 1) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'test.first-batch',
        message: `First batch ${index}`,
      });
    }
    expect(transportMocks.append).toHaveBeenCalledTimes(1);

    const firstDrain = flowChatDiagnostics.flushNow();
    const secondDrain = flowChatDiagnostics.flushNow();
    expect(secondDrain).toBe(firstDrain);

    for (let index = 0; index < 1_152; index += 1) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'test.overflow',
        message: `Overflow ${index}`,
      });
    }
    expect(transportMocks.append).toHaveBeenCalledTimes(1);

    resolveFirstWrite();
    await firstDrain;

    const entries = transportMocks.append.mock.calls.flatMap(
      call => call[0] as FlowChatDiagnosticTransportEntry[],
    );
    expect(entries.length).toBeGreaterThan(128);
    for (let index = 1; index < entries.length; index += 1) {
      expect(entries[index]!.sequence).toBeGreaterThan(entries[index - 1]!.sequence);
    }

    const droppedMarker = entries.find(
      entry => entry.location === 'FlowChatDiagnosticsRecorder.flush',
    );
    expect(droppedMarker).toMatchObject({
      sequence: 256,
      data: {
        droppedEntries: 128,
        firstDroppedSequence: 129,
        lastDroppedSequence: 256,
      },
    });
  });
});
