/**
 * Terminal utilities.
 */

export { TerminalInputQueue } from './TerminalInputQueue';
export { TerminalResizeDebouncer } from './TerminalResizeDebouncer';
export type { ResizeCallback, ResizeDebounceOptions } from './TerminalResizeDebouncer';
export { ResizeRepaintGuard, createResizeRepaintScreenSnapshot } from './resizeRepaintGuard';
export { terminalReplayHasScreenText } from './terminalReplay';
export {
  POWERSHELL_READLINE_PASTE_SEQUENCE,
  analyzeTerminalPaste,
  buildTerminalPastePreview,
  confirmTerminalMultiLinePaste,
  isPowerShellShellType,
  isWindowsClientPlatform,
  resolveTerminalPaste,
  shouldUsePowerShellReadlinePaste,
} from './terminalPaste';
export type {
  TerminalPasteAnalysis,
  TerminalPasteConfirmationRequest,
  TerminalPasteConfirmationResult,
  TerminalPasteDecision,
  TerminalPasteOptions,
  TerminalPasteWarningMode,
} from './terminalPaste';
export type {
  ResizeRepaintGuardDecision,
  ResizeRepaintMark,
  ResizeRepaintScreenSnapshot,
  ResizeRepaintSuppressionDetails,
} from './resizeRepaintGuard';
export {
  buildXtermTheme,
  getXtermAnsiPalette,
  getXtermFontWeights,
  DEFAULT_XTERM_MINIMUM_CONTRAST_RATIO,
} from './xtermTheme';

