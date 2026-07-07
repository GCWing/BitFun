import React, { useCallback, useMemo } from 'react';
import { AlertTriangle, Paintbrush, PanelRightOpen } from 'lucide-react';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { getToolCardConfig } from './toolCardMetadata';
import { flowChatStore } from '../store/FlowChatStore';
import { createTab } from '@/shared/utils/tabUtils';
import { createLogger } from '@/shared/utils/logger';
import './CanvasToolCard.scss';

const log = createLogger('CanvasToolCard');

interface CanvasToolResult {
  action?: string;
  artifactReference?: string;
  compiled?: boolean;
  diagnosticCount?: number;
  compiledPayload?: {
    contentHash?: string;
    sourceRevision?: string;
  } | null;
  canvas?: {
    artifact?: {
      title?: string;
      status?: string;
      sourceRevision?: string;
      lastKnownGoodRevision?: string;
    };
    status?: string;
    diagnostics?: Array<{ message?: string; code?: string; severity?: string }>;
    source?: {
      source?: string;
      filename?: string;
      revision?: string;
    };
  };
}

function parseCanvasResult(raw: unknown): CanvasToolResult | null {
  if (!raw) return null;
  if (typeof raw === 'string') {
    try {
      return JSON.parse(raw) as CanvasToolResult;
    } catch {
      return null;
    }
  }
  if (typeof raw === 'object') {
    return raw as CanvasToolResult;
  }
  return null;
}

function canvasTitle(result: CanvasToolResult | null, fallback: unknown): string {
  const fromResult = result?.canvas?.artifact?.title;
  if (typeof fromResult === 'string' && fromResult.trim()) {
    return fromResult.trim();
  }
  if (fallback && typeof fallback === 'object') {
    const fromInput = (fallback as Record<string, unknown>).title;
    if (typeof fromInput === 'string' && fromInput.trim()) {
      return fromInput.trim();
    }
  }
  return 'BitFun Canvas';
}

export const CanvasToolCard: React.FC<ToolCardProps> = ({ toolItem, sessionId }) => {
  const { status, toolCall, toolResult } = toolItem;
  const toolDisplayName = getToolCardConfig(toolItem.toolName).displayName;
  const resultData = useMemo(() => parseCanvasResult(toolResult?.result), [toolResult?.result]);
  const title = useMemo(() => canvasTitle(resultData, toolCall?.input), [resultData, toolCall?.input]);
  const diagnostics = useMemo(
    () => resultData?.canvas?.diagnostics || [],
    [resultData?.canvas?.diagnostics],
  );
  const artifactReference = resultData?.artifactReference;
  const session = sessionId ? flowChatStore.getState().sessions.get(sessionId) : null;
  const source = resultData?.canvas?.source?.source;
  const canvasStatus = resultData?.canvas?.status || resultData?.canvas?.artifact?.status;
  const isLoading =
    status === 'preparing' || status === 'streaming' || status === 'running' || status === 'pending';
  const isFailed = status === 'error' || toolResult?.success === false;
  const isOpenable = status === 'completed' && Boolean(artifactReference);

  const handleOpenPanel = useCallback(() => {
    if (!isOpenable) return;

    const duplicateCheckKey = `bitfun-canvas-${artifactReference}`;
    log.info('Opening Canvas panel', {
      artifactReference,
      title,
      canvasStatus,
      compiled: resultData?.compiled,
      diagnosticCount: resultData?.diagnosticCount ?? diagnostics.length,
      hasInlineSource: Boolean(source),
      inlineSourceLength: source?.length ?? 0,
      inlineSourceRevision: resultData?.canvas?.source?.revision,
      inlineCompiledRevision: resultData?.compiledPayload?.sourceRevision,
      inlineCompiledHash: resultData?.compiledPayload?.contentHash,
      workspacePath: session?.workspacePath,
      remoteConnectionId: session?.remoteConnectionId,
      remoteSshHost: session?.remoteSshHost,
    });

    createTab({
      type: 'bitfun-canvas',
      title,
      data: {
        artifactReference,
        source,
        status: canvasStatus,
        diagnostics,
        workspacePath: session?.workspacePath,
        remoteConnectionId: session?.remoteConnectionId,
        remoteSshHost: session?.remoteSshHost,
        _source: {
          type: 'tool-call',
          toolName: toolItem.toolName,
          sessionId,
          toolCallId: toolCall?.id,
          toolItemId: toolItem.id,
        },
      },
      metadata: {
        duplicateCheckKey,
        fromTool: true,
        toolName: toolItem.toolName,
        artifactReference,
      },
      checkDuplicate: true,
      duplicateCheckKey,
      replaceExisting: true,
      mode: 'agent',
    });
  }, [
    artifactReference,
    canvasStatus,
    diagnostics,
    isOpenable,
    resultData?.canvas?.source?.revision,
    resultData?.compiled,
    resultData?.compiledPayload?.contentHash,
    resultData?.compiledPayload?.sourceRevision,
    resultData?.diagnosticCount,
    session?.remoteConnectionId,
    session?.remoteSshHost,
    session?.workspacePath,
    sessionId,
    source,
    title,
    toolCall?.id,
    toolItem.id,
    toolItem.toolName,
  ]);

  const header = (
    <ToolCardHeader
      icon={<Paintbrush size={16} />}
      iconClassName="canvas-tool-card__icon"
      action={toolDisplayName}
      content={<span className="canvas-tool-card__title">{title}</span>}
      extra={(
        <div className="canvas-tool-card__extra">
          {diagnostics.length > 0 && (
            <span className="canvas-tool-card__diagnostics">
              <AlertTriangle size={13} />
              {diagnostics.length}
            </span>
          )}
          <span className="canvas-tool-card__status">
            {isLoading ? 'Rendering' : resultData?.compiled ? 'Preview ready' : canvasStatus || 'Saved'}
          </span>
          {isOpenable && <PanelRightOpen size={14} className="canvas-tool-card__open-icon" />}
        </div>
      )}
      statusIcon={null}
    />
  );

  const body = (
    <div className="canvas-tool-card__body">
      <div className="canvas-tool-card__meta">
        <span>{artifactReference || 'Waiting for artifact reference'}</span>
      </div>
      {diagnostics.length > 0 && (
        <ul className="canvas-tool-card__diagnostic-list">
          {diagnostics.slice(0, 3).map((diagnostic, index) => (
            <li key={`${diagnostic.code || diagnostic.message || 'diagnostic'}-${index}`}>
              {diagnostic.message || diagnostic.code || 'Canvas diagnostic'}
            </li>
          ))}
        </ul>
      )}
    </div>
  );

  return (
    <BaseToolCard
      status={status}
      isExpanded={!isOpenable || diagnostics.length > 0 || isFailed}
      onClick={isOpenable ? handleOpenPanel : undefined}
      className={`canvas-tool-card ${isOpenable ? 'clickable' : ''}`.trim()}
      header={header}
      expandedContent={body}
      errorContent={isFailed ? body : undefined}
      isFailed={isFailed}
      headerExpandAffordance={isOpenable}
      headerAffordanceKind="open-panel-right"
    />
  );
};

export default CanvasToolCard;
