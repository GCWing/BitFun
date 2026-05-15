import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { AlertTriangle, Check, Loader2, RefreshCw, Send, Trash2, X } from 'lucide-react';
import { Button, IconButton } from '@/component-library';
import type { MiniApp, MiniAppDraft } from '@/infrastructure/api/service-api/MiniAppAPI';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { buildMiniAppCustomizationPrompt } from './miniAppCustomizationPrompt';
import { requiresPermissionConfirmation } from './miniAppCustomizationRisk';
import type { MiniAppCustomizationState } from './miniAppCustomizationTypes';
import MiniAppDraftPreview from './MiniAppDraftPreview';
import MiniAppPermissionDiffDialog from './MiniAppPermissionDiffDialog';

const log = createLogger('MiniAppCustomizePanel');

const initialState: MiniAppCustomizationState = {
  stage: 'notice',
  draft: null,
  permissionDiff: null,
  assistantSessionId: null,
  error: null,
};

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

interface MiniAppCustomizePanelProps {
  open: boolean;
  app: MiniApp;
  appName: string;
  themeType?: string;
  workspacePath?: string;
  onClose: () => void;
  onApplied: (app: MiniApp) => void;
}

export const MiniAppCustomizePanel: React.FC<MiniAppCustomizePanelProps> = ({
  open,
  app,
  appName,
  themeType,
  workspacePath,
  onClose,
  onApplied,
}) => {
  const { t } = useI18n('scenes/miniapp');
  const [state, setState] = useState<MiniAppCustomizationState>(initialState);
  const [userRequest, setUserRequest] = useState('');
  const [previewKey, setPreviewKey] = useState(0);
  const [discarding, setDiscarding] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const theme = themeType ?? 'dark';

  const trimmedRequest = userRequest.trim();
  const busy = state.stage === 'drafting' || state.stage === 'applying' || discarding || refreshing;
  const hasPreview = state.draft !== null;

  useEffect(() => {
    setState(initialState);
    setUserRequest('');
    setPreviewKey(0);
  }, [app.id]);

  useEffect(() => {
    if (open && state.stage === 'idle' && !state.draft) {
      setState(initialState);
    }
  }, [open, state.draft, state.stage]);

  const ensureParentSession = useCallback(async (): Promise<string> => {
    const [{ flowChatStore }, { FlowChatManager }] = await Promise.all([
      import('@/flow_chat/store/FlowChatStore'),
      import('@/flow_chat/services/FlowChatManager'),
    ]);
    const currentState = flowChatStore.getState();
    const activeSessionId = currentState.activeSessionId;
    if (activeSessionId) {
      const activeSession = currentState.sessions.get(activeSessionId);
      if (!workspacePath || activeSession?.workspacePath === workspacePath) {
        return activeSessionId;
      }
    }
    if (!workspacePath) {
      throw new Error(t('customize.workspaceRequired'));
    }
    return FlowChatManager.getInstance().createChatSession({ workspacePath }, 'agentic');
  }, [t, workspacePath]);

  const launchAssistant = useCallback(async (draft: MiniAppDraft, request: string) => {
    const [
      { createBtwChildSession },
      { openBtwSessionInAuxPane },
      { FlowChatManager },
    ] = await Promise.all([
      import('@/flow_chat/services/BtwThreadService'),
      import('@/flow_chat/services/openBtwSession'),
      import('@/flow_chat/services/FlowChatManager'),
    ]);
    const parentSessionId = await ensureParentSession();
    const prompt = buildMiniAppCustomizationPrompt({
      appId: app.id,
      appName,
      draftId: draft.draftId,
      draftRoot: draft.draftRoot,
      userRequest: request,
    });

    const created = await createBtwChildSession({
      parentSessionId,
      workspacePath,
      childSessionName: t('customize.sessionName', { name: appName }),
      agentType: 'agentic',
      enableTools: true,
      safeMode: true,
      autoCompact: true,
      enableContextCompression: true,
      addMarker: false,
      sessionKind: 'btw',
    });

    openBtwSessionInAuxPane({
      childSessionId: created.childSessionId,
      parentSessionId,
      workspacePath,
      expand: true,
    });

    await FlowChatManager.getInstance().sendMessage(
      prompt,
      created.childSessionId,
      request,
    );

    setState((prev) => ({
      ...prev,
      stage: 'preview',
      assistantSessionId: created.childSessionId,
      error: null,
    }));
  }, [app.id, appName, ensureParentSession, t, workspacePath]);

  const handleStart = useCallback(async () => {
    if (!trimmedRequest || busy) {
      return;
    }

    setState((prev) => ({ ...prev, stage: 'drafting', error: null }));
    try {
      const draft = state.draft ?? await miniAppAPI.createDraft(app.id, theme, workspacePath);
      setState((prev) => ({
        ...prev,
        stage: 'preview',
        draft,
        permissionDiff: null,
        error: null,
      }));
      await launchAssistant(draft, trimmedRequest);
    } catch (error) {
      log.error('MiniApp customization launch failed', error);
      setState((prev) => ({
        ...prev,
        stage: prev.draft ? 'preview' : 'notice',
        error: t('customize.launchFailed', { error: formatError(error) }),
      }));
    }
  }, [app.id, busy, launchAssistant, state.draft, t, theme, trimmedRequest, workspacePath]);

  const handleRefreshPreview = useCallback(async () => {
    if (!state.draft || refreshing) {
      return;
    }

    setRefreshing(true);
    try {
      const draft = await miniAppAPI.syncDraftFromFs(
        app.id,
        state.draft.draftId,
        theme,
        workspacePath,
      );
      setState((prev) => ({ ...prev, draft, stage: 'preview', error: null }));
      setPreviewKey((value) => value + 1);
    } catch (error) {
      log.error('MiniApp draft preview refresh failed', error);
      setState((prev) => ({
        ...prev,
        error: t('customize.refreshFailed', { error: formatError(error) }),
      }));
    } finally {
      setRefreshing(false);
    }
  }, [app.id, refreshing, state.draft, t, theme, workspacePath]);

  const applyDraft = useCallback(async () => {
    if (!state.draft) {
      return;
    }

    setState((prev) => ({ ...prev, stage: 'applying', error: null }));
    try {
      const updated = await miniAppAPI.applyDraft(
        app.id,
        state.draft.draftId,
        theme,
        workspacePath,
      );
      setState(initialState);
      onApplied(updated);
      onClose();
    } catch (error) {
      log.error('MiniApp draft apply failed', error);
      setState((prev) => ({
        ...prev,
        stage: 'preview',
        error: t('customize.applyFailed', { error: formatError(error) }),
      }));
    }
  }, [app.id, onApplied, onClose, state.draft, t, theme, workspacePath]);

  const handleApply = useCallback(async () => {
    if (!state.draft || busy) {
      return;
    }

    setState((prev) => ({ ...prev, error: null }));
    try {
      const permissionDiff = await miniAppAPI.permissionDiffForDraft(app.id, state.draft.draftId);
      if (requiresPermissionConfirmation(permissionDiff)) {
        setState((prev) => ({ ...prev, stage: 'permission-review', permissionDiff }));
        return;
      }
      await applyDraft();
    } catch (error) {
      log.error('MiniApp permission diff failed', error);
      setState((prev) => ({
        ...prev,
        stage: 'preview',
        error: t('customize.permissionCheckFailed', { error: formatError(error) }),
      }));
    }
  }, [app.id, applyDraft, busy, state.draft, t]);

  const handleDiscard = useCallback(async () => {
    if (discarding) {
      return;
    }

    const draft = state.draft;
    setDiscarding(true);
    try {
      if (draft) {
        await miniAppAPI.discardDraft(app.id, draft.draftId);
      }
      setState({ ...initialState, stage: 'idle' });
      setUserRequest('');
      setPreviewKey(0);
      onClose();
    } catch (error) {
      log.error('MiniApp draft discard failed', error);
      setState((prev) => ({
        ...prev,
        error: t('customize.discardFailed', { error: formatError(error) }),
      }));
    } finally {
      setDiscarding(false);
    }
  }, [app.id, discarding, onClose, state.draft, t]);

  const assistantStatus = useMemo(() => {
    if (!state.assistantSessionId) {
      return null;
    }
    return t('customize.assistantOpened');
  }, [state.assistantSessionId, t]);

  if (!open) {
    return null;
  }

  return (
    <aside className="miniapp-customize-panel" aria-label={t('customize.title')}>
      <div className="miniapp-customize-panel__header">
        <div>
          <h3>{t('customize.title')}</h3>
          <span>{appName}</span>
        </div>
        <IconButton
          variant="ghost"
          size="small"
          onClick={() => void handleDiscard()}
          disabled={busy}
          tooltip={t('customize.close')}
          aria-label={t('customize.close')}
        >
          <X size={14} />
        </IconButton>
      </div>

      <div className="miniapp-customize-panel__notice">
        <AlertTriangle size={18} />
        <div>
          <strong>{t('customize.riskTitle')}</strong>
          <p>{t('customize.riskBody')}</p>
        </div>
      </div>

      <label className="miniapp-customize-panel__request">
        <span>{t('customize.requestLabel')}</span>
        <textarea
          value={userRequest}
          onChange={(event) => setUserRequest(event.target.value)}
          placeholder={t('customize.requestPlaceholder')}
          disabled={busy}
          rows={4}
        />
      </label>

      <div className="miniapp-customize-panel__actions">
        <Button
          variant="primary"
          size="small"
          onClick={() => void handleStart()}
          disabled={!trimmedRequest || busy}
          isLoading={state.stage === 'drafting'}
        >
          <Send size={14} />
          {state.draft ? t('customize.retryAssistant') : t('customize.start')}
        </Button>
        {state.draft && (
          <Button
            variant="secondary"
            size="small"
            onClick={() => void handleRefreshPreview()}
            disabled={busy}
            isLoading={refreshing}
          >
            <RefreshCw size={14} />
            {t('customize.refreshPreview')}
          </Button>
        )}
      </div>

      {state.error && (
        <div className="miniapp-customize-panel__error" role="alert">
          {state.error}
        </div>
      )}

      {assistantStatus && (
        <div className="miniapp-customize-panel__status">
          <Check size={14} />
          <span>{assistantStatus}</span>
        </div>
      )}

      {hasPreview && state.draft && (
        <div className="miniapp-customize-panel__preview">
          <div className="miniapp-customize-panel__preview-header">
            <span>{t('customize.previewTitle')}</span>
            <span>{t('customize.previewHint')}</span>
          </div>
          <MiniAppDraftPreview draft={state.draft} previewKey={previewKey} />
        </div>
      )}

      <div className="miniapp-customize-panel__footer">
        <Button
          variant="secondary"
          size="small"
          onClick={() => void handleDiscard()}
          disabled={busy}
          isLoading={discarding}
        >
          <Trash2 size={14} />
          {t('customize.discard')}
        </Button>
        <Button
          variant="success"
          size="small"
          onClick={() => void handleApply()}
          disabled={!hasPreview || busy}
          isLoading={state.stage === 'applying'}
        >
          {t('customize.apply')}
        </Button>
      </div>

      {state.stage === 'applying' && (
        <div className="miniapp-customize-panel__busy">
          <Loader2 size={16} className="miniapp-scene__spinning" />
          <span>{t('customize.applying')}</span>
        </div>
      )}

      <MiniAppPermissionDiffDialog
        isOpen={state.stage === 'permission-review'}
        diff={state.permissionDiff}
        applying={state.stage === 'applying'}
        onCancel={() => setState((prev) => ({ ...prev, stage: 'preview' }))}
        onConfirm={() => void applyDraft()}
      />
    </aside>
  );
};

export default MiniAppCustomizePanel;
