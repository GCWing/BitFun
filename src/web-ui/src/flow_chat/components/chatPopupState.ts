// Module-level popup state – used by ModernFlowChatContainer to conditionally
// disable the Escape shortcut so that slash-command and @-mention popups can be
// closed with Escape.
let _chatPopupActive = false;
const _chatPopupListeners = new Set<() => void>();

export function isChatPopupActive(): boolean {
  return _chatPopupActive;
}

export function subscribeChatPopupChange(listener: () => void): () => void {
  _chatPopupListeners.add(listener);
  return () => { _chatPopupListeners.delete(listener); };
}

export function setChatPopupActive(active: boolean) {
  if (_chatPopupActive !== active) {
    _chatPopupActive = active;
    _chatPopupListeners.forEach(fn => fn());
  }
}