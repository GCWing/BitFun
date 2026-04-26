export type TerminalWaitingMessageKey =
  | 'toolCards.terminal.receivingParams'
  | 'toolCards.terminal.executingCommand';

export type TerminalDisplayPhase =
  | 'idle'
  | 'receiving_params'
  | 'executing'
  | 'live_output'
  | 'completed'
  | 'cancelled_output';

export interface TerminalViewState {
  isLoading: boolean;
  isFailed: boolean;
  showInterruptButton: boolean;
  showCompletedResult: boolean;
  showCancelledResult: boolean;
  hasHeaderExtra: boolean;
  statusLabel: 'rejected' | 'cancelled' | 'failed' | null;
  statusClassName: 'status-rejected' | 'status-cancelled' | 'status-error' | null;
  displayPhase: TerminalDisplayPhase;
  waitingMessageKey: TerminalWaitingMessageKey | null;
}

interface GetTerminalViewStateParams {
  status: string;
  liveOutput: string;
  isParamsStreaming: boolean;
  interruptRequested: boolean;
  showConfirmButtons: boolean;
  wasInterrupted: boolean;
}

function deriveDisplayPhase(params: {
  status: string;
  liveOutput: string;
  isParamsStreaming: boolean;
}): Pick<TerminalViewState, 'displayPhase' | 'waitingMessageKey'> {
  const { status, liveOutput, isParamsStreaming } = params;
  const hasLiveOutput = liveOutput.length > 0;

  if (status === 'completed') {
    return {
      displayPhase: 'completed',
      waitingMessageKey: null,
    };
  }

  if (status === 'cancelled' && hasLiveOutput) {
    return {
      displayPhase: 'cancelled_output',
      waitingMessageKey: null,
    };
  }

  if (hasLiveOutput && (status === 'streaming' || status === 'running' || status === 'receiving')) {
    return {
      displayPhase: 'live_output',
      waitingMessageKey: null,
    };
  }

  if (isParamsStreaming && (status === 'preparing' || status === 'streaming' || status === 'receiving')) {
    return {
      displayPhase: 'receiving_params',
      waitingMessageKey: 'toolCards.terminal.receivingParams',
    };
  }

  if (status === 'running' || status === 'streaming' || status === 'receiving') {
    return {
      displayPhase: 'executing',
      waitingMessageKey: 'toolCards.terminal.executingCommand',
    };
  }

  return {
    displayPhase: 'idle',
    waitingMessageKey: null,
  };
}

export function getTerminalViewState(
  params: GetTerminalViewStateParams,
): TerminalViewState {
  const {
    status,
    liveOutput,
    isParamsStreaming,
    interruptRequested,
    showConfirmButtons,
    wasInterrupted,
  } = params;
  const isRunning = status === 'running';
  const isLoading =
    status === 'preparing' ||
    status === 'streaming' ||
    status === 'receiving' ||
    status === 'running';
  const showInterruptButton = isRunning && !interruptRequested;

  let statusLabel: TerminalViewState['statusLabel'] = null;
  let statusClassName: TerminalViewState['statusClassName'] = null;

  if (status === 'rejected') {
    statusLabel = 'rejected';
    statusClassName = 'status-rejected';
  } else if ((interruptRequested && isRunning) || wasInterrupted || status === 'cancelled') {
    statusLabel = 'cancelled';
    statusClassName = 'status-cancelled';
  } else if (status === 'error') {
    statusLabel = 'failed';
    statusClassName = 'status-error';
  }

  const { displayPhase, waitingMessageKey } = deriveDisplayPhase({
    status,
    liveOutput,
    isParamsStreaming,
  });

  return {
    isLoading,
    isFailed: status === 'error',
    showInterruptButton,
    showCompletedResult: displayPhase === 'completed',
    showCancelledResult: displayPhase === 'cancelled_output',
    hasHeaderExtra: Boolean(statusLabel || showConfirmButtons || showInterruptButton),
    statusLabel,
    statusClassName,
    displayPhase,
    waitingMessageKey,
  };
}
