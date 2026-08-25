import { describe, expect, it } from 'vitest';
import type { PeerHostCapabilities } from '@/infrastructure/peer-device/PeerConnectionManager';

import { getTerminalViewState, resolveCanCancelTool } from './terminalToolCardState';

function caps(overrides: Partial<PeerHostCapabilities>): PeerHostCapabilities {
  return {
    idempotentDialogSubmit: false,
    targetedSessionRollback: false,
    tokenUsageStatistics: false,
    cancelTool: null,
    toolCatalog: null,
    hostKind: null,
    ...overrides,
  };
}

describe('terminalToolCardState', () => {
  it('shows receiving params while bash input is still streaming', () => {
    const state = getTerminalViewState({
      status: 'streaming',
      liveOutput: '',
      isParamsStreaming: true,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
      canCancelTool: true,
    });

    expect(state.displayPhase).toBe('receiving_params');
    expect(state.waitingMessageKey).toBe('toolCards.terminal.receivingParams');
  });

  it('shows executing after params finish but before command output arrives', () => {
    const state = getTerminalViewState({
      status: 'running',
      liveOutput: '',
      isParamsStreaming: false,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
      canCancelTool: true,
    });

    expect(state.displayPhase).toBe('executing');
    expect(state.waitingMessageKey).toBe('toolCards.terminal.executingCommand');
  });

  it('prefers real terminal output even if params streaming flag lags behind', () => {
    const state = getTerminalViewState({
      status: 'streaming',
      liveOutput: 'npm test\n',
      isParamsStreaming: true,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
      canCancelTool: true,
    });

    expect(state.displayPhase).toBe('live_output');
    expect(state.waitingMessageKey).toBeNull();
  });

  it('switches to completed result once the tool finishes', () => {
    const state = getTerminalViewState({
      status: 'completed',
      liveOutput: 'partial output',
      isParamsStreaming: false,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
      canCancelTool: true,
    });

    expect(state.displayPhase).toBe('completed');
    expect(state.showCompletedResult).toBe(true);
  });

  it('hides the interrupt button when the host cannot cancel tools', () => {
    // A peer host that does not advertise `cancel_tool` must not offer an
    // interrupt that would be a no-op — the target command would keep running.
    const withoutCapability = getTerminalViewState({
      status: 'running',
      liveOutput: '',
      isParamsStreaming: false,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
      canCancelTool: false,
    });
    expect(withoutCapability.showInterruptButton).toBe(false);

    const withCapability = getTerminalViewState({
      status: 'running',
      liveOutput: '',
      isParamsStreaming: false,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
      canCancelTool: true,
    });
    expect(withCapability.showInterruptButton).toBe(true);
  });
});

describe('resolveCanCancelTool', () => {
  it('is always cancellable on the local (controller) surface', () => {
    expect(resolveCanCancelTool(false, null)).toBe(true);
    expect(resolveCanCancelTool(false, caps({ cancelTool: false }))).toBe(true);
  });

  it('stays optimistic while peer capabilities are still being probed', () => {
    expect(resolveCanCancelTool(true, null)).toBe(true);
  });

  it('honors an explicitly advertised cancel_tool flag', () => {
    expect(resolveCanCancelTool(true, caps({ cancelTool: true }))).toBe(true);
    expect(resolveCanCancelTool(true, caps({ cancelTool: false }))).toBe(false);
  });

  it('resolves a null cancel_tool by host kind (old CLI hides the button, old Desktop keeps it)', () => {
    // An older host did not advertise cancel_tool; cancelTool parses to null.
    // host_type still discriminates: an old Desktop always implemented it, an
    // old CLI never did — hide the button so Interrupt doesn't silently fail.
    // See PR #2428 round 5 #1.
    expect(resolveCanCancelTool(true, caps({ hostKind: 'cli' }))).toBe(false);
    expect(resolveCanCancelTool(true, caps({ hostKind: 'desktop' }))).toBe(true);
    expect(resolveCanCancelTool(true, caps({ hostKind: null }))).toBe(true);
  });

  it('an explicitly advertised false beats hostKind', () => {
    expect(resolveCanCancelTool(true, caps({ cancelTool: false, hostKind: 'desktop' }))).toBe(false);
    expect(resolveCanCancelTool(true, caps({ cancelTool: true, hostKind: 'cli' }))).toBe(true);
  });
});
