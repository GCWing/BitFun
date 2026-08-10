/**
 * Clipboard text for a single dialog turn, resolved from the FlowChat store.
 *
 * Shared by the round footer copy action and the FlowChat context menu so both
 * offer the same "full process" / "result only" scopes.
 */

import { FlowChatStore } from '../store/FlowChatStore';
import type { DialogTurn } from '../types/flow-chat';
import {
  collectRuntimeTurn,
  formatTranscriptTurnText,
  type TranscriptExportLabels,
  type TranscriptExportScope,
} from './dialogTranscriptExport';

export function findDialogTurnById(turnId: string): DialogTurn | undefined {
  const state = FlowChatStore.getInstance().getState();
  for (const [, session] of state.sessions) {
    const dialogTurn = session.dialogTurns.find(turn => turn.id === turnId);
    if (dialogTurn) {
      return dialogTurn;
    }
  }
  return undefined;
}

export function buildDialogTurnCopyText(
  turnId: string,
  scope: TranscriptExportScope,
  labels: TranscriptExportLabels
): string {
  const dialogTurn = findDialogTurnById(turnId);
  if (!dialogTurn) return '';

  return formatTranscriptTurnText(collectRuntimeTurn(dialogTurn), scope, labels);
}
