import {
  MINIAPP_COMPOSER_MESSAGE_EVENT,
  type MiniAppComposerMessageDetail,
} from './miniAppStore';

const MINIAPP_MESSAGE_TIMEOUT_MS = 30 * 60 * 1000;

interface PendingMiniAppMessage {
  token: string;
  timeoutId: number;
  removeAbortListener: () => void;
  resolve: () => void;
  reject: (error: Error) => void;
}

const pendingMessages = new Map<string, PendingMiniAppMessage>();
let messageSequence = 0;

function nextRequestId(): string {
  messageSequence += 1;
  return `miniapp-message-${Date.now()}-${messageSequence}`;
}

function dispatchMessage(
  detail: MiniAppComposerMessageDetail,
  requestId: string,
): void {
  window.dispatchEvent(new CustomEvent(MINIAPP_COMPOSER_MESSAGE_EVENT, {
    detail: {
      ...detail,
      requestId,
      source: detail.source ?? 'composer',
    },
  }));
}

function settlePendingMessage(
  requestId: string,
  settle: (pending: PendingMiniAppMessage) => void,
): boolean {
  const pending = pendingMessages.get(requestId);
  if (!pending) return false;
  pendingMessages.delete(requestId);
  window.clearTimeout(pending.timeoutId);
  pending.removeAbortListener();
  settle(pending);
  return true;
}

/**
 * Fire-and-forget route used by the shared text composer. Every dispatch still
 * gets an id so the injected MiniApp bridge can coalesce accidental replay
 * without conflating two intentional submissions that contain the same text.
 */
export function postMiniAppComposerMessage(
  detail: MiniAppComposerMessageDetail,
): string {
  const requestId = nextRequestId();
  dispatchMessage(detail, requestId);
  return requestId;
}

/**
 * Route used by realtime voice. It resolves only after all MiniApp
 * `app.chat.onUserMessage` callbacks settle. The caller separately observes
 * the bound Agent session, so both app-side post-processing and Agent work are
 * complete before Voice reports the result.
 */
export function requestMiniAppComposerMessage(
  detail: MiniAppComposerMessageDetail,
  signal?: AbortSignal,
): Promise<void> {
  if (signal?.aborted) {
    return Promise.reject(new Error('MiniApp message was cancelled before dispatch'));
  }

  const requestId = nextRequestId();
  const completion = new Promise<void>((resolve, reject) => {
    const handleAbort = () => {
      settlePendingMessage(requestId, pending => {
        pending.reject(new Error('MiniApp message was cancelled'));
      });
    };
    const timeoutId = window.setTimeout(() => {
      settlePendingMessage(requestId, pending => {
        pending.reject(new Error('MiniApp message timed out after 30 minutes'));
      });
    }, MINIAPP_MESSAGE_TIMEOUT_MS);
    signal?.addEventListener('abort', handleAbort, { once: true });
    pendingMessages.set(requestId, {
      token: detail.token,
      timeoutId,
      removeAbortListener: () => signal?.removeEventListener('abort', handleAbort),
      resolve,
      reject,
    });
  });

  dispatchMessage(detail, requestId);
  return completion;
}

/** Called only by the bridge instance that owns `token`. */
export function completeMiniAppComposerMessage(
  token: string,
  requestId: string,
  error?: string,
): boolean {
  const pending = pendingMessages.get(requestId);
  if (!pending || pending.token !== token) return false;
  return settlePendingMessage(requestId, entry => {
    if (error?.trim()) entry.reject(new Error(error.trim()));
    else entry.resolve();
  });
}

/** Fail pending voice work promptly when its owning iframe goes away. */
export function rejectPendingMiniAppComposerMessages(
  token: string,
  reason: string,
): void {
  for (const [requestId, pending] of pendingMessages) {
    if (pending.token !== token) continue;
    settlePendingMessage(requestId, entry => entry.reject(new Error(reason)));
  }
}
