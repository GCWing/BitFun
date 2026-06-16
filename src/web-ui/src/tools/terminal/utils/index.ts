/**
 * Terminal utilities.
 */

export { TerminalResizeDebouncer } from './TerminalResizeDebouncer';
export type { ResizeCallback, ResizeDebounceOptions } from './TerminalResizeDebouncer';
export { ResizeRepaintGuard, createResizeRepaintScreenSnapshot } from './resizeRepaintGuard';
export { terminalReplayHasScreenText } from './terminalReplay';
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

