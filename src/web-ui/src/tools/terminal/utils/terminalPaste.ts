import { confirmDialogChoice } from '@/component-library';
import { i18nService } from '@/infrastructure/i18n';

export type TerminalPasteWarningMode = 'auto' | 'always' | 'never';

export interface TerminalPasteConfirmationRequest {
  lineCount: number;
  preview: string;
}

export type TerminalPasteConfirmationResult = 'paste' | 'pasteAsSingleLine' | 'cancel';

export interface TerminalPasteOptions {
  bracketedPasteMode?: boolean;
  warningMode?: TerminalPasteWarningMode;
  confirmMultiLinePaste?: (
    request: TerminalPasteConfirmationRequest,
  ) => Promise<TerminalPasteConfirmationResult> | TerminalPasteConfirmationResult;
}

export type TerminalPasteDecision =
  | { allow: true; text: string }
  | { allow: false };

export interface TerminalPasteAnalysis {
  shouldConfirm: boolean;
  lineCount: number;
  preview: string;
  text: string;
}

const DEFAULT_WARNING_MODE: TerminalPasteWarningMode = 'auto';
const PREVIEW_LINE_COUNT = 3;
const PREVIEW_LINE_MAX_LENGTH = 30;
export const POWERSHELL_READLINE_PASTE_SEQUENCE = '\x16';

export function isWindowsClientPlatform(): boolean {
  if (typeof navigator === 'undefined') {
    return false;
  }

  const platform = navigator.platform ?? '';
  const userAgent = navigator.userAgent ?? '';
  return /\bWin/i.test(platform) || /\bWindows\b/i.test(userAgent);
}

export function isPowerShellShellType(shellType: string | undefined | null): boolean {
  const normalized = shellType?.trim().toLowerCase() ?? '';
  return normalized === 'powershell'
    || normalized === 'powershellcore'
    || normalized === 'pwsh'
    || normalized === 'windows powershell'
    || normalized === 'powershell 7';
}

export function shouldUsePowerShellReadlinePaste(
  shellType: string | undefined | null,
  options: { isWindowsClient?: boolean } = {},
): boolean {
  return (options.isWindowsClient ?? isWindowsClientPlatform())
    && isPowerShellShellType(shellType);
}

function splitPasteLines(text: string): string[] {
  return text.split(/\r?\n/);
}

function truncatePreviewLine(line: string): string {
  if (line.length <= PREVIEW_LINE_MAX_LENGTH) {
    return line;
  }

  return `${line.slice(0, PREVIEW_LINE_MAX_LENGTH)}...`;
}

export function buildTerminalPastePreview(lines: string[]): string {
  const previewLines = lines
    .slice(0, PREVIEW_LINE_COUNT)
    .map(truncatePreviewLine);

  if (lines.length > PREVIEW_LINE_COUNT) {
    previewLines.push('...');
  }

  return previewLines.join('\n');
}

export function analyzeTerminalPaste(
  text: string,
  options: Pick<TerminalPasteOptions, 'bracketedPasteMode' | 'warningMode'> = {},
): TerminalPasteAnalysis {
  const warningMode = options.warningMode ?? DEFAULT_WARNING_MODE;
  const lines = splitPasteLines(text);
  const lineCount = lines.length;

  if (lineCount === 1 || warningMode === 'never') {
    return {
      shouldConfirm: false,
      lineCount,
      preview: buildTerminalPastePreview(lines),
      text,
    };
  }

  if (warningMode === 'auto') {
    if (options.bracketedPasteMode) {
      return {
        shouldConfirm: false,
        lineCount,
        preview: buildTerminalPastePreview(lines),
        text,
      };
    }

    // Match VS Code's auto-mode safety behavior: a copied command with only a
    // trailing blank line should paste for review without immediately running.
    if (lineCount === 2 && lines[1].trim().length === 0) {
      return {
        shouldConfirm: false,
        lineCount,
        preview: buildTerminalPastePreview(lines),
        text: lines[0],
      };
    }
  }

  return {
    shouldConfirm: true,
    lineCount,
    preview: buildTerminalPastePreview(lines),
    text,
  };
}

export async function confirmTerminalMultiLinePaste(
  request: TerminalPasteConfirmationRequest,
): Promise<TerminalPasteConfirmationResult> {
  const choice = await confirmDialogChoice({
    title: i18nService.t('tools:terminal.pasteDialog.title'),
    message: i18nService.t('tools:terminal.pasteDialog.message', {
      lineCount: request.lineCount,
    }),
    type: 'warning',
    confirmText: i18nService.t('tools:terminal.pasteDialog.confirm'),
    secondaryText: i18nService.t('tools:terminal.pasteDialog.pasteAsOneLine'),
    cancelText: i18nService.t('tools:terminal.pasteDialog.cancel'),
    preview: request.preview,
    previewMaxHeight: 150,
  });

  if (choice === 'secondary') {
    return 'pasteAsSingleLine';
  }

  return choice === 'confirm' ? 'paste' : 'cancel';
}

export async function resolveTerminalPaste(
  text: string,
  options: TerminalPasteOptions = {},
): Promise<TerminalPasteDecision> {
  const analysis = analyzeTerminalPaste(text, options);

  if (!analysis.shouldConfirm) {
    return { allow: true, text: analysis.text };
  }

  const confirmation = await (options.confirmMultiLinePaste ?? confirmTerminalMultiLinePaste)({
    lineCount: analysis.lineCount,
    preview: analysis.preview,
  });

  if (confirmation === 'cancel') {
    return { allow: false };
  }

  if (confirmation === 'pasteAsSingleLine') {
    return { allow: true, text: analysis.text.replace(/\r?\n/g, '') };
  }

  return { allow: true, text: analysis.text };
}
