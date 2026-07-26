import {
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import { MessageSquarePlus, Sparkles } from 'lucide-react';
import type { Session } from '@/flow_chat/types/flow-chat';
import { globalEventBus } from '@/infrastructure/event-bus';
import { useI18n } from '@/infrastructure/i18n';
import { learningProposalAPI } from '@/infrastructure/api/service-api/LearningProposalAPI';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { notificationService } from '@/shared/notification-system';
import {
  resolveConversationSelection,
  type ConversationSelectionSnapshot,
} from './conversationSelection';
import { showLearningProposalNotification } from './learningProposalNotifications';
import './ConversationSelectionActions.scss';

interface ConversationSelectionActionsProps {
  scopeRef: RefObject<HTMLElement | null>;
  activeSession: Session | null;
  fallbackWorkspacePath: string;
}

function clearBrowserSelection(): void {
  window.getSelection()?.removeAllRanges();
}

function fittedAnchor(
  anchor: ConversationSelectionSnapshot['anchor'],
  popoverSize: { width: number; height: number },
): {
  left: number;
  top: number;
} {
  const viewportInset = 8;
  const availableWidth = Math.max(0, window.innerWidth - viewportInset * 2);
  const availableHeight = Math.max(0, window.innerHeight - viewportInset * 2);
  const halfWidth = Math.min(popoverSize.width, availableWidth) / 2;
  const renderedHeight = Math.min(popoverSize.height, availableHeight);
  const minLeft = viewportInset + halfWidth;
  const maxLeft = Math.max(minLeft, window.innerWidth - viewportInset - halfWidth);
  const maxTop = Math.max(viewportInset, window.innerHeight - viewportInset - renderedHeight);
  return {
    left: Math.min(Math.max(anchor.left, minLeft), maxLeft),
    top: Math.min(Math.max(anchor.top, viewportInset), maxTop),
  };
}

export function ConversationSelectionActions({
  scopeRef,
  activeSession,
  fallbackWorkspacePath,
}: ConversationSelectionActionsProps) {
  const { t } = useI18n('flow-chat');
  const [snapshot, setSnapshot] = useState<ConversationSelectionSnapshot | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [popoverSize, setPopoverSize] = useState({ width: 280, height: 38 });

  const dismiss = useCallback(() => {
    setSnapshot(null);
  }, []);

  useEffect(() => {
    let frameId: number | null = null;
    const updateSelection = () => {
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      frameId = window.requestAnimationFrame(() => {
        frameId = null;
        setSnapshot(resolveConversationSelection(window.getSelection(), scopeRef.current));
      });
    };
    const dismissForViewportChange = () => dismiss();

    document.addEventListener('selectionchange', updateSelection);
    window.addEventListener('scroll', dismissForViewportChange, true);
    window.addEventListener('resize', dismissForViewportChange);
    return () => {
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      document.removeEventListener('selectionchange', updateSelection);
      window.removeEventListener('scroll', dismissForViewportChange, true);
      window.removeEventListener('resize', dismissForViewportChange);
    };
  }, [activeSession?.sessionId, dismiss, scopeRef]);

  useEffect(() => {
    setSnapshot(null);
  }, [activeSession?.sessionId]);

  useLayoutEffect(() => {
    if (!snapshot || !popoverRef.current) {
      return;
    }
    const rect = popoverRef.current.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      setPopoverSize({ width: rect.width, height: rect.height });
    }
  }, [snapshot]);

  const handleAddToInput = useCallback(() => {
    if (!snapshot) {
      return;
    }
    globalEventBus.emit('fill-chat-input', {
      content: snapshot.selectedText,
      mode: 'append',
      separator: '\n\n',
    });
    clearBrowserSelection();
    setSnapshot(null);
  }, [snapshot]);

  const handleCreateProposal = useCallback(async () => {
    if (!isTauriRuntime() || !snapshot || !activeSession || isCreating) {
      return;
    }

    const workspacePath = activeSession.workspacePath
      || activeSession.config.workspacePath
      || fallbackWorkspacePath;
    if (!workspacePath) {
      notificationService.error(t('learningProposal.errors.workspaceRequired'));
      return;
    }

    const captured = snapshot;
    setIsCreating(true);
    setSnapshot(null);
    clearBrowserSelection();
    const loading = notificationService.loading({
      title: t('learningProposal.notification.analyzingTitle'),
      message: t('learningProposal.notification.analyzingMessage'),
    });

    try {
      const proposal = await learningProposalAPI.create({
        sessionId: activeSession.sessionId,
        workspacePath,
        remoteConnectionId: activeSession.remoteConnectionId || activeSession.config.remoteConnectionId,
        remoteSshHost: activeSession.remoteSshHost || activeSession.config.remoteSshHost,
        source: {
          selectedText: captured.selectedText,
          turnId: captured.turnId,
          roundId: captured.roundId,
          itemId: captured.itemId,
          sourceKind: captured.sourceKind,
        },
      });
      loading.cancel();
      showLearningProposalNotification(proposal, t);
    } catch (_error) {
      loading.cancel();
      notificationService.error(t('learningProposal.errors.createFailed'), {
        duration: 0,
      });
    } finally {
      setIsCreating(false);
    }
  }, [activeSession, fallbackWorkspacePath, isCreating, snapshot, t]);

  if (!snapshot || typeof document === 'undefined') {
    return null;
  }

  const anchor = fittedAnchor(snapshot.anchor, popoverSize);
  const canCaptureLearning = isTauriRuntime();
  return createPortal(
    <div
      ref={popoverRef}
      className="conversation-selection-actions"
      role="toolbar"
      aria-label={t('learningProposal.selectionToolbarLabel')}
      style={{ left: anchor.left, top: anchor.top }}
      onPointerDown={(event) => event.preventDefault()}
    >
      <button
        type="button"
        title={t('learningProposal.actions.addToInput')}
        onClick={handleAddToInput}
      >
        <MessageSquarePlus size={14} aria-hidden="true" />
        <span>{t('learningProposal.actions.addToInput')}</span>
      </button>
      {canCaptureLearning && (
        <>
          <span className="conversation-selection-actions__divider" aria-hidden="true" />
          <button
            type="button"
            title={t('learningProposal.actions.capture')}
            onClick={() => { void handleCreateProposal(); }}
            disabled={isCreating}
          >
            <Sparkles size={14} aria-hidden="true" />
            <span>{t('learningProposal.actions.capture')}</span>
          </button>
        </>
      )}
    </div>,
    document.body,
  );
}
