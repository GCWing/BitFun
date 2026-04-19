/**
 * Standalone chat input component
 * Separated from bottom bar, supports session-level state awareness
 */

import React, { useRef, useCallback, useEffect, useReducer, useState, useMemo } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { ArrowUp, Image, Maximize2, Minimize2, RotateCcw, Plus, X, Sparkles, Loader2, ChevronRight, Files, MessageSquarePlus } from 'lucide-react';
import { ContextDropZone, useContextStore } from '../../shared/context-system';
import { useActiveSessionState } from '../hooks/useActiveSessionState';
import { RichTextInput, type MentionState } from './RichTextInput';
import { FileMentionPicker } from './FileMentionPicker';
import { globalEventBus } from '../../infrastructure/event-bus';
import {
  useSessionDerivedState,
  useSessionStateMachine,
  useSessionStateMachineActions,
} from '../hooks/useSessionStateMachine';
import { SessionExecutionEvent } from '../state-machine/types';
import { ModelSelector } from './ModelSelector';
import { FlowChatStore } from '../store/FlowChatStore';
import type { FlowChatState } from '../types/flow-chat';
import type { FileContext, DirectoryContext } from '../../shared/types/context';
import { SmartRecommendations } from './smart-recommendations';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { WorkspaceKind } from '@/shared/types';
import { createImageContextFromFile, createImageContextFromClipboard } from '../utils/imageUtils';
import { notificationService } from '@/shared/notification-system';
import { inputReducer, initialInputState } from '../reducers/inputReducer';
import { modeReducer, initialModeState } from '../reducers/modeReducer';
import { CHAT_INPUT_CONFIG } from '../constants/chatInputConfig';
import { useMessageSender } from '../hooks/useMessageSender';
import { useChatInputState } from '../store/chatInputStateStore';
import { useInputHistoryStore } from '../store/inputHistoryStore';
import { startBtwThread } from '../services/BtwThreadService';
import { FlowChatManager } from '../services/FlowChatManager';
import { createLogger } from '@/shared/utils/logger';
import { Tooltip, IconButton } from '@/component-library';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import { openBtwSessionInAuxPane, selectActiveBtwSessionTab } from '../services/openBtwSession';
import { resolveSessionRelationship } from '../utils/sessionMetadata';
import { useOverlayStore } from '@/app/stores/overlayStore';
import type { OverlaySceneId } from '@/app/overlay/types';
import type { SkillInfo } from '@/infrastructure/config/types';
import { aiExperienceConfigService } from '@/infrastructure/config/services/AIExperienceConfigService';
import MCPAPI, { type MCPPrompt, type MCPPromptMessage, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { deriveChatInputPetMood } from '../utils/chatInputPetMood';
import { ChatInputPixelPet } from './ChatInputPixelPet';
import './ChatInput.scss';

const log = createLogger('ChatInput');
const IME_ENTER_GUARD_MS = 120;

export interface ChatInputProps {
  className?: string;
  onSendMessage?: (message: string) => void;
}

type SlashActionItem = {
  kind: 'action';
  id: string;
  command: string;
  label: string;
};

type SlashModeItem = {
  kind: 'mode';
  id: string;
  name: string;
};

type SlashMcpPromptItem = {
  kind: 'mcpPrompt';
  id: string;
  command: string;
  label: string;
  serverId: string;
  serverName: string;
  promptName: string;
  description?: string;
  arguments: Array<{
    name: string;
    required: boolean;
    description?: string;
  }>;
};

type SlashPickerItem = SlashActionItem | SlashModeItem | SlashMcpPromptItem;
type ChatInputTarget = 'main' | 'btw';
type PendingLargePasteMap = Record<string, string>;

function getCharacterCount(text: string): number {
  return Array.from(text).length;
}

function buildMcpPromptSlashCommand(serverId: string, promptName: string): string {
  return `/${serverId}:${promptName}`;
}

function parseSlashArguments(input: string): string[] {
  const matches = input.match(/"([^"]*)"|'([^']*)'|[^\s]+/g) || [];
  return matches.map(token => {
    if (
      (token.startsWith('"') && token.endsWith('"')) ||
      (token.startsWith('\'') && token.endsWith('\''))
    ) {
      return token.slice(1, -1);
    }
    return token;
  });
}

function renderMcpPromptContent(content: unknown): string {
  if (typeof content === 'string') {
    return content;
  }

  if (!content || typeof content !== 'object') {
    return '[Unsupported MCP prompt content]';
  }

  const block = content as Record<string, unknown>;
  const type = typeof block.type === 'string' ? block.type : undefined;

  if (type === 'text' && typeof block.text === 'string') {
    return block.text;
  }

  if (type === 'image') {
    return `[Image${typeof block.mimeType === 'string' ? `: ${block.mimeType}` : ''}]`;
  }

  if (type === 'audio') {
    return `[Audio${typeof block.mimeType === 'string' ? `: ${block.mimeType}` : ''}]`;
  }

  if (type === 'resource_link') {
    const uri = typeof block.uri === 'string' ? block.uri : 'unknown';
    const name = typeof block.name === 'string' ? block.name : undefined;
    return name ? `[Resource Link: ${name} (${uri})]` : `[Resource Link: ${uri}]`;
  }

  if (type === 'resource' && block.resource && typeof block.resource === 'object') {
    const resource = block.resource as Record<string, unknown>;
    const resourceText =
      typeof resource.text === 'string'
        ? resource.text
        : typeof resource.content === 'string'
          ? resource.content
          : undefined;
    if (resourceText) {
      return resourceText;
    }
    const uri = typeof resource.uri === 'string' ? resource.uri : 'unknown';
    return `[Resource: ${uri}]`;
  }

  return '[Unsupported MCP prompt content]';
}

function renderMcpPromptMessages(messages: MCPPromptMessage[]): string {
  return messages
    .map(message => {
      const text = renderMcpPromptContent(message.content).trim();
      if (!text) {
        return '';
      }

      switch (message.role) {
        case 'system':
          return text;
        case 'user':
          return `User: ${text}`;
        case 'assistant':
          return `Assistant: ${text}`;
        default:
          return `${message.role}: ${text}`;
      }
    })
    .filter(Boolean)
    .join('\n\n');
}

export const ChatInput: React.FC<ChatInputProps> = ({
  className = '',
  onSendMessage
}) => {
  const { t } = useTranslation('flow-chat');
  
  const [inputState, dispatchInput] = useReducer(inputReducer, initialInputState);
  const [modeState, dispatchMode] = useReducer(modeReducer, initialModeState);
  
  const richTextInputRef = useRef<HTMLDivElement>(null);
  const agentBoostRef = useRef<HTMLDivElement>(null);
  const isImeComposingRef = useRef(false);
  const lastImeCompositionEndAtRef = useRef(0);
  // Ref so the queuedInput sync effect can read the latest value without it being a dep
  const inputValueRef = useRef('');
  const pendingLargePastesRef = useRef<PendingLargePasteMap>({});
  const largePasteCountersRef = useRef<Record<number, number>>({});
  
  // History navigation state
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [savedDraft, setSavedDraft] = useState('');
  const [inputTarget, setInputTarget] = useState<ChatInputTarget>('main');
  const { addMessage: addToHistory, getSessionHistory } = useInputHistoryStore();
  
  const contexts = useContextStore(state => state.contexts);
  const addContext = useContextStore(state => state.addContext);
  const removeContext = useContextStore(state => state.removeContext);
  const clearContexts = useContextStore(state => state.clearContexts);

  const currentImageCount = useMemo(
    () => contexts.filter(c => c.type === 'image').length,
    [contexts],
  );
  
  const activeSessionState = useActiveSessionState();
  const activeBtwSessionTab = useAgentCanvasStore(state => selectActiveBtwSessionTab(state as any));
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => FlowChatStore.getInstance().getState());
  const currentSessionId = activeSessionState.sessionId;
  const currentSession = currentSessionId ? flowChatState.sessions.get(currentSessionId) : undefined;
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;
  const activeBtwSessionId = activeBtwSessionData?.parentSessionId === currentSessionId
    ? activeBtwSessionData.childSessionId
    : undefined;
  const effectiveTargetSessionId =
    inputTarget === 'btw' && activeBtwSessionId ? activeBtwSessionId : currentSessionId;
  const effectiveTargetSession = effectiveTargetSessionId
    ? flowChatState.sessions.get(effectiveTargetSessionId)
    : undefined;
  const isBtwSession = resolveSessionRelationship(effectiveTargetSession).isBtw;
  const showTargetSwitcher = !!activeBtwSessionId;
  const currentSessionTitle = currentSession?.title?.trim() || t('session.untitled');
  const activeBtwSessionTitle = activeBtwSessionId
    ? flowChatState.sessions.get(activeBtwSessionId)?.title?.trim() || t('btw.threadLabel')
    : '';
  
  // Memoize history so keyboard handlers don't see a fresh [] on every render.
  const inputHistory = useMemo(
    () => (effectiveTargetSessionId ? getSessionHistory(effectiveTargetSessionId) : []),
    [effectiveTargetSessionId, getSessionHistory],
  );
  const derivedState = useSessionDerivedState(
    effectiveTargetSessionId,
    inputState.value.trim()
  );
  const sessionMachineSnapshot = useSessionStateMachine(effectiveTargetSessionId);
  const petMood = useMemo(
    () => deriveChatInputPetMood(sessionMachineSnapshot),
    [sessionMachineSnapshot],
  );
  const [agentCompanionEnabled, setAgentCompanionEnabled] = useState(
    () => aiExperienceConfigService.getSettings().enable_agent_companion,
  );
  useEffect(() => {
    setAgentCompanionEnabled(aiExperienceConfigService.getSettings().enable_agent_companion);
    return aiExperienceConfigService.addChangeListener(settings => {
      setAgentCompanionEnabled(settings.enable_agent_companion);
    });
  }, []);
  const showCollapsedPet =
    agentCompanionEnabled && !inputState.isActive && !inputState.value.trim();
  const { transition, setQueuedInput } = useSessionStateMachineActions(effectiveTargetSessionId);

  const { workspace, workspacePath } = useCurrentWorkspace();
  
  const [tokenUsage, setTokenUsage] = React.useState({ current: 0, max: 128128 });
  const isAssistantWorkspace = workspace?.workspaceKind === WorkspaceKind.Assistant;
  const currentMode = modeState.current;
  // Cowork, Design, and Dispatcher sessions have a fixed agent type and do not support mode switching.
  const canSwitchModes =
    !isAssistantWorkspace &&
    currentMode !== 'Cowork' &&
    currentMode !== 'Design' &&
    currentMode !== 'Dispatcher';

  // Session-level mode policy: fixed-purpose sessions are not available as incremental mode switches.
  const switchableModes = useMemo(
    () =>
      modeState.available.filter(mode =>
        mode.enabled &&
        mode.id !== 'Cowork' &&
        mode.id !== 'Design' &&
        (isAssistantWorkspace || mode.id !== 'Claw')
      ),
    [isAssistantWorkspace, modeState.available]
  );

  /** Code session: only Plan and debug are optional on top of default agentic */
  const incrementalCodeModes = useMemo(
    () => switchableModes.filter(m => m.id === 'Plan' || m.id === 'debug'),
    [switchableModes]
  );

  const openOverlay = useOverlayStore(s => s.openOverlay);
  const [boostPanelSkills, setBoostPanelSkills] = useState<SkillInfo[]>([]);
  const [boostSkillsLoading, setBoostSkillsLoading] = useState(false);

  const [skillsFlyoutOpen, setSkillsFlyoutOpen] = useState(false);
  const [skillsFlyoutLeft, setSkillsFlyoutLeft] = useState(false);
  const [skillsFlyoutUp, setSkillsFlyoutUp] = useState(false);
  const skillsHostRef = useRef<HTMLDivElement>(null);
  const skillsTimerRef = useRef<number | null>(null);

  const clearSkillsTimer = useCallback(() => {
    if (skillsTimerRef.current !== null) {
      window.clearTimeout(skillsTimerRef.current);
      skillsTimerRef.current = null;
    }
  }, []);

  const openSkillsFlyout = useCallback(() => {
    clearSkillsTimer();
    const host = skillsHostRef.current;
    if (host) {
      const r = host.getBoundingClientRect();
      setSkillsFlyoutLeft(r.right + 260 > window.innerWidth - 8);
      setSkillsFlyoutUp(r.top + 200 > window.innerHeight - 8);
    }
    setSkillsFlyoutOpen(true);
  }, [clearSkillsTimer]);

  const closeSkillsFlyout = useCallback(() => {
    clearSkillsTimer();
    skillsTimerRef.current = window.setTimeout(() => {
      skillsTimerRef.current = null;
      setSkillsFlyoutOpen(false);
    }, 150);
  }, [clearSkillsTimer]);
  
  const setChatInputActive = useChatInputState(state => state.setActive);
  const setChatInputExpanded = useChatInputState(state => state.setExpanded);
  const setChatInputHeight = useChatInputState(state => state.setInputHeight);

  useEffect(() => {
    const unsubscribe = FlowChatStore.getInstance().subscribe(setFlowChatState);
    return () => unsubscribe();
  }, []);

  useEffect(() => {
    if (!showTargetSwitcher || !activeBtwSessionId) {
      setInputTarget('main');
    }
  }, [activeBtwSessionId, showTargetSwitcher]);

  useEffect(() => {
    setChatInputActive(inputState.isActive);
  }, [inputState.isActive, setChatInputActive]);
  
  useEffect(() => {
    setChatInputExpanded(inputState.isExpanded);
  }, [inputState.isExpanded, setChatInputExpanded]);
  
  // Reset history index when switching sessions
  useEffect(() => {
    setHistoryIndex(-1);
  }, [effectiveTargetSessionId]);
  
  const { sendMessage } = useMessageSender({
    currentSessionId: effectiveTargetSessionId || undefined,
    contexts,
    onClearContexts: clearContexts,
    onSuccess: onSendMessage,
    currentAgentType: effectiveTargetSession?.mode || modeState.current,
  });

  const [mcpPromptCommands, setMcpPromptCommands] = useState<SlashMcpPromptItem[]>([]);
  const [mcpPromptCommandsLoading, setMcpPromptCommandsLoading] = useState(false);

  const loadMcpPromptCommands = useCallback(async () => {
    setMcpPromptCommandsLoading(true);

    try {
      const servers = await MCPAPI.getServers();
      const connectedServers = servers.filter(
        server => server.status === 'Connected' || server.status === 'Healthy'
      );

      const promptGroups = await Promise.all(
        connectedServers.map(async (server: MCPServerInfo) => {
          try {
            const prompts = await MCPAPI.listPrompts({
              serverId: server.id,
              refresh: true,
            });
            return prompts.map((prompt: MCPPrompt) => ({
              kind: 'mcpPrompt' as const,
              id: `${server.id}:${prompt.name}`,
              command: buildMcpPromptSlashCommand(server.id, prompt.name),
              label:
                prompt.description?.trim() ||
                `${server.name} MCP prompt`,
              serverId: server.id,
              serverName: server.name,
              promptName: prompt.name,
              description: prompt.description,
              arguments: (prompt.arguments || []).map(argument => ({
                name: argument.name,
                required: argument.required,
                description: argument.description,
              })),
            }));
          } catch (error) {
            log.warn('Failed to load MCP prompts for server', {
              serverId: server.id,
              error,
            });
            return [] as SlashMcpPromptItem[];
          }
        })
      );

      setMcpPromptCommands(
        promptGroups
          .flat()
          .sort((a, b) => a.command.localeCompare(b.command))
      );
    } finally {
      setMcpPromptCommandsLoading(false);
    }
  }, []);
  
  const [recommendationContext, setRecommendationContext] = React.useState<{
    workspacePath?: string;
    sessionId?: string;
    turnIndex?: number;
    modifiedFiles?: string[];
  } | null>(null);
  
  const [mentionState, setMentionState] = useState<MentionState>({
    isActive: false,
    query: '',
    startOffset: 0,
  });
  
  const [slashCommandState, setSlashCommandState] = useState<{
    isActive: boolean;
    kind: 'modes' | 'actions' | 'all';
    query: string;
    selectedIndex: number;
  }>({
    isActive: false,
    kind: 'modes',
    query: '',
    selectedIndex: 0,
  });

  const clearPendingLargePastes = useCallback(() => {
    pendingLargePastesRef.current = {};
  }, []);

  const createLargePastePlaceholder = useCallback((text: string): string | null => {
    const charCount = getCharacterCount(text);
    if (charCount <= CHAT_INPUT_CONFIG.largePaste.thresholdChars) {
      return null;
    }

    const nextCounters = largePasteCountersRef.current;
    const nextSuffix = (nextCounters[charCount] ?? 0) + 1;
    nextCounters[charCount] = nextSuffix;

    const base = t('input.largePastePlaceholder', {
      count: charCount,
      defaultValue: '[Pasted Content {{count}} chars]',
    });
    const placeholder = nextSuffix === 1 ? base : `${base} #${nextSuffix}`;

    pendingLargePastesRef.current = {
      ...pendingLargePastesRef.current,
      [placeholder]: text,
    };

    return placeholder;
  }, [t]);

  const prunePendingLargePastes = useCallback((text: string) => {
    const entries = Object.entries(pendingLargePastesRef.current);
    if (entries.length === 0) {
      return;
    }

    pendingLargePastesRef.current = Object.fromEntries(
      entries.filter(([placeholder]) => text.includes(placeholder))
    );
  }, []);

  const expandPendingLargePastes = useCallback((text: string) => {
    let expanded = text;
    for (const [placeholder, actual] of Object.entries(pendingLargePastesRef.current)) {
      if (expanded.includes(placeholder)) {
        expanded = expanded.split(placeholder).join(actual);
      }
    }
    return expanded;
  }, []);

  React.useEffect(() => {
    if (inputState.value === '') {
      clearPendingLargePastes();
    }
  }, [clearPendingLargePastes, inputState.value]);
  
  React.useEffect(() => {
    const store = FlowChatStore.getInstance();
    
    const unsubscribe = store.subscribe((state: FlowChatState) => {
      if (effectiveTargetSessionId) {
        const session = state.sessions.get(effectiveTargetSessionId);
        if (session) {
          setTokenUsage({
            current: session.currentTokenUsage?.totalTokens || 0,
            max: session.maxContextTokens || 128128
          });
        }
      }
    });

    if (effectiveTargetSessionId) {
      const state = store.getState();
      const session = state.sessions.get(effectiveTargetSessionId);
      if (session) {
        setTokenUsage({
          current: session.currentTokenUsage?.totalTokens || 0,
          max: session.maxContextTokens || 128128
        });
      }
    }

    return () => unsubscribe();
  }, [effectiveTargetSessionId]);

  React.useEffect(() => {
    const handleFillInput = (event: Event) => {
      const customEvent = event as CustomEvent<{ message: string }>;
      const message = customEvent.detail?.message;
      
      if (message) {
        clearPendingLargePastes();
        dispatchInput({ type: 'ACTIVATE' });
        dispatchInput({ type: 'SET_VALUE', payload: message });
        
        if (richTextInputRef.current) {
          richTextInputRef.current.focus();
        }
      }
    };

    window.addEventListener('fill-chat-input', handleFillInput);
    
    return () => {
      window.removeEventListener('fill-chat-input', handleFillInput);
    };
  }, [clearPendingLargePastes]);

  React.useEffect(() => {
    const handleFillChatInput = (data: { content: string }) => {
      clearPendingLargePastes();
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: data.content });

      if (richTextInputRef.current) {
        richTextInputRef.current.focus();
      }
    };

    globalEventBus.on('fill-chat-input', handleFillChatInput);

    return () => {
      globalEventBus.off('fill-chat-input', handleFillChatInput);
    };
  }, [clearPendingLargePastes]);

  React.useEffect(() => {
    if (!slashCommandState.isActive || slashCommandState.kind !== 'all' || derivedState?.isProcessing) {
      return;
    }

    void loadMcpPromptCommands();
  }, [derivedState?.isProcessing, loadMcpPromptCommands, slashCommandState.isActive, slashCommandState.kind]);

  // Handle MCP App ui/message requests (aligned with VSCode behavior)
  React.useEffect(() => {
    const handleMcpAppMessage = async (event: import('@/infrastructure/api/service-api/MCPAPI').McpAppMessageEvent) => {
      const { requestId, params } = event;

      // Don't fill if input already has content (aligned with VSCode behavior)
      if (inputState.value.trim()) {
        log.warn('MCP App ui/message rejected: input already has content');
        // Send error response (VSCode returns { isError: true } in this case)
        globalEventBus.emit('mcp-app:message-response', {
          requestId,
          result: { isError: true }
        } as import('@/infrastructure/api/service-api/MCPAPI').McpAppMessageResponseEvent);
        return;
      }

      try {
        // Extract text content and set input
        const textContent = params.content
          .filter(c => c.type === 'text')
          .map(c => c.text)
          .join('\n\n');

        if (textContent) {
          clearPendingLargePastes();
          dispatchInput({ type: 'ACTIVATE' });
          dispatchInput({ type: 'SET_VALUE', payload: textContent });
        }

        // Handle image attachments (respect max image limit)
        let imgCount = currentImageCount;
        for (const block of params.content) {
          if (block.type === 'image') {
            if (imgCount >= CHAT_INPUT_CONFIG.image.maxCount) break;
            try {
              const mimeType = block.mimeType || 'image/png';
              const binaryString = atob(block.data);
              const bytes = new Uint8Array(binaryString.length);
              for (let i = 0; i < binaryString.length; i++) {
                bytes[i] = binaryString.charCodeAt(i);
              }
              const blob = new Blob([bytes], { type: mimeType });
              const file = new File([blob], `image.${mimeType.split('/')[1] || 'png'}`, { type: mimeType });
              const imageContext = await createImageContextFromClipboard(file);
              addContext(imageContext);
              imgCount++;
            } catch (err) {
              log.error('Failed to add image from MCP App message', { err });
            }
          }
        }

        // Focus input
        if (richTextInputRef.current) {
          richTextInputRef.current.focus();
        }

        // Send success response
        globalEventBus.emit('mcp-app:message-response', {
          requestId,
          result: { isError: false }
        } as import('@/infrastructure/api/service-api/MCPAPI').McpAppMessageResponseEvent);
      } catch (err) {
        log.error('Failed to handle MCP App ui/message', { err });
        // Send error response
        globalEventBus.emit('mcp-app:message-response', {
          requestId,
          result: { isError: true }
        } as import('@/infrastructure/api/service-api/MCPAPI').McpAppMessageResponseEvent);
      }
    };

    globalEventBus.on('mcp-app:message', handleMcpAppMessage);

    return () => {
      globalEventBus.off('mcp-app:message', handleMcpAppMessage);
    };
  }, [inputState.value, addContext, clearPendingLargePastes, currentImageCount]);

  React.useEffect(() => {
    const handleInsertContextTag = (event: Event) => {
      const customEvent = event as CustomEvent<{ context: any }>;
      const context = customEvent.detail?.context;
      
      if (context) {
        if (!inputState.isActive) {
          dispatchInput({ type: 'ACTIVATE' });
        }

        setTimeout(() => {
          if (richTextInputRef.current && (richTextInputRef.current as any).insertTag) {
            const el = richTextInputRef.current;
            if (!el.textContent?.trim() && !el.querySelector('[data-context-id]')) {
              el.innerHTML = '';
            }
            el.focus();
            const sel = window.getSelection();
            if (sel) {
              sel.selectAllChildren(el);
              sel.collapseToEnd();
            }
            (el as any).insertTag(context);
          }
        }, 50);
      }
    };

    window.addEventListener('insert-context-tag', handleInsertContextTag);
    
    return () => {
      window.removeEventListener('insert-context-tag', handleInsertContextTag);
    };
  }, [inputState.isActive]);

  React.useEffect(() => {
    const fetchAvailableModes = async () => {
      try {
        const { agentAPI } = await import('@/infrastructure/api/service-api/AgentAPI');
        const modes = await agentAPI.getAvailableModes();
        dispatchMode({ type: 'SET_AVAILABLE_MODES', payload: modes });
      } catch (error) {
        log.error('Failed to fetch available modes', { error });
      }
    };
    
    fetchAvailableModes();
    
    const handleModeConfigUpdated = () => {
      fetchAvailableModes();
    };
    
    globalEventBus.on('mode:config:updated', handleModeConfigUpdated);
    
    return () => {
      globalEventBus.off('mode:config:updated', handleModeConfigUpdated);
    };
  }, []);

  React.useEffect(() => {
    const handleSessionSwitched = (event: Event) => {
      const customEvent = event as CustomEvent<{ sessionId: string; mode: string }>;
      const { sessionId, mode } = customEvent.detail || {};
      
      if (sessionId && mode) {
        log.debug('Session switched, syncing mode', { sessionId, mode });
        dispatchMode({ type: 'SET_CURRENT_MODE', payload: mode });
        try {
          sessionStorage.setItem('bitfun:flowchat:lastMode', mode);
        } catch {
          // ignore
        }
      }
    };

    window.addEventListener('bitfun:session-switched', handleSessionSwitched);
    
    return () => {
      window.removeEventListener('bitfun:session-switched', handleSessionSwitched);
    };
  }, []);

  React.useEffect(() => {
    if (!effectiveTargetSessionId) return;
    
    const store = FlowChatStore.getInstance();
    const state = store.getState();
    const session = state.sessions.get(effectiveTargetSessionId);
    
    if (session?.mode) {
      log.debug('Session ID changed, syncing mode', { sessionId: effectiveTargetSessionId, mode: session.mode });
      dispatchMode({ type: 'SET_CURRENT_MODE', payload: session.mode });
      try {
        sessionStorage.setItem('bitfun:flowchat:lastMode', session.mode);
      } catch {
        // ignore
      }
    }
  }, [effectiveTargetSessionId]);

  React.useEffect(() => {
    if (!isAssistantWorkspace || currentMode === 'Claw') {
      return;
    }

    dispatchMode({ type: 'SET_CURRENT_MODE', payload: 'Claw' });
  }, [currentMode, isAssistantWorkspace]);

  React.useEffect(() => {
    const queuedInput = derivedState?.queuedInput;
    if (!queuedInput?.trim() || !effectiveTargetSessionId) {
      return;
    }
    // Sync machine queue into the input (e.g. failed turn restored by EventHandlerModule).
    // `queuedInput` is cleared on successful send via `setQueuedInput(null)` so we do not fight CLEAR_VALUE.
    // Use inputValueRef (not inputState.value) so this effect only re-runs when the machine's
    // queuedInput actually changes — not on every keystroke — avoiding the race condition where
    // a stale queuedInput would overwrite what the user is currently typing.
    const currentValue = inputValueRef.current;
    if (currentValue !== queuedInput && !currentValue.trim()) {
      // Only restore when the input is empty: this effect is for failure-recovery
      // (EventHandlerModule sets queuedInput on failed turns), NOT for live typing.
      // Restoring while the user is actively typing would overwrite their draft.
      log.debug('Detected queuedInput, restoring message to input', { queuedInput });
      clearPendingLargePastes();
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: queuedInput });
      inputValueRef.current = queuedInput;
      if (richTextInputRef.current) {
        richTextInputRef.current.focus();
      }
    }
  }, [
    derivedState?.queuedInput,
    effectiveTargetSessionId,
    clearPendingLargePastes,
  ]);

  React.useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (agentBoostRef.current && !agentBoostRef.current.contains(event.target as Node)) {
        dispatchMode({ type: 'CLOSE_DROPDOWN' });
      }
    };

    if (modeState.dropdownOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [modeState.dropdownOpen]);

  useEffect(() => {
    if (!modeState.dropdownOpen) {
      return;
    }
    let cancelled = false;
    setBoostSkillsLoading(true);
    (async () => {
      try {
        const { configAPI } = await import('@/infrastructure/api');
        const list = await configAPI.getSkillConfigs({
          workspacePath: workspacePath || undefined,
        });
        if (!cancelled) {
          setBoostPanelSkills(list);
        }
      } catch (err) {
        log.error('Failed to load skills for boost panel', { err });
        if (!cancelled) setBoostPanelSkills([]);
      } finally {
        if (!cancelled) setBoostSkillsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [modeState.dropdownOpen, workspacePath]);

  useEffect(() => {
    if (!modeState.dropdownOpen) {
      clearSkillsTimer();
      setSkillsFlyoutOpen(false);
    }
  }, [clearSkillsTimer, modeState.dropdownOpen]);

  useEffect(
    () => () => {
      clearSkillsTimer();
    },
    [clearSkillsTimer]
  );

  useEffect(() => {
    const handleImagePaste = async (event: Event) => {
      const customEvent = event as CustomEvent<{ file: File }>;
      const file = customEvent.detail?.file;
      
      if (!file) return;

      if (currentImageCount >= CHAT_INPUT_CONFIG.image.maxCount) {
        notificationService.warning(t('input.maxImagesWarning', { count: CHAT_INPUT_CONFIG.image.maxCount }), { duration: 3000 });
        return;
      }
      
      try {
        const imageContext = await createImageContextFromClipboard(file);
        
        addContext(imageContext);
        
        if (richTextInputRef.current && (richTextInputRef.current as any).insertTag) {
          (richTextInputRef.current as any).insertTag(imageContext);
        }
        
        notificationService.success(
          t('input.imageAddedSingle', { name: imageContext.imageName }),
          { duration: 2000 }
        );
      } catch (error) {
        log.error('Failed to process clipboard image', { fileName: file.name, error });
        notificationService.error(
          `${t('input.imagePasteFailed')}: ${error instanceof Error ? error.message : t('error.unknown')}`,
          { duration: 3000 }
        );
      }
    };
    
    const inputElement = richTextInputRef.current;
    if (inputElement) {
      inputElement.addEventListener('imagePaste', handleImagePaste);
    }
    
    return () => {
      if (inputElement) {
        inputElement.removeEventListener('imagePaste', handleImagePaste);
      }
    };
  }, [addContext, currentImageCount, t]);

  React.useEffect(() => {
    if (!effectiveTargetSessionId || !workspacePath) {
      return;
    }

    const store = FlowChatStore.getInstance();
    const state = store.getState();
    const session = state.sessions.get(effectiveTargetSessionId);

    if (!session || session.dialogTurns.length === 0) {
      return;
    }

    const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
    
    if (lastTurn.status === 'completed') {
      const modifiedFiles: string[] = [];
      
      for (const round of lastTurn.modelRounds) {
        for (const item of round.items) {
          if (item.type === 'tool') {
            const toolItem = item as import('../types/flow-chat').FlowToolItem;
            const fileModifyTools = ['write_file', 'edit_file', 'create_file', 'delete_file'];
            if (fileModifyTools.includes(toolItem.toolName)) {
              const toolInput = toolItem.toolCall?.input;
              if (toolInput && typeof toolInput === 'object') {
                const filePath = (toolInput as any).file_path || (toolInput as any).path || (toolInput as any).filePath;
                if (filePath && typeof filePath === 'string') {
                  modifiedFiles.push(filePath);
                }
              }
            }
          }
        }
      }

      if (modifiedFiles.length > 0) {
        log.debug('File modifications detected, updating recommendation context', { modifiedFiles });
        setRecommendationContext({
          workspacePath,
          sessionId: effectiveTargetSessionId,
          turnIndex: session.dialogTurns.length - 1,
          modifiedFiles: [...new Set(modifiedFiles)]
        });
      }
    }
  }, [effectiveTargetSessionId, workspacePath, derivedState?.isProcessing]);

  const getFilteredActions = useCallback(() => {
    const items: SlashActionItem[] = [
      ...(isBtwSession
        ? []
        : [{
            kind: 'action' as const,
            id: 'btw',
            command: '/btw',
            label: t('btw.title', { defaultValue: 'Side question' }),
          }]),
      {
        kind: 'action',
        id: 'compact',
        command: '/compact',
        label: t('chatInput.compactAction', { defaultValue: 'Compact session' }),
      },
      {
        kind: 'action',
        id: 'init',
        command: '/init',
        label: t('chatInput.initAction', { defaultValue: 'Generate AGENTS.md' }),
      },
    ];

    const q = (slashCommandState.query || '').trim().toLowerCase();
    if (!q) return items;

    return items.filter(i => {
      const cmd = i.command.slice(1).toLowerCase();
      return cmd.includes(q) || i.label.toLowerCase().includes(q);
    });
  }, [isBtwSession, slashCommandState.query, t]);

  const getFilteredMcpPromptCommands = useCallback((): SlashMcpPromptItem[] => {
    const q = (slashCommandState.query || '').trim().toLowerCase();
    if (!q) {
      return mcpPromptCommands;
    }

    return mcpPromptCommands.filter(item => {
      const commandToken = item.command.slice(1).toLowerCase();
      return (
        commandToken.includes(q) ||
        item.serverName.toLowerCase().includes(q) ||
        item.label.toLowerCase().includes(q)
      );
    });
  }, [mcpPromptCommands, slashCommandState.query]);

  const resolveTypedMcpPromptCommand = useCallback((text: string): SlashMcpPromptItem | null => {
    const trimmed = text.trim();
    if (!trimmed.startsWith('/')) {
      return null;
    }

    const token = trimmed.slice(1).split(/\s+/, 1)[0]?.toLowerCase() || '';
    if (!token) {
      return null;
    }

    return (
      mcpPromptCommands.find(item => item.command.slice(1).toLowerCase() === token) || null
    );
  }, [mcpPromptCommands]);

  const getSlashPickerItems = useCallback((): SlashPickerItem[] => {
    const actions = getFilteredActions();
    const mcpPrompts = getFilteredMcpPromptCommands();
    let modeList = incrementalCodeModes;
    if (canSwitchModes && slashCommandState.query) {
      const q = slashCommandState.query;
      modeList = incrementalCodeModes.filter(
        mode =>
          mode.name.toLowerCase().includes(q) ||
          mode.id.toLowerCase().includes(q)
      );
    }
    const modes: SlashModeItem[] = (canSwitchModes ? modeList : []).map(mode => ({
      kind: 'mode',
      id: mode.id,
      name: mode.name,
    }));
    return [...actions, ...mcpPrompts, ...modes];
  }, [canSwitchModes, getFilteredActions, getFilteredMcpPromptCommands, incrementalCodeModes, slashCommandState.query]);
  
  const handleInputChange = useCallback((text: string, activeContexts: import('../../shared/types/context').ContextItem[]) => {
    if (!inputState.isActive && text.length > 0) {
      dispatchInput({ type: 'ACTIVATE' });
    }

    const activeContextIds = new Set(activeContexts.map(context => context.id));
    contexts.forEach(context => {
      if (!activeContextIds.has(context.id)) {
        removeContext(context.id);
      }
    });
    
    prunePendingLargePastes(text);
    dispatchInput({ type: 'SET_VALUE', payload: text });
    inputValueRef.current = text;

    const trimmedLower = text.trim().toLowerCase();
    const isBtwCommand = trimmedLower.startsWith('/btw');
    const isCompactCommand = trimmedLower.startsWith('/compact');
    const isProcessing = !!derivedState?.isProcessing;

    // Don't queue /btw while the main session is processing; /btw runs independently.
    if (derivedState?.isProcessing && !isBtwCommand && !isCompactCommand) {
      setQueuedInput(text);
    }

    if (text.startsWith('/')) {
      const afterSlash = text.slice(1);
      const hasWhitespace = /\s/.test(afterSlash);
      const firstToken = afterSlash.trimStart().split(/\s+/, 1)[0]?.toLowerCase?.() ?? '';
      const query = firstToken;
      const matchedMcpPrompt = resolveTypedMcpPromptCommand(text);

      // While the main session is running, expose a single quick action (/btw) via the same picker UX.
      if (isProcessing) {
        // Only show the picker for "/..." patterns that are plausibly a command (/ or /b...).
        // Once the user types a space (starts composing the real question), stop showing the picker
        // so Enter can submit "/btw ..." instead of selecting from the picker.
        if (!hasWhitespace && (query === '' || query.startsWith('b'))) {
          setSlashCommandState({
            isActive: true,
            kind: 'actions',
            query,
            selectedIndex: 0,
          });
        } else if (slashCommandState.isActive && slashCommandState.kind === 'actions') {
          setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });
        }
        return;
      }

      // When idle, keep the picker for mode switching, but don't interfere with executable slash commands.
      if (!isBtwCommand && !isCompactCommand && !matchedMcpPrompt) {
        setSlashCommandState({
          isActive: true,
          kind: 'all',
          query,
          selectedIndex: 0,
        });
        return;
      }
    }

    if (slashCommandState.isActive) {
      setSlashCommandState({
        isActive: false,
        kind: 'modes',
        query: '',
        selectedIndex: 0,
      });
    }
  }, [contexts, derivedState, inputState.isActive, prunePendingLargePastes, removeContext, resolveTypedMcpPromptCommand, setQueuedInput, slashCommandState.isActive, slashCommandState.kind]);

  const submitBtwFromInput = useCallback(async () => {
    if (!derivedState) return;
    if (!currentSessionId) {
      notificationService.error(t('btw.noSession', { defaultValue: 'No active session for /btw' }));
      return;
    }
    if (isBtwSession) {
      notificationService.warning(t('btw.nestedDisabled', { defaultValue: 'Side questions cannot create another side question' }));
      return;
    }

    const originalMessage = inputState.value.trim();
    const originalPendingLargePastes = { ...pendingLargePastesRef.current };
    const message = expandPendingLargePastes(originalMessage).trim();
    const messageCharCount = getCharacterCount(message);
    const question = message.replace(/^\/btw\b/i, '').trim();

    // Clear input without adding to main history.
    dispatchInput({ type: 'CLEAR_VALUE' });
    clearPendingLargePastes();
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

    if (!question) {
      notificationService.warning(t('btw.empty', { defaultValue: 'Please provide a question after /btw' }));
      return;
    }

    if (messageCharCount > CHAT_INPUT_CONFIG.largePaste.maxMessageChars) {
      notificationService.error(
        t('input.messageTooLarge', {
          max: CHAT_INPUT_CONFIG.largePaste.maxMessageChars,
          count: messageCharCount,
          defaultValue: 'Message exceeds the maximum length of {{max}} characters ({{count}} provided).',
        }),
        { duration: 4000 }
      );
      pendingLargePastesRef.current = originalPendingLargePastes;
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: originalMessage });
      return;
    }

    try {
      const { childSessionId } = await startBtwThread({
        parentSessionId: currentSessionId,
        workspacePath,
        question,
        modelId: 'fast',
        maxContextMessages: 60,
      });
      openBtwSessionInAuxPane({
        childSessionId,
        parentSessionId: currentSessionId,
        workspacePath,
        expand: true,
      });
      setInputTarget('btw');
      dispatchInput({ type: 'DEACTIVATE' });
    } catch (e) {
      log.error('Failed to start /btw thread', { e });
      dispatchInput({ type: 'ACTIVATE' });
      pendingLargePastesRef.current = originalPendingLargePastes;
      dispatchInput({ type: 'SET_VALUE', payload: originalMessage });
    }
  }, [clearPendingLargePastes, currentSessionId, derivedState, expandPendingLargePastes, inputState.value, isBtwSession, setQueuedInput, t, workspacePath]);

  const submitCompactFromInput = useCallback(async () => {
    if (!effectiveTargetSessionId || !effectiveTargetSession) {
      notificationService.error(
        t('chatInput.compactNoSession', { defaultValue: 'No active session for /compact' })
      );
      return;
    }

    if (derivedState?.isProcessing) {
      notificationService.warning(
        t('chatInput.compactBusy', {
          defaultValue: 'Wait until the session is idle before using /compact.',
        })
      );
      return;
    }

    const message = inputState.value.trim();
    if (!/^\/compact\s*$/i.test(message)) {
      notificationService.warning(
        t('chatInput.compactUsage', { defaultValue: 'Use /compact without extra arguments.' })
      );
      return;
    }

    dispatchInput({ type: 'CLEAR_VALUE' });
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

    try {
      const { agentAPI } = await import('@/infrastructure/api');
      await agentAPI.compactSession({
        sessionId: effectiveTargetSessionId,
        workspacePath: effectiveTargetSession.workspacePath,
        remoteConnectionId: effectiveTargetSession.remoteConnectionId,
        remoteSshHost: effectiveTargetSession.remoteSshHost,
      });
    } catch (error) {
      log.error('Failed to trigger /compact', {
        error,
        sessionId: effectiveTargetSessionId,
      });
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: message });
      notificationService.error(
        error instanceof Error ? error.message : t('error.unknown'),
        {
          title: t('chatInput.compactFailed', { defaultValue: 'Session compaction failed' }),
          duration: 5000,
        }
      );
    }
  }, [
    derivedState?.isProcessing,
    effectiveTargetSession,
    effectiveTargetSessionId,
    inputState.value,
    setQueuedInput,
    t,
  ]);

  const submitInitFromInput = useCallback(async () => {
    if (!effectiveTargetSessionId || !effectiveTargetSession) {
      notificationService.error(
        t('chatInput.initNoSession', { defaultValue: 'No active session for /init' })
      );
      return;
    }

    if (derivedState?.isProcessing) {
      notificationService.warning(
        t('chatInput.initBusy', {
          defaultValue: 'Wait until the session is idle before using /init.',
        })
      );
      return;
    }

    const message = inputState.value.trim();
    if (!/^\/init\s*$/i.test(message)) {
      notificationService.warning(
        t('chatInput.initUsage', { defaultValue: 'Use /init without extra arguments.' })
      );
      return;
    }

    const initInstruction = t('chatInput.initPrompt', {
      defaultValue: 'Please generate or update AGENTS.md so it matches the current project. Write it in English and keep the English version complete.',
    });

    dispatchInput({ type: 'CLEAR_VALUE' });
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

    try {
      const flowChatManager = FlowChatManager.getInstance();
      await flowChatManager.sendMessage(
        initInstruction,
        effectiveTargetSessionId,
        initInstruction,
        'Init'
      );
      onSendMessage?.(initInstruction);
      dispatchInput({ type: 'DEACTIVATE' });
    } catch (error) {
      log.error('Failed to trigger /init', {
        error,
        sessionId: effectiveTargetSessionId,
      });
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: message });
      notificationService.error(
        error instanceof Error ? error.message : t('error.unknown'),
        {
          title: t('chatInput.initFailed', { defaultValue: 'Session init failed' }),
          duration: 5000,
        }
      );
    }
  }, [
    derivedState?.isProcessing,
    effectiveTargetSession,
    effectiveTargetSessionId,
    inputState.value,
    onSendMessage,
    setQueuedInput,
    t,
  ]);

  const submitMcpPromptFromInput = useCallback(async () => {
    const originalMessage = inputState.value.trim();
    let command = resolveTypedMcpPromptCommand(originalMessage);

    if (!command) {
      await loadMcpPromptCommands();
      command = resolveTypedMcpPromptCommand(originalMessage);
    }

    if (!command) {
      notificationService.warning(
        t('chatInput.noMatchingCommand', { defaultValue: 'No matching command' })
      );
      return;
    }

    const argsText = originalMessage
      .slice(command.command.length)
      .trim();
    const argValues = parseSlashArguments(argsText);
    const requiredArgs = command.arguments.filter(argument => argument.required);

    if (argValues.length < requiredArgs.length) {
      const requiredNames = requiredArgs.map(argument => argument.name).join(', ');
      notificationService.warning(
        t('chatInput.mcpPromptMissingArgs', {
          defaultValue: 'This MCP prompt requires arguments: {{args}}',
          args: requiredNames,
        })
      );
      return;
    }

    const originalPendingLargePastes = { ...pendingLargePastesRef.current };
    if (effectiveTargetSessionId) {
      addToHistory(effectiveTargetSessionId, originalMessage);
    }
    setHistoryIndex(-1);
    setSavedDraft('');
    dispatchInput({ type: 'CLEAR_VALUE' });
    clearPendingLargePastes();
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

    try {
      const promptArguments = command.arguments.reduce<Record<string, string>>((acc, argument, index) => {
        const value = argValues[index];
        if (typeof value === 'string' && value.length > 0) {
          acc[argument.name] = value;
        }
        return acc;
      }, {});

      const prompt = await MCPAPI.getPrompt({
        serverId: command.serverId,
        promptName: command.promptName,
        arguments: Object.keys(promptArguments).length > 0 ? promptArguments : undefined,
      });

      const renderedPrompt = renderMcpPromptMessages(prompt.messages);
      if (!renderedPrompt.trim()) {
        throw new Error('MCP prompt returned no displayable content');
      }

      await sendMessage(renderedPrompt, {
        displayMessage: originalMessage,
      });
      dispatchInput({ type: 'DEACTIVATE' });
    } catch (error) {
      log.error('Failed to run MCP prompt command', {
        command: originalMessage,
        error,
      });
      pendingLargePastesRef.current = originalPendingLargePastes;
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: originalMessage });
      notificationService.error(
        error instanceof Error ? error.message : t('error.unknown'),
        {
          title: t('chatInput.mcpPromptFailed', { defaultValue: 'MCP prompt failed' }),
          duration: 5000,
        }
      );
    }
  }, [
    clearPendingLargePastes,
    addToHistory,
    effectiveTargetSessionId,
    inputState.value,
    loadMcpPromptCommands,
    resolveTypedMcpPromptCommand,
    sendMessage,
    setQueuedInput,
    t,
  ]);
  
  const handleSendOrCancel = useCallback(async () => {
    if (!derivedState) return;
    
    const { sendButtonMode } = derivedState;
    const draftTrimmed = inputState.value.trim();

    // While generating, an empty control in `cancel` mode means stop. If the user has typed a follow-up,
    // never treat this path as cancel — that would call cancel_dialog_turn and abort the current round early.
    if (sendButtonMode === 'cancel' && !draftTrimmed) {
      await transition(SessionExecutionEvent.USER_CANCEL);
      return;
    }
    
    if (sendButtonMode === 'retry') {
      await transition(SessionExecutionEvent.RESET);
    }
    
    if (!draftTrimmed) return;
    
    const originalMessage = draftTrimmed;
    const originalPendingLargePastes = { ...pendingLargePastesRef.current };
    const message = expandPendingLargePastes(originalMessage).trim();
    const messageCharCount = getCharacterCount(message);

    if (message.toLowerCase().startsWith('/btw')) {
      // When idle, /btw can be sent via the normal send button.
      await submitBtwFromInput();
      return;
    }

    if (/^\/compact\s*$/i.test(message)) {
      await submitCompactFromInput();
      return;
    }

    if (/^\/init\s*$/i.test(message)) {
      await submitInitFromInput();
      return;
    }

    if (resolveTypedMcpPromptCommand(message)) {
      await submitMcpPromptFromInput();
      return;
    }

    if (message.toLowerCase().startsWith('/compact')) {
      notificationService.warning(
        t('chatInput.compactUsage', { defaultValue: 'Use /compact without extra arguments.' })
      );
      return;
    }

    if (message.toLowerCase().startsWith('/init')) {
      notificationService.warning(
        t('chatInput.initUsage', { defaultValue: 'Use /init without extra arguments.' })
      );
      return;
    }
    
    // Add to history before clearing (session-scoped)
    if (effectiveTargetSessionId) {
      addToHistory(effectiveTargetSessionId, message);
    }
    setHistoryIndex(-1);
    setSavedDraft('');
    
    dispatchInput({ type: 'CLEAR_VALUE' });
    clearPendingLargePastes();
    // Clear machine queue too; otherwise the queuedInput→input sync effect puts the text back after send.
    setQueuedInput(null);

    if (messageCharCount > CHAT_INPUT_CONFIG.largePaste.maxMessageChars) {
      notificationService.error(
        t('input.messageTooLarge', {
          max: CHAT_INPUT_CONFIG.largePaste.maxMessageChars,
          count: messageCharCount,
          defaultValue: 'Message exceeds the maximum length of {{max}} characters ({{count}} provided).',
        }),
        { duration: 4000 }
      );
      pendingLargePastesRef.current = originalPendingLargePastes;
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: originalMessage });
      return;
    }

    try {
      await sendMessage(message);
      clearPendingLargePastes();
      dispatchInput({ type: 'CLEAR_VALUE' });
      dispatchInput({ type: 'DEACTIVATE' });
    } catch (error) {
      log.error('Failed to send message', { error });
      pendingLargePastesRef.current = originalPendingLargePastes;
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: originalMessage });
      if (derivedState?.isProcessing) {
        setQueuedInput(originalMessage);
      }
    }
  }, [
    inputState.value,
    derivedState,
    transition,
    sendMessage,
    addToHistory,
    effectiveTargetSessionId,
    clearPendingLargePastes,
    expandPendingLargePastes,
    setQueuedInput,
    submitBtwFromInput,
    submitCompactFromInput,
    submitInitFromInput,
    submitMcpPromptFromInput,
    t,
    resolveTypedMcpPromptCommand,
  ]);
  
  const getFilteredIncrementalModes = useCallback(() => {
    if (!canSwitchModes) return [];
    if (!slashCommandState.query) return incrementalCodeModes;
    return incrementalCodeModes.filter(
      mode =>
        mode.name.toLowerCase().includes(slashCommandState.query) ||
        mode.id.toLowerCase().includes(slashCommandState.query)
    );
  }, [canSwitchModes, incrementalCodeModes, slashCommandState.query]);

  const applyModeChange = useCallback((modeId: string) => {
    dispatchMode({
      type: 'SET_CURRENT_MODE',
      payload: modeId,
    });

    try {
      sessionStorage.setItem('bitfun:flowchat:lastMode', modeId);
    } catch {
      // ignore
    }

    if (effectiveTargetSessionId) {
      FlowChatStore.getInstance().updateSessionMode(effectiveTargetSessionId, modeId);
    }
  }, [effectiveTargetSessionId]);

  const requestModeChange = useCallback((modeId: string) => {
    if (!canSwitchModes) {
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      return;
    }

    if (modeId === currentMode) {
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      return;
    }

    if (!switchableModes.some(mode => mode.id === modeId)) {
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      return;
    }

    applyModeChange(modeId);
    dispatchMode({ type: 'CLOSE_DROPDOWN' });
  }, [applyModeChange, canSwitchModes, currentMode, switchableModes]);
  
  const selectSlashCommandMode = useCallback((modeId: string) => {
    requestModeChange(modeId);
    
    dispatchInput({ type: 'CLEAR_VALUE' });
    setSlashCommandState({
      isActive: false,
      kind: 'modes',
      query: '',
      selectedIndex: 0,
    });
  }, [requestModeChange]);

  const selectSlashCommandAction = useCallback((actionId: string) => {
    const raw = inputState.value || '';
    const lower = raw.trimStart().toLowerCase();

    let next = raw;

    if (actionId === 'btw') {
      if (isBtwSession) {
        return;
      }
      if (!lower.startsWith('/btw')) {
        next = '/btw ';
      } else {
        // Normalize to "/btw " + rest, preserving any already typed question.
        const m = raw.match(/^(\s*)\/btw\b/i);
        if (m) {
          const leadingWs = m[1] || '';
          const rest = raw.slice(m[0].length);
          next = `${leadingWs}/btw ${rest.trimStart()}`;
        } else {
          next = '/btw ';
        }
      }
    } else if (actionId === 'compact') {
      next = '/compact';
    } else if (actionId === 'init') {
      next = '/init';
    } else {
      return;
    }

    dispatchInput({ type: 'SET_VALUE', payload: next });
    // Clear the machine's queued input so the queuedInput sync effect does not overwrite
    // the just-set "/btw ..." value back to the stale "/" that was queued while processing.
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });
    window.setTimeout(() => richTextInputRef.current?.focus(), 0);
  }, [inputState.value, isBtwSession, setQueuedInput]);

  const selectSlashPromptCommand = useCallback((item: SlashMcpPromptItem) => {
    const hasArguments = item.arguments.length > 0;
    dispatchInput({
      type: 'SET_VALUE',
      payload: hasArguments ? `${item.command} ` : item.command,
    });
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });
    window.setTimeout(() => richTextInputRef.current?.focus(), 0);
  }, [setQueuedInput]);

  const handleBoostStartBtw = useCallback(
    (e: React.SyntheticEvent) => {
      e.stopPropagation();
      if (!currentSessionId) {
        notificationService.error(t('btw.noSession', { defaultValue: 'No active session for /btw' }));
        return;
      }
      if (isBtwSession) {
        notificationService.warning(
          t('btw.nestedDisabled', { defaultValue: 'Side questions cannot create another side question' })
        );
        return;
      }
      selectSlashCommandAction('btw');
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
    },
    [currentSessionId, isBtwSession, selectSlashCommandAction, t]
  );
  
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    // Local /btw shortcut (Ctrl/Cmd+Alt+B) should work even when ChatInput is focused.
    if ((e.ctrlKey || e.metaKey) && e.altKey && !e.shiftKey && e.key.toLowerCase() === 'b') {
      e.preventDefault();
      e.stopPropagation();

      if (!currentSessionId) {
        notificationService.error(t('btw.noSession', { defaultValue: 'No active session for /btw' }));
        return;
      }
      if (isBtwSession) {
        notificationService.warning(t('btw.nestedDisabled', { defaultValue: 'Side questions cannot create another side question' }));
        return;
      }

      const selected = (window.getSelection?.()?.toString() ?? '').trim();
      const initial = selected ? `/btw Explain this:\n\n${selected}` : '/btw ';
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: initial });
      window.setTimeout(() => richTextInputRef.current?.focus(), 0);
      return;
    }

    if (slashCommandState.isActive) {
      if (!(slashCommandState.kind === 'modes' && !canSwitchModes)) {
        const items =
          slashCommandState.kind === 'modes'
            ? getFilteredIncrementalModes()
            : slashCommandState.kind === 'actions'
              ? getFilteredActions()
              : getSlashPickerItems();
        const maxIndex = Math.max(0, items.length - 1);
        
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          setSlashCommandState(prev => ({
            ...prev,
            selectedIndex: Math.min(prev.selectedIndex + 1, maxIndex),
          }));
          return;
        }
        
        if (e.key === 'ArrowUp') {
          e.preventDefault();
          setSlashCommandState(prev => ({
            ...prev,
            selectedIndex: Math.max(prev.selectedIndex - 1, 0),
          }));
          return;
        }
        
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          if (items.length > 0) {
            if (slashCommandState.kind === 'modes') {
              const mode = items[slashCommandState.selectedIndex] as any;
              selectSlashCommandMode(mode.id);
            } else if (slashCommandState.kind === 'actions') {
              const action = items[slashCommandState.selectedIndex] as any;
              selectSlashCommandAction(action.id);
            } else {
              const item = items[slashCommandState.selectedIndex] as SlashPickerItem;
              if (item.kind === 'mode') {
                selectSlashCommandMode(item.id);
              } else if (item.kind === 'mcpPrompt') {
                selectSlashPromptCommand(item);
              } else {
                selectSlashCommandAction(item.id);
              }
            }
          }
          return;
        }
        
        if (e.key === 'Escape') {
          e.preventDefault();
          const kind = slashCommandState.kind;
          setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

          // For mode switching picker, "/" is just a trigger and should be cleared on cancel.
          if (kind !== 'actions') {
            dispatchInput({ type: 'CLEAR_VALUE' });
          }
          return;
        }
        
        if (e.key === 'Tab') {
          e.preventDefault();
          if (items.length > 0) {
            if (slashCommandState.kind === 'modes') {
              const mode = items[slashCommandState.selectedIndex] as any;
              selectSlashCommandMode(mode.id);
            } else if (slashCommandState.kind === 'actions') {
              const action = items[slashCommandState.selectedIndex] as any;
              selectSlashCommandAction(action.id);
            } else {
              const item = items[slashCommandState.selectedIndex] as SlashPickerItem;
              if (item.kind === 'mode') {
                selectSlashCommandMode(item.id);
              } else if (item.kind === 'mcpPrompt') {
                selectSlashPromptCommand(item);
              } else {
                selectSlashCommandAction(item.id);
              }
            }
          }
          return;
        }
      }
    }
    
    // Tab key: toggle send target when the btw session switcher is visible
    if (showTargetSwitcher && e.key === 'Tab' && !e.shiftKey && !slashCommandState.isActive) {
      e.preventDefault();
      setInputTarget(prev => prev === 'main' ? 'btw' : 'main');
      return;
    }

    // History navigation with up/down arrows
    // Only handle when not in slash command mode and not composing
    if (!slashCommandState.isActive && inputHistory.length > 0) {
      const selection = window.getSelection();
      const editor = richTextInputRef.current;
      
      if (selection && selection.rangeCount > 0 && editor) {
        const range = selection.getRangeAt(0);
        
        // Check cursor position
        const isAtStart = range.collapsed && range.startOffset === 0 && 
                          (range.startContainer === editor || 
                           (range.startContainer.nodeType === Node.TEXT_NODE && 
                            range.startContainer.previousSibling === null &&
                            range.startContainer.parentNode === editor));
        
        // For end position, we need to check if cursor is at the end of content
        const isAtEnd = (() => {
          if (!range.collapsed) return false;
          const editorContent = editor.textContent || '';
          let cursorPos = 0;
          const traverse = (node: Node): boolean => {
            if (node === range.startContainer) {
              if (node.nodeType === Node.TEXT_NODE) {
                cursorPos += range.startOffset;
              }
              return true;
            }
            if (node.nodeType === Node.TEXT_NODE) {
              cursorPos += (node.textContent || '').length;
            } else if (node.nodeType === Node.ELEMENT_NODE) {
              for (const child of Array.from(node.childNodes)) {
                if (traverse(child)) return true;
              }
            }
            return false;
          };
          traverse(editor);
          return cursorPos === editorContent.length;
        })();
        
        // Arrow Up at start of line -> go back in history
        if (e.key === 'ArrowUp' && isAtStart) {
          e.preventDefault();
          
          // Save draft if starting navigation
          if (historyIndex === -1 && inputState.value.trim()) {
            setSavedDraft(inputState.value);
          }
          
          // Navigate back (older messages)
          if (historyIndex < inputHistory.length - 1) {
            const newIndex = historyIndex + 1;
            setHistoryIndex(newIndex);
            dispatchInput({ type: 'SET_VALUE', payload: inputHistory[newIndex] });
          }
          return;
        }
        
        // Arrow Down at end of line -> go forward in history
        if (e.key === 'ArrowDown' && isAtEnd) {
          e.preventDefault();
          
          if (historyIndex > 0) {
            // Navigate forward (newer messages)
            const newIndex = historyIndex - 1;
            setHistoryIndex(newIndex);
            dispatchInput({ type: 'SET_VALUE', payload: inputHistory[newIndex] });
          } else if (historyIndex === 0) {
            // Return to draft/empty
            setHistoryIndex(-1);
            dispatchInput({ type: 'SET_VALUE', payload: savedDraft });
          }
          return;
        }
      }
    }
    
    const isComposing = (e.nativeEvent as KeyboardEvent).isComposing || isImeComposingRef.current;
    const justFinishedComposition = Date.now() - lastImeCompositionEndAtRef.current < IME_ENTER_GUARD_MS;
    
    if (e.key === 'Enter' && !e.shiftKey) {
      if (isComposing || justFinishedComposition) {
        return;
      }
      
      e.preventDefault();

      const isBtwCommand = inputState.value.trim().toLowerCase().startsWith('/btw');
      if (isBtwCommand) {
        // Allow /btw submission even while the main session is generating.
        void submitBtwFromInput();
        return;
      }

      if (derivedState?.isProcessing) {
        if (!inputState.value.trim()) return;
        void handleSendOrCancel();
        return;
      }

      handleSendOrCancel();
    }
    
    if (e.key === 'Escape' && derivedState?.canCancel) {
      e.preventDefault();
      transition(SessionExecutionEvent.USER_CANCEL);
    }
  }, [handleSendOrCancel, submitBtwFromInput, derivedState, transition, slashCommandState, getFilteredIncrementalModes, getFilteredActions, getSlashPickerItems, selectSlashCommandMode, selectSlashCommandAction, selectSlashPromptCommand, canSwitchModes, historyIndex, inputHistory, savedDraft, inputState.value, currentSessionId, isBtwSession, showTargetSwitcher, setInputTarget, t]);

  const handleImeCompositionStart = useCallback(() => {
    isImeComposingRef.current = true;
  }, []);

  const handleImeCompositionEnd = useCallback(() => {
    isImeComposingRef.current = false;
    lastImeCompositionEndAtRef.current = Date.now();
  }, []);

  const handleImageInput = useCallback(() => {
    const remaining = CHAT_INPUT_CONFIG.image.maxCount - currentImageCount;
    if (remaining <= 0) {
      notificationService.warning(t('input.maxImagesWarning', { count: CHAT_INPUT_CONFIG.image.maxCount }), { duration: 3000 });
      return;
    }

    const input = document.createElement('input');
    input.type = 'file';
    input.accept = CHAT_INPUT_CONFIG.image.acceptedTypes.join(',');
    input.multiple = true;
    
    input.onchange = async (e) => {
      const files = (e.target as HTMLInputElement).files;
      if (!files || files.length === 0) return;
      
      const fileArray = Array.from(files).slice(0, remaining);
      if (files.length > remaining) {
        notificationService.warning(t('input.maxImagesWarning', { count: CHAT_INPUT_CONFIG.image.maxCount }), { duration: 3000 });
      }
      
      let successCount = 0;
      
      for (const file of fileArray) {
        try {
          const imageContext = await createImageContextFromFile(file);
          addContext(imageContext);
          
          if (richTextInputRef.current && (richTextInputRef.current as any).insertTag) {
            (richTextInputRef.current as any).insertTag(imageContext);
          }
          
          successCount++;
        } catch (error) {
          log.error('Failed to process image', { fileName: file.name, error });
          notificationService.error(
            `${file.name}: ${error instanceof Error ? error.message : t('error.processingFailed')}`,
            { duration: 3000 }
          );
        }
      }
      
      if (successCount > 0) {
        notificationService.success(
          t('input.imageAddedSuccess', { count: successCount }),
          { duration: 2000 }
        );
      }
    };
    
    input.click();
  }, [addContext, currentImageCount, t]);
  
  const toggleExpand = useCallback(() => {
    dispatchInput({ type: 'TOGGLE_EXPAND' });
  }, []);

  const focusRichTextInputSoon = useCallback(() => {
    window.requestAnimationFrame(() => {
      richTextInputRef.current?.focus();
    });
  }, []);

  const insertSkillIntoInput = useCallback(
    (skillName: string) => {
      const line = t('chatInput.insertSkillLine', { name: skillName });
      dispatchInput({ type: 'ACTIVATE' });
      const cur = inputState.value;
      const next = cur.trim() ? `${cur.trimEnd()}\n\n${line}` : line;
      dispatchInput({ type: 'SET_VALUE', payload: next });
      clearSkillsTimer();
      setSkillsFlyoutOpen(false);
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      focusRichTextInputSoon();
    },
    [clearSkillsTimer, focusRichTextInputSoon, inputState.value, t]
  );

  const handleBoostPickImage = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      handleImageInput();
    },
    [handleImageInput]
  );

  const handleBoostOpenAtContext = useCallback((e: React.SyntheticEvent) => {
    e.stopPropagation();
    dispatchMode({ type: 'CLOSE_DROPDOWN' });
    dispatchInput({ type: 'ACTIVATE' });
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        const el = richTextInputRef.current;
        if (el && typeof (el as unknown as { openMention?: () => void }).openMention === 'function') {
          (el as unknown as { openMention: () => void }).openMention();
        }
      });
    });
  }, []);

  const handleOpenSkillsLibrary = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      clearSkillsTimer();
      setSkillsFlyoutOpen(false);
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      openOverlay('skills' as OverlaySceneId);
    },
    [clearSkillsTimer, openOverlay]
  );
  
  const handleActivate = useCallback((e?: React.MouseEvent) => {
    if (e?.target instanceof HTMLButtonElement || 
        (e?.target instanceof Element && e.target.closest('button'))) {
      if (!inputState.isActive) {
        dispatchInput({ type: 'ACTIVATE' });
      }
      return;
    }
    
    if (!inputState.isActive) {
      dispatchInput({ type: 'ACTIVATE' });
      focusRichTextInputSoon();
    }
  }, [focusRichTextInputSoon, inputState.isActive]);

  // Global space-to-activate: when collapsed and no editable element is focused
  useEffect(() => {
    if (inputState.isActive) return;

    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isEditable =
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable ||
        target.closest('[contenteditable="true"]') !== null;

      if (e.key === 'Escape' && derivedState?.canCancel) {
        if (isEditable) return;
        e.preventDefault();
        void transition(SessionExecutionEvent.USER_CANCEL);
        return;
      }

      if (e.key !== ' ') return;
      if (isEditable) return;

      e.preventDefault();
      dispatchInput({ type: 'ACTIVATE' });
      focusRichTextInputSoon();
    };

    // Capture phase so activation runs before nested handlers; Space must dispatch ACTIVATE, not only focus().
    document.addEventListener('keydown', handleGlobalKeyDown, true);
    return () => document.removeEventListener('keydown', handleGlobalKeyDown, true);
  }, [derivedState?.canCancel, focusRichTextInputSoon, inputState.isActive, transition]);
  
  const containerRef = useRef<HTMLDivElement>(null);
  
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      // Do not collapse when clicking the scroll-to-latest bar.
      if ((target as Element)?.closest?.('.scroll-to-latest-bar')) return;
      if (
        inputState.isActive &&
        containerRef.current &&
        !containerRef.current.contains(target)
      ) {
        // While IME is composing, React value can still be empty (RichTextInput skips onChange),
        // but the editor DOM holds preedit text — collapsing would show space-hint on top of it.
        if (inputState.value.trim() === '' && !isImeComposingRef.current) {
          dispatchInput({ type: 'DEACTIVATE' });
        }
      }
    };
    
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [inputState.isActive, inputState.value]);

  useEffect(() => {
    const dropZone = containerRef.current?.closest('.bitfun-chat-input-drop-zone') as HTMLElement | null;
    const el = dropZone ?? containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      setChatInputHeight(el.offsetHeight);
    });
    observer.observe(el);
    setChatInputHeight(el.offsetHeight);
    return () => observer.disconnect();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const isCollapsedProcessing = !inputState.isActive && !!derivedState?.isProcessing;
  const petReplacesStopChrome = agentCompanionEnabled && isCollapsedProcessing;
  const petStopClickable = petReplacesStopChrome && !!derivedState?.canCancel;
  const collapsedPetSplitSend =
    petReplacesStopChrome && derivedState?.sendButtonMode === 'split';

  const renderActionButton = () => {
    if (!derivedState) return <IconButton className="bitfun-chat-input__send-button" disabled size="small"><ArrowUp size={11} /></IconButton>;

    if (petReplacesStopChrome) {
      const { sendButtonMode } = derivedState;
      if (sendButtonMode === 'cancel') {
        return null;
      }
      if (sendButtonMode === 'split') {
        return (
          <IconButton
            className="bitfun-chat-input__send-button"
            onClick={handleSendOrCancel}
            disabled={!inputState.value.trim()}
            data-testid="chat-input-send-btn"
            tooltip={t('input.sendShortcut')}
            size="small"
          >
            <ArrowUp size={11} />
          </IconButton>
        );
      }
    }

    const { sendButtonMode, hasQueuedInput } = derivedState;
    
    if (sendButtonMode === 'cancel') {
      return (
        <Tooltip content={t('input.stopGeneration')}>
          <div
            className="bitfun-chat-input__send-button bitfun-chat-input__send-button--breathing"
            onClick={handleSendOrCancel}
            data-testid="chat-input-cancel-btn"
          >
            <div className="bitfun-chat-input__breathing-circle" />
            {hasQueuedInput && <span className="bitfun-chat-input__queued-badge">1</span>}
          </div>
        </Tooltip>
      );
    }
    
    if (sendButtonMode === 'retry') {
      return (
        <IconButton
          className="bitfun-chat-input__send-button bitfun-chat-input__send-button--retry"
          onClick={handleSendOrCancel}
          tooltip={t('input.retry')}
          size="small"
        >
          <RotateCcw size={11} />
        </IconButton>
      );
    }

    if (sendButtonMode === 'split') {
      return (
        <div className="bitfun-chat-input__split-actions">
          <Tooltip content={t('input.stopGeneration')}>
            <div
              className="bitfun-chat-input__send-button bitfun-chat-input__send-button--breathing"
              onClick={() => {
                void transition(SessionExecutionEvent.USER_CANCEL);
              }}
              data-testid="chat-input-cancel-btn"
            >
              <div className="bitfun-chat-input__breathing-circle" />
            </div>
          </Tooltip>
          <IconButton
            className="bitfun-chat-input__send-button"
            onClick={handleSendOrCancel}
            disabled={!inputState.value.trim()}
            data-testid="chat-input-send-btn"
            tooltip={t('input.sendShortcut')}
            size="small"
          >
            <ArrowUp size={11} />
          </IconButton>
        </div>
      );
    }
    
    return (
      <IconButton
        className="bitfun-chat-input__send-button"
        onClick={handleSendOrCancel}
        disabled={!inputState.value.trim()}
        data-testid="chat-input-send-btn"
        tooltip={t('input.sendShortcut')}
        size="small"
      >
        <ArrowUp size={11} />
      </IconButton>
    );
  };

  return (
    <>
      <ContextDropZone
        acceptedTypes={['file', 'directory', 'image', 'code-snippet']}
        className="bitfun-chat-input-drop-zone"
        onContextAdded={(context) => {
          if (context.type === 'image' && currentImageCount >= CHAT_INPUT_CONFIG.image.maxCount) {
            notificationService.warning(t('input.maxImagesWarning', { count: CHAT_INPUT_CONFIG.image.maxCount }), { duration: 3000 });
            return;
          }
          if (richTextInputRef.current && (richTextInputRef.current as any).insertTag) {
            (richTextInputRef.current as any).insertTag(context);
          }
          if (!inputState.isActive) {
            dispatchInput({ type: 'ACTIVATE' });
          }
        }}
      >
        <div 
          ref={containerRef}
          className={`bitfun-chat-input ${inputState.isActive ? 'bitfun-chat-input--active' : 'bitfun-chat-input--collapsed'} ${inputState.isExpanded ? 'bitfun-chat-input--expanded' : ''} ${derivedState?.isProcessing ? 'bitfun-chat-input--processing' : ''} ${showCollapsedPet ? 'bitfun-chat-input--pet-visible' : ''} ${petReplacesStopChrome ? 'bitfun-chat-input--pet-replaces-stop' : ''} ${collapsedPetSplitSend ? 'bitfun-chat-input--pet-split-send' : ''} ${className}`}
          onClick={!inputState.isActive ? handleActivate : undefined}
          data-testid="chat-input-container"
        >
        {recommendationContext && (
          <SmartRecommendations
            context={recommendationContext}
            className="bitfun-chat-input__recommendations"
          />
        )}

        <div className="bitfun-chat-input__container">
          <div className={`bitfun-chat-input__box ${inputState.isExpanded ? 'bitfun-chat-input__box--expanded' : ''}`}>
            {showCollapsedPet && (
              <div
                className={[
                  'bitfun-chat-input__pet-wrap',
                  petReplacesStopChrome ? 'bitfun-chat-input__pet-wrap--shift' : '',
                  collapsedPetSplitSend ? 'bitfun-chat-input__pet-wrap--split' : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
              >
                <div className="bitfun-chat-input__pet-inner">
                  {petStopClickable ? (
                    <button
                      type="button"
                      className="bitfun-chat-input__pet-stop-btn"
                      onClick={e => {
                        e.stopPropagation();
                        void transition(SessionExecutionEvent.USER_CANCEL);
                      }}
                      aria-label={t('input.stopGeneration')}
                    >
                      <ChatInputPixelPet
                        mood={petMood}
                        layout={petReplacesStopChrome ? 'stopRight' : 'center'}
                      />
                    </button>
                  ) : (
                    <ChatInputPixelPet
                      mood={petMood}
                      layout={petReplacesStopChrome ? 'stopRight' : 'center'}
                    />
                  )}
                </div>
              </div>
            )}
            {showTargetSwitcher && (
              <div className="bitfun-chat-input__target-switcher" data-testid="chat-input-target-switcher">
                <span className="bitfun-chat-input__target-switcher-label">{t('chatInput.conversationTarget')}</span>
                <button
                  type="button"
                  tabIndex={-1}
                  className={`bitfun-chat-input__target-tab ${inputTarget === 'main' ? 'bitfun-chat-input__target-tab--active' : ''}`}
                  onClick={() => setInputTarget('main')}
                >
                  {t('chatInput.targetMain')}
                  {inputTarget === 'main' && currentSessionTitle && (
                    <span className="bitfun-chat-input__target-tab-name">{currentSessionTitle}</span>
                  )}
                </button>
                <button
                  type="button"
                  tabIndex={-1}
                  className={`bitfun-chat-input__target-tab ${inputTarget === 'btw' ? 'bitfun-chat-input__target-tab--active' : ''}`}
                  onClick={() => setInputTarget('btw')}
                >
                  {t('chatInput.targetBtw')}
                  {inputTarget === 'btw' && activeBtwSessionTitle && (
                    <span className="bitfun-chat-input__target-tab-name">{activeBtwSessionTitle}</span>
                  )}
                </button>
              </div>
            )}
            <div className="bitfun-chat-input__input-area">
              <RichTextInput
                ref={richTextInputRef}
                value={inputState.value}
                onChange={handleInputChange}
                onLargePaste={createLargePastePlaceholder}
                onKeyDown={handleKeyDown}
                onCompositionStart={handleImeCompositionStart}
                onCompositionEnd={handleImeCompositionEnd}
                placeholder={inputState.isActive ? t('input.placeholder') : ''}
                disabled={false}
                contexts={contexts}
                onRemoveContext={removeContext}
                onMentionStateChange={setMentionState}
                data-testid="chat-input-textarea"
              />

              {!inputState.isActive &&
                !inputState.value.trim() &&
                !agentCompanionEnabled && (
                <span className="bitfun-chat-input__space-hint">
                  <Trans
                    i18nKey="input.spaceToActivate"
                    t={t}
                    components={{
                      space: <span className="bitfun-chat-input__space-key" />,
                    }}
                  />
                </span>
              )}
              
              <FileMentionPicker
                isOpen={mentionState.isActive}
                searchQuery={mentionState.query}
                workspacePath={workspacePath}
                onSelect={(context: FileContext | DirectoryContext) => {
                  addContext(context);
                  
                  if (richTextInputRef.current && (richTextInputRef.current as any).insertTagReplacingMention) {
                    (richTextInputRef.current as any).insertTagReplacingMention(context);
                  }
                }}
                onClose={() => {
                  if (richTextInputRef.current && (richTextInputRef.current as any).closeMention) {
                    (richTextInputRef.current as any).closeMention();
                  }
                  setMentionState({ isActive: false, query: '', startOffset: 0 });
                }}
              />
              
              {slashCommandState.isActive && (() => {
                if (slashCommandState.kind === 'actions') {
                  const actions = getFilteredActions();
                  return (
                    <div className="bitfun-chat-input__slash-command-picker">
                      <div className="bitfun-chat-input__slash-command-header">
                        <span>{t('chatInput.quickAction', { defaultValue: 'Quick action' })}</span>
                        <span className="bitfun-chat-input__slash-command-hint">{t('chatInput.selectHint')}</span>
                      </div>
                      <div className="bitfun-chat-input__slash-command-list">
                        {actions.length > 0 ? (
                          actions.map((action, index) => (
                            <div
                              key={action.id}
                              className={`bitfun-chat-input__slash-command-item ${index === slashCommandState.selectedIndex ? 'bitfun-chat-input__slash-command-item--selected' : ''}`}
                              onClick={() => selectSlashCommandAction(action.id)}
                              onMouseEnter={() => setSlashCommandState(prev => ({ ...prev, selectedIndex: index }))}
                            >
                              <span className="bitfun-chat-input__slash-command-name">{action.command}</span>
                              <span className="bitfun-chat-input__slash-command-label">{action.label}</span>
                            </div>
                          ))
                        ) : (
                          <div className="bitfun-chat-input__slash-command-empty">
                            {t('chatInput.noMatchingCommand', { defaultValue: 'No matching command' })}
                          </div>
                        )}
                      </div>
                    </div>
                  );
                }

                if (slashCommandState.kind === 'all') {
                  const items = getSlashPickerItems();
                  return (
                    <div className="bitfun-chat-input__slash-command-picker">
                      <div className="bitfun-chat-input__slash-command-header">
                        <span>{t('chatInput.quickAction', { defaultValue: 'Commands' })}</span>
                        <span className="bitfun-chat-input__slash-command-hint">{t('chatInput.selectHint')}</span>
                      </div>
                      <div className="bitfun-chat-input__slash-command-list">
                        {mcpPromptCommandsLoading && items.length === 0 ? (
                          <div className="bitfun-chat-input__slash-command-empty">
                            {t('chatInput.loadingMcpPrompts', { defaultValue: 'Loading MCP prompts…' })}
                          </div>
                        ) : items.length > 0 ? (
                          items.map((item, index) => (
                            <div
                              key={`${item.kind}-${item.id}`}
                              className={`bitfun-chat-input__slash-command-item ${index === slashCommandState.selectedIndex ? 'bitfun-chat-input__slash-command-item--selected' : ''} ${item.kind === 'mode' && item.id === modeState.current ? 'bitfun-chat-input__slash-command-item--active' : ''}`}
                              onClick={() => {
                                if (item.kind === 'mode') {
                                  selectSlashCommandMode(item.id);
                                } else if (item.kind === 'mcpPrompt') {
                                  selectSlashPromptCommand(item);
                                } else {
                                  selectSlashCommandAction(item.id);
                                }
                              }}
                              onMouseEnter={() => setSlashCommandState(prev => ({ ...prev, selectedIndex: index }))}
                            >
                              <span className="bitfun-chat-input__slash-command-name">
                                {item.kind === 'mode' ? `/${item.id}` : item.command}
                              </span>
                              <span className="bitfun-chat-input__slash-command-label">
                                {item.kind === 'mode'
                                  ? item.name
                                  : item.kind === 'mcpPrompt'
                                    ? `${item.serverName} · ${item.label}`
                                    : item.label}
                              </span>
                              {item.kind === 'mode' && item.id === modeState.current && <span className="bitfun-chat-input__slash-command-current">{t('chatInput.current')}</span>}
                            </div>
                          ))
                        ) : (
                          <div className="bitfun-chat-input__slash-command-empty">
                            {t('chatInput.noMatchingCommand', { defaultValue: 'No matching command' })}
                          </div>
                        )}
                      </div>
                    </div>
                  );
                }

                if (!canSwitchModes) return null;

                const filteredModes = getFilteredIncrementalModes();
                return (
                  <div className="bitfun-chat-input__slash-command-picker">
                    <div className="bitfun-chat-input__slash-command-header">
                      <span>{t('chatInput.addModeMenuTitle')}</span>
                      <span className="bitfun-chat-input__slash-command-hint">{t('chatInput.selectHint')}</span>
                    </div>
                    <div className="bitfun-chat-input__slash-command-list">
                      {filteredModes.length > 0 ? (
                        filteredModes.map((mode, index) => (
                          <div
                            key={mode.id}
                            className={`bitfun-chat-input__slash-command-item ${index === slashCommandState.selectedIndex ? 'bitfun-chat-input__slash-command-item--selected' : ''} ${mode.id === modeState.current ? 'bitfun-chat-input__slash-command-item--active' : ''}`}
                            onClick={() => selectSlashCommandMode(mode.id)}
                            onMouseEnter={() => setSlashCommandState(prev => ({ ...prev, selectedIndex: index }))}
                          >
                            <span className="bitfun-chat-input__slash-command-name">/{mode.id}</span>
                            <span className="bitfun-chat-input__slash-command-label">{mode.name}</span>
                            {mode.id === modeState.current && <span className="bitfun-chat-input__slash-command-current">{t('chatInput.current')}</span>}
                          </div>
                        ))
                      ) : (
                        <div className="bitfun-chat-input__slash-command-empty">
                          {t('chatInput.noMatchingMode')}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })()}
            </div>
            
            <IconButton
              className="bitfun-chat-input__expand-button"
              variant="ghost"
              size="xs"
              shape="circle"
              onClick={toggleExpand}
              tooltip={inputState.isExpanded ? t('input.collapseInput') : t('input.expandInput')}
            >
              {inputState.isExpanded ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
            </IconButton>
            <div className="bitfun-chat-input__actions">
              <div className="bitfun-chat-input__actions-left">
                <div className="bitfun-chat-input__agent-boost" ref={agentBoostRef}>
                  <Tooltip content={t('chatInput.addBoostTooltip')}>
                    <IconButton
                      className="bitfun-chat-input__agent-boost-add"
                      variant="ghost"
                      size="xs"
                      aria-haspopup="menu"
                      aria-expanded={modeState.dropdownOpen}
                      onClick={e => {
                        e.stopPropagation();
                        dispatchMode({ type: 'TOGGLE_DROPDOWN' });
                      }}
                    >
                      <Plus size={14} strokeWidth={2.25} />
                    </IconButton>
                  </Tooltip>

                  {canSwitchModes && modeState.current !== 'agentic' && (
                    <div
                      className={`bitfun-chat-input__agent-capsule bitfun-chat-input__agent-capsule--${modeState.current === 'debug' ? 'debug' : modeState.current}`}
                    >
                      <span className="bitfun-chat-input__agent-capsule-label">
                        {t(`chatInput.modeNames.${modeState.current}`, { defaultValue: '' }) ||
                          modeState.available.find(m => m.id === modeState.current)?.name ||
                          modeState.current}
                      </span>
                      <button
                        type="button"
                        className="bitfun-chat-input__agent-capsule-close"
                        aria-label={t('chatInput.resetToAgentic')}
                        onClick={e => {
                          e.stopPropagation();
                          applyModeChange('agentic');
                          dispatchMode({ type: 'CLOSE_DROPDOWN' });
                        }}
                      >
                        <X size={12} strokeWidth={2.5} />
                      </button>
                    </div>
                  )}

                  {modeState.dropdownOpen && (
                    <div className="bitfun-chat-input__mode-dropdown bitfun-chat-input__mode-dropdown--agent-boost">
                      {canSwitchModes && (
                        <>
                          <div className="bitfun-chat-input__boost-section">
                            {incrementalCodeModes.length > 0 ? (
                              incrementalCodeModes.map(modeOption => {
                                const modeDescription =
                                  t(`chatInput.modeDescriptions.${modeOption.id}`, { defaultValue: '' }) ||
                                  modeOption.description ||
                                  modeOption.name;
                                const modeName =
                                  t(`chatInput.modeNames.${modeOption.id}`, { defaultValue: '' }) || modeOption.name;
                                return (
                                  <Tooltip key={modeOption.id} content={modeDescription} placement="left">
                                    <div
                                      className={`bitfun-chat-input__mode-option ${modeState.current === modeOption.id ? 'bitfun-chat-input__mode-option--active' : ''}`}
                                      onClick={e => {
                                        e.stopPropagation();
                                        requestModeChange(modeOption.id);
                                      }}
                                    >
                                      <span className="bitfun-chat-input__mode-option-name">{modeName}</span>
                                      {modeState.current === modeOption.id && (
                                        <span className="bitfun-chat-input__slash-command-current">{t('chatInput.current')}</span>
                                      )}
                                    </div>
                                  </Tooltip>
                                );
                              })
                            ) : (
                              <div className="bitfun-chat-input__agent-boost-empty bitfun-chat-input__agent-boost-empty--inline">
                                {t('chatInput.noIncrementalModes')}
                              </div>
                            )}
                          </div>

                          <div className="bitfun-chat-input__boost-section-divider" aria-hidden />
                        </>
                      )}

                      <div className="bitfun-chat-input__boost-section">
                        <div
                          role="button"
                          tabIndex={0}
                          className="bitfun-chat-input__boost-context-row"
                          onClick={handleBoostOpenAtContext}
                          onKeyDown={e => e.key === 'Enter' && handleBoostOpenAtContext(e)}
                        >
                          <Files size={14} className="bitfun-chat-input__boost-context-icon" aria-hidden />
                          <span>{t('chatInput.boostAddContext')}</span>
                        </div>

                        <div
                          role="button"
                          tabIndex={0}
                          className="bitfun-chat-input__boost-context-row"
                          onClick={handleBoostPickImage}
                          onKeyDown={e => e.key === 'Enter' && handleBoostPickImage(e as any)}
                        >
                          <Image size={14} className="bitfun-chat-input__boost-context-icon" aria-hidden />
                          <span>{t('input.addImage')}</span>
                        </div>

                        <div
                          ref={skillsHostRef}
                          className="bitfun-chat-input__boost-submenu-host"
                          onMouseEnter={openSkillsFlyout}
                          onMouseLeave={closeSkillsFlyout}
                        >
                          <div
                            role="button"
                            tabIndex={0}
                            className="bitfun-chat-input__boost-submenu-trigger"
                            aria-haspopup="menu"
                            aria-expanded={skillsFlyoutOpen}
                          >
                            <span className="bitfun-chat-input__boost-submenu-trigger-main">
                              <Sparkles size={14} className="bitfun-chat-input__boost-context-icon" aria-hidden />
                              <span>{t('chatInput.boostSkills')}</span>
                            </span>
                            <ChevronRight size={14} className="bitfun-chat-input__boost-submenu-chevron" aria-hidden />
                          </div>
                          <div
                            className={[
                              'bitfun-chat-input__boost-submenu-shell',
                              skillsFlyoutOpen ? 'bitfun-chat-input__boost-submenu-shell--open' : '',
                              skillsFlyoutLeft ? 'bitfun-chat-input__boost-submenu-shell--left' : '',
                              skillsFlyoutUp ? 'bitfun-chat-input__boost-submenu-shell--up' : '',
                            ].filter(Boolean).join(' ')}
                            onMouseEnter={openSkillsFlyout}
                            onMouseLeave={closeSkillsFlyout}
                          >
                            <div className="bitfun-chat-input__boost-submenu-panel">
                              {boostSkillsLoading ? (
                                <div className="bitfun-chat-input__boost-submenu-loading">
                                  <Loader2 size={14} className="bitfun-chat-input__boost-submenu-spinner" aria-hidden />
                                  <span>{t('chatInput.boostSkillsLoading')}</span>
                                </div>
                              ) : boostPanelSkills.length === 0 ? (
                                <div className="bitfun-chat-input__boost-submenu-empty">{t('chatInput.boostSkillsEmpty')}</div>
                              ) : (
                                <div className="bitfun-chat-input__boost-submenu-list">
                                  {boostPanelSkills.map(skill => (
                                    <div
                                      key={skill.name}
                                      role="button"
                                      tabIndex={0}
                                      className="bitfun-chat-input__boost-submenu-item"
                                      title={skill.description || skill.name}
                                      onClick={e => {
                                        e.stopPropagation();
                                        insertSkillIntoInput(skill.name);
                                      }}
                                      onKeyDown={e => e.key === 'Enter' && insertSkillIntoInput(skill.name)}
                                    >
                                      <Sparkles size={12} className="bitfun-chat-input__boost-submenu-item-icon" aria-hidden />
                                      <span className="bitfun-chat-input__boost-submenu-item-name">{skill.name}</span>
                                    </div>
                                  ))}
                                </div>
                              )}
                              <div
                                role="button"
                                tabIndex={0}
                                className="bitfun-chat-input__boost-submenu-manage"
                                onClick={handleOpenSkillsLibrary}
                                onKeyDown={e => e.key === 'Enter' && handleOpenSkillsLibrary(e as any)}
                              >
                                {t('chatInput.openSkillsLibrary')}
                              </div>
                            </div>
                          </div>
                        </div>

                        {!!currentSessionId && !isBtwSession && (
                          <>
                            <div className="bitfun-chat-input__boost-section-divider" aria-hidden />
                            <div
                              role="button"
                              tabIndex={0}
                              className="bitfun-chat-input__boost-context-row"
                              data-testid="chat-input-boost-start-btw"
                              onClick={handleBoostStartBtw}
                              onKeyDown={e => e.key === 'Enter' && handleBoostStartBtw(e)}
                            >
                              <MessageSquarePlus size={14} className="bitfun-chat-input__boost-context-icon" aria-hidden />
                              <span>{t('chatInput.boostStartBtw')}</span>
                            </div>
                          </>
                        )}
                      </div>
                    </div>
                  )}
                </div>

                <ModelSelector
                  currentMode={modeState.current}
                  sessionId={effectiveTargetSessionId || undefined}
                  currentTokens={tokenUsage.current}
                  maxTokens={tokenUsage.max}
                />
              </div>
              <div className="bitfun-chat-input__actions-right">
                {isCollapsedProcessing && !petReplacesStopChrome && (
                  <>
                    <span className="bitfun-chat-input__capsule-divider" />
                    <span className="bitfun-chat-input__cancel-shortcut">
                      <span className="bitfun-chat-input__space-key">Esc</span>
                      <span>{t('input.cancelShortcut')}</span>
                    </span>
                  </>
                )}

                {renderActionButton()}
              </div>
            </div>
          </div>
        </div>
      </div>
    </ContextDropZone>
    </>
  );
};

export default ChatInput;
