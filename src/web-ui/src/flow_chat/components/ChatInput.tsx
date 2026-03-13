/**
 * Standalone chat input component
 * Separated from bottom bar, supports session-level state awareness
 */

import React, { useRef, useCallback, useEffect, useReducer, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowUp, Image, Network, ChevronsUp, ChevronsDown, RotateCcw, FileText } from 'lucide-react';
import { ContextDropZone, useContextStore } from '../../shared/context-system';
import { useActiveSessionState } from '../hooks/useActiveSessionState';
import { RichTextInput, type MentionState } from './RichTextInput';
import { FileMentionPicker } from './FileMentionPicker';
import { globalEventBus } from '../../infrastructure/event-bus';
import { useSessionDerivedState, useSessionStateMachineActions, useSessionStateMachine } from '../hooks/useSessionStateMachine';
import { SessionExecutionEvent } from '../state-machine/types';
import TokenUsageIndicator from './TokenUsageIndicator';
import { ModelSelector } from './ModelSelector';
import { FlowChatStore } from '../store/FlowChatStore';
import type { FlowChatState, Session } from '../types/flow-chat';
import type { FileContext, DirectoryContext } from '../../shared/types/context';
import type { PromptTemplate } from '../../shared/types/prompt-template';
import { SmartRecommendations } from './smart-recommendations';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { WorkspaceKind } from '@/shared/types';
import { createImageContextFromFile, createImageContextFromClipboard } from '../utils/imageUtils';
import { notificationService } from '@/shared/notification-system';
import { TemplatePickerPanel } from './TemplatePickerPanel';
import { promptTemplateService } from '@/infrastructure/services/PromptTemplateService';
import { shortcutManager } from '@/infrastructure/services/ShortcutManager';
import { inputReducer, initialInputState } from '../reducers/inputReducer';
import { templateReducer, initialTemplateState } from '../reducers/templateReducer';
import { modeReducer, initialModeState } from '../reducers/modeReducer';
import { CHAT_INPUT_CONFIG } from '../constants/chatInputConfig';
import { MERMAID_INTERACTIVE_EXAMPLE } from '../constants/mermaidExamples';
import { useMessageSender } from '../hooks/useMessageSender';
import { useTemplateEditor } from '../hooks/useTemplateEditor';
import { useChatInputState } from '../store/chatInputStateStore';
import { useInputHistoryStore } from '../store/inputHistoryStore';
import { startBtwThread } from '../services/BtwThreadService';
import { createLogger } from '@/shared/utils/logger';
import { Tooltip, IconButton } from '@/component-library';
import './ChatInput.scss';

const log = createLogger('ChatInput');
const IME_ENTER_GUARD_MS = 120;

export interface ChatInputProps {
  className?: string;
  onSendMessage?: (message: string) => void;
}

export const ChatInput: React.FC<ChatInputProps> = ({
  className = '',
  onSendMessage
}) => {
  const { t } = useTranslation('flow-chat');
  
  const [inputState, dispatchInput] = useReducer(inputReducer, initialInputState);
  const [templateState, dispatchTemplate] = useReducer(templateReducer, initialTemplateState);
  const [modeState, dispatchMode] = useReducer(modeReducer, initialModeState);
  
  const richTextInputRef = useRef<HTMLDivElement>(null);
  const modeDropdownRef = useRef<HTMLDivElement>(null);
  const isImeComposingRef = useRef(false);
  const lastImeCompositionEndAtRef = useRef(0);
  
  // History navigation state
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [savedDraft, setSavedDraft] = useState('');
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
  const currentSessionId = activeSessionState.sessionId;
  
  // Get input history for current session (after currentSessionId is defined)
  const inputHistory = currentSessionId ? getSessionHistory(currentSessionId) : [];
  const derivedState = useSessionDerivedState(currentSessionId);
  const { transition, setQueuedInput } = useSessionStateMachineActions(currentSessionId);
  const stateMachine = useSessionStateMachine(currentSessionId);

  const { workspace, workspacePath } = useCurrentWorkspace();
  
  const [tokenUsage, setTokenUsage] = React.useState({ current: 0, max: 128128 });
  const isAssistantWorkspace = workspace?.workspaceKind === WorkspaceKind.Assistant;
  const canSwitchModes = !isAssistantWorkspace && modeState.current !== 'Cowork';

  // Session-level mode policy: Cowork sessions are fixed; code sessions should not switch into Cowork.
  const switchableModes = useMemo(
    () =>
      modeState.available.filter(mode =>
        mode.enabled &&
        mode.id !== 'Cowork' &&
        (isAssistantWorkspace || mode.id !== 'Claw')
      ),
    [isAssistantWorkspace, modeState.available]
  );
  
  const setChatInputActive = useChatInputState(state => state.setActive);
  const setChatInputExpanded = useChatInputState(state => state.setExpanded);

  // /btw now creates a child session ("side thread") and adds a lightweight marker on the parent session.
  const [btwOrigin, setBtwOrigin] = React.useState<Session['btwOrigin'] | null>(null);
  const [btwParentTitle, setBtwParentTitle] = React.useState<string>('');
  
  useEffect(() => {
    setChatInputActive(inputState.isActive);
  }, [inputState.isActive, setChatInputActive]);
  
  useEffect(() => {
    setChatInputExpanded(inputState.isExpanded);
  }, [inputState.isExpanded, setChatInputExpanded]);
  
  // Reset history index when switching sessions
  useEffect(() => {
    setHistoryIndex(-1);
  }, [currentSessionId]);
  
  const { sendMessage } = useMessageSender({
    currentSessionId: currentSessionId || undefined,
    contexts,
    onClearContexts: clearContexts,
    onSuccess: onSendMessage,
    onExitTemplateMode: () => {
      if (templateState.fillState?.isActive) {
        dispatchTemplate({ type: 'EXIT_FILL' });
        
        if (richTextInputRef.current) {
          const editor = richTextInputRef.current as HTMLElement;
          editor.innerHTML = '';
        }
      }
    },
    currentAgentType: modeState.current,
  });
  
  const {
    handleTemplateSelect: originalHandleTemplateSelect,
    exitTemplateMode,
    moveToNextPlaceholder,
    moveToPrevPlaceholder,
  } = useTemplateEditor({
    editorRef: richTextInputRef,
    templateFillState: templateState.fillState,
    onValueChange: (value: string) => dispatchInput({ type: 'SET_VALUE', payload: value }),
    onStartFill: (state) => dispatchTemplate({ type: 'START_FILL', payload: state }),
    onExitFill: () => dispatchTemplate({ type: 'EXIT_FILL' }),
    onUpdateCurrentIndex: (index) => dispatchTemplate({ type: 'UPDATE_CURRENT_INDEX', payload: index }),
    onNextPlaceholder: () => dispatchTemplate({ type: 'NEXT_PLACEHOLDER' }),
    onPrevPlaceholder: () => dispatchTemplate({ type: 'PREV_PLACEHOLDER' }),
  });
  
  const handleTemplateSelect = useCallback((template: PromptTemplate) => {
    dispatchInput({ type: 'ACTIVATE' });
    originalHandleTemplateSelect(template);
  }, [originalHandleTemplateSelect]);
  
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
    kind: 'modes' | 'actions';
    query: string;
    selectedIndex: number;
  }>({
    isActive: false,
    kind: 'modes',
    query: '',
    selectedIndex: 0,
  });
  
  React.useEffect(() => {
    const store = FlowChatStore.getInstance();
    
    const unsubscribe = store.subscribe((state: FlowChatState) => {
      if (currentSessionId) {
        const session = state.sessions.get(currentSessionId);
        if (session) {
          setTokenUsage({
            current: session.currentTokenUsage?.totalTokens || 0,
            max: session.maxContextTokens || 128128
          });
          const nextOrigin = (session.btwOrigin ||
            (session.sessionKind === 'btw' && session.parentSessionId ? { parentSessionId: session.parentSessionId } : null)) as any;
          setBtwOrigin(nextOrigin);
          const parentId = nextOrigin?.parentSessionId || session.parentSessionId;
          const parent = parentId ? state.sessions.get(parentId) : undefined;
          setBtwParentTitle(parent?.title || '');
        }
      }
    });

    if (currentSessionId) {
      const state = store.getState();
      const session = state.sessions.get(currentSessionId);
      if (session) {
        setTokenUsage({
          current: session.currentTokenUsage?.totalTokens || 0,
          max: session.maxContextTokens || 128128
        });
        const nextOrigin = (session.btwOrigin ||
          (session.sessionKind === 'btw' && session.parentSessionId ? { parentSessionId: session.parentSessionId } : null)) as any;
        setBtwOrigin(nextOrigin);
        const parentId = nextOrigin?.parentSessionId || session.parentSessionId;
        const parent = parentId ? state.sessions.get(parentId) : undefined;
        setBtwParentTitle(parent?.title || '');
      } else {
        setBtwOrigin(null);
        setBtwParentTitle('');
      }
    }

    return () => unsubscribe();
  }, [currentSessionId]);

  React.useEffect(() => {
    const initializeTemplateService = async () => {
      await promptTemplateService.initialize();
      
      const config = promptTemplateService.getConfig();
      const shortcutConfig = shortcutManager.parseShortcut(config.globalShortcut);
      
      if (shortcutConfig) {
        const unregister = shortcutManager.register(
          'prompt-template-picker',
          shortcutConfig,
          () => {
            dispatchTemplate({ type: 'OPEN_PICKER' });
          },
          {
            description: 'Open prompt template picker panel',
            priority: 10
          }
        );
        
        return unregister;
      }
    };
    
    const unregisterPromise = initializeTemplateService();
    
    return () => {
      unregisterPromise.then(unregister => {
        if (unregister) unregister();
      });
    };
  }, []);

  React.useEffect(() => {
    const handleFillInput = (event: Event) => {
      const customEvent = event as CustomEvent<{ message: string }>;
      const message = customEvent.detail?.message;
      
      if (message) {
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
  }, []);

  React.useEffect(() => {
    const handleFillChatInput = (data: { content: string }) => {
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
  }, []);

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
  }, [inputState.value, addContext, currentImageCount]);

  React.useEffect(() => {
    const handleInsertContextTag = (event: Event) => {
      const customEvent = event as CustomEvent<{ context: any }>;
      const context = customEvent.detail?.context;
      
      if (context) {
        if (richTextInputRef.current && (richTextInputRef.current as any).insertTag) {
          (richTextInputRef.current as any).insertTag(context);
          
          richTextInputRef.current.focus();
        }
      }
    };

    window.addEventListener('insert-context-tag', handleInsertContextTag);
    
    return () => {
      window.removeEventListener('insert-context-tag', handleInsertContextTag);
    };
  }, []);

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
    if (!currentSessionId) return;
    
    const store = FlowChatStore.getInstance();
    const state = store.getState();
    const session = state.sessions.get(currentSessionId);
    
    if (session?.mode) {
      log.debug('Session ID changed, syncing mode', { sessionId: currentSessionId, mode: session.mode });
      dispatchMode({ type: 'SET_CURRENT_MODE', payload: session.mode });
      try {
        sessionStorage.setItem('bitfun:flowchat:lastMode', session.mode);
      } catch {
        // ignore
      }
    }
  }, [currentSessionId]);

  React.useEffect(() => {
    if (!isAssistantWorkspace || modeState.current === 'Claw') {
      return;
    }

    dispatchMode({ type: 'SET_CURRENT_MODE', payload: 'Claw' });
  }, [isAssistantWorkspace, modeState.current]);

  React.useEffect(() => {
    const queuedInput = stateMachine?.context?.queuedInput;
    if (queuedInput && currentSessionId) {
      log.debug('Detected queuedInput, restoring message to input', { queuedInput });
      dispatchInput({ type: 'ACTIVATE' });
      dispatchInput({ type: 'SET_VALUE', payload: queuedInput });
      
      setQueuedInput(null);
      
      if (richTextInputRef.current) {
        richTextInputRef.current.focus();
      }
    }
  }, [stateMachine?.context?.queuedInput, currentSessionId, setQueuedInput, stateMachine?.context]);

  React.useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (modeDropdownRef.current && !modeDropdownRef.current.contains(event.target as Node)) {
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
  }, [addContext, currentImageCount]);

  React.useEffect(() => {
    if (!currentSessionId || !workspacePath) {
      return;
    }

    const store = FlowChatStore.getInstance();
    const state = store.getState();
    const session = state.sessions.get(currentSessionId);

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
          sessionId: currentSessionId,
          turnIndex: session.dialogTurns.length - 1,
          modifiedFiles: [...new Set(modifiedFiles)]
        });
      }
    }
  }, [currentSessionId, workspacePath, derivedState?.isProcessing]);
  
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
    
    dispatchInput({ type: 'SET_VALUE', payload: text });

    const isBtwCommand = text.trim().toLowerCase().startsWith('/btw');
    const isProcessing = !!derivedState?.isProcessing;

    // Don't queue /btw while the main session is processing; /btw runs independently.
    if (derivedState?.isProcessing && !isBtwCommand) {
      setQueuedInput(text);
    }

    if (text.startsWith('/')) {
      const afterSlash = text.slice(1);
      const hasWhitespace = /\s/.test(afterSlash);
      const firstToken = afterSlash.trimStart().split(/\s+/, 1)[0]?.toLowerCase?.() ?? '';
      const query = firstToken;

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

      // When idle, keep the picker for mode switching, but don't interfere with /btw being a real command.
      if (!isBtwCommand) {
        setSlashCommandState({
          isActive: true,
          kind: 'modes',
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
  }, [contexts, derivedState, inputState.isActive, removeContext, setQueuedInput, slashCommandState.isActive, slashCommandState.kind]);

  const submitBtwFromInput = useCallback(async () => {
    if (!derivedState) return;
    if (!currentSessionId) {
      notificationService.error(t('btw.noSession', { defaultValue: 'No active session for /btw' }));
      return;
    }

    const message = inputState.value.trim();
    const question = message.replace(/^\/btw\b/i, '').trim();

    // Clear input without adding to main history.
    dispatchInput({ type: 'CLEAR_VALUE' });
    setQueuedInput(null);
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

    if (!question) {
      notificationService.warning(t('btw.empty', { defaultValue: 'Please provide a question after /btw' }));
      return;
    }

    try {
      await startBtwThread({
        parentSessionId: currentSessionId,
        workspacePath,
        question,
        modelId: 'fast',
        maxContextMessages: 60,
      });
    } catch (e) {
      log.error('Failed to start /btw thread', { e });
    }
  }, [currentSessionId, derivedState, inputState.value, setQueuedInput, t, workspacePath]);
  
  const handleSendOrCancel = useCallback(async () => {
    if (!derivedState) return;
    
    const { sendButtonMode } = derivedState;
    
    if (sendButtonMode === 'cancel') {
      await transition(SessionExecutionEvent.USER_CANCEL);
      return;
    }
    
    if (sendButtonMode === 'retry') {
      await transition(SessionExecutionEvent.RESET);
    }
    
    if (!inputState.value.trim()) return;
    
    const message = inputState.value.trim();

    if (message.toLowerCase().startsWith('/btw')) {
      // When idle, /btw can be sent via the normal send button.
      await submitBtwFromInput();
      return;
    }
    
    // Add to history before clearing (session-scoped)
    if (currentSessionId) {
      addToHistory(currentSessionId, message);
    }
    setHistoryIndex(-1);
    setSavedDraft('');
    
    dispatchInput({ type: 'CLEAR_VALUE' });
    
    try {
      await sendMessage(message);
      dispatchInput({ type: 'CLEAR_VALUE' });
    } catch (error) {
      log.error('Failed to send message', { error });
      dispatchInput({ type: 'SET_VALUE', payload: message });
    }
  }, [inputState.value, derivedState, transition, sendMessage, addToHistory, setQueuedInput, submitBtwFromInput]);
  
  const getFilteredModes = useCallback(() => {
    if (!canSwitchModes) {
      return [];
    }
    if (!slashCommandState.query) {
      return switchableModes;
    }
    return switchableModes.filter(mode =>
      mode.name.toLowerCase().includes(slashCommandState.query) ||
      mode.id.toLowerCase().includes(slashCommandState.query)
    );
  }, [canSwitchModes, switchableModes, slashCommandState.query]);

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

    if (currentSessionId) {
      FlowChatStore.getInstance().updateSessionMode(currentSessionId, modeId);
    }
  }, [currentSessionId]);

  const requestModeChange = useCallback((modeId: string) => {
    if (!canSwitchModes) {
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      return;
    }

    if (modeId === modeState.current) {
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      return;
    }

    if (!switchableModes.some(mode => mode.id === modeId)) {
      dispatchMode({ type: 'CLOSE_DROPDOWN' });
      return;
    }

    applyModeChange(modeId);
    dispatchMode({ type: 'CLOSE_DROPDOWN' });
  }, [applyModeChange, modeState.current, canSwitchModes, switchableModes]);
  
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

  const getFilteredActions = useCallback(() => {
    // For now we only support one action: /btw.
    const items = [
      {
        id: 'btw',
        command: '/btw',
        label: t('btw.title', { defaultValue: 'Side question' }),
      },
    ];

    const q = (slashCommandState.query || '').trim().toLowerCase();
    if (!q) return items;

    return items.filter(i => {
      const cmd = i.command.slice(1).toLowerCase();
      return cmd.includes(q) || i.label.toLowerCase().includes(q);
    });
  }, [slashCommandState.query, t]);

  const selectSlashCommandAction = useCallback((actionId: string) => {
    if (actionId !== 'btw') return;

    const raw = inputState.value || '';
    const lower = raw.trimStart().toLowerCase();

    let next = raw;
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

    dispatchInput({ type: 'SET_VALUE', payload: next });
    setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });
    window.setTimeout(() => richTextInputRef.current?.focus(), 0);
  }, [inputState.value]);
  
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    // Local /btw shortcut (Ctrl/Cmd+Alt+B) should work even when ChatInput is focused.
    if ((e.ctrlKey || e.metaKey) && e.altKey && !e.shiftKey && e.key.toLowerCase() === 'b') {
      e.preventDefault();
      e.stopPropagation();

      if (!currentSessionId) {
        notificationService.error(t('btw.noSession', { defaultValue: 'No active session for /btw' }));
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
        const items = slashCommandState.kind === 'modes' ? getFilteredModes() : getFilteredActions();
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
            } else {
              const action = items[slashCommandState.selectedIndex] as any;
              selectSlashCommandAction(action.id);
            }
          }
          return;
        }
        
        if (e.key === 'Escape') {
          e.preventDefault();
          const kind = slashCommandState.kind;
          setSlashCommandState({ isActive: false, kind: 'modes', query: '', selectedIndex: 0 });

          // For mode switching picker, "/" is just a trigger and should be cleared on cancel.
          if (kind === 'modes') {
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
            } else {
              const action = items[slashCommandState.selectedIndex] as any;
              selectSlashCommandAction(action.id);
            }
          }
          return;
        }
      }
    }
    
    if (templateState.fillState?.isActive) {
      if (e.key === 'Tab') {
        e.preventDefault();
        
        if (e.shiftKey) {
          moveToPrevPlaceholder();
        } else {
          moveToNextPlaceholder();
        }
        return;
      }
      
      if (e.key === 'Escape') {
        e.preventDefault();
        exitTemplateMode();
        return;
      }
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
        return;
      }

      if (templateState.fillState?.isActive) {
        exitTemplateMode();
      }
      handleSendOrCancel();
    }
    
    if (e.key === 'Escape' && derivedState?.canCancel) {
      e.preventDefault();
      transition(SessionExecutionEvent.USER_CANCEL);
    }
  }, [handleSendOrCancel, submitBtwFromInput, derivedState, transition, templateState.fillState, moveToNextPlaceholder, moveToPrevPlaceholder, exitTemplateMode, slashCommandState, getFilteredModes, getFilteredActions, selectSlashCommandMode, selectSlashCommandAction, canSwitchModes, historyIndex, inputHistory, savedDraft, inputState.value, currentSessionId, t]);

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
  }, [addContext, currentImageCount]);
  
  const handleMermaidEditor = useCallback(() => {
    window.dispatchEvent(new CustomEvent('expand-right-panel'));
    
    setTimeout(() => {
      const event = new CustomEvent('agent-create-tab', {
        detail: {
          type: 'mermaid-editor',
          title: t('input.mermaidDualModeDemo'),
          data: MERMAID_INTERACTIVE_EXAMPLE,
          metadata: {
            duplicateCheckKey: 'mermaid-dual-mode-demo'
          },
          checkDuplicate: true,
          duplicateCheckKey: 'mermaid-dual-mode-demo',
          replaceExisting: false
        }
      });
      window.dispatchEvent(event);
    }, 250);
  }, []);
  
  const toggleExpand = useCallback(() => {
    dispatchInput({ type: 'TOGGLE_EXPAND' });
  }, []);
  
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
      setTimeout(() => {
        if (richTextInputRef.current) {
          richTextInputRef.current.focus();
        }
      }, 50);
    }
  }, [inputState.isActive]);
  
  const containerRef = useRef<HTMLDivElement>(null);
  
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        inputState.isActive &&
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        if (inputState.value.trim() === '') {
          dispatchInput({ type: 'DEACTIVATE' });
        }
      }
    };
    
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [inputState.isActive, inputState.value]);
  
  const renderActionButton = () => {
    if (!derivedState) return <IconButton className="bitfun-chat-input__send-button" disabled size="small"><ArrowUp size={11} /></IconButton>;
    
    const { sendButtonMode, hasQueuedInput } = derivedState;
    
    if (sendButtonMode === 'cancel') {
      return (
        <Tooltip content={t('input.stopGeneration')}>
          <div className="bitfun-chat-input__send-button bitfun-chat-input__send-button--breathing" onClick={handleSendOrCancel}>
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
      <TemplatePickerPanel
        isOpen={templateState.isPickerOpen}
        onClose={() => dispatchTemplate({ type: 'CLOSE_PICKER' })}
        onSelect={handleTemplateSelect}
      />
      
      <ContextDropZone
        acceptedTypes={['file', 'directory', 'image', 'code-snippet', 'mermaid-diagram']}
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
          className={`bitfun-chat-input ${inputState.isActive ? 'bitfun-chat-input--active' : 'bitfun-chat-input--collapsed'} ${inputState.isExpanded ? 'bitfun-chat-input--expanded' : ''} ${derivedState?.isProcessing ? 'bitfun-chat-input--processing' : ''} ${className} ${templateState.fillState?.isActive ? 'bitfun-chat-input--template-mode' : ''}`}
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
          {!!btwOrigin?.parentSessionId && (
            <div className="bitfun-chat-input__btw-origin" data-testid="btw-origin-banner">
              <div className="bitfun-chat-input__btw-origin-text">
                <span className="bitfun-chat-input__btw-origin-label">
                  {t('btw.origin', { defaultValue: 'Asked from' })}
                </span>
                <span className="bitfun-chat-input__btw-origin-parent">
                  {btwParentTitle || t('btw.parent', { defaultValue: 'parent session' })}
                </span>
                {btwOrigin?.parentTurnIndex ? (
                  <span className="bitfun-chat-input__btw-origin-turn">
                    {t('btw.turnLabel', { index: btwOrigin.parentTurnIndex, defaultValue: `turn ${btwOrigin.parentTurnIndex}` })}
                  </span>
                ) : null}
              </div>
              <IconButton
                className="bitfun-chat-input__btw-origin-back"
                variant="ghost"
                size="xs"
                onClick={() => {
                  const parentId = btwOrigin?.parentSessionId;
                  if (!parentId) return;
                  const requestId = btwOrigin?.requestId;
                  const itemId = requestId ? `btw_marker_${requestId}` : undefined;
                  globalEventBus.emit('flowchat:focus-item', {
                    sessionId: parentId,
                    turnIndex: btwOrigin?.parentTurnIndex,
                    itemId,
                  }, 'ChatInput');
                }}
                tooltip={t('btw.backToParent', { defaultValue: 'Back to parent session' })}
                disabled={!btwOrigin?.parentSessionId}
              >
                {t('btw.back', { defaultValue: 'Back' })}
              </IconButton>
            </div>
          )}

          {templateState.fillState?.isActive && (
<div className="bitfun-chat-input__template-hint">
                <span className="bitfun-chat-input__template-hint-text" dangerouslySetInnerHTML={{ __html: t('chatInput.templateHint') }} />
                <span className="bitfun-chat-input__template-hint-progress">
                  {t('chatInput.templateProgress', { current: templateState.fillState.currentIndex + 1, total: templateState.fillState.placeholders.length })}
                </span>
              </div>
          )}
          
          <div className={`bitfun-chat-input__box ${inputState.isExpanded ? 'bitfun-chat-input__box--expanded' : ''}`}>
            <div className="bitfun-chat-input__input-area">
              <RichTextInput
                ref={richTextInputRef}
                value={inputState.value}
                onChange={handleInputChange}
                onKeyDown={handleKeyDown}
                onCompositionStart={handleImeCompositionStart}
                onCompositionEnd={handleImeCompositionEnd}
                placeholder={t('input.placeholder')}
                disabled={false}
                contexts={contexts}
                onRemoveContext={removeContext}
                onMentionStateChange={setMentionState}
                data-testid="chat-input-textarea"
              />
              
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

                if (!canSwitchModes) return null;

                const filteredModes = getFilteredModes();
                return (
                  <div className="bitfun-chat-input__slash-command-picker">
                    <div className="bitfun-chat-input__slash-command-header">
                      <span>{t('chatInput.switchMode')}</span>
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
              
            {derivedState?.hasQueuedInput && (
              <div className="bitfun-chat-input__queued-indicator">
                <span>{t('input.willSendAfterStop')}</span>
                <IconButton
                  className="bitfun-chat-input__queued-clear"
                  variant="ghost"
                  size="xs"
                  onClick={() => {
                    if (templateState.fillState?.isActive) {
                      dispatchTemplate({ type: 'EXIT_FILL' });
                      
                      if (richTextInputRef.current) {
                        const editor = richTextInputRef.current as HTMLElement;
                        editor.innerHTML = '';
                      }
                    }
                    
                    dispatchInput({ type: 'CLEAR_VALUE' });
                    setQueuedInput(null);
                  }}
                >
                  {t('input.clear')}
                </IconButton>
              </div>
            )}
            </div>
            
            <IconButton
              className="bitfun-chat-input__expand-button"
              variant="ghost"
              size="xs"
              onClick={toggleExpand}
              tooltip={inputState.isExpanded ? t('input.collapseInput') : t('input.expandInput')}
            >
              {inputState.isExpanded ? <ChevronsDown size={14} /> : <ChevronsUp size={14} />}
            </IconButton>
            <div className="bitfun-chat-input__actions">
              <div className="bitfun-chat-input__actions-left">
                <ModelSelector
                  currentMode={modeState.current}
                  sessionId={currentSessionId || undefined}
                />
                
                {tokenUsage.current > 0 && (
                  <TokenUsageIndicator
                    currentTokens={tokenUsage.current}
                    maxTokens={tokenUsage.max}
                  />
                )}
              </div>
              <div className="bitfun-chat-input__actions-right">
                {canSwitchModes && (
                  <div 
                    className="bitfun-chat-input__mode-selector"
                    ref={modeDropdownRef}
                  >
                    <IconButton
                      className={`bitfun-chat-input__mode-selector-button${modeState.current !== 'agentic' ? ` bitfun-chat-input__mode-selector-button--${modeState.current}` : ''}`}
                      variant="ghost"
                      size="xs"
                      onClick={() => dispatchMode({ type: 'TOGGLE_DROPDOWN' })}
                      tooltip={t('chatInput.currentMode', { mode: t(`chatInput.modeNames.${modeState.current}`, { defaultValue: '' }) || modeState.available.find(m => m.id === modeState.current)?.name || modeState.current })}
                    >
                      {t(`chatInput.modeNames.${modeState.current}`, { defaultValue: '' }) || modeState.available.find(m => m.id === modeState.current)?.name || modeState.current}
                    </IconButton>
                  {modeState.dropdownOpen && (() => {
                    const modeOrder = ['agentic', 'Claw', 'Plan', 'debug'];
                    
                    const sortedModes = [...switchableModes].sort((a, b) => {
                      const aIndex = modeOrder.indexOf(a.id);
                      const bIndex = modeOrder.indexOf(b.id);
                      if (aIndex === -1 && bIndex === -1) return 0;
                      if (aIndex === -1) return 1;
                      if (bIndex === -1) return -1;
                      return aIndex - bIndex;
                    });
                    
                    const renderModeOption = (modeOption: typeof switchableModes[0]) => {
                      const modeDescription = t(`chatInput.modeDescriptions.${modeOption.id}`, { defaultValue: '' }) || modeOption.description || modeOption.name;
                      const modeName = t(`chatInput.modeNames.${modeOption.id}`, { defaultValue: '' }) || modeOption.name;
                      return (
                      <Tooltip key={modeOption.id} content={modeDescription} placement="left">
                        <div
                          className={`bitfun-chat-input__mode-option ${modeState.current === modeOption.id ? 'bitfun-chat-input__mode-option--active' : ''}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            requestModeChange(modeOption.id);
                          }}
                        >
                          <span className="bitfun-chat-input__mode-option-name">{modeName}</span>
                          {!modeOrder.includes(modeOption.id) && <span className="bitfun-chat-input__mode-option-badge bitfun-chat-input__mode-option-badge--wip">{t('chatInput.wip')}</span>}
                        </div>
                      </Tooltip>
                      );
                    };
                    
                    return (
                      <div className="bitfun-chat-input__mode-dropdown">
                        {sortedModes.map(m => renderModeOption(m))}
                      </div>
                    );
                  })()}
                  </div>
                )}
                
                <IconButton
                  className="bitfun-chat-input__action-button"
                  variant="ghost"
                  size="xs"
                  onClick={handleImageInput}
                  tooltip={t('input.addImage')}
                >
                  <Image size={12} />
                </IconButton>
                
                <IconButton
                  className="bitfun-chat-input__action-button"
                  variant="ghost"
                  size="xs"
                  onClick={handleMermaidEditor}
                  tooltip={t('input.openMermaidEditor')}
                >
                  <Network size={12} />
                </IconButton>
                
                <IconButton
                  className="bitfun-chat-input__action-button"
                  variant="ghost"
                  size="xs"
                  onClick={() => dispatchTemplate({ type: 'TOGGLE_PICKER' })}
                  tooltip={t('input.selectPromptTemplate')}
                >
                  <FileText size={12} />
                </IconButton>

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
