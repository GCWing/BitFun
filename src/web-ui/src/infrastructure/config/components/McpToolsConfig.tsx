/**
 * McpToolsConfig — MCP servers only.
 * Tool execution behavior lives on Session Config.
 * Uses settings/mcp-tools for page title/subtitle, settings/mcp for the MCP section.
 */

import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  FileJson,
  RefreshCw,
  X,
  Play,
  Square,
  CheckCircle,
  AlertTriangle,
  MinusCircle,
  KeyRound,
  Trash2,
} from 'lucide-react';
import { Button, Textarea, IconButton, Modal, ToolProcessingDots } from '@/component-library';
import {
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageContent,
  ConfigPageSection,
  ConfigCollectionItem,
} from './common';
import { useNotification } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { isTauriRuntime } from '@/infrastructure/runtime';
import {
  MCPAPI,
  MCPRemoteOAuthSessionSnapshot,
  MCPServerInfo,
} from '../../api/service-api/MCPAPI';
import { systemAPI } from '../../api/service-api/SystemAPI';
import ExternalMcpOverview from './ExternalMcpOverview';
import './McpToolsConfig.scss';

const log = createLogger('McpToolsConfig');

// ─── MCP error classifier (from MCPConfig) ────────────────────────────────────
interface ErrorInfo {
  title: string;
  message: string;
  duration: number;
  suggestions?: string[];
}

function createErrorClassifier(t: (key: string, options?: any) => any) {
  const getSuggestions = (key: string): string[] | undefined => {
    const suggestions = t(key, { returnObjects: true });
    if (!Array.isArray(suggestions)) return undefined;
    return suggestions.map((s) => String(s));
  };

  return function classifyError(error: unknown, context: string = 'operation'): ErrorInfo {
    let errorMessage = t('errors.unknownError');
    if (error instanceof Error) errorMessage = error.message;
    else if (typeof error === 'string') errorMessage = error;

    const normalizedMessage = errorMessage.toLowerCase();
    const matches = (patterns: string[]) => patterns.some((p) => normalizedMessage.includes(p));

    if (matches(['json parsing failed', 'json parse failed', 'invalid json', 'json format']))
      return {
        title: t('errors.jsonFormatError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.jsonFormat'),
      };
    if (matches(["config missing 'mcpservers' field", "'mcpservers' field must be an object"]))
      return {
        title: t('errors.configStructureError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.configStructure'),
      };
    if (
      matches([
        "must not set both 'command' and 'url'",
        "must provide either 'command' (stdio) or 'url' (sse)",
        "unsupported 'type' value",
        "'type' conflicts with provided fields",
        "(stdio) must provide 'command' field",
        "(sse) must provide 'url' field",
        "'args' field must be an array",
        "'env' field must be an object",
        'config must be an object',
      ])
    )
      return {
        title: t('errors.serverConfigError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.serverConfig'),
      };
    if (matches(['permission denied', 'access is denied']))
      return {
        title: t('errors.permissionError'),
        message: errorMessage,
        duration: 15000,
        suggestions: getSuggestions('errors.suggestions.permission'),
      };
    if (matches(['address already in use', 'failed to bind oauth callback listener']))
      return {
        title: t('errors.operationFailed', { context: 'oauth' }),
        message: errorMessage,
        duration: 10000,
        suggestions: [
          'Change the OAuth callback port in the MCP config or stop the process already using it.',
        ],
      };
    if (matches(['authorization timed out', 'oauth authorization timed out']))
      return {
        title: t('errors.operationFailed', { context: 'oauth' }),
        message: errorMessage,
        duration: 10000,
        suggestions: ['Restart OAuth and complete sign-in before the callback window expires.'],
      };
    if (
      matches([
        'failed to write config file',
        'failed to serialize config',
        'failed to save config',
        'io error',
        'write failed',
      ])
    )
      return {
        title: t('errors.fileOperationError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.fileOperation'),
      };
    if (matches(['not found']))
      return { title: t('errors.resourceNotFound'), message: errorMessage, duration: 8000 };
    if (
      matches([
        'failed to start mcp server',
        'failed to capture stdin',
        'failed to capture stdout',
        'max restart attempts',
        'process error',
      ])
    )
      return {
        title: t('errors.serverStartError'),
        message: errorMessage,
        duration: 10000,
        suggestions: getSuggestions('errors.suggestions.serverStart'),
      };
    return {
      title: t('errors.operationFailed', { context }),
      message: errorMessage,
      duration: 8000,
      suggestions: getSuggestions('errors.suggestions.default'),
    };
  };
}

const McpToolsConfig: React.FC = () => {
  const { t: tPage } = useTranslation('settings/mcp-tools');
  const { t: tMcp } = useTranslation('settings/mcp');

  const notification = useNotification();
  const peerDevice = usePeerDeviceModeOptional();
  const remoteConnectionActive = peerDevice?.peerMode.active === true;
  const desktopConfigAvailable = isTauriRuntime() && !remoteConnectionActive;
  const classifyError = createErrorClassifier(tMcp);

  // ─── MCP state ─────────────────────────────────────────────────────────────
  const jsonEditorRef = useRef<HTMLTextAreaElement>(null);
  const jsonLintSeqRef = useRef(0);
  const oauthPollTimerRef = useRef<number | null>(null);
  const serverLoadRequestIdRef = useRef(0);
  const jsonLoadRequestIdRef = useRef(0);
  const capabilityRef = useRef({ available: desktopConfigAvailable, epoch: 0 });
  const [servers, setServers] = useState<MCPServerInfo[]>([]);
  const [mcpLoading, setMcpLoading] = useState(true);
  const [serverLoadFailed, setServerLoadFailed] = useState(false);
  const [showJsonEditor, setShowJsonEditor] = useState(false);
  const [jsonConfig, setJsonConfig] = useState('');
  const [jsonLoading, setJsonLoading] = useState(true);
  const [jsonLoadFailed, setJsonLoadFailed] = useState(false);
  const [authDialogServer, setAuthDialogServer] = useState<MCPServerInfo | null>(null);
  const [authValue, setAuthValue] = useState('');
  const [authSubmitting, setAuthSubmitting] = useState(false);
  const [oauthSession, setOauthSession] = useState<MCPRemoteOAuthSessionSnapshot | null>(null);
  const [oauthStarting, setOauthStarting] = useState(false);
  const [oauthCancelling, setOauthCancelling] = useState(false);
  const [jsonLintError, setJsonLintError] = useState<{
    message: string;
    line?: number;
    column?: number;
    position?: number;
  } | null>(null);

  useLayoutEffect(() => {
    if (capabilityRef.current.available !== desktopConfigAvailable) {
      capabilityRef.current = {
        available: desktopConfigAvailable,
        epoch: capabilityRef.current.epoch + 1,
      };
    }
  }, [desktopConfigAvailable]);

  const currentCapabilityEpoch = useCallback((): number | null => (
    capabilityRef.current.available ? capabilityRef.current.epoch : null
  ), []);
  const capabilityIsCurrent = useCallback((epoch: number): boolean => (
    capabilityRef.current.available && capabilityRef.current.epoch === epoch
  ), []);

  const tryFormatJson = (input: string): string | null => {
    try {
      return JSON.stringify(JSON.parse(input), null, 2);
    } catch {
      return null;
    }
  };

  // ─── MCP effects & handlers ─────────────────────────────────────────────────
  const LOAD_SERVERS_TIMEOUT_MS = 15_000;

  const loadServers = useCallback(async (): Promise<boolean> => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return false;
    const requestId = ++serverLoadRequestIdRef.current;
    try {
      setMcpLoading(true);
      setServerLoadFailed(false);
      const serverList = await Promise.race([
        MCPAPI.getServers(),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('MCP servers load timed out')), LOAD_SERVERS_TIMEOUT_MS)
        ),
      ]);
      if (
        requestId !== serverLoadRequestIdRef.current
        || !capabilityIsCurrent(capabilityEpoch)
      ) {
        return false;
      }
      setServers(serverList);
      setServerLoadFailed(false);
      return true;
    } catch (error) {
      if (
        requestId !== serverLoadRequestIdRef.current
        || !capabilityIsCurrent(capabilityEpoch)
      ) {
        return false;
      }
      log.error('Failed to load MCP servers', error);
      setServerLoadFailed(true);
      return false;
    } finally {
      if (
        requestId === serverLoadRequestIdRef.current
        && capabilityIsCurrent(capabilityEpoch)
      ) {
        setMcpLoading(false);
      }
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch]);

  const loadJsonConfig = useCallback(async (): Promise<boolean> => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return false;
    const requestId = ++jsonLoadRequestIdRef.current;
    setJsonLoading(true);
    setJsonLoadFailed(false);
    try {
      const config = await MCPAPI.loadMCPJsonConfig();
      if (
        requestId !== jsonLoadRequestIdRef.current
        || !capabilityIsCurrent(capabilityEpoch)
      ) {
        return false;
      }
      setJsonConfig(config);
      setJsonLoadFailed(false);
      return true;
    } catch (error) {
      if (
        requestId !== jsonLoadRequestIdRef.current
        || !capabilityIsCurrent(capabilityEpoch)
      ) {
        return false;
      }
      log.error('Failed to load MCP JSON configuration', error);
      setJsonLoadFailed(true);
      return false;
    } finally {
      if (
        requestId === jsonLoadRequestIdRef.current
        && capabilityIsCurrent(capabilityEpoch)
      ) {
        setJsonLoading(false);
      }
    }
  }, [capabilityIsCurrent, currentCapabilityEpoch]);

  function stopOAuthPolling() {
    if (oauthPollTimerRef.current !== null) {
      window.clearInterval(oauthPollTimerRef.current);
      oauthPollTimerRef.current = null;
    }
  }

  const handleOAuthSessionUpdate = async (
    serverId: string,
    session: MCPRemoteOAuthSessionSnapshot | null,
    capabilityEpoch: number,
  ) => {
    if (!capabilityIsCurrent(capabilityEpoch)) return;
    setOauthSession(session);

    const status = session?.status;
    if (!status || !['authorized', 'failed', 'cancelled'].includes(status)) {
      return;
    }

    stopOAuthPolling();

    if (status === 'authorized') {
      notification.success(
        session?.message || tMcp('messages.remoteOAuthAuthorized', { serverId }),
        {
          title: tMcp('notifications.saveSuccess'),
          duration: 4000,
        }
      );
      await loadServers();
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      closeAuthDialog();
      return;
    }

    if (status === 'failed') {
      notification.error(
        session?.message || tMcp('messages.remoteOAuthFailed', { serverId }),
        {
          title: tMcp('notifications.operationFailed'),
          duration: 6000,
        }
      );
    }
  };

  const pollOAuthSession = (serverId: string, capabilityEpoch: number) => {
    if (!capabilityIsCurrent(capabilityEpoch)) return;
    stopOAuthPolling();
    oauthPollTimerRef.current = window.setInterval(async () => {
      if (!capabilityIsCurrent(capabilityEpoch)) {
        stopOAuthPolling();
        return;
      }
      try {
        const session = await MCPAPI.getRemoteOAuthSession({ serverId });
        if (!capabilityIsCurrent(capabilityEpoch)) return;
        await handleOAuthSessionUpdate(serverId, session, capabilityEpoch);
      } catch (error) {
        if (!capabilityIsCurrent(capabilityEpoch)) return;
        stopOAuthPolling();
        notification.error(
          error instanceof Error ? error.message : String(error),
          {
            title: tMcp('notifications.operationFailed'),
            duration: 5000,
          }
        );
      }
    }, 1000);
  };

  useEffect(() => {
    serverLoadRequestIdRef.current += 1;
    jsonLoadRequestIdRef.current += 1;
    if (!desktopConfigAvailable) {
      setServers([]);
      setMcpLoading(false);
      setServerLoadFailed(false);
      setShowJsonEditor(false);
      setJsonLoading(false);
      setJsonLoadFailed(false);
      setAuthDialogServer(null);
      setAuthSubmitting(false);
      setOauthSession(null);
      setOauthStarting(false);
      setOauthCancelling(false);
      stopOAuthPolling();
      return;
    }
    void loadServers();
    void loadJsonConfig();
  }, [desktopConfigAvailable, loadJsonConfig, loadServers]);

  useEffect(() => {
    return () => {
      if (oauthPollTimerRef.current !== null) {
        window.clearInterval(oauthPollTimerRef.current);
        oauthPollTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (!showJsonEditor) {
      setJsonLintError(null);
      return;
    }
    const seq = ++jsonLintSeqRef.current;
    const handle = window.setTimeout(() => {
      if (seq !== jsonLintSeqRef.current) return;
      if (!jsonConfig.trim()) {
        setJsonLintError(null);
        return;
      }
      try {
        JSON.parse(jsonConfig);
        setJsonLintError(null);
      } catch (error) {
        if (seq !== jsonLintSeqRef.current) return;
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
          setJsonLintError({
            message,
            line: lines.length,
            column: (lines[lines.length - 1]?.length ?? 0) + 1,
            position,
          });
        } else {
          setJsonLintError({ message });
        }
      }
    }, 150);
    return () => window.clearTimeout(handle);
  }, [jsonConfig, showJsonEditor]);

  const handleSaveJsonConfig = async () => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    try {
      let parsedConfig;
      try {
        parsedConfig = JSON.parse(jsonConfig);
      } catch (parseError) {
        throw new Error(
          tMcp('errors.jsonParseError', {
            message: parseError instanceof Error ? parseError.message : 'Invalid JSON',
          })
        );
      }
      if (!parsedConfig.mcpServers) throw new Error(tMcp('errors.mcpServersRequired'));
      if (typeof parsedConfig.mcpServers !== 'object' || Array.isArray(parsedConfig.mcpServers))
        throw new Error(tMcp('errors.mcpServersMustBeObject'));

      await MCPAPI.saveMCPJsonConfig(jsonConfig);
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      notification.success(tMcp('messages.saveSuccess'), {
        title: tMcp('notifications.saveSuccess'),
        duration: 3000,
      });
      setShowJsonEditor(false);

      void (async () => {
        try {
          await loadServers();
          if (!capabilityIsCurrent(capabilityEpoch)) return;
          await MCPAPI.initializeServers();
          if (!capabilityIsCurrent(capabilityEpoch)) return;
        } catch {
          if (!capabilityIsCurrent(capabilityEpoch)) return;
          notification.warning(tMcp('messages.partialStartFailed'), {
            title: tMcp('notifications.partialStartFailed'),
            duration: 5000,
          });
        } finally {
          if (capabilityIsCurrent(capabilityEpoch)) {
            await loadServers();
            if (capabilityIsCurrent(capabilityEpoch)) {
              await loadJsonConfig();
            }
          }
        }
      })();
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      const errorInfo = classifyError(error, tMcp('actions.saveConfig'));
      let fullMessage = errorInfo.message;
      if (errorInfo.suggestions?.length) {
        fullMessage +=
          '\n\n' +
          tMcp('notifications.suggestionPrefix') +
          '\n' +
          errorInfo.suggestions.map((s) => `• ${s}`).join('\n');
      }
      notification.error(fullMessage, {
        title: errorInfo.title,
        duration: errorInfo.duration,
      });
    }
  };

  const handleJsonEditorKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== 'Tab') return;
    e.preventDefault();
    const value = jsonConfig;
    const indent = '  ';
    const selectionStart = e.currentTarget.selectionStart ?? 0;
    const selectionEnd = e.currentTarget.selectionEnd ?? 0;
    const setSelection = (start: number, end: number) => {
      requestAnimationFrame(() => {
        const el = jsonEditorRef.current;
        if (!el) return;
        el.focus();
        el.setSelectionRange(start, end);
      });
    };

    if (selectionStart === selectionEnd) {
      if (!e.shiftKey) {
        setJsonConfig(value.slice(0, selectionStart) + indent + value.slice(selectionEnd));
        setSelection(selectionStart + indent.length, selectionStart + indent.length);
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
      if (removeFromLineStart === 0) return;
      setJsonConfig(value.slice(0, lineStart) + line.slice(removeFromLineStart) + value.slice(lineEnd));
      setSelection(
        Math.max(lineStart, selectionStart - removeFromLineStart),
        Math.max(lineStart, selectionStart - removeFromLineStart)
      );
      return;
    }

    let endForLineCalc = selectionEnd;
    if (selectionEnd > 0 && value[selectionEnd - 1] === '\n') endForLineCalc = selectionEnd - 1;
    const lineStart = value.lastIndexOf('\n', Math.max(0, selectionStart - 1)) + 1;
    const nextNewline = value.indexOf('\n', endForLineCalc);
    const lineEnd = nextNewline === -1 ? value.length : nextNewline;
    const selectedBlock = value.slice(lineStart, lineEnd);
    const lines = selectedBlock.split('\n');

    if (!e.shiftKey) {
      const nextBlock = lines.map((l) => indent + l).join('\n');
      setJsonConfig(value.slice(0, lineStart) + nextBlock + value.slice(lineEnd));
      setSelection(selectionStart + indent.length, selectionEnd + indent.length * lines.length);
      return;
    }

    let removedTotal = 0;
    const removedPerLine: number[] = [];
    const nextBlock = lines
      .map((line) => {
        let removed = 0;
        if (line.startsWith(indent)) removed = indent.length;
        else if (line.startsWith('\t')) removed = 1;
        else {
          while (removed < indent.length && line[removed] === ' ') removed++;
        }
        removedPerLine.push(removed);
        removedTotal += removed;
        return line.slice(removed);
      })
      .join('\n');
    const nextStart = Math.max(lineStart, selectionStart - (removedPerLine[0] ?? 0));
    setJsonConfig(value.slice(0, lineStart) + nextBlock + value.slice(lineEnd));
    setSelection(nextStart, Math.max(nextStart, selectionEnd - removedTotal));
  };

  const handleJsonEditorPaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const pasted = e.clipboardData.getData('text');
    if (!pasted) return;
    const current = jsonConfig;
    const selectionStart = e.currentTarget.selectionStart ?? 0;
    const selectionEnd = e.currentTarget.selectionEnd ?? 0;
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

  const isCommandDrivenServer = (server: MCPServerInfo) => {
    return server.transport.toLowerCase() === 'stdio';
  };

  const isRemoteServer = (server: MCPServerInfo) => {
    return server.serverType.toLowerCase().includes('remote');
  };

  const canStartServer = (server: MCPServerInfo) => {
    if (server.startSupported === false) return false;
    if (!isCommandDrivenServer(server)) return true;
    return server.commandAvailable !== false;
  };

  const getErrorMessage = (error: unknown) =>
    error instanceof Error ? error.message : String(error);

  const isLikelyRemoteAuthError = (error: unknown) => {
    const message = getErrorMessage(error).toLowerCase();
    return [
      'auth required',
      'authorization required',
      'authentication required',
      'www-authenticate',
      'status code: 401',
      'status code: 403',
      'unauthorized',
      'forbidden',
    ].some((pattern) => message.includes(pattern));
  };

  const notifyServerStartUnavailable = (server: MCPServerInfo) => {
    const message = server.startDisabledReason
      ? getStartDisabledReasonLabel(server)
      : tMcp('messages.commandUnavailable', { serverId: server.id });
    notification.warning(
      message,
      {
        title: tMcp('notifications.startFailed'),
        duration: 5000,
      }
    );
  };

  const handleStartServer = async (server: MCPServerInfo) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    if (!canStartServer(server)) {
      notifyServerStartUnavailable(server);
      return;
    }

    const serverId = server.id;
    try {
      await MCPAPI.startServer(serverId);
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      notification.success(tMcp('messages.startSuccess', { serverId }), {
        title: tMcp('notifications.startSuccess'),
        duration: 3000,
      });
      await loadServers();
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      if (isRemoteServer(server) && isLikelyRemoteAuthError(error)) {
        handleOpenAuthDialog(server);
        if (server.oauthEnabled) {
          void startRemoteOAuthFlow(server);
        }
      }
      notification.error(
        tMcp('messages.startFailed', { serverId }) +
          ': ' +
          getErrorMessage(error),
        { title: tMcp('notifications.startFailed'), duration: 5000 }
      );
    }
  };

  const handleStopServer = async (serverId: string) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    try {
      await MCPAPI.stopServer(serverId);
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      notification.success(tMcp('messages.stopSuccess', { serverId }), {
        title: tMcp('notifications.stopSuccess'),
        duration: 3000,
      });
      await loadServers();
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      notification.error(
        tMcp('messages.stopFailed', { serverId }) +
          ': ' +
          (error instanceof Error ? error.message : String(error)),
        { title: tMcp('notifications.stopFailed'), duration: 5000 }
      );
    }
  };

  const handleRestartServer = async (server: MCPServerInfo) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    if (!canStartServer(server)) {
      notifyServerStartUnavailable(server);
      return;
    }

    const serverId = server.id;
    try {
      await MCPAPI.restartServer(serverId);
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      notification.success(tMcp('messages.restartSuccess', { serverId }), {
        title: tMcp('notifications.restartSuccess'),
        duration: 3000,
      });
      await loadServers();
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      if (isRemoteServer(server) && isLikelyRemoteAuthError(error)) {
        handleOpenAuthDialog(server);
        if (server.oauthEnabled) {
          void startRemoteOAuthFlow(server);
        }
      }
      notification.error(
        tMcp('messages.restartFailed', { serverId }) +
          ': ' +
          getErrorMessage(error),
        { title: tMcp('notifications.restartFailed'), duration: 5000 }
      );
    }
  };

  function handleOpenAuthDialog(server: MCPServerInfo) {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    setAuthDialogServer(server);
    setAuthValue('');
    setOauthSession(null);
    setOauthStarting(false);
    setOauthCancelling(false);
    stopOAuthPolling();

    if (server.oauthEnabled) {
      void (async () => {
        try {
          const session = await MCPAPI.getRemoteOAuthSession({ serverId: server.id });
          if (!capabilityIsCurrent(capabilityEpoch)) return;
          setOauthSession(session);
          if (session && !['authorized', 'failed', 'cancelled'].includes(session.status)) {
            pollOAuthSession(server.id, capabilityEpoch);
          }
        } catch (error) {
          if (!capabilityIsCurrent(capabilityEpoch)) return;
          log.warn('Failed to load remote OAuth session', { serverId: server.id, error });
        }
      })();
    }
  }

  function closeAuthDialog() {
    stopOAuthPolling();
    setAuthDialogServer(null);
    setAuthValue('');
    setOauthSession(null);
    setOauthStarting(false);
    setOauthCancelling(false);
  }

  const handleCloseAuthDialog = () => {
    if (authSubmitting || oauthCancelling) return;
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) {
      closeAuthDialog();
      return;
    }

    if (
      authDialogServer &&
      oauthSession &&
      !['authorized', 'failed', 'cancelled'].includes(oauthSession.status)
    ) {
      setOauthCancelling(true);
      void (async () => {
        try {
          await MCPAPI.cancelRemoteOAuth({ serverId: authDialogServer.id });
        } catch (error) {
          if (!capabilityIsCurrent(capabilityEpoch)) return;
          log.warn('Failed to cancel remote OAuth session', {
            serverId: authDialogServer.id,
            error,
          });
        } finally {
          if (capabilityIsCurrent(capabilityEpoch)) {
            setOauthCancelling(false);
            closeAuthDialog();
          }
        }
      })();
      return;
    }

    closeAuthDialog();
  };

  const handleSaveRemoteAuth = async () => {
    if (!authDialogServer || authSubmitting) return;
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;

    const trimmed = authValue.trim();
    if (!trimmed) {
      notification.warning(tMcp('messages.remoteAuthRequired'), {
        title: tMcp('notifications.operationFailed'),
        duration: 5000,
      });
      return;
    }

    setAuthSubmitting(true);
    try {
      await MCPAPI.updateRemoteAuth({
        serverId: authDialogServer.id,
        authorizationValue: trimmed,
      });
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      notification.success(
        tMcp('messages.remoteAuthUpdated', { serverId: authDialogServer.id }),
        {
          title: tMcp('notifications.saveSuccess'),
          duration: 3000,
        }
      );
      closeAuthDialog();
      await loadServers();
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      const errorInfo = classifyError(error, tMcp('actions.saveConfig'));
      notification.error(errorInfo.message, {
        title: errorInfo.title,
        duration: errorInfo.duration,
      });
    } finally {
      if (capabilityIsCurrent(capabilityEpoch)) {
        setAuthSubmitting(false);
      }
    }
  };

  const handleDeleteServer = async (server: MCPServerInfo) => {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    const confirmed = await window.confirm(tMcp('messages.deleteConfirm'));
    if (!confirmed || !capabilityIsCurrent(capabilityEpoch)) return;

    try {
      await MCPAPI.deleteServer({ serverId: server.id });
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      if (authDialogServer?.id === server.id) {
        closeAuthDialog();
      }
      notification.success(tMcp('messages.deleteSuccess'), {
        title: tMcp('notifications.saveSuccess'),
        duration: 3000,
      });
      await loadServers();
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      const errorInfo = classifyError(error, tMcp('actions.delete'));
      notification.error(
        tMcp('errors.deleteServerFailed', {
          serverId: server.id,
          message: errorInfo.message,
        }),
        {
          title: tMcp('messages.deleteFailed'),
          duration: errorInfo.duration,
        }
      );
    }
  };

  async function startRemoteOAuthFlow(server: MCPServerInfo) {
    const capabilityEpoch = currentCapabilityEpoch();
    if (capabilityEpoch === null) return;
    setOauthStarting(true);
    try {
      const session = await MCPAPI.startRemoteOAuth({ serverId: server.id });
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      setOauthSession(session);
      if (session.authorizationUrl) {
        await systemAPI.openExternal(session.authorizationUrl);
        if (!capabilityIsCurrent(capabilityEpoch)) return;
      }
      pollOAuthSession(server.id, capabilityEpoch);
      notification.success(
        session.message || tMcp('messages.remoteOAuthStarted', { serverId: server.id }),
        {
          title: tMcp('notifications.startSuccess'),
          duration: 3000,
        }
      );
    } catch (error) {
      if (!capabilityIsCurrent(capabilityEpoch)) return;
      const errorInfo = classifyError(error, tMcp('actions.remoteAuth'));
      notification.error(errorInfo.message, {
        title: errorInfo.title,
        duration: errorInfo.duration,
      });
    } finally {
      if (capabilityIsCurrent(capabilityEpoch)) {
        setOauthStarting(false);
      }
    }
  }

  const handleStartRemoteOAuth = async () => {
    if (!authDialogServer || oauthStarting || authSubmitting) return;
    await startRemoteOAuthFlow(authDialogServer);
  };

  const getStatusClass = (status: string): string => {
    const s = status.toLowerCase();
    if (s.includes('healthy') || s.includes('connected')) return 'is-healthy';
    if (s.includes('starting') || s.includes('reconnecting')) return 'is-pending';
    if (s.includes('failed') || s.includes('stopped') || s.includes('auth')) return 'is-error';
    return '';
  };

  const getStatusIcon = (status: string): React.ReactNode => {
    const s = status.toLowerCase();
    if (s.includes('healthy') || s.includes('connected')) return <CheckCircle size={10} />;
    if (s.includes('starting') || s.includes('reconnecting')) return <ToolProcessingDots size={10} />;
    if (s.includes('failed') || s.includes('stopped') || s.includes('auth'))
      return <AlertTriangle size={10} />;
    return <MinusCircle size={10} />;
  };

  const isStopped = (status: string) => {
    const s = status.toLowerCase();
    return s.includes('stopped') || s.includes('failed') || s.includes('auth');
  };

  const getServerStatusLabel = (status: string) => {
    const normalized = status.trim().toLowerCase();
    switch (normalized) {
      case 'uninitialized':
        return tMcp('status.uninitialized');
      case 'starting':
        return tMcp('status.starting');
      case 'connected':
        return tMcp('status.connected');
      case 'healthy':
        return tMcp('status.healthy');
      case 'needsauth':
        return tMcp('status.needsAuth');
      case 'reconnecting':
        return tMcp('status.reconnecting');
      case 'failed':
        return tMcp('status.failed');
      case 'stopping':
        return tMcp('status.stopping');
      case 'stopped':
        return tMcp('status.stopped');
      default:
        return status;
    }
  };

  const getRuntimeSourceLabel = (server: MCPServerInfo) => {
    if (!server.commandSource) {
      return tMcp('server.runtime.unknown');
    }
    return server.commandSource === 'managed'
      ? tMcp('server.runtime.managed')
      : tMcp('server.runtime.system');
  };

  const getOAuthStatusLabel = (session: MCPRemoteOAuthSessionSnapshot | null) => {
    if (!session) {
      return tMcp('server.remoteOAuthIdle');
    }

    switch (session.status) {
      case 'awaitingBrowser':
        return tMcp('server.remoteOAuthAwaitingBrowser');
      case 'awaitingCallback':
        return tMcp('server.remoteOAuthAwaitingCallback');
      case 'exchangingToken':
        return tMcp('server.remoteOAuthExchangingToken');
      case 'authorized':
        return tMcp('server.remoteOAuthAuthorized');
      case 'failed':
        return tMcp('server.remoteOAuthFailed');
      case 'cancelled':
        return tMcp('server.remoteOAuthCancelled');
      default:
        return session.status;
    }
  };

  const getAuthSourceLabel = (authSource?: MCPServerInfo['authSource']) => {
    if (!authSource) return '';
    switch (authSource) {
      case 'headers':
        return tMcp('server.authSource.headers');
      case 'env':
        return tMcp('server.authSource.env');
      case 'oauth':
        return tMcp('server.authSource.oauth');
      default:
        return authSource;
    }
  };

  const getRemoteAuthSummary = (server: MCPServerInfo) => {
    if (server.authConfigured) {
      if (server.authSource) {
        return tMcp('server.remoteAuthConfiguredWithSource', {
          source: getAuthSourceLabel(server.authSource),
        });
      }
      return tMcp('server.remoteAuthConfigured');
    }

    if (server.oauthEnabled) {
      return tMcp('server.remoteOAuthReady');
    }

    return tMcp('server.remoteAuthMissing');
  };

  const getRemoteAuthMethodLabel = (server: MCPServerInfo) => {
    if (server.oauthEnabled && server.xaaEnabled) {
      return tMcp('server.remoteAuthMethodOAuthXaa');
    }
    if (server.oauthEnabled) {
      return tMcp('server.remoteAuthMethodOAuth');
    }
    return tMcp('server.remoteAuthMethodXaa');
  };

  function getStartDisabledReasonLabel(server: MCPServerInfo) {
    if (server.transport.toLowerCase() === 'sse' && server.startSupported === false) {
      return tMcp('server.runtime.unsupportedRemoteSse');
    }

    return server.startDisabledReason || '';
  }

  const isOAuthFlowActive = !!oauthSession && !['authorized', 'failed', 'cancelled'].includes(oauthSession.status);

  const getOAuthActionLabel = (server: MCPServerInfo) => {
    if (isOAuthFlowActive) {
      return tMcp('actions.restartRemoteOAuth');
    }
    if (server.authSource === 'oauth' && server.authConfigured) {
      return tMcp('actions.reconnectRemoteOAuth');
    }
    return tMcp('actions.startRemoteOAuth');
  };

  const mcpSectionExtra = (
    <>
      {serverLoadFailed && !showJsonEditor ? (
        <>
          {servers.length > 0 ? (
            <span className="bitfun-mcp-tools__status-badge is-pending">
              {tMcp('external.status.stale')}
            </span>
          ) : null}
          <IconButton
            variant="ghost"
            size="small"
            onClick={() => void loadServers()}
            tooltip={tMcp('actions.refresh')}
            aria-label={tMcp('actions.refresh')}
          >
            <RefreshCw size={16} aria-hidden="true" />
          </IconButton>
        </>
      ) : null}
      <IconButton
        variant="ghost"
        size="small"
        onClick={() => setShowJsonEditor(!showJsonEditor)}
        tooltip={showJsonEditor ? tMcp('actions.backToList') : tMcp('actions.jsonConfig')}
        aria-label={showJsonEditor ? tMcp('actions.backToList') : tMcp('actions.jsonConfig')}
      >
        {showJsonEditor ? <X size={16} /> : <FileJson size={16} />}
      </IconButton>
    </>
  );

  const renderServerBadge = (server: MCPServerInfo) => (
    <span className={`bitfun-mcp-tools__status-badge ${getStatusClass(server.status)}`}>
      {getStatusIcon(server.status)}
      {getServerStatusLabel(server.status)}
    </span>
  );

  const renderServerControl = (server: MCPServerInfo) => (
    <>
      {isRemoteServer(server) && (
        <IconButton
          size="small"
          variant="ghost"
          onClick={() => handleOpenAuthDialog(server)}
          tooltip={tMcp('actions.remoteAuth')}
          aria-label={tMcp('actions.remoteAuth')}
        >
          <KeyRound size={14} />
        </IconButton>
      )}
      <IconButton
        size="small"
        variant="ghost"
        onClick={() => handleDeleteServer(server)}
        tooltip={tMcp('actions.delete')}
        aria-label={tMcp('actions.delete')}
      >
        <Trash2 size={14} />
      </IconButton>
      {isStopped(server.status) ? (
        <IconButton
          size="small"
          variant="success"
          onClick={() => handleStartServer(server)}
          tooltip={
            canStartServer(server)
              ? tMcp('actions.start')
              : tMcp('messages.commandUnavailable', { serverId: server.id })
          }
          aria-label={
            canStartServer(server)
              ? tMcp('actions.start')
              : tMcp('messages.commandUnavailable', { serverId: server.id })
          }
        >
          <Play size={14} />
        </IconButton>
      ) : (
        <IconButton
          size="small"
          variant="warning"
          onClick={() => handleStopServer(server.id)}
          tooltip={tMcp('actions.stop')}
          aria-label={tMcp('actions.stop')}
        >
          <Square size={14} />
        </IconButton>
      )}
      <IconButton
        size="small"
        variant="ghost"
        onClick={() => handleRestartServer(server)}
        tooltip={
          canStartServer(server)
            ? tMcp('actions.restart')
            : tMcp('messages.commandUnavailable', { serverId: server.id })
        }
        aria-label={
          canStartServer(server)
            ? tMcp('actions.restart')
            : tMcp('messages.commandUnavailable', { serverId: server.id })
        }
      >
        <RefreshCw size={14} />
      </IconButton>
    </>
  );

  const renderServerDetails = (server: MCPServerInfo) => {
    if (!server.statusMessage && !isCommandDrivenServer(server) && !isRemoteServer(server)) return null;

    return (
      <div className="bitfun-mcp-tools__server-details">
        <div className="bitfun-mcp-tools__server-detail-item">
          <span className="bitfun-mcp-tools__server-detail-label">
            {tMcp('server.transport')}:
          </span>
          <code className="bitfun-mcp-tools__server-detail-value">{server.transport}</code>
        </div>
        {server.statusMessage && (
          <div className="bitfun-mcp-tools__server-detail-item">
            <span className="bitfun-mcp-tools__server-detail-label">
              {tMcp('server.statusDetail')}:
            </span>
            <span className="bitfun-mcp-tools__server-detail-value">
              {server.statusMessage}
            </span>
          </div>
        )}
        {server.startDisabledReason && (
          <div className="bitfun-mcp-tools__server-detail-item">
            <span className="bitfun-mcp-tools__server-detail-label">
              {tMcp('server.runtime.unsupportedReason')}:
            </span>
            <span className="bitfun-mcp-tools__server-detail-value">
              {getStartDisabledReasonLabel(server)}
            </span>
          </div>
        )}
        {isRemoteServer(server) && (
          <>
            <div className="bitfun-mcp-tools__server-detail-item">
              <span className="bitfun-mcp-tools__server-detail-label">
                {tMcp('server.remoteUrl')}:
              </span>
              <code className="bitfun-mcp-tools__server-detail-value">
                {server.url || '-'}
              </code>
            </div>
            <div className="bitfun-mcp-tools__server-detail-item">
              <span className="bitfun-mcp-tools__server-detail-label">
                {tMcp('server.remoteAuth')}:
              </span>
              <span className="bitfun-mcp-tools__server-detail-value">
                {getRemoteAuthSummary(server)}
              </span>
            </div>
            {(server.oauthEnabled || server.xaaEnabled) && (
              <div className="bitfun-mcp-tools__server-detail-item">
                <span className="bitfun-mcp-tools__server-detail-label">
                  {tMcp('server.remoteAuthMethod')}:
                </span>
                <span className="bitfun-mcp-tools__server-detail-value">
                  {getRemoteAuthMethodLabel(server)}
                </span>
              </div>
            )}
          </>
        )}
        {!isCommandDrivenServer(server) ? null : (
          <>
        <div className="bitfun-mcp-tools__server-detail-item">
          <span className="bitfun-mcp-tools__server-detail-label">
            {tMcp('server.command')}:
          </span>
          <code className="bitfun-mcp-tools__server-detail-value">
            {server.command || '-'}
          </code>
        </div>
        <div className="bitfun-mcp-tools__server-detail-item">
          <span className="bitfun-mcp-tools__server-detail-label">
            {tMcp('server.runtime.source')}:
          </span>
          <span className="bitfun-mcp-tools__server-detail-value">
            {getRuntimeSourceLabel(server)}
          </span>
        </div>
        {server.commandResolvedPath && (
          <div className="bitfun-mcp-tools__server-detail-item">
            <span className="bitfun-mcp-tools__server-detail-label">
              {tMcp('server.runtime.path')}:
            </span>
            <code className="bitfun-mcp-tools__server-detail-value">
              {server.commandResolvedPath}
            </code>
          </div>
        )}
          </>
        )}
      </div>
    );
  };

  return (
    <ConfigPageLayout className="bitfun-mcp-tools">
      <ConfigPageHeader
        title={tPage('title')}
        subtitle={desktopConfigAvailable ? tPage('subtitle') : tMcp('subtitleReadOnly')}
      />

      <ConfigPageContent>
        <ConfigPageSection
          title={tMcp('section.serverList.title')}
          extra={desktopConfigAvailable ? mcpSectionExtra : undefined}
        >
          {!desktopConfigAvailable && (
            <div className="bitfun-collection-empty" data-testid="mcp-management-unavailable">
              <p>{tMcp(remoteConnectionActive
                ? 'section.serverList.remoteUnavailable'
                : 'section.serverList.desktopUnavailable')}</p>
            </div>
          )}

          {desktopConfigAvailable && showJsonEditor && jsonLoading && (
            <div className="bitfun-collection-empty">
              <p>{tMcp('loading')}</p>
            </div>
          )}

          {desktopConfigAvailable && showJsonEditor && !jsonLoading && jsonLoadFailed && (
            <div className="bitfun-collection-empty" role="status">
              <p>{tMcp('jsonEditor.loadFailed')}</p>
              <IconButton
                variant="ghost"
                size="small"
                onClick={() => void loadJsonConfig()}
                tooltip={tMcp('actions.refresh')}
                aria-label={tMcp('actions.refresh')}
              >
                <RefreshCw size={16} aria-hidden="true" />
              </IconButton>
            </div>
          )}

          {desktopConfigAvailable && showJsonEditor && !jsonLoading && !jsonLoadFailed && (
            <div className="bitfun-mcp-tools__json-editor">
              <div className="bitfun-mcp-tools__json-editor-header">
                <h3>{tMcp('jsonEditor.title')}</h3>
                <p className="bitfun-mcp-tools__json-hint">{tMcp('jsonEditor.hint1')}</p>
                <p className="bitfun-mcp-tools__json-hint">{tMcp('jsonEditor.hint2')}</p>
              </div>
              <Textarea
                ref={jsonEditorRef}
                value={jsonConfig}
                onChange={(e) => setJsonConfig(e.target.value)}
                onKeyDown={handleJsonEditorKeyDown}
                onPaste={handleJsonEditorPaste}
                rows={18}
                placeholder={`{\n  "mcpServers": {\n    "server-name": {\n      "command": "npx",\n      "args": ["-y", "@package/name"],\n      "env": {}\n    }\n  }\n}`}
                variant="outlined"
                className="bitfun-mcp-tools__json-textarea"
                spellCheck={false}
                error={!!jsonLintError}
                errorMessage={
                  jsonLintError
                    ? tMcp('jsonEditor.lintError', {
                        location:
                          typeof jsonLintError.line === 'number' && typeof jsonLintError.column === 'number'
                            ? tMcp('jsonEditor.lintLocation', {
                                line: jsonLintError.line,
                                column: jsonLintError.column,
                              })
                            : '',
                        message: jsonLintError.message,
                      })
                    : undefined
                }
              />
              <div className="bitfun-mcp-tools__json-actions">
                <Button variant="secondary" onClick={() => setShowJsonEditor(false)}>
                  {tMcp('actions.cancel')}
                </Button>
                <Button variant="primary" onClick={handleSaveJsonConfig}>
                  {tMcp('actions.saveConfig')}
                </Button>
              </div>
              <div className="bitfun-mcp-tools__json-examples">
                <h4>{tMcp('jsonEditor.exampleTitle')}</h4>
                <div className="bitfun-mcp-tools__example">
                  <h5>{tMcp('jsonEditor.localProcess')}</h5>
                  <pre>{`{\n  "mcpServers": {\n    "zai-mcp-server": {\n      "command": "npx",\n      "args": ["-y", "@z_ai/mcp-server"],\n      "env": { "Z_AI_API_KEY": "your_api_key" }\n    }\n  }\n}`}</pre>
                </div>
                <div className="bitfun-mcp-tools__example">
                  <h5>{tMcp('jsonEditor.remoteService')}</h5>
                  <pre>{`{\n  "mcpServers": {\n    "remote-mcp": {\n      "url": "http://localhost:3000/sse"\n    }\n  }\n}`}</pre>
                </div>
              </div>
            </div>
          )}

          {desktopConfigAvailable && !showJsonEditor && mcpLoading && (
            <div className="bitfun-collection-empty">
              <p>{tMcp('loading')}</p>
            </div>
          )}

          {desktopConfigAvailable && !showJsonEditor && !mcpLoading
            && serverLoadFailed && servers.length === 0 && (
            <div className="bitfun-collection-empty" role="status">
              <p>{tMcp('section.serverList.loadFailed')}</p>
            </div>
          )}

          {desktopConfigAvailable && !showJsonEditor && !mcpLoading
            && !serverLoadFailed && servers.length === 0 && (
            <div className="bitfun-collection-empty">
              <Button variant="dashed" size="small" onClick={() => setShowJsonEditor(true)}>
                <FileJson size={14} />
                {tMcp('actions.jsonConfig')}
              </Button>
            </div>
          )}

          {desktopConfigAvailable && !showJsonEditor &&
            servers.map((server) => (
              <ConfigCollectionItem
                key={server.id}
                label={server.name}
                badge={renderServerBadge(server)}
                control={renderServerControl(server)}
                details={renderServerDetails(server)}
              />
            ))}
        </ConfigPageSection>

        <ExternalMcpOverview />
      </ConfigPageContent>
      <Modal
        isOpen={desktopConfigAvailable && !!authDialogServer}
        onClose={handleCloseAuthDialog}
        title={
          authDialogServer
            ? tMcp('modal.remoteAuthTitle', { serverName: authDialogServer.name })
            : tMcp('actions.remoteAuth')
        }
        size="medium"
        showCloseButton={!authSubmitting && !oauthCancelling}
      >
        {authDialogServer && (
          <div className="bitfun-mcp-tools__json-editor">
            {authDialogServer.oauthEnabled && (
              <>
                <p className="bitfun-mcp-tools__json-hint">
                  {tMcp('modal.remoteOAuthHint')}
                </p>
                <p className="bitfun-mcp-tools__json-hint">
                  {tMcp('modal.remoteOAuthCurrentStatus', {
                    status: getOAuthStatusLabel(oauthSession),
                  })}
                </p>
                {oauthSession?.redirectUri && (
                  <p className="bitfun-mcp-tools__json-hint">
                    {tMcp('modal.remoteOAuthRedirectUri', {
                      redirectUri: oauthSession.redirectUri,
                    })}
                  </p>
                )}
                {oauthSession?.message && (
                  <p className="bitfun-mcp-tools__json-hint">
                    {tMcp('modal.remoteOAuthStatus', {
                      status: getOAuthStatusLabel(oauthSession),
                      message: oauthSession.message,
                    })}
                  </p>
                )}
                <div className="bitfun-mcp-tools__json-actions">
                  <Button
                    variant="primary"
                    onClick={handleStartRemoteOAuth}
                    isLoading={oauthStarting}
                    disabled={authSubmitting || oauthCancelling}
                  >
                    {getOAuthActionLabel(authDialogServer)}
                  </Button>
                </div>
              </>
            )}
            <p className="bitfun-mcp-tools__json-hint">
              {tMcp('modal.remoteAuthHint')}
            </p>
            {authDialogServer.url && (
              <p className="bitfun-mcp-tools__json-hint">
                {tMcp('modal.remoteAuthServerUrl', {
                  url: authDialogServer.url,
                })}
              </p>
            )}
            <Textarea
              value={authValue}
              onChange={(e) => setAuthValue(e.target.value)}
              rows={4}
              placeholder={tMcp('modal.remoteAuthPlaceholder')}
              variant="outlined"
              className="bitfun-mcp-tools__json-textarea"
              spellCheck={false}
            />
            <div className="bitfun-mcp-tools__json-actions">
              <Button
                variant="secondary"
                onClick={handleCloseAuthDialog}
                disabled={authSubmitting || oauthStarting || oauthCancelling}
              >
                {isOAuthFlowActive
                  ? tMcp('actions.cancelRemoteOAuth')
                  : tMcp('actions.cancel')}
              </Button>
              <Button
                variant="primary"
                onClick={handleSaveRemoteAuth}
                isLoading={authSubmitting}
                disabled={oauthStarting || oauthCancelling}
              >
                {tMcp('actions.saveRemoteAuth')}
              </Button>
            </div>
          </div>
        )}
      </Modal>
    </ConfigPageLayout>
  );
};

export default McpToolsConfig;
