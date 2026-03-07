/**
 * MiniAppToolDisplay — shows InitMiniApp result with Open in Toolbox button.
 */
import React, { useMemo } from 'react';
import { Wrench, ExternalLink, CheckCircle2, Loader2 } from 'lucide-react';
import type { ToolCardProps } from '../types/flow-chat';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import './MiniAppToolDisplay.scss';

export const InitMiniAppDisplay: React.FC<ToolCardProps> = ({ toolItem }) => {
  const { status, toolResult, partialParams, isParamsStreaming } = toolItem;
  const { openScene } = useSceneManager();

  const name = useMemo(() => {
    if (isParamsStreaming) return partialParams?.name || '';
    return (toolItem.toolCall.input as Record<string, unknown>)?.name as string | undefined || '';
  }, [isParamsStreaming, partialParams, toolItem]);

  const appId = toolResult?.result?.app_id as string | undefined;
  const path = toolResult?.result?.path as string | undefined;
  const success = toolResult?.success;
  const isLoading = status === 'running' || status === 'streaming' || status === 'preparing';

  return (
    <div className="miniapp-tool-display">
      <div className="miniapp-tool-display__header">
        <div className="miniapp-tool-display__icon">
          {isLoading ? <Loader2 size={14} className="spinning" /> : <Wrench size={14} />}
        </div>
        <span className="miniapp-tool-display__label">
          {isLoading ? 'Creating MiniApp…' : success ? 'MiniApp Skeleton Created' : 'Init MiniApp'}
        </span>
        {success && <CheckCircle2 size={14} className="miniapp-tool-display__check" />}
      </div>
      <div className="miniapp-tool-display__body">
        {name && <span className="miniapp-tool-display__app-name">{name}</span>}
        {appId && success && (
          <>
            <span className="miniapp-tool-display__meta">app_id: {appId}</span>
            {path && <span className="miniapp-tool-display__meta">path: {path}</span>}
            <button
              className="miniapp-tool-display__open-btn"
              onClick={() => openScene('toolbox')}
              title="Open in Toolbox"
            >
              <ExternalLink size={12} />
              <span>Open in Toolbox</span>
            </button>
          </>
        )}
      </div>
    </div>
  );
};
