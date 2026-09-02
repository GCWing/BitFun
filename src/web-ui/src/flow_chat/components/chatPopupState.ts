// Module-level popup state used by Escape shortcut owners so slash-command,
// mention, and modal surfaces can close without cancelling the current task.
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
