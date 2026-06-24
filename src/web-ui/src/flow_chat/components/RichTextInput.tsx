/**
 * Rich text input component.
 * Supports inserting file tags inline and using @ to select files/folders.
 */

import React, { useRef, useEffect, useCallback, useState } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Puzzle } from 'lucide-react';
import type { ContextItem } from '../../shared/types/context';
import { getRichTextExternalSyncAction } from './richTextInputSync';
import {
  getWidgetPromptReferenceMatches,
  parseWidgetPromptReferenceToken,
} from '@/tools/generative-widget/widgetPromptReference';
import {
  getSkillPromptReferenceMatches,
  parseSkillPromptReferenceToken,
} from '../utils/skillPromptReference';
import './RichTextInput.scss';

const SKILL_REFERENCE_BADGE_ICON = renderToStaticMarkup(
  <Puzzle size={12} strokeWidth={2.2} aria-hidden="true" />,
);

/** @ mention state */
export interface MentionState {
  isActive: boolean;
  query: string;
  startOffset: number;  // Position of the @ symbol in text
}

export interface InlineTriggerState {
  isActive: boolean;
  trigger: '/' | '$' | null;
  query: string;
  startOffset: number;
}

export interface RichTextInputProps
  extends Omit<
    React.HTMLAttributes<HTMLDivElement>,
    'onChange' | 'onFocus' | 'onBlur' | 'onCompositionStart' | 'onCompositionEnd'
  > {
  value: string;
  onChange: (value: string, contexts: ContextItem[]) => void;
  onLargePaste?: (text: string) => string | null;
  onKeyDown?: (e: React.KeyboardEvent) => void;
  onCompositionStart?: () => void;
  onCompositionEnd?: () => void;
  onFocus?: () => void;
  onBlur?: () => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  contexts: ContextItem[];
  onRemoveContext: (id: string) => void;
  /** Callback when @ mention state changes */
  onMentionStateChange?: (state: MentionState) => void;
  /** Callback when inline trigger state changes for / or $ */
  onInlineTriggerStateChange?: (state: InlineTriggerState) => void;
}

function isWhitespaceCharacter(char: string | undefined): boolean {
  return !char || /\s/.test(char);
}

function getContextDisplayName(context: ContextItem): string {
  switch (context.type) {
    case 'file': return context.fileName;
    case 'directory': return context.directoryName;
    case 'code-snippet': return `${context.fileName}:${context.startLine}-${context.endLine}`;
    case 'pull-request': return context.label;
    case 'image': return context.imageName;
    case 'terminal-command': return context.command;
    case 'git-ref': return context.refValue;
    case 'url': return context.title || context.url;
    case 'mermaid-node': return context.nodeText;
    case 'mermaid-diagram': return context.diagramTitle || 'Mermaid diagram';
    case 'web-element': return context.tagName;
    default: {
      const exhaustive: never = context;
      return String(exhaustive);
    }
  }
}

function getContextTagFormat(context: ContextItem): string {
  switch (context.type) {
    case 'file': return `#file:${context.fileName}`;
    case 'directory': return `#dir:${context.directoryName}`;
    case 'code-snippet': return `#code:${context.fileName}:${context.startLine}-${context.endLine}`;
    case 'pull-request': return `#pr:${context.label.replace(/\s+/g, '_')}`;
    case 'image': return `#img:${context.imageName}`;
    case 'terminal-command': return `#cmd:${context.command}`;
    case 'git-ref': return `#git:${context.refValue}`;
    case 'url': return `#link:${context.title || context.url}`;
    case 'mermaid-node': return `#chart:${context.nodeText}`;
    case 'mermaid-diagram': return `#mermaid:${context.diagramTitle || 'Mermaid diagram'}`;
    case 'web-element': return `#element:${context.tagName}`;
    default: {
      const exhaustive: never = context;
      return String(exhaustive);
    }
  }
}

function getContextFullPath(context: ContextItem): string {
  switch (context.type) {
    case 'file':
      return context.filePath;
    case 'directory':
      return context.directoryPath + (context.recursive ? ' (recursive)' : '');
    case 'code-snippet':
      return `${context.filePath} (lines ${context.startLine}-${context.endLine})`;
    case 'pull-request':
      return [
        context.repository,
        context.remoteId ? `remote:${context.remoteId}` : null,
        context.pullRequestNumber ? `PR #${context.pullRequestNumber}` : null,
        context.section,
        context.sourceUrl,
      ].filter(Boolean).join(' · ') || context.label;
    case 'image':
      return context.imagePath;
    case 'terminal-command':
      return context.workingDirectory ? `${context.command} @ ${context.workingDirectory}` : context.command;
    case 'git-ref':
      return `Git ${context.refType}: ${context.refValue}`;
    case 'url':
      return context.url;
    case 'mermaid-node':
      return context.diagramTitle ? `${context.diagramTitle} - ${context.nodeText}` : context.nodeText;
    case 'mermaid-diagram':
      return `Mermaid diagram${context.diagramTitle ? ': ' + context.diagramTitle : ''} (${context.diagramCode.length} chars)`;
    case 'web-element':
      return context.path;
    default: {
      const exhaustive: never = context;
      return String(exhaustive);
    }
  }
}

export const RichTextInput = React.forwardRef<HTMLDivElement, RichTextInputProps>(({
  value,
  onChange,
  onLargePaste,
  onKeyDown,
  onCompositionStart,
  onCompositionEnd,
  onFocus,
  onBlur,
  placeholder = 'Describe your request...',
  disabled = false,
  className = '',
  contexts,
  onRemoveContext,
  onMentionStateChange,
  onInlineTriggerStateChange,
  ...restProps
}, ref) => {
  const editorRef = useRef<HTMLDivElement>(null);
  const internalRef = (ref as React.RefObject<HTMLDivElement>) || editorRef;
  const [isFocused, setIsFocused] = useState(false);
  const isComposingRef = useRef(false);
  const lastContextIdsRef = useRef<Set<string>>(new Set());
  const mentionStateRef = useRef<MentionState>({ isActive: false, query: '', startOffset: 0 });
  const inlineTriggerStateRef = useRef<InlineTriggerState>({
    isActive: false,
    trigger: null,
    query: '',
    startOffset: 0,
  });
  const triggerSyncRef = useRef<(() => void) | null>(null);

  const closeMention = useCallback(() => {
    if (!mentionStateRef.current.isActive) {
      return;
    }

    mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
    onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
  }, [onMentionStateChange]);

  const closeInlineTrigger = useCallback(() => {
    if (!inlineTriggerStateRef.current.isActive) {
      return;
    }

    inlineTriggerStateRef.current = {
      isActive: false,
      trigger: null,
      query: '',
      startOffset: 0,
    };
    onInlineTriggerStateChange?.({
      isActive: false,
      trigger: null,
      query: '',
      startOffset: 0,
    });
  }, [onInlineTriggerStateChange]);

  // Create tag element with pill style
  const createTagElement = useCallback((context: ContextItem): HTMLSpanElement => {
    const tag = document.createElement('span');
    tag.className = 'rich-text-tag-pill';
    tag.contentEditable = 'false';
    tag.dataset.contextId = context.id;
    tag.dataset.contextType = context.type;
    // Store full tag format for text extraction
    tag.dataset.tagFormat = getContextTagFormat(context);
    tag.title = getContextFullPath(context);
    
    const text = document.createElement('span');
    text.className = 'rich-text-tag-pill__text';
    // Show name only, no # prefix
    text.textContent = getContextDisplayName(context);
    
    const remove = document.createElement('button');
    remove.className = 'rich-text-tag-pill__remove';
    remove.textContent = '×';
    remove.title = 'Remove';
    remove.onclick = (e) => {
      e.preventDefault();
      e.stopPropagation();
      onRemoveContext(context.id);
    };
    
    tag.appendChild(text);
    tag.appendChild(remove);
    
    return tag;
  }, [onRemoveContext]);

  const removeInlineTokenElement = useCallback((element: HTMLElement) => {
    const nextSibling = element.nextSibling;
    if (nextSibling && nextSibling.nodeType === Node.TEXT_NODE && nextSibling.textContent === ' ') {
      nextSibling.remove();
    }
    element.remove();
  }, []);

  const createWidgetReferenceElement = useCallback((token: string): HTMLSpanElement | null => {
    const payload = parseWidgetPromptReferenceToken(token);
    if (!payload) {
      return null;
    }

    const tag = document.createElement('span');
    tag.className = 'rich-text-tag-pill rich-text-tag-pill--widget-ref';
    tag.contentEditable = 'false';
    tag.dataset.tagFormat = token;
    tag.dataset.inlineTokenType = 'widget-ref';
    tag.title = payload.promptText;

    const badge = document.createElement('span');
    badge.className = 'rich-text-tag-pill__badge';
    badge.textContent = 'UI';

    const text = document.createElement('span');
    text.className = 'rich-text-tag-pill__text rich-text-tag-pill__text--widget-ref';
    text.textContent = payload.displayText;

    const remove = document.createElement('button');
    remove.className = 'rich-text-tag-pill__remove';
    remove.textContent = '×';
    remove.title = 'Remove';
    remove.onclick = (e) => {
      e.preventDefault();
      e.stopPropagation();
      removeInlineTokenElement(tag);
      requestAnimationFrame(() => {
        internalRef.current?.focus();
        triggerSyncRef.current?.();
      });
    };

    tag.appendChild(badge);
    tag.appendChild(text);
    tag.appendChild(remove);

    return tag;
  }, [internalRef, removeInlineTokenElement]);

  const createSkillReferenceElement = useCallback((token: string): HTMLSpanElement | null => {
    const payload = parseSkillPromptReferenceToken(token);
    if (!payload) {
      return null;
    }

    const tag = document.createElement('span');
    tag.className = 'rich-text-tag-pill rich-text-tag-pill--skill-ref';
    tag.contentEditable = 'false';
    tag.dataset.tagFormat = token;
    tag.dataset.inlineTokenType = 'skill-ref';
    tag.title = `Skill: ${payload.skillName}`;

    const badge = document.createElement('span');
    badge.className = 'rich-text-tag-pill__badge rich-text-tag-pill__badge--icon';
    badge.innerHTML = SKILL_REFERENCE_BADGE_ICON;

    const text = document.createElement('span');
    text.className = 'rich-text-tag-pill__text rich-text-tag-pill__text--skill-ref';
    text.textContent = payload.skillName;

    const remove = document.createElement('button');
    remove.className = 'rich-text-tag-pill__remove';
    remove.textContent = '×';
    remove.title = 'Remove';
    remove.onclick = (e) => {
      e.preventDefault();
      e.stopPropagation();
      removeInlineTokenElement(tag);
      requestAnimationFrame(() => {
        internalRef.current?.focus();
        triggerSyncRef.current?.();
      });
    };

    tag.appendChild(badge);
    tag.appendChild(text);
    tag.appendChild(remove);

    return tag;
  }, [internalRef, removeInlineTokenElement]);

  const createInlineTokenElement = useCallback((token: string): HTMLSpanElement | null => {
    return createWidgetReferenceElement(token) ?? createSkillReferenceElement(token);
  }, [createSkillReferenceElement, createWidgetReferenceElement]);

  const renderValueWithInlineTokens = useCallback((editor: HTMLElement, text: string) => {
    const fragment = document.createDocumentFragment();
    const matches = [
      ...getWidgetPromptReferenceMatches(text).map(match => ({
        ...match,
        kind: 'widget-ref' as const,
      })),
      ...getSkillPromptReferenceMatches(text).map(match => ({
        ...match,
        kind: 'skill-ref' as const,
      })),
    ].sort((a, b) => a.start - b.start);

    if (matches.length === 0) {
      editor.textContent = text;
      return;
    }

    let cursor = 0;
    for (const match of matches) {
      if (match.start > cursor) {
        fragment.appendChild(document.createTextNode(text.slice(cursor, match.start)));
      }

      const tokenElement = match.kind === 'widget-ref'
        ? createWidgetReferenceElement(match.token)
        : createSkillReferenceElement(match.token);
      if (tokenElement) {
        fragment.appendChild(tokenElement);
      } else {
        fragment.appendChild(document.createTextNode(match.token));
      }
      cursor = match.end;
    }

    if (cursor < text.length) {
      fragment.appendChild(document.createTextNode(text.slice(cursor)));
    }

    editor.replaceChildren(fragment);
  }, [createSkillReferenceElement, createWidgetReferenceElement]);

  /** Map textContent offsets to a DOM Range to replace only the @ span. */
  const getRangeByTextOffsets = useCallback((root: Node, start: number, end: number): Range | null => {
    let current = 0;
    let startNode: Node | null = null;
    let startOffset = 0;
    let endNode: Node | null = null;
    let endOffset = 0;

    const walk = (node: Node): boolean => {
      if (node.nodeType === Node.TEXT_NODE) {
        const len = (node.textContent || '').length;
        if (startNode === null && start < current + len) {
          startNode = node;
          startOffset = Math.min(start - current, len);
        }
        if (endNode === null && end <= current + len) {
          endNode = node;
          endOffset = Math.min(end - current, len);
          return true;
        }
        current += len;
      } else if (node.nodeType === Node.ELEMENT_NODE) {
        for (const child of Array.from(node.childNodes)) {
          if (walk(child)) return true;
        }
      }
      return false;
    };
    walk(root);
    if (startNode && endNode) {
      const range = document.createRange();
      range.setStart(startNode, startOffset);
      range.setEnd(endNode, endOffset);
      return range;
    }
    return null;
  }, []);

  function sanitizeText(text: string): string {
    // Strip zero-width and control characters that WebKit/WebView may inject
    // (e.g. from dead-key sequences, function keys, arrow keys, etc.)
    // Preserve normal whitespace: space (0x20), tab (0x09), newline (0x0A), carriage return (0x0D).
    // eslint-disable-next-line no-control-regex -- This intentionally removes specific ASCII control-character ranges.
    return text.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F\u200B-\u200F\u2028\u2029\uFEFF\u2060\u00AD]/g, '');
  }

  // Extract plain text including # tag format
  const extractTextContent = useCallback((): string => {
    if (!internalRef.current) return '';
    
    let text = '';
    const traverse = (node: Node) => {
      if (node.nodeType === Node.TEXT_NODE) {
        text += node.textContent || '';
      } else if (node.nodeType === Node.ELEMENT_NODE) {
        const element = node as HTMLElement;
        
        const isBlock = element.tagName === 'DIV' || element.tagName === 'P';
        if (isBlock && text.length > 0 && !text.endsWith('\n')) {
          text += '\n';
        }
        
        // For tag elements, use the stored full format with # prefix
        if (element.hasAttribute('data-tag-format')) {
          const tagFormat = element.getAttribute('data-tag-format');
          if (tagFormat) {
            text += tagFormat;
          }
        } else if (element.tagName === 'BR') {
          text += '\n';
        } else {
          node.childNodes.forEach(traverse);
        }
      }
    };
    
    internalRef.current.childNodes.forEach(traverse);
    return sanitizeText(text).trim();
  }, [internalRef]);

  // Detect @ mention plus inline / and $ triggers near the caret.
  const detectActiveTrigger = useCallback(() => {
    if (!internalRef.current) return;
    
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) {
      closeMention();
      closeInlineTrigger();
      return;
    }
    
    const range = selection.getRangeAt(0);
    if (!range.collapsed) {
      closeMention();
      closeInlineTrigger();
      return;
    }
    
    // Full editor text
    const fullText = internalRef.current.textContent || '';
    
    // Compute cursor position in full text
    let cursorPosition = 0;
    const traverseForPosition = (node: Node): boolean => {
      if (node === range.startContainer) {
        if (node.nodeType === Node.TEXT_NODE) {
          cursorPosition += range.startOffset;
        }
        return true;
      }
      if (node.nodeType === Node.TEXT_NODE) {
        cursorPosition += (node.textContent || '').length;
      } else if (node.nodeType === Node.ELEMENT_NODE) {
        for (const child of Array.from(node.childNodes)) {
          if (traverseForPosition(child)) return true;
        }
      }
      return false;
    };
    
    traverseForPosition(internalRef.current);
    
    const textBeforeCursor = fullText.slice(0, cursorPosition);
    
    const candidates = ['@', '/', '$'] as const;
    let selectedTrigger: (typeof candidates)[number] | null = null;
    let selectedIndex = -1;

    for (const trigger of candidates) {
      const index = textBeforeCursor.lastIndexOf(trigger);
      if (index > selectedIndex) {
        selectedIndex = index;
        selectedTrigger = trigger;
      }
    }

    if (selectedTrigger !== null && selectedIndex !== -1) {
      const charBeforeTrigger = textBeforeCursor[selectedIndex - 1];
      const query = textBeforeCursor.slice(selectedIndex + 1);

      if (
        isWhitespaceCharacter(charBeforeTrigger) &&
        !query.includes(' ') &&
        !query.includes('\n')
      ) {
        if (selectedTrigger === '@') {
          const newState: MentionState = {
            isActive: true,
            query,
            startOffset: selectedIndex,
          };

          if (
            !mentionStateRef.current.isActive ||
            mentionStateRef.current.query !== query ||
            mentionStateRef.current.startOffset !== selectedIndex
          ) {
            mentionStateRef.current = newState;
            onMentionStateChange?.(newState);
          }
          closeInlineTrigger();
          return;
        }

        closeMention();
        const nextInlineTriggerState: InlineTriggerState = {
          isActive: true,
          trigger: selectedTrigger,
          query,
          startOffset: selectedIndex,
        };
        const currentInlineTriggerState = inlineTriggerStateRef.current;
        if (
          currentInlineTriggerState.isActive !== nextInlineTriggerState.isActive ||
          currentInlineTriggerState.trigger !== nextInlineTriggerState.trigger ||
          currentInlineTriggerState.query !== nextInlineTriggerState.query ||
          currentInlineTriggerState.startOffset !== nextInlineTriggerState.startOffset
        ) {
          inlineTriggerStateRef.current = nextInlineTriggerState;
          onInlineTriggerStateChange?.(nextInlineTriggerState);
        }
        return;
      }
    }

    closeMention();
    closeInlineTrigger();
  }, [closeInlineTrigger, closeMention, internalRef, onInlineTriggerStateChange, onMentionStateChange]);

  /** Compute the cursor's character offset within the editor. */
  const getCursorOffset = useCallback((editor: HTMLElement): number => {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) return -1;
    const range = sel.getRangeAt(0);
    if (!range.collapsed) return -1;
    const preRange = document.createRange();
    preRange.selectNodeContents(editor);
    preRange.setEnd(range.startContainer, range.startOffset);
    return preRange.toString().length;
  }, []);

  /** Restore the cursor to a character offset within the editor. */
  const setCursorOffset = useCallback((editor: HTMLElement, offset: number) => {
    let remaining = offset;
    const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
    let node: Text | null;
    while ((node = walker.nextNode() as Text | null)) {
      const len = (node.textContent || '').length;
      if (remaining <= len) {
        const sel = window.getSelection();
        if (sel) {
          sel.collapse(node, remaining);
        }
        return;
      }
      remaining -= len;
    }
    // Offset past all text – place cursor at end
    const sel = window.getSelection();
    if (sel) {
      const range = document.createRange();
      range.selectNodeContents(editor);
      range.collapse(false);
      sel.removeAllRanges();
      sel.addRange(range);
    }
  }, []);

  const handleInput = useCallback(() => {
    if (isComposingRef.current) return;

    const editor = internalRef.current;

    // Scrub any invisible characters the browser may have inserted.
    // Save and restore the cursor (as a character offset) so cleaning
    // never disturbs the caret position.
    if (editor) {
      const cursorOffset = getCursorOffset(editor);
      let didClean = false;
      let removedBeforeCursor = 0;

      const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
      let charsSoFar = 0;
      let node: Text | null;
      while ((node = walker.nextNode() as Text | null)) {
        const original = node.textContent || '';
        const cleaned = sanitizeText(original);
        if (cleaned !== original) {
          // Count how many invisible chars were removed before the cursor
          if (cursorOffset >= 0) {
            if (cursorOffset > charsSoFar) {
              const relevantSlice = original.slice(0, Math.min(cursorOffset - charsSoFar, original.length));
              removedBeforeCursor += relevantSlice.length - sanitizeText(relevantSlice).length;
            }
          }
          node.textContent = cleaned;
          didClean = true;
        }
        charsSoFar += original.length;
      }

      if (didClean && cursorOffset >= 0) {
        setCursorOffset(editor, Math.max(cursorOffset - removedBeforeCursor, 0));
      }
    }

    const textContent = extractTextContent();
    const visibleContextIds = new Set(
      Array.from(internalRef.current?.querySelectorAll<HTMLElement>('[data-context-id]') ?? [])
        .map(element => element.dataset.contextId)
        .filter((id): id is string => !!id)
    );
    const visibleContexts = contexts.filter(context => visibleContextIds.has(context.id));

    onChange(textContent, visibleContexts);
    
    // Ensure detection runs after DOM updates
    requestAnimationFrame(() => {
      detectActiveTrigger();
    });
  }, [contexts, detectActiveTrigger, extractTextContent, getCursorOffset, internalRef, onChange, setCursorOffset]);

  triggerSyncRef.current = handleInput;

  const handleBeforeInput = useCallback((e: React.FormEvent<HTMLDivElement>) => {
    const inputEvent = e.nativeEvent as InputEvent;
    const inputType = inputEvent.inputType;

    // Only act on insertText – block attempts to insert purely-invisible content.
    // We intentionally avoid a blanket whitelist so that we never accidentally
    // block browser-internal input types (cursor movement, spellcheck, etc.).
    if (inputType === 'insertText' && inputEvent.data != null) {
      const cleaned = sanitizeText(inputEvent.data);
      if (cleaned.length === 0) {
        e.preventDefault();
      }
    }
  }, []);

  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    e.preventDefault();
    
    // Detect image paste
    const items = Array.from(e.clipboardData.items);
    const imageItem = items.find(item => item.type.startsWith('image/'));
    
    if (imageItem) {
      // Dispatch image paste event for parent handling
      const file = imageItem.getAsFile();
      if (file && internalRef.current) {
        const customEvent = new CustomEvent('imagePaste', { 
          detail: { file },
          bubbles: true 
        });
        internalRef.current.dispatchEvent(customEvent);
      }
      return;
    }
    
    // Plain text paste - close active triggers so pasted marker characters do not immediately reopen pickers
    closeMention();
    closeInlineTrigger();
    
    const text = e.clipboardData.getData('text/plain');
    const largePastePlaceholder = onLargePaste?.(text);
    document.execCommand('insertText', false, largePastePlaceholder ?? text);
    
    // Mark that we just pasted to prevent mention detection in the next input event
    isComposingRef.current = true;
    requestAnimationFrame(() => {
      isComposingRef.current = false;
    });
  }, [closeInlineTrigger, closeMention, internalRef, onLargePaste]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const nativeEvent = e.nativeEvent as KeyboardEvent;
    const composing = nativeEvent.isComposing || isComposingRef.current || nativeEvent.keyCode === 229;
    
    if (!composing && e.key === 'Backspace' && internalRef.current) {
      const selection = window.getSelection();
      if (selection) {
        const range = selection.getRangeAt(0);
        
        if (range.collapsed && range.startOffset === 0) {
          const previousSibling = range.startContainer.previousSibling;
          const tokenElement = previousSibling instanceof HTMLElement && previousSibling.hasAttribute('data-tag-format')
            ? previousSibling
            : null;
          if (tokenElement) {
            e.preventDefault();
            const contextId = tokenElement.dataset.contextId;
            if (contextId) {
              onRemoveContext(contextId);
            } else {
              removeInlineTokenElement(tokenElement);
              handleInput();
            }
            return;
          }
        }
      }
    }
    
    if (composing && (e.key === 'Enter' || e.key === 'Escape')) {
      return;
    }

    onKeyDown?.(e);
  }, [handleInput, internalRef, onKeyDown, onRemoveContext, removeInlineTokenElement]);

  // Insert tag at cursor
  const insertTagAtCursor = useCallback((context: ContextItem) => {
    if (!internalRef.current) return;
    
    internalRef.current.focus();
    const selection = window.getSelection();
    
    if (selection && selection.rangeCount > 0) {
      const range = selection.getRangeAt(0);
      range.deleteContents();
      
      const tag = createTagElement(context);
      const space = document.createTextNode(' ');
      
      range.insertNode(space);
      range.insertNode(tag);
      
      range.setStartAfter(space);
      range.setEndAfter(space);
      selection.removeAllRanges();
      selection.addRange(range);
      
      handleInput();
    } else {
      const tag = createTagElement(context);
      const space = document.createTextNode(' ');
      internalRef.current.appendChild(tag);
      internalRef.current.appendChild(space);
      handleInput();
    }
  }, [createTagElement, handleInput, internalRef]);

  // Replace @ mention span with a tag, preserving existing tags
  const insertTagReplacingMention = useCallback((context: ContextItem) => {
    if (!internalRef.current || !mentionStateRef.current.isActive) {
      insertTagAtCursor(context);
      return;
    }

    const editor = internalRef.current;
    const mentionStart = mentionStateRef.current.startOffset;
    const mentionEnd = mentionStart + 1 + mentionStateRef.current.query.length; // @ + query

    const range = getRangeByTextOffsets(editor, mentionStart, mentionEnd);
    if (range) {
      range.deleteContents();
      const tag = createTagElement(context);
      const space = document.createTextNode(' ');
      range.insertNode(space);
      range.insertNode(tag);

      const selection = window.getSelection();
      if (selection) {
        const newRange = document.createRange();
        newRange.setStartAfter(space);
        newRange.setEndAfter(space);
        selection.removeAllRanges();
        selection.addRange(newRange);
      }
      editor.focus();
      closeMention();
      handleInput();
      return;
    }

    // Fallback to cursor insertion if range cannot be found
    insertTagAtCursor(context);
    closeMention();
  }, [closeMention, createTagElement, getRangeByTextOffsets, handleInput, insertTagAtCursor, internalRef]);

  const replaceActiveInlineTrigger = useCallback((replacementText: string) => {
    if (!internalRef.current || !inlineTriggerStateRef.current.isActive) {
      return;
    }

    const editor = internalRef.current;
    const triggerStart = inlineTriggerStateRef.current.startOffset;
    const triggerEnd = triggerStart + 1 + inlineTriggerStateRef.current.query.length;
    const range = getRangeByTextOffsets(editor, triggerStart, triggerEnd);
    if (!range) {
      return;
    }

    range.deleteContents();
    const selection = window.getSelection();
    if (replacementText) {
      const inlineTokenElement = createInlineTokenElement(replacementText);
      const replacementNode = inlineTokenElement ?? document.createTextNode(replacementText);
      const trailingSpace = document.createTextNode(' ');
      const fragment = document.createDocumentFragment();
      fragment.appendChild(replacementNode);
      fragment.appendChild(trailingSpace);
      range.insertNode(fragment);

      if (selection) {
        const newRange = document.createRange();
        newRange.setStartAfter(trailingSpace);
        newRange.setEndAfter(trailingSpace);
        selection.removeAllRanges();
        selection.addRange(newRange);
      }
    } else if (selection) {
      const newRange = document.createRange();
      newRange.setStart(range.startContainer, range.startOffset);
      newRange.collapse(true);
      selection.removeAllRanges();
      selection.addRange(newRange);
    }

    editor.focus();
    closeInlineTrigger();
    handleInput();
  }, [closeInlineTrigger, createInlineTokenElement, getRangeByTextOffsets, handleInput, internalRef]);

  const appendInlineTokenAtEnd = useCallback((token: string) => {
    if (!internalRef.current) {
      return;
    }

    const editor = internalRef.current;
    const currentTextContent = extractTextContent();
    if (!currentTextContent) {
      editor.replaceChildren();
    }
    editor.focus();

    const range = document.createRange();
    range.selectNodeContents(editor);
    range.collapse(false);

    const fragment = document.createDocumentFragment();
    if (currentTextContent) {
      fragment.appendChild(document.createTextNode(' '));
    }

    const inlineTokenElement = createInlineTokenElement(token);
    fragment.appendChild(inlineTokenElement ?? document.createTextNode(token));

    const trailingSpace = document.createTextNode(' ');
    fragment.appendChild(trailingSpace);
    range.insertNode(fragment);

    const selection = window.getSelection();
    if (selection) {
      const newRange = document.createRange();
      newRange.setStartAfter(trailingSpace);
      newRange.setEndAfter(trailingSpace);
      selection.removeAllRanges();
      selection.addRange(newRange);
    }

    handleInput();
  }, [createInlineTokenElement, extractTextContent, handleInput, internalRef]);

  /** Insert @ at caret and open the file/folder mention picker (e.g. from ChatInput + menu). */
  const openMention = useCallback(() => {
    const editor = internalRef.current;
    if (!editor) return;

    editor.focus();
    const sel = window.getSelection();
    let range: Range | null = null;
    if (sel && sel.rangeCount > 0) {
      range = sel.getRangeAt(0);
    }
    if (!range || !editor.contains(range.commonAncestorContainer)) {
      range = document.createRange();
      range.selectNodeContents(editor);
      range.collapse(false);
      sel?.removeAllRanges();
      sel?.addRange(range);
    }

    const cursorOffset = getCursorOffset(editor);
    const textBeforeCursor = cursorOffset >= 0
      ? (editor.textContent || '').slice(0, cursorOffset)
      : (editor.textContent || '');
    const charBeforeCursor = textBeforeCursor[textBeforeCursor.length - 1];
    const mentionTriggerText = isWhitespaceCharacter(charBeforeCursor) ? '@' : ' @';

    document.execCommand('insertText', false, mentionTriggerText);
    requestAnimationFrame(() => {
      detectActiveTrigger();
    });
  }, [detectActiveTrigger, getCursorOffset, internalRef]);

  // Expose methods to parent
  useEffect(() => {
    if (internalRef.current) {
      (internalRef.current as any).insertTag = insertTagAtCursor;
      (internalRef.current as any).insertTagReplacingMention = insertTagReplacingMention;
      (internalRef.current as any).replaceActiveInlineTrigger = replaceActiveInlineTrigger;
      (internalRef.current as any).appendInlineTokenAtEnd = appendInlineTokenAtEnd;
      (internalRef.current as any).openMention = openMention;
      (internalRef.current as any).closeMention = closeMention;
      (internalRef.current as any).closeInlineTrigger = closeInlineTrigger;
    }
  }, [appendInlineTokenAtEnd, closeInlineTrigger, closeMention, insertTagAtCursor, insertTagReplacingMention, openMention, replaceActiveInlineTrigger, internalRef]);

  // Initialize and sync value changes from external sources.
  // This editor is effectively controlled by comparing the parent's value
  // with the current DOM content, rather than tracking a "skip next sync" flag.
  useEffect(() => {
    const editor = internalRef.current;
    if (!editor) return;

    if (isComposingRef.current) return;
    
    // Detect template fill mode via placeholder elements
    const hasPlaceholders = editor.querySelector('.rich-text-placeholder') !== null;
    if (hasPlaceholders) {
      // Skip value sync; template rendering owns the content
      return;
    }
    
    const currentContent = extractTextContent();
    const syncAction = getRichTextExternalSyncAction(value, currentContent);
    
    if (syncAction === 'noop') {
      return;
    }

    if (syncAction === 'clear') {
      editor.textContent = '';
      return;
    }
    
    if (syncAction === 'replace') {
      renderValueWithInlineTokens(editor, value);
      
      // Restore cursor to the end
      requestAnimationFrame(() => {
        if (editor.childNodes.length > 0) {
          const range = document.createRange();
          const sel = window.getSelection();
          range.selectNodeContents(editor);
          range.collapse(false);
          sel?.removeAllRanges();
          sel?.addRange(range);
        }
        editor.focus();
      });
    }
  }, [extractTextContent, internalRef, renderValueWithInlineTokens, value]);

  // Remove tags for deleted contexts
  useEffect(() => {
    const editor = internalRef.current;
    if (!editor) return;

    const currentContextIds = new Set(contexts.map(c => c.id));
    const previousContextIds = lastContextIdsRef.current;

    const deletedIds = Array.from(previousContextIds).filter(id => !currentContextIds.has(id));

    deletedIds.forEach(id => {
      const tagElement = editor.querySelector(`[data-context-id="${id}"]`);
      if (tagElement) {
        const nextSibling = tagElement.nextSibling;
        if (nextSibling && nextSibling.nodeType === Node.TEXT_NODE && nextSibling.textContent === ' ') {
          nextSibling.remove();
        }
        tagElement.remove();
      }
    });

    lastContextIdsRef.current = currentContextIds;
  }, [contexts, internalRef]);

  const handleFocus = useCallback(() => {
    setIsFocused(true);
    onFocus?.();
  }, [onFocus]);

  const handleBlur = useCallback(() => {
    setIsFocused(false);
    // Delay closing to allow picker clicks
    setTimeout(() => {
      closeMention();
      closeInlineTrigger();
    }, 200);
    onBlur?.();
  }, [closeInlineTrigger, closeMention, onBlur]);

  // Handle IME composition
  const handleCompositionStart = useCallback(() => {
    isComposingRef.current = true;
    onCompositionStart?.();
  }, [onCompositionStart]);

  const handleCompositionEnd = useCallback(() => {
    isComposingRef.current = false;
    onCompositionEnd?.();
    handleInput();
  }, [handleInput, onCompositionEnd]);

  return (
    <div
      {...restProps}
      ref={internalRef}
      className={`rich-text-input ${isFocused ? 'rich-text-input--focused' : ''} ${className}`}
      contentEditable={!disabled}
      onBeforeInput={handleBeforeInput}
      onInput={handleInput}
      onPaste={handlePaste}
      onKeyDown={handleKeyDown}
      onFocus={handleFocus}
      onBlur={handleBlur}
      onCompositionStart={handleCompositionStart}
      onCompositionEnd={handleCompositionEnd}
      data-placeholder={placeholder}
      suppressContentEditableWarning
    />
  );
});

RichTextInput.displayName = 'RichTextInput';

export default RichTextInput;
