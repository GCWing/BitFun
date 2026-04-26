import { describe, expect, it } from 'vitest';

import { getTerminalViewState } from './terminalToolCardState';

describe('terminalToolCardState', () => {
  it('shows receiving params while bash input is still streaming', () => {
    const state = getTerminalViewState({
      status: 'streaming',
      liveOutput: '',
      isParamsStreaming: true,
      interruptRequested: false,
      showConfirmButtons: false,
      wasInterrupted: false,
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
    });

    expect(state.displayPhase).toBe('completed');
    expect(state.showCompletedResult).toBe(true);
  });
});
