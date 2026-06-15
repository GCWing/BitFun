/**
 * Guards xterm.js against incomplete resize repaint streams from Windows PTYs.
 */

import type { Terminal } from '@xterm/xterm';

const RESIZE_REPAINT_SUPPRESSION_WINDOW_MS = 750;
const REPAINT_TAIL_SUPPRESSION_MS = 80;
const MIN_EXISTING_LINES_FOR_REPAINT_MATCH = 3;
const MIN_MATCHING_EXISTING_LINES = 2;
const MIN_LINE_SIGNATURE_LENGTH = 8;

export interface ResizeRepaintScreenSnapshot {
  bufferType: 'normal' | 'alternate';
  cols: number;
  rows: number;
  viewportY: number;
  baseY: number;
  visibleNonEmptyLines: string[];
}

export interface ResizeRepaintMark {
  cols: number;
  rows: number;
  previousCols?: number;
  previousRows?: number;
  shellType?: string;
  screen: ResizeRepaintScreenSnapshot;
  nowMs?: number;
}

export interface ResizeRepaintSuppressionDetails {
  reason: string;
  pending: {
    cols: number;
    rows: number;
    previousCols?: number;
    previousRows?: number;
    shellType?: string;
    ageMs: number;
  };
  topLine: string;
  matchingLines: string[];
}

export type ResizeRepaintGuardDecision =
  | { suppress: false; reason: string }
  | { suppress: true; details: ResizeRepaintSuppressionDetails };

interface PendingResizeRepaint extends Omit<ResizeRepaintMark, 'nowMs'> {
  markedAtMs: number;
  expiresAtMs: number;
}

function currentTimeMs(): number {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
    return performance.now();
  }
  return Date.now();
}

export function createResizeRepaintScreenSnapshot(
  terminal: Terminal | null,
): ResizeRepaintScreenSnapshot | null {
  if (!terminal) {
    return null;
  }

  const buffer = terminal.buffer.active;
  const visibleNonEmptyLines: string[] = [];
  const maxRows = Math.min(terminal.rows, 80);

  for (let row = 0; row < maxRows; row += 1) {
    const line = buffer.getLine(buffer.viewportY + row)?.translateToString(true) ?? '';
    if (line.trim().length > 0) {
      visibleNonEmptyLines.push(line);
    }
  }

  return {
    bufferType: buffer.type,
    cols: terminal.cols,
    rows: terminal.rows,
    viewportY: buffer.viewportY,
    baseY: buffer.baseY,
    visibleNonEmptyLines,
  };
}

// eslint-disable-next-line no-control-regex -- Terminal output uses ESC control sequences.
const OSC_SEQUENCE_RE = /\x1b\][^\x07]*(?:\x07|\x1b\\)/g;
// eslint-disable-next-line no-control-regex -- Terminal output uses ESC control sequences.
const CSI_SEQUENCE_RE = /\x1b\[[0-?]*[ -/]*[@-~]/g;
// eslint-disable-next-line no-control-regex -- Terminal output uses ESC control sequences.
const ESC_SEQUENCE_RE = /\x1b(?:[@-Z\\-_]|\([A-Za-z0-9]|\)[A-Za-z0-9])/g;

function stripTerminalControlSequences(value: string): string {
  return value
    .replace(OSC_SEQUENCE_RE, '')
    .replace(CSI_SEQUENCE_RE, '')
    .replace(ESC_SEQUENCE_RE, '');
}

function normalizeLineForComparison(value: string): string {
  return stripTerminalControlSequences(value)
    .replace(/[\r\n]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase();
}

function normalizeOutputForComparison(value: string): string {
  return stripTerminalControlSequences(value)
    .replace(/\r/g, '\n')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase();
}

function lineSignature(value: string): string | null {
  const normalized = normalizeLineForComparison(value);
  return normalized.length >= MIN_LINE_SIGNATURE_LENGTH ? normalized : null;
}

function getTextAfterHome(data: string): string {
  // eslint-disable-next-line no-control-regex -- Matching CSI cursor-home commands.
  const homeMatch = /\x1b\[(?:1;1)?[Hf]/.exec(data);
  if (!homeMatch) {
    return data;
  }

  return data.slice(homeMatch.index + homeMatch[0].length);
}

function repaintStartsWithLine(data: string, line: string): boolean {
  const afterHome = getTextAfterHome(data);
  const firstNonEmptyLine = afterHome
    .split(/\r\n|\r|\n/)
    .map(lineSignature)
    .find((signature): signature is string => Boolean(signature));

  return firstNonEmptyLine === line;
}

function hasHomeRepaintPrefix(data: string): boolean {
  const prefix = data.slice(0, 160);
  // eslint-disable-next-line no-control-regex -- Matching CSI cursor-home commands.
  const homeMatch = /\x1b\[(?:1;1)?[Hf]/.exec(prefix);
  if (!homeMatch || homeMatch.index > 48) {
    return false;
  }

  const beforeHome = stripTerminalControlSequences(prefix.slice(0, homeMatch.index))
    .replace(/[\r\n]/g, '')
    .trim();
  if (beforeHome.length > 0) {
    return false;
  }

  const afterHome = prefix.slice(homeMatch.index + homeMatch[0].length);
  // eslint-disable-next-line no-control-regex -- Matching CSI erase-in-line commands.
  const clearMatch = /\x1b\[[0-2]?K/.exec(afterHome);
  if (!clearMatch) {
    return false;
  }

  const firstLineBreak = afterHome.search(/\r|\n/);
  if (firstLineBreak >= 0) {
    return clearMatch.index <= firstLineBreak;
  }

  return clearMatch.index <= 96;
}

function buildSuppressionDecision(
  pending: PendingResizeRepaint,
  data: string,
  nowMs: number,
): ResizeRepaintGuardDecision {
  if (nowMs > pending.expiresAtMs) {
    return { suppress: false, reason: 'expired' };
  }

  if (pending.screen.bufferType !== 'normal') {
    return { suppress: false, reason: 'alternate-buffer' };
  }

  if (!hasHomeRepaintPrefix(data)) {
    return { suppress: false, reason: 'no-home-repaint-prefix' };
  }

  const existingLines = pending.screen.visibleNonEmptyLines
    .map(lineSignature)
    .filter((line): line is string => Boolean(line));

  if (existingLines.length < MIN_EXISTING_LINES_FOR_REPAINT_MATCH) {
    return { suppress: false, reason: 'insufficient-existing-lines' };
  }

  const topLine = existingLines[0];
  const outputText = normalizeOutputForComparison(data);
  // A full repaint that starts with the current top line is safe. The damaging
  // ConPTY repaint starts from the old viewport instead and overwrites xterm's
  // preserved scrollback head.
  if (repaintStartsWithLine(data, topLine)) {
    return { suppress: false, reason: 'repaint-starts-with-top-line' };
  }

  // Require overlap with later existing lines before suppressing. This keeps
  // ordinary prompt/output that happens to move home from being dropped.
  const matchingLines = existingLines
    .slice(1)
    .filter(line => outputText.includes(line));

  if (matchingLines.length < MIN_MATCHING_EXISTING_LINES) {
    return { suppress: false, reason: 'insufficient-existing-line-overlap' };
  }

  return {
    suppress: true,
    details: {
      reason: 'incomplete-resize-repaint',
      pending: {
        cols: pending.cols,
        rows: pending.rows,
        previousCols: pending.previousCols,
        previousRows: pending.previousRows,
        shellType: pending.shellType,
        ageMs: Math.round(nowMs - pending.markedAtMs),
      },
      topLine,
      matchingLines: matchingLines.slice(0, 5),
    },
  };
}

export class ResizeRepaintGuard {
  private pending: PendingResizeRepaint | null = null;
  private suppressTailUntilMs = 0;

  markResize(mark: ResizeRepaintMark): PendingResizeRepaint | null {
    const nowMs = mark.nowMs ?? currentTimeMs();

    // Empty screens are common during terminal startup. There is no existing
    // content to protect, and suppressing the initial clear-screen repaint would
    // hide legitimate shell initialization.
    if (mark.screen.visibleNonEmptyLines.length === 0) {
      this.pending = null;
      return null;
    }

    this.pending = {
      cols: mark.cols,
      rows: mark.rows,
      previousCols: mark.previousCols,
      previousRows: mark.previousRows,
      shellType: mark.shellType,
      screen: mark.screen,
      markedAtMs: nowMs,
      expiresAtMs: nowMs + RESIZE_REPAINT_SUPPRESSION_WINDOW_MS,
    };
    this.suppressTailUntilMs = 0;
    return this.pending;
  }

  inspect(data: string, nowMs = currentTimeMs()): ResizeRepaintGuardDecision {
    if (this.suppressTailUntilMs > 0 && nowMs <= this.suppressTailUntilMs) {
      return {
        suppress: true,
        details: {
          reason: 'resize-repaint-tail',
          pending: {
            cols: this.pending?.cols ?? 0,
            rows: this.pending?.rows ?? 0,
            previousCols: this.pending?.previousCols,
            previousRows: this.pending?.previousRows,
            shellType: this.pending?.shellType,
            ageMs: this.pending ? Math.round(nowMs - this.pending.markedAtMs) : 0,
          },
          topLine: '',
          matchingLines: [],
        },
      };
    }

    if (!this.pending) {
      return { suppress: false, reason: 'no-pending-resize' };
    }

    const decision = buildSuppressionDecision(this.pending, data, nowMs);
    if (decision.suppress) {
      this.pending = null;
      this.suppressTailUntilMs = nowMs + REPAINT_TAIL_SUPPRESSION_MS;
      return decision;
    }

    if (decision.reason === 'expired') {
      this.pending = null;
    }

    return decision;
  }

  clear(): void {
    this.pending = null;
    this.suppressTailUntilMs = 0;
  }
}
