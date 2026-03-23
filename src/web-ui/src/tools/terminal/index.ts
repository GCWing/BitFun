/**
 * Terminal module exports.
 */

export { Terminal, ConnectedTerminal } from './components';
export type { 
  TerminalProps, 
  TerminalRef, 
  TerminalOptions,
  ConnectedTerminalProps,
} from './components';

export {
  TerminalService,
  getTerminalService,
  deleteManualTerminalProfile,
  generateManualTerminalProfileId,
  getManualTerminalProfileById,
  getManualTerminalProfileBySessionId,
  listManualTerminalProfiles,
  loadManualTerminalProfiles,
  saveManualTerminalProfiles,
  upsertManualTerminalProfile,
  type ManualTerminalProfile,
  type ManualTerminalProfileInput,
  type ManualTerminalProfilesState,
} from './services';

export { useTerminal } from './hooks';
export type { UseTerminalOptions, UseTerminalReturn } from './hooks';

export { TerminalResizeDebouncer } from './utils';
export type { ResizeCallback, ResizeDebounceOptions } from './utils';

export type {
  SessionStatus,
  ShellType,
  CreateSessionRequest,
  SessionResponse,
  ShellInfo,
  WriteRequest,
  ResizeRequest,
  CloseSessionRequest,
  SignalRequest,
  AcknowledgeRequest,
  ExecuteCommandRequest,
  ExecuteCommandResponse,
  SendCommandRequest,
  TerminalEventType,
  TerminalEventBase,
  TerminalReadyEvent,
  TerminalOutputEvent,
  TerminalExitEvent,
  TerminalErrorEvent,
  TerminalResizeEvent,
  TerminalTitleEvent,
  TerminalCwdEvent,
  TerminalEvent,
  TerminalEventCallback,
  UnsubscribeFunction,
} from './types';

