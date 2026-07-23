import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, Modal } from '@/component-library';
import { globalEventBus } from '@/infrastructure/event-bus';
import { MCPAPI } from '@/infrastructure/api/service-api/MCPAPI';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './MCPInteractionDialog.scss';

const log = createLogger('MCPInteractionDialog');

interface MCPInteractionRequestEvent {
  interactionId: string;
  serverId: string;
  serverName: string;
  method: string;
  params?: unknown;
}

function buildDefaultResult(method: string): string {
  if (method === 'sampling/createMessage') {
    return '{\n  "role": "assistant",\n  "content": [\n    {\n      "type": "text",\n      "text": ""\n    }\n  ]\n}';
  }
  return '{\n  "action": "accept"\n}';
}

function stringifyParams(params: unknown): string {
  try {
    return JSON.stringify(params ?? null, null, 2);
  } catch {
    return String(params ?? '');
  }
}

export const MCPInteractionDialog: React.FC = () => {
  const [queue, setQueue] = useState<MCPInteractionRequestEvent[]>([]);
  const [editorValue, setEditorValue] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const currentRequest = queue[0] ?? null;
  const isOpen = !!currentRequest;
  const queueCount = queue.length;

  useEffect(() => {
    const handleRequest = (event: MCPInteractionRequestEvent) => {
      if (!event?.interactionId || !event?.method) {
        log.warn('Ignoring invalid MCP interaction event payload', { event });
        return;
      }

      setQueue((prev) => {
        if (prev.some((item) => item.interactionId === event.interactionId)) {
          return prev;
        }
        return [...prev, event];
      });
    };

    globalEventBus.on('mcp:interaction:request', handleRequest);
    return () => {
      globalEventBus.off('mcp:interaction:request', handleRequest);
    };
  }, []);

  useEffect(() => {
    if (!currentRequest) {
      setEditorValue('');
      return;
    }
    setEditorValue(buildDefaultResult(currentRequest.method));
  }, [currentRequest]);

  const popCurrentRequest = useCallback(() => {
    setQueue((prev) => prev.slice(1));
  }, []);

  const handleReject = useCallback(async () => {
    if (!currentRequest || isSubmitting) return;
    setIsSubmitting(true);
    try {
      await MCPAPI.submitMCPInteractionResponse({
        interactionId: currentRequest.interactionId,
        approve: false,
        error: {
          message: 'User rejected MCP interaction request',
        },
      });
      popCurrentRequest();
    } catch (error) {
      log.error('Failed to submit MCP interaction rejection', { error, currentRequest });
      notificationService.error(`Failed to reject MCP request: ${currentRequest.method}`);
    } finally {
      setIsSubmitting(false);
    }
  }, [currentRequest, isSubmitting, popCurrentRequest]);

  const handleApprove = useCallback(async () => {
    if (!currentRequest || isSubmitting) return;

    const trimmed = editorValue.trim();
    let parsedResult: unknown = {};
    if (trimmed.length > 0) {
      try {
        parsedResult = JSON.parse(trimmed);
      } catch {
        notificationService.error('Result must be valid JSON');
        return;
      }
    }

    setIsSubmitting(true);
    try {
      await MCPAPI.submitMCPInteractionResponse({
        interactionId: currentRequest.interactionId,
        approve: true,
        result: parsedResult as any,
      });
      popCurrentRequest();
    } catch (error) {
      log.error('Failed to submit MCP interaction approval', { error, currentRequest });
      notificationService.error(`Failed to approve MCP request: ${currentRequest.method}`);
    } finally {
      setIsSubmitting(false);
    }
  }, [currentRequest, editorValue, isSubmitting, popCurrentRequest]);

  const paramsPreview = useMemo(() => {
    if (!currentRequest) return '';
    return stringifyParams(currentRequest.params);
  }, [currentRequest]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {}}
      title={currentRequest ? `MCP Interaction: ${currentRequest.method}` : 'MCP Interaction'}
      size="large"
      showCloseButton={false}
    >
      {currentRequest && (
        <div className="mcp-interaction-dialog">
          <div className="mcp-interaction-dialog__meta">
            <span className="mcp-interaction-dialog__server">
              Server: {currentRequest.serverName || currentRequest.serverId}
            </span>
            {queueCount > 1 && (
              <span className="mcp-interaction-dialog__queue">Queue: {queueCount}</span>
            )}
          </div>

          <div className="mcp-interaction-dialog__section">
            <div className="mcp-interaction-dialog__label">Request Params</div>
            <pre className="mcp-interaction-dialog__params">{paramsPreview}</pre>
          </div>

          <div className="mcp-interaction-dialog__section">
            <div className="mcp-interaction-dialog__label">Response JSON</div>
            <textarea
              className="mcp-interaction-dialog__editor"
              value={editorValue}
              onChange={(e) => setEditorValue(e.target.value)}
              placeholder="{}"
              spellCheck={false}
            />
          </div>

          <div className="mcp-interaction-dialog__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => void handleReject()}
              disabled={isSubmitting}
            >
              Reject
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={() => void handleApprove()}
              isLoading={isSubmitting}
              disabled={isSubmitting}
            >
              Approve
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
};

export default MCPInteractionDialog;
