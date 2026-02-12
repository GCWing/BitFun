 

import React, { useRef, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { FileJson, RefreshCw, X, Play, Square, CheckCircle, Clock, AlertTriangle, MinusCircle, Plug, Loader2 } from 'lucide-react';
import { MCPAPI, MCPServerInfo } from '../../api/service-api/MCPAPI';
import { Button, Textarea, Search, IconButton, Card } from '../../../component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './MCPConfig.scss';

const log = createLogger('MCPConfig');

interface MCPConfigProps {
  onClose?: () => void;
}

 
interface ErrorInfo {
  title: string;
  message: string;
  duration: number;
  suggestions?: string[];
}

function createErrorClassifier(t: (key: string, options?: any) => any) {
  const getSuggestions = (key: string): string[] | undefined => {
    const suggestions = t(key, { returnObjects: true });
    if (!Array.isArray(suggestions)) {
      return undefined;
    }
    return suggestions.map((s) => String(s));
  };

  return function classifyError(error: unknown, context: string = 'operation'): ErrorInfo {
    let errorMessage = t('errors.unknownError');
    
    if (error instanceof Error) {
      errorMessage = error.message;
    } else if (typeof error === 'string') {
      errorMessage = error;
    }
    
    const normalizedMessage = errorMessage.toLowerCase();
    const matches = (patterns: string[]) =>
      patterns.some((pattern) => normalizedMessage.includes(pattern));
    
    if (matches(['json parsing failed', 'json parse failed', 'invalid json', 'json format'])) {
      return {
        title: t('errors.jsonFormatError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.jsonFormat')
      };
    }
    
    
    if (matches(["config missing 'mcpservers' field", "'mcpservers' field must be an object"])) {
      return {
        title: t('errors.configStructureError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.configStructure')
      };
    }
    
    
    if (matches([
      "must not set both 'command' and 'url'",
      "must provide either 'command' (stdio) or 'url' (streamable-http)",
      "unsupported 'type' value",
      "'type' conflicts with provided fields",
      "(stdio) must provide 'command' field",
      "(sse) must provide 'url' field",
      "(streamable-http) must provide 'url' field",
      "'args' field must be an array",
      "'env' field must be an object",
      'config must be an object'
    ])) {
      return {
        title: t('errors.serverConfigError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.serverConfig')
      };
    }
    
    
    if (matches(['permission denied', 'access is denied'])) {
      return {
        title: t('errors.permissionError'),
        message: errorMessage,
        duration: 15000,
        suggestions: getSuggestions('errors.suggestions.permission')
      };
    }
    
    
    if (matches([
      'failed to write config file',
      'failed to serialize config',
      'failed to save config',
      'io error',
      'write failed'
    ])) {
      return {
        title: t('errors.fileOperationError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.fileOperation')
      };
    }
    
    
    if (matches(['not found'])) {
      return {
        title: t('errors.resourceNotFound'),
        message: errorMessage,
        duration: 8000
      };
    }
    
    
    if (matches([
      'failed to start mcp server',
      'failed to capture stdin',
      'failed to capture stdout',
      'max restart attempts',
      'process error'
    ])) {
      return {
        title: t('errors.serverStartError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.serverStart')
      };
    }
    
    
    return {
      title: t('errors.operationFailed', { context }),
      message: errorMessage,
      duration: 8000,
      suggestions: getSuggestions('errors.suggestions.default')
    };
  };
}

export const MCPConfig: React.FC<MCPConfigProps> = () => {
  const { t } = useTranslation('settings/mcp');
  const classifyError = createErrorClassifier(t);
  const jsonEditorRef = useRef<HTMLTextAreaElement>(null);
  const jsonLintSeqRef = useRef(0);
  const [servers, setServers] = useState<MCPServerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchKeyword, setSearchKeyword] = useState('');
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  const [jsonConfig, setJsonConfig] = useState('');
  const [serverAction, setServerAction] = useState<Record<string, 'start' | 'stop' | 'restart'>>({});
  const [jsonLintError, setJsonLintError] = useState<{
    message: string;
    line?: number;
    column?: number;
    position?: number;
  } | null>(null);
  const notification = useNotification();

  const tryFormatJson = (input: string): string | null => {
    try {
      const parsed = JSON.parse(input);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return null;
    }
  };

  useEffect(() => {
    loadServers();
    loadJsonConfig();
  }, []);

  useEffect(() => {
    if (!showJsonEditor) {
      setJsonLintError(null);
      return;
    }

    const seq = ++jsonLintSeqRef.current;
    const handle = window.setTimeout(() => {
      if (seq !== jsonLintSeqRef.current) {
        return;
      }
      if (!jsonConfig.trim()) {
        setJsonLintError(null);
        return;
      }

      try {
        JSON.parse(jsonConfig);
        setJsonLintError(null);
      } catch (error) {
        if (seq !== jsonLintSeqRef.current) {
          return;
        }
        const rawMessage = error instanceof Error ? error.message : String(error);
        const message = rawMessage.replace(/\s+at position \d+$/, '');

        const posMatch =
          rawMessage.match(/position\s+(\d+)/i) ??
          rawMessage.match(/at position\s+(\d+)/i) ??
          rawMessage.match(/char(?:acter)?\s+(\d+)/i);
        const position = posMatch ? Number(posMatch[1]) : undefined;

        if (typeof position === 'number' && Number.isFinite(position)) {
          const prefix = jsonConfig.slice(0, Math.max(0, position));
          const lines = prefix.split('\n');
          const line = lines.length;
          const column = (lines[lines.length - 1]?.length ?? 0) + 1;
          setJsonLintError({ message, line, column, position });
        } else {
          setJsonLintError({ message });
        }
      }
    }, 150);

    return () => window.clearTimeout(handle);
  }, [jsonConfig, showJsonEditor]);

  const loadServers = async () => {
    try {
      setLoading(true);
      const serverList = await MCPAPI.getServers();
      setServers(serverList);
    } catch (error) {
      log.error('Failed to load MCP servers', error);
    } finally {
      setLoading(false);
    }
  };

  const loadJsonConfig = async () => {
    try {
      const config = await MCPAPI.loadMCPJsonConfig();
      setJsonConfig(config);
    } catch (error) {
      log.error('Failed to load MCP JSON config', error);
      
      setJsonConfig(JSON.stringify({
        mcpServers: {
          "example-server": {
            command: "npx",
            args: ["-y", "@example/mcp-server"],
            env: {}
          }
        }
      }, null, 2));
    }
  };

  const handleSaveJsonConfig = async () => {
    try {
      
      let parsedConfig;
      try {
        parsedConfig = JSON.parse(jsonConfig);
      } catch (parseError) {
        const errorMessage = parseError instanceof Error ? parseError.message : 'Invalid JSON';
        throw new Error(t('errors.jsonParseError', { message: errorMessage }));
      }
      
      
      if (!parsedConfig.mcpServers) {
        throw new Error(t('errors.mcpServersRequired'));
      }
      
      if (typeof parsedConfig.mcpServers !== 'object' || Array.isArray(parsedConfig.mcpServers)) {
        throw new Error(t('errors.mcpServersMustBeObject'));
      }
      
      
      await MCPAPI.saveMCPJsonConfig(jsonConfig);
      
      notification.success(t('messages.saveSuccess'), {
        title: t('notifications.saveSuccess'),
        duration: 3000
      });
      
      
      setShowJsonEditor(false);

      
      void (async () => {
        try {
          await loadServers();
          await MCPAPI.initializeServers();
        } catch (initError) {
          log.warn('MCP server initialization failed after config save', initError);
          notification.warning(t('messages.partialStartFailed'), {
            title: t('notifications.partialStartFailed'),
            duration: 5000
          });
        } finally {
          await loadServers();
          await loadJsonConfig();
        }
      })();
    } catch (error) {
      log.error('Failed to save config', error);
      
      
      const errorInfo = classifyError(error, t('actions.saveConfig'));
      
      
      let fullMessage = errorInfo.message;
      if (errorInfo.suggestions && errorInfo.suggestions.length > 0) {
        fullMessage += '\n\n' + t('notifications.suggestionPrefix') + '\n' + errorInfo.suggestions.map(s => `• ${s}`).join('\n');
      }
      
      
      notification.error(fullMessage, {
        title: errorInfo.title,
        duration: errorInfo.duration
      });
    }
  };

  const handleJsonEditorKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== 'Tab') {
      return;
    }

    e.preventDefault();

    const textarea = e.currentTarget;
    const value = jsonConfig;
    const indent = '  ';

    const selectionStart = textarea.selectionStart ?? 0;
    const selectionEnd = textarea.selectionEnd ?? 0;

    const setSelection = (start: number, end: number) => {
      requestAnimationFrame(() => {
        const el = jsonEditorRef.current;
        if (!el) return;
        el.focus();
        el.setSelectionRange(start, end);
      });
    };

    // No selection: Tab inserts indentation; Shift+Tab outdents current line.
    if (selectionStart === selectionEnd) {
      if (!e.shiftKey) {
        const nextValue =
          value.slice(0, selectionStart) + indent + value.slice(selectionEnd);
        const nextPos = selectionStart + indent.length;
        setJsonConfig(nextValue);
        setSelection(nextPos, nextPos);
        return;
      }

      const lineStart = value.lastIndexOf('\n', Math.max(0, selectionStart - 1)) + 1;
      const lineEndIdx = value.indexOf('\n', selectionStart);
      const lineEnd = lineEndIdx === -1 ? value.length : lineEndIdx;
      const line = value.slice(lineStart, lineEnd);

      const removeFromLineStart = (() => {
        if (line.startsWith(indent)) return indent.length;
        if (line.startsWith('\t')) return 1;
        let spaces = 0;
        while (spaces < indent.length && line[spaces] === ' ') spaces++;
        return spaces;
      })();

      if (removeFromLineStart === 0) {
        return;
      }

      const nextValue =
        value.slice(0, lineStart) +
        line.slice(removeFromLineStart) +
        value.slice(lineEnd);
      const nextPos = Math.max(lineStart, selectionStart - removeFromLineStart);
      setJsonConfig(nextValue);
      setSelection(nextPos, nextPos);
      return;
    }

    // Multi-line selection: indent/outdent all selected lines.
    let endForLineCalc = selectionEnd;
    if (selectionEnd > 0 && value[selectionEnd - 1] === '\n') {
      endForLineCalc = selectionEnd - 1;
    }

    const lineStart = value.lastIndexOf('\n', Math.max(0, selectionStart - 1)) + 1;
    const nextNewline = value.indexOf('\n', endForLineCalc);
    const lineEnd = nextNewline === -1 ? value.length : nextNewline;

    const selectedBlock = value.slice(lineStart, lineEnd);
    const lines = selectedBlock.split('\n');

    if (!e.shiftKey) {
      const nextBlock = lines.map((l) => indent + l).join('\n');
      const nextValue = value.slice(0, lineStart) + nextBlock + value.slice(lineEnd);
      const nextStart = selectionStart + indent.length;
      const nextEnd = selectionEnd + indent.length * lines.length;
      setJsonConfig(nextValue);
      setSelection(nextStart, nextEnd);
      return;
    }

    let removedTotal = 0;
    const removedPerLine: number[] = [];
    const nextBlock = lines
      .map((line) => {
        let removed = 0;
        if (line.startsWith(indent)) {
          removed = indent.length;
        } else if (line.startsWith('\t')) {
          removed = 1;
        } else {
          while (removed < indent.length && line[removed] === ' ') removed++;
        }
        removedPerLine.push(removed);
        removedTotal += removed;
        return line.slice(removed);
      })
      .join('\n');

    const nextValue = value.slice(0, lineStart) + nextBlock + value.slice(lineEnd);
    const removedFirst = removedPerLine[0] ?? 0;
    const nextStart = Math.max(lineStart, selectionStart - removedFirst);
    const nextEnd = Math.max(nextStart, selectionEnd - removedTotal);
    setJsonConfig(nextValue);
    setSelection(nextStart, nextEnd);
  };

  const handleJsonEditorPaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const pasted = e.clipboardData.getData('text');
    if (!pasted) return;

    const textarea = e.currentTarget;
    const current = jsonConfig;
    const selectionStart = textarea.selectionStart ?? 0;
    const selectionEnd = textarea.selectionEnd ?? 0;

    // Only auto-format when pasting a full JSON document (empty editor or replacing all).
    const isWholeReplace =
      current.trim().length === 0 || (selectionStart === 0 && selectionEnd === current.length);
    if (!isWholeReplace) return;

    const formatted = tryFormatJson(pasted);
    if (!formatted) return;

    e.preventDefault();
    setJsonConfig(formatted);
    requestAnimationFrame(() => {
      jsonEditorRef.current?.focus();
      jsonEditorRef.current?.setSelectionRange(formatted.length, formatted.length);
    });
  };

  const handleStartServer = async (serverId: string) => {
    try {
      setServerAction((prev) => ({ ...prev, [serverId]: 'start' }));
      await MCPAPI.startServer(serverId);
      notification.success(t('messages.startSuccess', { serverId }), {
        title: t('notifications.startSuccess'),
        duration: 3000
      });
      await loadServers();
    } catch (error) {
      log.error('Failed to start server', { serverId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      notification.error(t('messages.startFailed', { serverId }) + ': ' + errorMessage, {
        title: t('notifications.startFailed'),
        duration: 5000
      });
    } finally {
      setServerAction((prev) => {
        const next = { ...prev };
        delete next[serverId];
        return next;
      });
    }
  };

  const handleStopServer = async (serverId: string) => {
    try {
      setServerAction((prev) => ({ ...prev, [serverId]: 'stop' }));
      await MCPAPI.stopServer(serverId);
      notification.success(t('messages.stopSuccess', { serverId }), {
        title: t('notifications.stopSuccess'),
        duration: 3000
      });
      await loadServers();
    } catch (error) {
      log.error('Failed to stop server', { serverId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      notification.error(t('messages.stopFailed', { serverId }) + ': ' + errorMessage, {
        title: t('notifications.stopFailed'),
        duration: 5000
      });
    } finally {
      setServerAction((prev) => {
        const next = { ...prev };
        delete next[serverId];
        return next;
      });
    }
  };

  const handleRestartServer = async (serverId: string) => {
    try {
      setServerAction((prev) => ({ ...prev, [serverId]: 'restart' }));
      await MCPAPI.restartServer(serverId);
      notification.success(t('messages.restartSuccess', { serverId }), {
        title: t('notifications.restartSuccess'),
        duration: 3000
      });
      await loadServers();
    } catch (error) {
      log.error('Failed to restart server', { serverId, error });
      const errorMessage = error instanceof Error ? error.message : String(error);
      notification.error(t('messages.restartFailed', { serverId }) + ': ' + errorMessage, {
        title: t('notifications.restartFailed'),
        duration: 5000
      });
    } finally {
      setServerAction((prev) => {
        const next = { ...prev };
        delete next[serverId];
        return next;
      });
    }
  };

  const getStatusColor = (status: string): string => {
    const statusLower = status.toLowerCase();
    if (statusLower.includes('healthy') || statusLower.includes('connected')) {
      return 'status-healthy';
    } else if (statusLower.includes('starting') || statusLower.includes('reconnecting') || statusLower.includes('stopping')) {
      return 'status-pending';
    } else if (statusLower.includes('failed') || statusLower.includes('stopped')) {
      return 'status-error';
    }
    return 'status-unknown';
  };

  const getStatusIcon = (status: string): React.ReactNode => {
    const statusLower = status.toLowerCase();
    if (statusLower.includes('healthy') || statusLower.includes('connected')) {
      return <CheckCircle size={12} />;
    } else if (statusLower.includes('starting') || statusLower.includes('reconnecting') || statusLower.includes('stopping')) {
      return <Clock size={12} />;
    } else if (statusLower.includes('failed') || statusLower.includes('stopped')) {
      return <AlertTriangle size={12} />;
    }
    return <MinusCircle size={12} />;
  };

  const getEffectiveStatus = (server: MCPServerInfo): string => {
    const action = serverAction[server.id];
    if (action === 'start') return 'Starting';
    if (action === 'stop') return 'Stopping';
    if (action === 'restart') return 'Reconnecting';
    return server.status;
  };

  const isStartableStatus = (status: string): boolean => {
    const s = status.toLowerCase();
    return s.includes('stopped') || s.includes('failed') || s.includes('uninitialized');
  };

  
  const filteredServers = servers.filter(server => {
    if (searchKeyword) {
      const keyword = searchKeyword.toLowerCase();
      return (
        server.name.toLowerCase().includes(keyword) ||
        server.id.toLowerCase().includes(keyword) ||
        server.serverType.toLowerCase().includes(keyword) ||
        server.status.toLowerCase().includes(keyword)
      );
    }
    return true;
  });

  return (
    <ConfigPageLayout className="mcp-config-panel">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="mcp-config-panel__content">
        
        <div className="mcp-config-toolbar">
        <div className="mcp-config-search-box">
          <Search
            placeholder={t('search.placeholder')}
            value={searchKeyword}
            onChange={(val) => setSearchKeyword(val)}
            clearable
            size="small"
          />
        </div>
        <div className="mcp-config-toolbar-actions">
          <IconButton
            variant={showJsonEditor ? 'ghost' : 'primary'}
            size="small"
            onClick={() => {
              setShowJsonEditor(!showJsonEditor);
            }}
            title={showJsonEditor ? t('actions.backToList') : t('actions.jsonConfig')}
          >
            {showJsonEditor ? <X /> : <FileJson />}
          </IconButton>
          <IconButton
            variant="ghost"
            size="small"
            onClick={loadServers}
            tooltip={t('actions.refresh')}
          >
            <RefreshCw />
          </IconButton>
        </div>
      </div>

      
      {showJsonEditor && (
        <div className="mcp-json-editor">
          <div className="json-editor-header">
            <h3>{t('jsonEditor.title')}</h3>
            <p className="json-editor-hint">
              {t('jsonEditor.hint1')}
            </p>
            <p className="json-editor-hint">
              {t('jsonEditor.hint2')}
            </p>
          </div>
          <Textarea
            ref={jsonEditorRef}
            value={jsonConfig}
            onChange={(e) => setJsonConfig(e.target.value)}
            onKeyDown={handleJsonEditorKeyDown}
            onPaste={handleJsonEditorPaste}
            rows={20}
            placeholder={`{\n  "mcpServers": {\n    "server-name": {\n      "command": "npx",\n      "args": ["-y", "@package/name"],\n      "env": {}\n    }\n  }\n}`}
            variant="outlined"
            className="json-editor-textarea"
            spellCheck={false}
            error={!!jsonLintError}
            errorMessage={
              jsonLintError
                ? t('jsonEditor.lintError', {
                    location:
                      typeof jsonLintError.line === 'number' && typeof jsonLintError.column === 'number'
                        ? t('jsonEditor.lintLocation', {
                            line: jsonLintError.line,
                            column: jsonLintError.column,
                          })
                        : '',
                    message: jsonLintError.message,
                  })
                : undefined
            }
          />
          <div className="json-editor-actions">
            <Button variant="secondary" onClick={() => setShowJsonEditor(false)}>
              {t('actions.cancel')}
            </Button>
            <Button variant="primary" onClick={handleSaveJsonConfig}>
              {t('actions.saveConfig')}
            </Button>
          </div>
          <div className="json-editor-example">
            <h4>{t('jsonEditor.exampleTitle')}</h4>
            <div className="example-section">
              <h5>{t('jsonEditor.localProcess')}</h5>
              <pre>{`{
  "mcpServers": {
    "zai-mcp-server": {
      "command": "npx",
      "args": ["-y", "@z_ai/mcp-server"],
      "env": {
        "Z_AI_API_KEY": "your_api_key",
        "Z_AI_MODE": "ZHIPU"
      }
    }
  }
}`}</pre>
            </div>
            <div className="example-section">
              <h5>{t('jsonEditor.remoteService')}</h5>
              <pre>{`{
  "mcpServers": {
    "remote-mcp": {
      "url": "http://localhost:3000/sse"
    }
  }
}`}</pre>
            </div>
          </div>
        </div>
      )}

      {!showJsonEditor && loading ? (
        <div className="mcp-loading">{t('loading')}</div>
      ) : filteredServers.length === 0 ? (
        <div className="mcp-empty-state">
          <div className="empty-icon">
            <Plug size={28} />
          </div>
          {!searchKeyword && (
            <Button variant="dashed" onClick={() => setShowJsonEditor(true)}>
              {t('actions.jsonConfig')}
            </Button>
          )}
        </div>
      ) : !showJsonEditor ? (
        <div className="mcp-servers-list">
          {filteredServers.map((server) => {
            const effectiveStatus = getEffectiveStatus(server);
            const busyAction = serverAction[server.id];
            const actionBusy = !!busyAction;
            const startable = isStartableStatus(effectiveStatus);
            const statusColor = getStatusColor(effectiveStatus);

            return (
            <Card key={server.id} variant="default" padding="none" className="mcp-server-card">
              <div className="server-header">
                <span className={`status-indicator ${statusColor} ${actionBusy ? 'status-indicator--busy' : ''}`}>
                  {actionBusy ? <Loader2 size={12} /> : getStatusIcon(effectiveStatus)}
                </span>
                <h3>{server.name}</h3>
                <span className="server-id">{server.id}</span>
                <div className="detail-item">
                  <span className="label">{t('labels.type')}:</span>
                  <span className="value">{server.serverType}</span>
                </div>
                <div className="detail-item">
                  <span className="label">{t('labels.enabled')}:</span>
                  <span className="value">{server.enabled ? t('labels.yes') : t('labels.no')}</span>
                </div>
              </div>
              
              <div className="server-footer">
                <div className="server-details">
                  <div className="detail-item">
                    <span className="label">{t('labels.autoStart')}:</span>
                    <span className="value">{server.autoStart ? t('labels.yes') : t('labels.no')}</span>
                  </div>
                  <div className="detail-item">
                    <span className="label">{t('labels.status')}:</span>
                    <span className={`value ${statusColor}`}>
                      {effectiveStatus}
                    </span>
                  </div>
                </div>

                <div className="server-actions">
                  {startable ? (
                    <IconButton
                      size="medium"
                      variant="success"
                      onClick={() => handleStartServer(server.id)}
                      disabled={actionBusy}
                      isLoading={busyAction === 'start'}
                      tooltip={t('actions.start')}
                    >
                      {busyAction === 'start' ? <Loader2 /> : <Play />}
                    </IconButton>
                  ) : (
                    <IconButton
                      size="medium"
                      variant="warning"
                      onClick={() => handleStopServer(server.id)}
                      disabled={actionBusy}
                      isLoading={busyAction === 'stop'}
                      tooltip={t('actions.stop')}
                    >
                      {busyAction === 'stop' ? <Loader2 /> : <Square />}
                    </IconButton>
                  )}
                  <IconButton
                    size="medium"
                    variant="ghost"
                    onClick={() => handleRestartServer(server.id)}
                    disabled={actionBusy}
                    isLoading={busyAction === 'restart'}
                    tooltip={t('actions.restart')}
                  >
                    {busyAction === 'restart' ? <Loader2 /> : <RefreshCw />}
                  </IconButton>
                </div>
              </div>
            </Card>
            );
          })}
        </div>
      ) : null}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default MCPConfig;
