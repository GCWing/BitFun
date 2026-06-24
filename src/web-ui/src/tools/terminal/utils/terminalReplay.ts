import type { TerminalReplayEvent } from '../types';

// eslint-disable-next-line no-control-regex -- Terminal replay can contain OSC metadata sequences.
const OSC_SEQUENCE_RE = /\x1b\][^\x07]*(?:\x07|\x1b\\)/g;
// eslint-disable-next-line no-control-regex -- Terminal replay can contain DCS metadata sequences.
const DCS_SEQUENCE_RE = /\x1bP[\s\S]*?(?:\x07|\x1b\\)/g;
// eslint-disable-next-line no-control-regex -- Terminal replay can contain CSI styling and cursor sequences.
const CSI_SEQUENCE_RE = /\x1b\[[0-?]*[ -/]*[@-~]/g;
// eslint-disable-next-line no-control-regex -- Terminal replay can contain single ESC control sequences.
const ESC_SEQUENCE_RE = /\x1b(?:[@-Z\\-_]|\([A-Za-z0-9]|\)[A-Za-z0-9])/g;
// eslint-disable-next-line no-control-regex -- Remove remaining C0/C1 control characters before checking screen text.
const CONTROL_CHARACTER_RE = /[\x00-\x1f\x7f-\x9f]/g;

function screenTextFromReplayData(data: string): string {
  return data
    .replace(OSC_SEQUENCE_RE, '')
    .replace(DCS_SEQUENCE_RE, '')
    .replace(CSI_SEQUENCE_RE, '')
    .replace(ESC_SEQUENCE_RE, '')
    .replace(CONTROL_CHARACTER_RE, '')
    .trim();
}

/**
 * The replay column guard protects historical screen text from being wrapped or
 * truncated by intermediate panel widths. Pure resize events and shell metadata
 * do not need that protection and should not block the first real fit.
 */
export function terminalReplayHasScreenText(events: TerminalReplayEvent[]): boolean {
  return events.some(event => screenTextFromReplayData(event.data).length > 0);
}
