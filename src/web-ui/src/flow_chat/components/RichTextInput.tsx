/**
 * Rich text input component.
 * Supports inserting file tags inline and using @ to select files/folders.
 */

import React, { useRef, useEffect, useCallback, useState } from 'react';
import type { ContextItem } from '../../shared/types/context';
import './RichTextInput.scss';

/** @ mention state */
export interface MentionState {
  isActive: boolean;
  query: string;
  startOffset: number;  // Position of the @ symbol in text
}

export interface RichTextInputProps {
  value: string;
  onChange: (value: string, contexts: ContextItem[]) => void;
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
}

export const RichTextInput = React.forwardRef<HTMLDivElement, RichTextInputProps>(({
  value,
  onChange,
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
}, ref) => {
  const editorRef = useRef<HTMLDivElement>(null);
  const internalRef = (ref as React.RefObject<HTMLDivElement>) || editorRef;
  const [isFocused, setIsFocused] = useState(false);
  const isComposingRef = useRef(false);
  const lastContextIdsRef = useRef<Set<string>>(new Set());
  const mentionStateRef = useRef<MentionState>({ isActive: false, query: '', startOffset: 0 });
  const isLocalChangeRef = useRef(false);

  // Display name without the # prefix
  const getContextDisplayName = (context: ContextItem): string => {
    switch (context.type) {
      case 'file': return context.fileName;
      case 'directory': return context.directoryName;
      case 'code-snippet': return `${context.fileName}:${context.startLine}-${context.endLine}`;
      case 'image': return context.imageName;
      case 'terminal-command': return context.command;
      case 'git-ref': return context.refValue;
      case 'url': return context.title || context.url;
      case 'mermaid-node': return context.nodeText;
      case 'mermaid-diagram': return context.diagramTitle || 'Mermaid diagram';
      case 'web-element': return context.tagName;
      default:
        // TypeScript exhaustiveness check
        const _exhaustive: never = context;
        return String(_exhaustive);
    }
  };

  // # tag format for text extraction
  const getContextTagFormat = (context: ContextItem): string => {
    switch (context.type) {
      case 'file': return `#file:${context.fileName}`;
      case 'directory': return `#dir:${context.directoryName}`;
      case 'code-snippet': return `#code:${context.fileName}:${context.startLine}-${context.endLine}`;
      case 'image': return `#img:${context.imageName}`;
      case 'terminal-command': return `#cmd:${context.command}`;
      case 'git-ref': return `#git:${context.refValue}`;
      case 'url': return `#link:${context.title || context.url}`;
      case 'mermaid-node': return `#chart:${context.nodeText}`;
      case 'mermaid-diagram': return `#mermaid:${context.diagramTitle || 'Mermaid diagram'}`;
      case 'web-element': return `#element:${context.tagName}`;
      default:
        // TypeScript exhaustiveness check
        const _exhaustive: never = context;
        return String(_exhaustive);
    }
  };

  // Full context path for tooltips
  const getContextFullPath = (context: ContextItem): string => {
    switch (context.type) {
      case 'file': 
        return context.filePath;
      case 'directory': 
        return context.directoryPath + (context.recursive ? ' (recursive)' : '');
      case 'code-snippet': 
        return `${context.filePath} (lines ${context.startLine}-${context.endLine})`;
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
      default:
        // TypeScript exhaustiveness check
        const _exhaustive: never = context;
        return String(_exhaustive);
    }
  };

  // Create tag element with pill style
  const createTagElement = (context: ContextItem): HTMLSpanElement => {
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
  };

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

  // Extract plain text including # tag format
  const extractTextContent = (): string => {
    if (!internalRef.current) return '';
    
    let text = '';
    const traverse = (node: Node) => {
      if (node.nodeType === Node.TEXT_NODE) {
        text += node.textContent || '';
      } else if (node.nodeType === Node.ELEMENT_NODE) {
        const element = node as HTMLElement;
        // For tag elements, use the stored full format with # prefix
        if (element.classList.contains('rich-text-tag-pill')) {
          const tagFormat = element.getAttribute('data-tag-format');
          if (tagFormat) {
            text += tagFormat;
          }
        } else {
          node.childNodes.forEach(traverse);
        }
      }
    };
    
    internalRef.current.childNodes.forEach(traverse);
    return sanitizeText(text).trim();
  };

  // Detect @ mention
  const detectMention = useCallback(() => {
    if (!internalRef.current) return;
    
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) {
      // No selection, close mention
      if (mentionStateRef.current.isActive) {
        mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
        onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
      }
      return;
    }
    
    const range = selection.getRangeAt(0);
    if (!range.collapsed) {
      // Non-collapsed selection, close mention
      if (mentionStateRef.current.isActive) {
        mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
        onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
      }
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
    
    // Find the last @
    const lastAtIndex = textBeforeCursor.lastIndexOf('@');
    
    if (lastAtIndex !== -1) {
      // Extract query after @ up to the cursor
      const query = textBeforeCursor.slice(lastAtIndex + 1);
      
      // If query contains whitespace, the mention is complete
      if (!query.includes(' ') && !query.includes('\n')) {
        const newState: MentionState = {
          isActive: true,
          query,
          startOffset: lastAtIndex,
        };
        
        // Update only on state changes
        if (!mentionStateRef.current.isActive || 
            mentionStateRef.current.query !== query ||
            mentionStateRef.current.startOffset !== lastAtIndex) {
          mentionStateRef.current = newState;
          onMentionStateChange?.(newState);
        }
        return;
      }
    }
    
    // No valid mention, close it
    if (mentionStateRef.current.isActive) {
      mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
      onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
    }
  }, [onMentionStateChange, internalRef]);

  const sanitizeText = (text: string): string => {
    // Strip zero-width and control characters that WebKit/WebView may inject
    // (e.g. from dead-key sequences, function keys, arrow keys, etc.)
    // Preserve normal whitespace: space (0x20), tab (0x09), newline (0x0A), carriage return (0x0D).
    return text.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F\u200B-\u200F\u2028\u2029\uFEFF\u2060\u00AD]/g, '');
  };

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

    isLocalChangeRef.current = true;
    onChange(textContent, visibleContexts);
    
    // Ensure detection runs after DOM updates
    requestAnimationFrame(() => {
      detectMention();
    });
  }, [contexts, onChange, detectMention, internalRef]);

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
    
    // Plain text paste - close any active mention to prevent @ in pasted content from triggering mention mode
    if (mentionStateRef.current.isActive) {
      mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
      onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
    }
    
    const text = e.clipboardData.getData('text/plain');
    document.execCommand('insertText', false, text);
    
    // Mark that we just pasted to prevent mention detection in the next input event
    isComposingRef.current = true;
    requestAnimationFrame(() => {
      isComposingRef.current = false;
    });
  }, [onMentionStateChange]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const nativeIsComposing = (e.nativeEvent as KeyboardEvent).isComposing;
    const composing = nativeIsComposing || isComposingRef.current;
    
    if (!composing && e.key === 'Backspace' && internalRef.current) {
      const selection = window.getSelection();
      if (selection) {
        const range = selection.getRangeAt(0);
        
        if (range.collapsed && range.startOffset === 0) {
          const previousSibling = range.startContainer.previousSibling;
          if (previousSibling && (previousSibling as HTMLElement).classList?.contains('rich-text-tag-pill')) {
            e.preventDefault();
            const contextId = (previousSibling as HTMLElement).dataset.contextId;
            if (contextId) {
              onRemoveContext(contextId);
            }
            return;
          }
        }
      }
    }
    
    if (composing && e.key === 'Enter') {
      return;
    }

    onKeyDown?.(e);
  }, [onKeyDown, onRemoveContext]);

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
  }, [createTagElement, handleInput]);

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
      mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
      onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
      handleInput();
      return;
    }

    // Fallback to cursor insertion if range cannot be found
    insertTagAtCursor(context);
    mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
    onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
  }, [createTagElement, getRangeByTextOffsets, handleInput, insertTagAtCursor, onMentionStateChange]);

  // Expose methods to parent
  useEffect(() => {
    if (internalRef.current) {
      (internalRef.current as any).insertTag = insertTagAtCursor;
      (internalRef.current as any).insertTagReplacingMention = insertTagReplacingMention;
      (internalRef.current as any).closeMention = () => {
        if (mentionStateRef.current.isActive) {
          mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
          onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
        }
      };
    }
  }, [insertTagAtCursor, insertTagReplacingMention, onMentionStateChange, internalRef]);

  // Initialize and sync value changes from external sources.
  // Skip syncing when the change originated from local user input
  // to avoid resetting the cursor position.
  useEffect(() => {
    if (isLocalChangeRef.current) {
      isLocalChangeRef.current = false;
      return;
    }

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
    
    // If value is empty, clear editor content
    if (!value && currentContent !== '') {
      editor.textContent = '';
      return;
    }
    
    // External updates require syncing
    if (value && value !== currentContent) {
      editor.textContent = value;
      
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
  }, [value]);

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
      if (mentionStateRef.current.isActive) {
        mentionStateRef.current = { isActive: false, query: '', startOffset: 0 };
        onMentionStateChange?.({ isActive: false, query: '', startOffset: 0 });
      }
    }, 200);
    onBlur?.();
  }, [onBlur, onMentionStateChange]);

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
