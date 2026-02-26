/**
 * Display component for MCP tools.
 * Supports MCP Apps: when tool result contains ui:// resource, renders interactive UI in sandboxed iframe.
 */

import React, { useState, useCallback, useEffect, useLayoutEffect, useRef } from 'react';
import { ChevronDown, ChevronUp, Package, CheckCircle, Check, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { CubeLoading, IconButton } from '../../component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard, ToolCardHeader } from './BaseToolCard';
import { createLogger } from '@/shared/utils/logger';
import { MCPAPI, MCP_APPS_PROTOCOL_VERSION, type McpUiResourceCsp, type McpUiResourcePermissions, type McpUiMessageParams, type McpUiMessageResult, type McpAppMessageEvent, type McpAppMessageResponseEvent } from '@/infrastructure/api/service-api/MCPAPI';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { globalEventBus } from '@/infrastructure/event-bus';
import './MCPToolDisplay.scss';

const log = createLogger('MCPToolDisplay');

/** JSON-RPC 2.0 request/response from MCP App iframe (postMessage). */
interface MCPAppJSONRPC {
  jsonrpc?: string;
  id?: number | string;
  method?: string;
  params?: Record<string, unknown>;
  result?: unknown;
  error?: { code: number; message: string };
}

/** UI Resource metadata for security and rendering configuration. */
interface McpUiResourceMeta {
  /** Content Security Policy configuration. */
  csp?: McpUiResourceCsp;
  /** Sandbox permissions requested by the UI. */
  permissions?: McpUiResourcePermissions;
}

interface MCPToolResultContent {
  type: 'text' | 'image' | 'resource';
  text?: string;
  data?: string;
  mime_type?: string;
  resource?: {
    uri: string;
    name?: string;
    description?: string;
    mime_type?: string;
    content?: string;
  };
}

interface MCPToolResult {
  content?: MCPToolResultContent[];
  is_error?: boolean;
}

/** Clean and escape domains for CSP injection (prevent HTML injection). */
const cleanDomains = (domains: string[] | undefined): string => {
  return (domains?.join(' ') || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
};

/** Inject CSP meta tag and scroll boundary handling script into HTML (aligned with VSCode). */
const injectPreamble = (html: string, csp?: McpUiResourceCsp): string => {
  const cspContent = `
    default-src 'none';
    script-src 'self' 'unsafe-inline' 'unsafe-eval' ${cleanDomains(csp?.resourceDomains)};
    style-src 'self' 'unsafe-inline' ${cleanDomains(csp?.resourceDomains)};
    connect-src 'self' ${cleanDomains(csp?.connectDomains)};
    img-src 'self' data: ${cleanDomains(csp?.resourceDomains)};
    font-src 'self' ${cleanDomains(csp?.resourceDomains)};
    media-src 'self' data: ${cleanDomains(csp?.resourceDomains)};
    frame-src ${cleanDomains(csp?.frameDomains) || `'none'`};
    object-src 'none';
    base-uri ${cleanDomains(csp?.baseUriDomains) || `'self'`};
  `;
  const cspTag = `<meta http-equiv="Content-Security-Policy" content="${cspContent}">`;

  // Scroll boundary detection: bubble wheel events to parent when at scroll boundaries
  const scrollBoundaryScript = `
    <script>(() => {
      const shouldBubbleScroll = (event) => {
        for (let node = event.target; node; node = node.parentNode) {
          if (!(node instanceof Element)) continue;
          if (node === document.documentElement || node === document.body) continue;
          const overflow = window.getComputedStyle(node).overflowY;
          if (overflow === 'hidden' || overflow === 'visible') continue;
          if (event.deltaY < 0 && node.scrollTop > 0) return false;
          if (event.deltaY > 0 && node.scrollTop + node.clientHeight < node.scrollHeight) {
            if (node.scrollHeight - node.scrollTop - node.clientHeight < 2) continue;
            return false;
          }
        }
        const docEl = document.documentElement;
        const scrollTop = window.scrollY || docEl.scrollTop || document.body.scrollTop || 0;
        const scrollHeight = Math.max(docEl.scrollHeight, document.body.scrollHeight);
        const clientHeight = docEl.clientHeight;
        const scrollableDistance = scrollHeight - clientHeight;
        if (scrollableDistance > 2) {
          if (event.deltaY < 0 && scrollTop > 0) return false;
          if (event.deltaY > 0 && scrollTop < scrollableDistance - 2) return false;
        }
        return true;
      };
      window.addEventListener('wheel', (event) => {
        if (event.defaultPrevented || !shouldBubbleScroll(event)) return;
        window.parent.postMessage({
          jsonrpc: '2.0',
          method: 'ui/notifications/sandbox-wheel',
          params: {
            deltaMode: event.deltaMode,
            deltaX: event.deltaX,
            deltaY: event.deltaY,
            deltaZ: event.deltaZ,
          }
        }, '*');
      }, { passive: true });
    })();</script>
  `;

  const content = cspTag + '\n' + scrollBoundaryScript;

  // Try to inject into <head>
  const headMatch = html.match(/<head[^>]*>/i);
  if (headMatch) {
    const insertIndex = headMatch.index! + headMatch[0].length;
    return html.slice(0, insertIndex) + '\n' + content + html.slice(insertIndex);
  }

  // If no <head>, try to inject after <html>
  const htmlMatch = html.match(/<html[^>]*>/i);
  if (htmlMatch) {
    const insertIndex = htmlMatch.index! + htmlMatch[0].length;
    return html.slice(0, insertIndex) + '\n<head>' + content + '</head>' + html.slice(insertIndex);
  }

  // If no <html>, wrap with proper structure
  return `<!DOCTYPE html><html><head>${content}</head><body>${html}</body></html>`;
};

export const MCPToolDisplay: React.FC<ToolCardProps> = ({
  toolItem,
  config,
  onConfirm,
  onReject
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult, requiresConfirmation, userConfirmed } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);

  const getResultData = (): MCPToolResult | null => {
    if (!toolResult?.result) return null;
    
    try {
      if (typeof toolResult.result === 'string') {
        return JSON.parse(toolResult.result);
      }
      return toolResult.result as MCPToolResult;
    } catch (e) {
      log.error('Failed to parse MCP tool result', e);
      return null;
    }
  };

  const resultData = getResultData();

  const getToolInfo = () => {
    const fullToolName = config.toolName;
    const parts = fullToolName.split('_');
    const actualToolName = parts.slice(2).join('_') || fullToolName;
    const serverName = parts[1] || 'unknown';
    
    return { toolName: actualToolName, serverName };
  };

  const { toolName, serverName } = getToolInfo();
  const serverId = serverName;
  const isFailed = status === 'error';

  const mcpAppIframeRef = useRef<HTMLIFrameElement | null>(null);
  const [mcpAppHeight, setMcpAppHeight] = useState<number | undefined>(undefined);
  const bridgeDataRef = useRef({ config, toolCall, resultData, status, isFailed });
  bridgeDataRef.current = { config, toolCall, resultData, status, isFailed };

  // MCP Apps: ui:// resource to fetch and render in iframe
  const [mcpAppState, setMcpAppState] = useState<{
    uri: string;
    html: string | null;
    rawHtml: string | null;
    meta: McpUiResourceMeta | null;
    loading: boolean;
    error: string | null;
  } | null>(null);

  // Latest CSP for hostCapabilities (updated when MCP App loads)
  const latestCspRef = useRef<McpUiResourceCsp | undefined>(undefined);

  // Find first ui:// resource in result for MCP App rendering
  // Fallback: MCP Apps declare UI in tool metadata (_meta.ui.resourceUri), not in result
  const uiResourceUriFromResult = resultData?.content
    ?.find((item): item is MCPToolResultContent & { resource: { uri: string } } =>
      item.type === 'resource' && !!(item.resource?.uri?.startsWith('ui://'))
    )
    ?.resource?.uri;

  const [toolMetaUiUri, setToolMetaUiUri] = useState<string | null>(null);

  useEffect(() => {
    if (
      uiResourceUriFromResult ||
      !config.toolName.startsWith('mcp_') ||
      status !== 'completed' ||
      isFailed
    ) {
      setToolMetaUiUri(null);
      return;
    }
    MCPAPI.getMCPToolUiUri(config.toolName)
      .then((uri) => setToolMetaUiUri(uri))
      .catch(() => setToolMetaUiUri(null));
  }, [config.toolName, uiResourceUriFromResult, status, isFailed]);

  // Auto-expand when MCP App UI is ready so user sees the interactive UI immediately
  useEffect(() => {
    if (mcpAppState?.html && !isExpanded) {
      setIsExpanded(true);
    }
  }, [mcpAppState?.html, isExpanded]);

  // Iframe <-> parent postMessage bridge (MCP App protocol). Register in useLayoutEffect so listener is attached before iframe script runs.
  useLayoutEffect(() => {
    if (!mcpAppState?.html || !serverId) return;

    const postToIframe = (payload: Record<string, unknown>) => {
      const win = mcpAppIframeRef.current?.contentWindow;
      try {
        if (win) win.postMessage(payload, '*');
        else log.warn('MCP App postMessage skipped: iframe ref or contentWindow missing');
      } catch (e) {
        log.error('MCP App postMessage to iframe failed', e);
      }
    };

    const handleMessage = async (event: MessageEvent) => {
      const iframeWin = mcpAppIframeRef.current?.contentWindow;
      if (!iframeWin || event.source !== iframeWin) return;
      const data = event.data as MCPAppJSONRPC;
      if (!data || data.jsonrpc !== '2.0' || !data.method) return;

      const id = data.id;
      const method = data.method;
      const params = data.params ?? {};

      try {
        switch (method) {
          case 'ui/initialize': {
            const { config: cfg, toolCall: tc, resultData: resData, status: st, isFailed: failed } = bridgeDataRef.current;
            const args = typeof tc?.input === 'string'
              ? (() => { try { return JSON.parse(tc.input); } catch { return tc.input; } })()
              : tc?.input;
            const hostContext = {
              toolInfo: {
                id: tc?.id,
                tool: {
                  name: cfg.toolName,
                  description: undefined,
                  inputSchema: { type: 'object' as const, properties: {}, additionalProperties: true }
                }
              },
              displayMode: 'inline' as const,
              containerDimensions: { maxHeight: 600 }
            };
            const initResult = {
              jsonrpc: '2.0',
              id,
              result: {
                protocolVersion: MCP_APPS_PROTOCOL_VERSION,
                hostInfo: { name: 'BitFun', version: '1.0.0' },
                hostCapabilities: {
                  openLinks: {},
                  serverTools: { listChanged: true },
                  serverResources: { listChanged: true },
                  logging: {},
                  sandbox: {
                    csp: latestCspRef.current,
                    permissions: { clipboardWrite: {} }
                  },
                  updateModelContext: {
                    audio: {},
                    image: {},
                    resourceLink: {},
                    resource: {},
                    structuredContent: {},
                    text: {}
                  },
                  message: { text: {}, image: {}, resourceLink: {}, resource: {}, structuredContent: {} }
                },
                hostContext
              }
            };
            postToIframe(initResult);
            setTimeout(() => {
              postToIframe({
                jsonrpc: '2.0',
                method: 'ui/notifications/tool-input',
                params: { arguments: args ?? {} }
              });
              if (resData && st === 'completed' && !failed) {
                const mcpOutput = resData as unknown as { content?: unknown[]; is_error?: boolean; structuredContent?: unknown };
                postToIframe({
                  jsonrpc: '2.0',
                  method: 'ui/notifications/tool-result',
                  params: mcpOutput
                });
              }
            }, 0);
            break;
          }
          case 'tools/call':
          case 'resources/read':
          case 'ping': {
            const response = await MCPAPI.sendMCPAppMessage({ serverId, ...data });
            postToIframe(response);
            break;
          }
          case 'ui/notifications/size-changed': {
            const height = params?.height as number | undefined;
            if (typeof height === 'number') setMcpAppHeight(height);
            break;
          }
          case 'ui/notifications/sandbox-wheel': {
            // Scroll boundary event from iframe - bubble to parent for smooth scrolling
            // In a web context, we can't directly control parent scroll, but we can
            // dispatch a wheel event on the iframe container for the parent to handle
            const iframe = mcpAppIframeRef.current;
            if (iframe) {
              const wheelEvent = new WheelEvent('wheel', {
                deltaX: params?.deltaX as number ?? 0,
                deltaY: -(params?.deltaY as number ?? 0),
                deltaZ: params?.deltaZ as number ?? 0,
                deltaMode: params?.deltaMode as number ?? 0,
                bubbles: true,
                cancelable: true,
              });
              iframe.dispatchEvent(wheelEvent);
            }
            break;
          }
          case 'ui/notifications/initialized':
            break;
          case 'ui/open-link': {
            // MCP App requests to open a URL in external browser
            const url = params?.url as string | undefined;
            if (typeof url === 'string' && url) {
              try {
                await systemAPI.openExternal(url);
                if (id !== undefined) {
                  postToIframe({ jsonrpc: '2.0', id, result: {} });
                }
              } catch (err) {
                log.error('Failed to open external link from MCP App', { url, err });
                if (id !== undefined) {
                  postToIframe({
                    jsonrpc: '2.0',
                    id,
                    error: { code: -32000, message: err instanceof Error ? err.message : String(err) }
                  });
                }
              }
            } else {
              if (id !== undefined) {
                postToIframe({
                  jsonrpc: '2.0',
                  id,
                  error: { code: -32602, message: 'Missing or invalid url parameter' }
                });
              }
            }
            break;
          }
          case 'ui/message': {
            // MCP App requests to send a message to the conversation
            // Use request-response pattern via globalEventBus
            const messageParams = params as unknown as McpUiMessageParams;
            log.info('MCP App ui/message received', { params: messageParams });

            const requestId = `mcp-msg-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

            // Set up response listener before emitting
            const responsePromise = new Promise<McpUiMessageResult>((resolve) => {
              const timeout = setTimeout(() => {
                globalEventBus.off('mcp-app:message-response', handleResponse);
                resolve({ isError: true });
              }, 5000); // 5 second timeout

              const handleResponse = (response: McpAppMessageResponseEvent) => {
                if (response.requestId === requestId) {
                  clearTimeout(timeout);
                  globalEventBus.off('mcp-app:message-response', handleResponse);
                  resolve(response.result);
                }
              };

              globalEventBus.on('mcp-app:message-response', handleResponse);
            });

            // Emit event for ChatInput to handle
            const eventPayload: McpAppMessageEvent = {
              requestId,
              params: messageParams
            };
            globalEventBus.emit('mcp-app:message', eventPayload);

            // Wait for response and return to iframe
            try {
              const result = await responsePromise;
              if (id !== undefined) {
                postToIframe({
                  jsonrpc: '2.0',
                  id,
                  result
                });
              }
            } catch (err) {
              log.error('Failed to handle MCP App ui/message', { err });
              if (id !== undefined) {
                postToIframe({
                  jsonrpc: '2.0',
                  id,
                  result: { isError: true }
                });
              }
            }
            break;
          }
          default:
            if (id !== undefined) {
              postToIframe({
                jsonrpc: '2.0',
                id,
                error: { code: -32601, message: `Method not found: ${method}` }
              });
            }
        }
      } catch (err) {
        log.error('MCP App message handling failed', { method, err });
        if (id !== undefined) {
          postToIframe({
            jsonrpc: '2.0',
            id,
            error: { code: -32000, message: err instanceof Error ? err.message : String(err) }
          });
        }
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, [mcpAppState?.html, serverId]);

  const handleIframeLoad = useCallback(() => {
    /* iframe loaded, ref is ready for postMessage bridge */
  }, []);

  const uiResourceUri = uiResourceUriFromResult ?? toolMetaUiUri;

  useEffect(() => {
    if (!uiResourceUri || !serverId || status !== 'completed' || isFailed) {
      setMcpAppState(null);
      return;
    }
    let cancelled = false;
    setMcpAppState({
      uri: uiResourceUri,
      html: null,
      rawHtml: null,
      meta: null,
      loading: true,
      error: null
    });
    MCPAPI.fetchMCPAppResource({ serverId, resourceUri: uiResourceUri })
      .then((res) => {
        if (cancelled) return;
        const htmlContent = res.contents.find((c) => c.uri === uiResourceUri) ?? res.contents[0];
        const rawHtml = htmlContent?.content ?? '';
        // Extract CSP and permissions from response if available
        const meta: McpUiResourceMeta = {
          csp: (htmlContent as unknown as { csp?: McpUiResourceCsp })?.csp,
          permissions: (htmlContent as unknown as { permissions?: McpUiResourcePermissions })?.permissions,
        };
        // Inject CSP preamble into HTML
        const html = injectPreamble(rawHtml, meta.csp);
        // Update latest CSP ref for hostCapabilities
        latestCspRef.current = meta.csp;
        setMcpAppState((s) => s ? { ...s, html, rawHtml, meta, loading: false, error: html ? null : 'No content' } : null);
      })
      .catch((err) => {
        if (cancelled) return;
        log.error('Failed to fetch MCP App resource', { uiResourceUri, serverId, err });
        setMcpAppState((s) =>
          s ? { ...s, loading: false, error: err?.message ?? String(err) } : null
        );
      });
    return () => { cancelled = true; };
  }, [uiResourceUri, serverId, status, isFailed]);

  const getContentSummary = () => {
    if (!resultData?.content && !uiResourceUri) return null;

    const hasUiAppFromResult = resultData?.content?.some(
      (item) => item.type === 'resource' && item.resource?.uri?.startsWith('ui://')
    ) ?? false;
    const hasUiApp = hasUiAppFromResult || !!uiResourceUri;
    const counts = {
      text: 0,
      image: 0,
      resource: 0
    };

    resultData?.content?.forEach((item) => {
      if (item.type in counts) {
        counts[item.type as keyof typeof counts]++;
      }
    });

    const parts = [];
    if (hasUiApp) parts.push(t('toolCards.mcp.interactiveApp', 'interactive app'));
    if (counts.text > 0) parts.push(`${counts.text} text`);
    if (counts.image > 0) parts.push(`${counts.image} images`);
    if (counts.resource > 0 && !hasUiApp) parts.push(`${counts.resource} resources`);

    return parts.length > 0 ? parts.join(' · ') : null;
  };

  const contentSummary = getContentSummary();
  const hasContent =
    status === 'completed' &&
    (!!uiResourceUri || (resultData?.content && resultData.content.length > 0));
  const isLoading = status === 'preparing' || status === 'streaming' || status === 'running';

  const getErrorMessage = () => {
    if (toolResult && 'error' in toolResult) {
      return toolResult.error;
    }
    return 'MCP tool execution failed';
  };

  const handleCardClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.preview-toggle-btn')) {
      return;
    }
    
    if (isFailed) {
      return;
    }
    
    if (hasContent) {
      setIsExpanded(!isExpanded);
    }
  }, [isFailed, isExpanded, hasContent]);

  const renderToolIcon = () => {
    return <Package size={16} />;
  };

  const renderStatusIcon = () => {
    if (isLoading) {
      return <CubeLoading size="small" />;
    }
    if (status === 'completed' && !isFailed) {
      return <CheckCircle className="icon-completed" size={14} />;
    }
    return null;
  };

  const renderHeader = () => (
    <ToolCardHeader
      icon={renderToolIcon()}
      iconClassName="mcp-icon"
      action={isFailed ? 'MCP tool failed' : 'MCP tool:'}
      content={
        <span className="mcp-tool-info">
          <span className="tool-name">{toolName}</span>
          <span className="server-tag">from {serverName}</span>
        </span>
      }
      extra={
        <>
          {!isFailed && contentSummary && status === 'completed' && (
            <span className="content-summary">
              {contentSummary}
            </span>
          )}
          
          {requiresConfirmation && !userConfirmed && status !== 'completed' && (
            <div className="mcp-action-buttons">
              <IconButton
                className="mcp-icon-button mcp-confirm-btn"
                variant="success"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation();
                  onConfirm?.(toolCall?.input);
                }}
                disabled={status === 'streaming'}
                tooltip={t('toolCards.mcp.confirmExecute')}
              >
                <Check size={14} />
              </IconButton>
              <IconButton
                className="mcp-icon-button mcp-reject-btn"
                variant="danger"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation();
                  onReject?.();
                }}
                disabled={status === 'streaming'}
                tooltip={t('toolCards.mcp.cancel')}
              >
                <X size={14} />
              </IconButton>
            </div>
          )}
          
          {!isFailed && hasContent && (
            <IconButton
              className="preview-toggle-btn"
              variant="ghost"
              size="xs"
              onClick={(e) => {
                e.stopPropagation();
                setIsExpanded(!isExpanded);
              }}
              tooltip={isExpanded ? t('toolCards.common.collapseContent') : t('toolCards.common.expandContent')}
            >
              {isExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </IconButton>
          )}
          
          {isFailed && (
            <div className="error-expand-indicator">
              <span className="error-text">Failed</span>
            </div>
          )}
        </>
      }
      statusIcon={renderStatusIcon()}
    />
  );

  const renderExpandedContent = () => {
    const hasResultContent = resultData?.content && resultData.content.length > 0;
    const hasMcpApp = mcpAppState?.html;
    if (!hasResultContent && !hasMcpApp) {
      return null;
    }

    return (
      <div className="mcp-expanded-content">
        {/* MCP App: sandboxed iframe for ui:// resources */}
        {mcpAppState && (
          <div className="content-item content-item-mcp-app">
            {mcpAppState.loading && (
              <div className="mcp-app-loading">
                <CubeLoading size="small" />
                <span>{t('toolCards.mcp.loadingApp', 'Loading interactive app...')}</span>
              </div>
            )}
            {mcpAppState.error && (
              <div className="mcp-app-error">
                <span>{t('toolCards.mcp.appLoadError', 'Failed to load app')}: {mcpAppState.error}</span>
              </div>
            )}
            {mcpAppState.html && !mcpAppState.loading && (
              <iframe
                ref={mcpAppIframeRef}
                className="mcp-app-iframe"
                sandbox="allow-scripts allow-forms"
                title="MCP App"
                srcDoc={mcpAppState.html}
                style={mcpAppHeight !== undefined ? { minHeight: mcpAppHeight } : undefined}
                onLoad={handleIframeLoad}
              />
            )}
          </div>
        )}
        {/* Text, image, and non-ui resources */}
        {(resultData?.content ?? []).map((item, index) => {
          const isUiResource = item.type === 'resource' && item.resource?.uri?.startsWith('ui://');
          if (isUiResource && mcpAppState) return null;
          return (
            <div key={index} className={`content-item content-item-${item.type}`}>
              {item.type === 'text' && (
                <div className="text-content">
                  <pre>{item.text}</pre>
                </div>
              )}
              {item.type === 'image' && item.data && (
                <div className="image-content">
                  <img src={`data:${item.mime_type ?? 'image/png'};base64,${item.data}`} alt="" />
                </div>
              )}
              {item.type === 'resource' && item.resource && (
                <div className="resource-content">
                  <div className="resource-name">{item.resource.name || 'Resource'}</div>
                  <div className="resource-uri">{item.resource.uri}</div>
                  {item.resource.description && (
                    <div className="resource-description">{item.resource.description}</div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    );
  };

  const renderErrorContent = () => (
    <div className="error-content">
      <div className="error-message">{getErrorMessage()}</div>
      <div className="error-meta">
        <span className="error-tool">Tool: {toolName}</span>
        <span className="error-separator">|</span>
        <span className="error-server">Server: {serverName}</span>
      </div>
    </div>
  );

  return (
    <BaseToolCard
      status={status}
      isExpanded={isExpanded}
      onClick={handleCardClick}
      className="mcp-tool-display"
      header={renderHeader()}
      expandedContent={renderExpandedContent()}
      errorContent={renderErrorContent()}
      isFailed={isFailed}
      requiresConfirmation={requiresConfirmation && !userConfirmed}
    />
  );
};
