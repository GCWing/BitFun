/**
 * Standalone chat input component
 * Separated from bottom bar, supports session-level state awareness
 */

import React, { useRef, useCallback, useEffect, useReducer, useState } from 'react';
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
import type { FlowChatState } from '../types/flow-chat';
import type { FileContext, DirectoryContext } from '../../shared/types/context';
import type { PromptTemplate } from '../../shared/types/prompt-template';
import { SmartRecommendations } from './smart-recommendations';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
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
import { createLogger } from '@/shared/utils/logger';
import { Tooltip, IconButton } from '@/component-library';
import './ChatInput.scss';

const log = createLogger('ChatInput');

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
  
  const contexts = useContextStore(state => state.contexts);
  const addContext = useContextStore(state => state.addContext);
  const removeContext = useContextStore(state => state.removeContext);
  const clearContexts = useContextStore(state => state.clearContexts);
  
  const activeSessionState = useActiveSessionState();
  const currentSessionId = activeSessionState.sessionId;
  const derivedState = useSessionDerivedState(currentSessionId);
  const { transition, setQueuedInput } = useSessionStateMachineActions(currentSessionId);
  const stateMachine = useSessionStateMachine(currentSessionId);
  const isProcessing = derivedState?.isProcessing || false;

  const { workspacePath } = useCurrentWorkspace();
  
  const [tokenUsage, setTokenUsage] = React.useState({ current: 0, max: 128128 });
  
  const setChatInputActive = useChatInputState(state => state.setActive);
  const setChatInputExpanded = useChatInputState(state => state.setExpanded);
  
  useEffect(() => {
    setChatInputActive(inputState.isActive);
  }, [inputState.isActive, setChatInputActive]);
  
  useEffect(() => {
    setChatInputExpanded(inputState.isExpanded);
  }, [inputState.isExpanded, setChatInputExpanded]);
  
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
    query: string;
    selectedIndex: number;
  }>({
    isActive: false,
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

        // Handle image attachments
        for (const block of params.content) {
          if (block.type === 'image') {
            try {
              // Convert base64 to File object
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
  }, [inputState.value, addContext]);

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
    }
  }, [currentSessionId]);

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
  }, [addContext]);

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
  
  const handleInputChange = useCallback((text: string) => {
    if (!inputState.isActive && text.length > 0) {
      dispatchInput({ type: 'ACTIVATE' });
    }
    
    dispatchInput({ type: 'SET_VALUE', payload: text });
    
    if (derivedState?.isProcessing) {
      setQueuedInput(text);
    }
    
    if (text.startsWith('/')) {
      const query = text.slice(1).toLowerCase();
      setSlashCommandState({
        isActive: true,
        query,
        selectedIndex: 0,
      });
    } else {
      if (slashCommandState.isActive) {
        setSlashCommandState({
          isActive: false,
          query: '',
          selectedIndex: 0,
        });
      }
    }
  }, [derivedState, setQueuedInput, inputState.isActive, slashCommandState.isActive]);
  
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
    
    dispatchInput({ type: 'CLEAR_VALUE' });
    
    try {
      await sendMessage(message);
      dispatchInput({ type: 'CLEAR_VALUE' });
    } catch (error) {
      log.error('Failed to send message', { error });
      dispatchInput({ type: 'SET_VALUE', payload: message });
    }
  }, [inputState.value, derivedState, transition, sendMessage]);
  
  const getFilteredModes = useCallback(() => {
    const enabledModes = modeState.available.filter(mode => mode.enabled);
    if (!slashCommandState.query) {
      return enabledModes;
    }
    return enabledModes.filter(mode => 
      mode.name.toLowerCase().includes(slashCommandState.query) ||
      mode.id.toLowerCase().includes(slashCommandState.query)
    );
  }, [modeState.available, slashCommandState.query]);
  
  const selectSlashCommandMode = useCallback((modeId: string) => {
    dispatchMode({ 
      type: 'SET_CURRENT_MODE', 
      payload: modeId 
    });
    
    if (currentSessionId) {
      FlowChatStore.getInstance().updateSessionMode(currentSessionId, modeId);
    }
    
    dispatchInput({ type: 'CLEAR_VALUE' });
    setSlashCommandState({
      isActive: false,
      query: '',
      selectedIndex: 0,
    });
  }, [currentSessionId]);
  
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (slashCommandState.isActive) {
      const filteredModes = getFilteredModes();
      
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSlashCommandState(prev => ({
          ...prev,
          selectedIndex: Math.min(prev.selectedIndex + 1, filteredModes.length - 1),
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
        if (filteredModes.length > 0) {
          selectSlashCommandMode(filteredModes[slashCommandState.selectedIndex].id);
        }
        return;
      }
      
      if (e.key === 'Escape') {
        e.preventDefault();
        setSlashCommandState({
          isActive: false,
          query: '',
          selectedIndex: 0,
        });
        dispatchInput({ type: 'CLEAR_VALUE' });
        return;
      }
      
      if (e.key === 'Tab') {
        e.preventDefault();
        if (filteredModes.length > 0) {
          selectSlashCommandMode(filteredModes[slashCommandState.selectedIndex].id);
        }
        return;
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
    
    const isComposing = (e.nativeEvent as KeyboardEvent).isComposing;
    
    if (e.key === 'Enter' && !e.shiftKey) {
      if (isComposing) {
        return;
      }
      
      e.preventDefault();
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
  }, [handleSendOrCancel, derivedState, transition, templateState.fillState, moveToNextPlaceholder, moveToPrevPlaceholder, exitTemplateMode, slashCommandState, getFilteredModes, selectSlashCommandMode]);
  
  const handleImageInput = useCallback(() => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = CHAT_INPUT_CONFIG.image.acceptedTypes.join(',');
    input.multiple = true;
    
    input.onchange = async (e) => {
      const files = (e.target as HTMLInputElement).files;
      if (!files || files.length === 0) return;
      
      if (files.length > CHAT_INPUT_CONFIG.image.maxCount) {
        notificationService.warning(t('input.maxImagesWarning', { count: CHAT_INPUT_CONFIG.image.maxCount }), { duration: 3000 });
        return;
      }
      
      const fileArray = Array.from(files);
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
  }, [addContext]);
  
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
                  const enabledModes = modeState.available.filter(mode => mode.enabled);
                  
                  const modeOrder = ['agentic', 'Plan', 'debug'];
                  
                  const sortedModes = [...enabledModes].sort((a, b) => {
                    const aIndex = modeOrder.indexOf(a.id);
                    const bIndex = modeOrder.indexOf(b.id);
                    if (aIndex === -1 && bIndex === -1) return 0;
                    if (aIndex === -1) return 1;
                    if (bIndex === -1) return -1;
                    return aIndex - bIndex;
                  });
                  
                  const renderModeOption = (modeOption: typeof enabledModes[0]) => {
                    const modeDescription = t(`chatInput.modeDescriptions.${modeOption.id}`, { defaultValue: '' }) || modeOption.description || modeOption.name;
                    const modeName = t(`chatInput.modeNames.${modeOption.id}`, { defaultValue: '' }) || modeOption.name;
                    return (
                    <Tooltip key={modeOption.id} content={modeDescription} placement="left">
                      <div
                        className={`bitfun-chat-input__mode-option ${modeState.current === modeOption.id ? 'bitfun-chat-input__mode-option--active' : ''}`}
                        onClick={(e) => {
                          e.stopPropagation();
                          if (modeOption.id !== modeState.current) {
                            dispatchMode({ 
                              type: 'SET_CURRENT_MODE', 
                              payload: modeOption.id 
                            });
                            
                            if (currentSessionId) {
                              FlowChatStore.getInstance().updateSessionMode(currentSessionId, modeOption.id);
                            }
                          }
                          dispatchMode({ type: 'CLOSE_DROPDOWN' });
                        }}
                      >
                        <span className="bitfun-chat-input__mode-option-name">{modeName}</span>
                        {!['agentic', 'Plan', 'debug'].includes(modeOption.id) && <span className="bitfun-chat-input__mode-option-badge bitfun-chat-input__mode-option-badge--wip">{t('chatInput.wip')}</span>}
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
