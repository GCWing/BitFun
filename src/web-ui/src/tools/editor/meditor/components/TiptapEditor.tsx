import React, {
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Details, DetailsContent, DetailsSummary } from '@tiptap/extension-details';
import Placeholder from '@tiptap/extension-placeholder';
import TaskItem from '@tiptap/extension-task-item';
import TaskList from '@tiptap/extension-task-list';
import Link from '@tiptap/extension-link';
import { ArrowUp, FileText, ListTodo, PenLine } from 'lucide-react';
import type { Editor as TiptapEditorInstance, JSONContent } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Selection } from '@tiptap/pm/state';
import { useI18n } from '@/infrastructure/i18n';
import { Input } from '@/component-library';
import { editorAiAPI } from '@/infrastructure/api/service-api/EditorAiAPI';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { activeEditTargetService } from '@/tools/editor/services/ActiveEditTargetService';
import { MarkdownAlignmentExtension } from '../extensions/MarkdownAlignmentExtension';
import { BlockIdExtension } from '../extensions/BlockIdExtension';
import { MarkdownImage } from '../extensions/MarkdownImageExtension';
import {
  MarkdownTable,
  MarkdownTableCell,
  MarkdownTableHeader,
  MarkdownTableRow,
} from '../extensions/MarkdownTableExtensions';
import {
  InlineAiPreviewExtension,
} from '../extensions/InlineAiPreviewExtension';
import { inlineAiPreviewPluginKey } from '../extensions/InlineAiPreviewPluginKey';
import { RawHtmlBlock, RawHtmlInline, RenderOnlyBlock } from '../extensions/RawHtmlExtensions';
import { getBlockIndexForLine } from '../utils/markdownBlocks';
import {
  buildInlineContinuePrompt,
  buildInlineSummaryPrompt,
  buildInlineTodoPrompt,
  sanitizeInlineAiMarkdownResponse,
} from '../utils/inlineAi';
import { getCachedLocalImageDataUrl, loadLocalImages } from '../utils/loadLocalImages';
import { isLocalPath, resolveImagePath } from '../utils/rehype-local-images';
import { markdownToTiptapDoc, tiptapDocToMarkdown, tiptapDocToTopLevelMarkdownBlocks } from '../utils/tiptapMarkdown';
import './TiptapEditor.scss';

const log = createLogger('TiptapEditor');

export interface TiptapEditorHandle {
  scrollToLine: (line: number, highlight?: boolean) => void;
  undo: () => boolean;
  redo: () => boolean;
  canUndo: boolean;
  canRedo: boolean;
  focus: () => void;
  getContent: () => string;
  markSaved: () => void;
  setInitialContent: (content: string) => void;
  isDirty: boolean;
}

interface TiptapEditorProps {
  value: string;
  onChange: (value: string) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  placeholder?: string;
  readonly?: boolean;
  autofocus?: boolean;
  onDirtyChange?: (isDirty: boolean) => void;
  filePath?: string;
  basePath?: string;
}

function executeContentEditableAction(action: 'copy' | 'cut'): boolean {
  return document.execCommand(action);
}

function focusEditorWithoutScroll(instance: TiptapEditorInstance | null | undefined): void {
  if (!instance) {
    return;
  }

  const dom = instance.view.dom as HTMLElement | null;
  if (dom) {
    try {
      dom.focus({ preventScroll: true });
      return;
    } catch {
      dom.focus();
      return;
    }
  }

  instance.commands.focus();
}

function getTopLevelBlockIds(doc: JSONContent | null | undefined): string[] {
  return (doc?.content ?? [])
    .map((node: JSONContent) => (typeof node.attrs?.blockId === 'string' ? node.attrs.blockId : null))
    .filter((value: string | null): value is string => typeof value === 'string');
}

function syncInlineAiHints(
  instance: TiptapEditorInstance,
  root: HTMLDivElement | null,
  hintText: string
): void {
  if (!root) {
    return;
  }

  root.querySelectorAll<HTMLElement>('[data-inline-ai-hint]').forEach(element => {
    element.removeAttribute('data-inline-ai-hint');
  });
  root.querySelectorAll<HTMLElement>('[data-inline-ai-active]').forEach(element => {
    element.removeAttribute('data-inline-ai-active');
  });

  const { selection } = instance.state;
  const activeBlockId =
    selection.empty &&
    selection.$from.depth === 1 &&
    selection.$from.parent.type.name === 'paragraph' &&
    selection.$from.parent.textContent.trim().length === 0 &&
    typeof selection.$from.parent.attrs?.blockId === 'string'
      ? selection.$from.parent.attrs.blockId
      : null;

  instance.state.doc.forEach((node) => {
    if (node.type.name !== 'paragraph' || node.textContent.trim().length > 0) {
      return;
    }

    const blockId = typeof node.attrs?.blockId === 'string' ? node.attrs.blockId : null;
    if (!blockId) {
      return;
    }

    const element = root.querySelector<HTMLElement>(`[data-block-id="${blockId}"]`);
    if (!element) {
      return;
    }

    element.setAttribute('data-inline-ai-hint', hintText);
    if (blockId === activeBlockId) {
      element.setAttribute('data-inline-ai-active', 'true');
    }
  });
}

async function resolveEditorLocalImages(
  root: HTMLDivElement | null,
  basePath?: string,
): Promise<void> {
  const container = root?.querySelector<HTMLElement>('.ProseMirror');
  if (!container) {
    return;
  }

  const imageNodes = container.querySelectorAll<HTMLImageElement>('img');

  imageNodes.forEach((img) => {
    if (img.dataset.localResolved === 'true') {
      return;
    }

    const src = img.getAttribute('src');
    if (!src || !isLocalPath(src)) {
      img.dataset.localResolved = 'true';
      return;
    }

    const absolutePath = resolveImagePath(src, basePath);
    const cachedDataUrl = getCachedLocalImageDataUrl(absolutePath);
    img.setAttribute('data-local-image', 'true');
    img.setAttribute('data-local-path', absolutePath);
    img.setAttribute('data-original-src', src);
    img.dataset.localResolved = 'true';

    if (cachedDataUrl) {
      img.src = cachedDataUrl;
      img.classList.remove('local-image-loading', 'local-image-error');
      img.classList.add('local-image-loaded');
      img.removeAttribute('data-local-image');
      img.removeAttribute('data-local-path');
      return;
    }

    if (!img.classList.contains('local-image-loaded')) {
      img.classList.add('local-image-loading');
      img.src = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
    }
  });

  await loadLocalImages(container);
}

type InlineAiStatus = 'idle' | 'submitting' | 'streaming' | 'ready' | 'error';
type InlineAiPromptKind = 'continue' | 'summary' | 'todo';

type InlineAiState = {
  isOpen: boolean;
  promptKind: InlineAiPromptKind;
  query: string;
  status: InlineAiStatus;
  response: string;
  error: string | null;
  blockId: string;
  blockIndex: number;
  anchorTop: number;
  anchorLeft: number;
};

type InlineAiRequest = {
  requestId: string;
  cancel: () => Promise<void>;
  cleanup: () => void;
};

type TopLevelBlockPosition = {
  blockId: string;
  blockIndex: number;
  pos: number;
  nodeSize: number;
};

function createInlineSessionId(prefix: string): string {
  try {
    const fn = (globalThis as { crypto?: { randomUUID?: () => string } }).crypto?.randomUUID;
    if (fn) {
      return `${prefix}-${fn()}`;
    }
  } catch {
    // Ignore and fall through.
  }

  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function getTopLevelBlockPositionById(
  instance: TiptapEditorInstance,
  blockId: string
): TopLevelBlockPosition | null {
  let result: TopLevelBlockPosition | null = null;

  instance.state.doc.forEach((node, offset, index) => {
    if (typeof node.attrs?.blockId !== 'string' || node.attrs.blockId !== blockId) {
      return;
    }

    result = {
      blockId,
      blockIndex: index,
      pos: offset,
      nodeSize: node.nodeSize,
    };
  });

  return result;
}

function getCurrentEmptyParagraphContext(
  instance: TiptapEditorInstance,
  root: HTMLDivElement | null
): Omit<InlineAiState, 'isOpen' | 'promptKind' | 'query' | 'status' | 'response' | 'error'> | null {
  const { selection } = instance.state;
  if (!selection.empty || selection.$from.depth !== 1) {
    return null;
  }

  const parentNode = selection.$from.parent;
  if (parentNode.type.name !== 'paragraph' || parentNode.textContent.trim().length > 0) {
    return null;
  }

  const blockId = typeof parentNode.attrs?.blockId === 'string' ? parentNode.attrs.blockId : '';
  if (!blockId) {
    return null;
  }

  const blockIndex = selection.$from.index(0);
  const blockElement = root?.querySelector<HTMLElement>(`[data-block-id="${blockId}"]`);
  const rootRect = root?.getBoundingClientRect();
  const blockRect = blockElement?.getBoundingClientRect();

  return {
    blockId,
    blockIndex,
    anchorTop: root && rootRect && blockRect
      ? blockRect.top - rootRect.top + root.scrollTop + blockRect.height + 8
      : 16,
    anchorLeft: root && rootRect && blockRect
      ? blockRect.left - rootRect.left + root.scrollLeft
      : 16,
  };
}

function isMarkdownTableCellNode(node: ProseMirrorNode): boolean {
  return node.type.name === 'markdownTableHeader' || node.type.name === 'markdownTableCell';
}

function isEffectivelyEmptyMarkdownTableCell(node: ProseMirrorNode): boolean {
  if (!isMarkdownTableCellNode(node)) {
    return false;
  }

  if (node.childCount === 0) {
    return true;
  }

  let hasMeaningfulContent = false;

  node.descendants((child) => {
    if (child.isText) {
      if ((child.text ?? '').trim().length > 0) {
        hasMeaningfulContent = true;
        return false;
      }
      return;
    }

    if (child.isInline) {
      hasMeaningfulContent = true;
      return false;
    }
  });

  return !hasMeaningfulContent;
}

function isEffectivelyEmptyMarkdownTable(node: ProseMirrorNode): boolean {
  if (node.type.name !== 'markdownTable') {
    return false;
  }

  let hasCells = false;
  let hasMeaningfulContent = false;

  node.descendants((child) => {
    if (!isMarkdownTableCellNode(child)) {
      return;
    }

    hasCells = true;
    if (!isEffectivelyEmptyMarkdownTableCell(child)) {
      hasMeaningfulContent = true;
      return false;
    }
  });

  return hasCells && !hasMeaningfulContent;
}

function deleteEmptyMarkdownTableAtSelection(instance: TiptapEditorInstance): boolean {
  const { selection } = instance.state;
  if (!selection.empty) {
    return false;
  }

  const { $from } = selection;
  let cellDepth = -1;

  for (let depth = $from.depth; depth > 0; depth -= 1) {
    if (isMarkdownTableCellNode($from.node(depth))) {
      cellDepth = depth;
      break;
    }
  }

  if (cellDepth < 0) {
    return false;
  }

  const cellNode = $from.node(cellDepth);
  if ($from.parentOffset !== 0 || !isEffectivelyEmptyMarkdownTableCell(cellNode)) {
    return false;
  }

  let tableDepth = -1;
  for (let depth = cellDepth - 1; depth >= 0; depth -= 1) {
    if ($from.node(depth).type.name === 'markdownTable') {
      tableDepth = depth;
      break;
    }
  }

  if (tableDepth < 0) {
    return false;
  }

  const tableNode = $from.node(tableDepth);
  if (!isEffectivelyEmptyMarkdownTable(tableNode)) {
    return false;
  }

  const tablePos = $from.before(tableDepth);
  const tr = instance.state.tr.deleteRange(tablePos, tablePos + tableNode.nodeSize);

  if (tr.doc.childCount === 0) {
    const paragraph = tr.doc.type.schema.nodes.paragraph?.create();
    if (paragraph) {
      tr.insert(0, paragraph);
    }
  }

  const nextSelectionPos = Math.min(tablePos, tr.doc.content.size);
  tr.setSelection(Selection.near(tr.doc.resolve(nextSelectionPos), nextSelectionPos > 0 ? -1 : 1));
  instance.view.dispatch(tr.scrollIntoView());
  return true;
}

function replaceEditorContentWithoutHistory(
  instance: TiptapEditorInstance,
  markdown: string,
): void {
  instance
    .chain()
    .setMeta('addToHistory', false)
    .setContent(markdownToTiptapDoc(markdown), {
      emitUpdate: false,
    })
    .run();
}

export const TiptapEditor = React.forwardRef<TiptapEditorHandle, TiptapEditorProps>(({
  value,
  onChange,
  onFocus,
  onBlur,
  placeholder,
  readonly = false,
  autofocus = false,
  onDirtyChange,
  filePath,
  basePath,
}, ref) => {
  const { t } = useI18n('tools');
  const { t: tCommon } = useI18n('common');
  const rootRef = useRef<HTMLDivElement>(null);
  const inlineRequestRef = useRef<InlineAiRequest | null>(null);
  const inlineAiStateRef = useRef<InlineAiState | null>(null);
  const readonlyRef = useRef(readonly);
  const editorRef = useRef<TiptapEditorInstance | null>(null);
  const savedContentRef = useRef(value);
  const currentMarkdownRef = useRef(value);
  const preserveTrailingNewlineRef = useRef(value.endsWith('\n'));
  const applyingExternalValueRef = useRef(false);
  const highlightTimerRef = useRef<number | null>(null);
  const targetIdRef = useRef(`markdown-ir-tiptap-${Math.random().toString(36).slice(2, 10)}`);
  const inlineAiInputRef = useRef<HTMLInputElement | null>(null);
  const inlineAiInputComposingRef = useRef(false);
  const [inlineAiState, setInlineAiState] = useState<InlineAiState | null>(null);

  const initialContent = useMemo(() => markdownToTiptapDoc(value), [value]);
  const inlineAiTriggerHint = t('editor.meditor.inlineAi.triggerHint');

  useEffect(() => {
    inlineAiStateRef.current = inlineAiState;
  }, [inlineAiState]);

  useEffect(() => {
    if (!inlineAiState?.isOpen || inlineAiState.status !== 'idle') {
      return;
    }

    window.setTimeout(() => {
      const input = inlineAiInputRef.current;
      if (!input) {
        return;
      }

      input.focus();
      const value = input.value;
      input.setSelectionRange(value.length, value.length);
    }, 0);
  }, [inlineAiState?.isOpen, inlineAiState?.status]);

  useEffect(() => {
    readonlyRef.current = readonly;
  }, [readonly]);

  const serializeEditorMarkdown = useCallback((instance: TiptapEditorInstance): string => (
    tiptapDocToMarkdown(instance.getJSON(), {
      preserveTrailingNewline: preserveTrailingNewlineRef.current,
    })
  ), []);

  const closeInlineAi = useCallback((options?: { cancelRequest?: boolean; focusEditor?: boolean }) => {
    const shouldCancel = options?.cancelRequest ?? false;
    const shouldFocusEditor = options?.focusEditor ?? true;

    const activeRequest = inlineRequestRef.current;
    if (shouldCancel && activeRequest) {
      void activeRequest.cancel().catch(error => {
        log.warn('Failed to cancel inline AI request', {
          requestId: activeRequest.requestId,
          error,
        });
      });
    }

    activeRequest?.cleanup();
    inlineRequestRef.current = null;
    setInlineAiState(null);

    if (shouldFocusEditor) {
      window.setTimeout(() => {
        focusEditorWithoutScroll(editorRef.current);
      }, 0);
    }
  }, []);

  const openInlineAi = useCallback((instance: TiptapEditorInstance) => {
    const nextState = getCurrentEmptyParagraphContext(instance, rootRef.current);
    if (!nextState) {
      return false;
    }

    setInlineAiState({
      ...nextState,
      isOpen: true,
      promptKind: 'continue',
      query: '',
      status: 'idle',
      response: '',
      error: null,
    });

    return true;
  }, []);

  const insertGeneratedMarkdown = useCallback((instance: TiptapEditorInstance, blockId: string, markdown: string) => {
    const normalized = sanitizeInlineAiMarkdownResponse(markdown);
    if (!normalized) {
      return false;
    }

    const targetBlock = getTopLevelBlockPositionById(instance, blockId);
    if (!targetBlock) {
      return false;
    }

    const content = markdownToTiptapDoc(normalized).content ?? [];
    if (content.length === 0) {
      return false;
    }

    return instance
      .chain()
      .focus()
      .insertContentAt(
        {
          from: targetBlock.pos,
          to: targetBlock.pos + targetBlock.nodeSize,
        },
        content
      )
      .run();
  }, []);

  const editor = useEditor({
    immediatelyRender: false,
    autofocus,
    editable: !readonly,
    extensions: [
      StarterKit.configure({
        heading: {
          levels: [1, 2, 3, 4, 5, 6],
        },
      }),
      TaskList,
      TaskItem.configure({
        nested: true,
      }),
      Link.configure({
        openOnClick: false,
      }),
      Placeholder.configure({
        placeholder: ({ node, hasAnchor }) => {
          if (!hasAnchor || node.type.name !== 'paragraph') {
            return placeholder ?? '';
          }

          return inlineAiTriggerHint;
        },
      }),
      MarkdownAlignmentExtension,
      BlockIdExtension,
      MarkdownImage.configure({
        basePath,
      }),
      Details.configure({
        persist: true,
      }),
      DetailsSummary,
      DetailsContent,
      // Keep raw/render-only fallbacks for HTML we still can't round-trip safely.
      RenderOnlyBlock.configure({
        basePath,
      }),
      RawHtmlBlock.configure({
        basePath,
      }),
      RawHtmlInline.configure({
        label: t('editor.meditor.rawHtml.inlineLabel'),
      }),
      MarkdownTable,
      MarkdownTableRow,
      MarkdownTableHeader,
      MarkdownTableCell,
      InlineAiPreviewExtension,
    ],
    editorProps: {
      handleKeyDown: (_view, event) => {
        if (readonlyRef.current || inlineAiStateRef.current?.isOpen) {
          return false;
        }

        const instance = editorRef.current;
        if (
          event.key === 'Backspace' &&
          !event.ctrlKey &&
          !event.metaKey &&
          !event.altKey &&
          !!instance &&
          deleteEmptyMarkdownTableAtSelection(instance)
        ) {
          event.preventDefault();
          return true;
        }

        if (event.key !== ' ' || event.ctrlKey || event.metaKey || event.altKey) {
          return false;
        }

        if (!instance) {
          return false;
        }

        const opened = openInlineAi(instance);
        if (!opened) {
          return false;
        }

        event.preventDefault();
        return true;
      },
    },
    content: initialContent,
    onCreate: ({ editor: instance }: { editor: TiptapEditorInstance }) => {
      editorRef.current = instance;
      preserveTrailingNewlineRef.current = value.endsWith('\n');
      const markdown = serializeEditorMarkdown(instance);
      currentMarkdownRef.current = markdown;
      savedContentRef.current = markdown;
      syncInlineAiHints(instance, rootRef.current, inlineAiTriggerHint);
      onDirtyChange?.(false);
    },
    onFocus: ({ editor: instance }: { editor: TiptapEditorInstance }) => {
      activeEditTargetService.setActiveTarget(targetIdRef.current);
      syncInlineAiHints(instance, rootRef.current, inlineAiTriggerHint);
      onFocus?.();
    },
    onBlur: ({ editor: instance }: { editor: TiptapEditorInstance }) => {
      syncInlineAiHints(instance, rootRef.current, inlineAiTriggerHint);
      window.setTimeout(() => {
        const root = rootRef.current;
        const activeElement = typeof document !== 'undefined' ? document.activeElement : null;
        if (root && activeElement && root.contains(activeElement)) {
          return;
        }

        activeEditTargetService.clearActiveTarget(targetIdRef.current);
      }, 0);
      onBlur?.();
    },
    onSelectionUpdate: ({ editor: instance }: { editor: TiptapEditorInstance }) => {
      syncInlineAiHints(instance, rootRef.current, inlineAiTriggerHint);
    },
    onUpdate: ({ editor: instance }: { editor: TiptapEditorInstance }) => {
      const markdown = serializeEditorMarkdown(instance);
      currentMarkdownRef.current = markdown;
      syncInlineAiHints(instance, rootRef.current, inlineAiTriggerHint);

      if (applyingExternalValueRef.current) {
        applyingExternalValueRef.current = false;
        return;
      }

      onChange(markdown);
      onDirtyChange?.(markdown !== savedContentRef.current);
    },
  });

  useEffect(() => {
    if (!editor) {
      return;
    }

    editorRef.current = editor;
    editor.setEditable(!readonly);
  }, [editor, readonly]);

  useEffect(() => {
    if (!editor) {
      return;
    }

    syncInlineAiHints(editor, rootRef.current, inlineAiTriggerHint);
  }, [editor, inlineAiTriggerHint]);

  useEffect(() => {
    if (!editor) {
      return;
    }

    let cancelled = false;
    let running = false;
    let rerunRequested = false;

    const run = async () => {
      if (cancelled || running) {
        rerunRequested = true;
        return;
      }

      running = true;
      rerunRequested = false;

      do {
        rerunRequested = false;
        try {
          await resolveEditorLocalImages(rootRef.current, basePath);
        } catch (error) {
          if (!cancelled) {
            log.error('Failed to resolve editor local images', { error, basePath });
          }
        }
      } while (!cancelled && rerunRequested);

      running = false;
    };

    const scheduleRun = () => {
      if (cancelled) {
        return;
      }
      void run();
    };

    scheduleRun();

    const container = rootRef.current?.querySelector<HTMLElement>('.ProseMirror');
    const observer = container
      ? new MutationObserver(() => {
          scheduleRun();
        })
      : null;

    if (observer && container) {
      observer.observe(container, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['src'],
      });
    }

    return () => {
      cancelled = true;
      observer?.disconnect();
    };
  }, [basePath, editor, value]);

  useEffect(() => {
    if (!editor || value === currentMarkdownRef.current) {
      return;
    }

    applyingExternalValueRef.current = true;
    preserveTrailingNewlineRef.current = value.endsWith('\n');
    currentMarkdownRef.current = value;
    replaceEditorContentWithoutHistory(editor, value);
    syncInlineAiHints(editor, rootRef.current, inlineAiTriggerHint);
    onDirtyChange?.(value !== savedContentRef.current);
  }, [editor, inlineAiTriggerHint, value, onDirtyChange]);

  useEffect(() => {
    if (!editor) {
      return;
    }

    return activeEditTargetService.bindTarget({
      id: targetIdRef.current,
      kind: 'markdown-ir',
      focus: () => {
        focusEditorWithoutScroll(editor);
      },
      hasTextFocus: () => editor.isFocused,
      undo: () => editor.commands.undo(),
      redo: () => editor.commands.redo(),
      cut: () => {
        if (readonly) {
          return false;
        }

        editor.commands.focus();
        return executeContentEditableAction('cut');
      },
      copy: () => {
        editor.commands.focus();
        return executeContentEditableAction('copy');
      },
      selectAll: () => {
        editor.commands.focus();
        return editor.commands.selectAll();
      },
      containsElement: (element) => {
        const root = rootRef.current;
        return !!root && !!element && root.contains(element);
      },
    });
  }, [editor, readonly]);

  useEffect(() => {
    return () => {
      if (highlightTimerRef.current) {
        window.clearTimeout(highlightTimerRef.current);
      }

      inlineRequestRef.current?.cancel().catch(error => {
        log.warn('Failed to cancel inline AI request during cleanup', { error });
      });
      inlineRequestRef.current?.cleanup();
      inlineRequestRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!inlineAiState?.isOpen) {
      return;
    }

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (!target) {
        return;
      }

      if (target.closest('.m-editor-inline-ai') || target.closest('.m-editor-inline-ai-preview')) {
        return;
      }

      const shouldCancel =
        inlineAiStateRef.current?.status === 'submitting' ||
        inlineAiStateRef.current?.status === 'streaming';
      closeInlineAi({ cancelRequest: shouldCancel, focusEditor: false });
    };

    document.addEventListener('mousedown', handlePointerDown);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
    };
  }, [closeInlineAi, inlineAiState?.isOpen]);

  const focusBlockByIndex = useCallback((instance: TiptapEditorInstance, index: number, highlight: boolean) => {
    const blockIds = getTopLevelBlockIds(instance.getJSON());
    const blockId = blockIds[index];

    if (!blockId) {
      instance.commands.focus('end');
      return;
    }

    const root = rootRef.current;
    const element = root?.querySelector<HTMLElement>(`[data-block-id="${blockId}"]`);

    if (!element) {
      instance.commands.focus('end');
      return;
    }

    element.scrollIntoView({ behavior: 'smooth', block: 'center' });

    if (highlight) {
      element.classList.add('m-editor-tiptap-block-highlighted');

      if (highlightTimerRef.current) {
        window.clearTimeout(highlightTimerRef.current);
      }

      highlightTimerRef.current = window.setTimeout(() => {
        element.classList.remove('m-editor-tiptap-block-highlighted');
      }, 3000);
    }

    instance.commands.focus();
  }, []);

  const handleContinueWriting = useCallback(async (
    options?: { userInputOverride?: string; promptKindOverride?: InlineAiPromptKind }
  ) => {
    if (!inlineAiState || inlineAiState.status === 'submitting' || inlineAiState.status === 'streaming') {
      return;
    }

    inlineRequestRef.current?.cleanup();
    inlineRequestRef.current = null;

    const requestId = createInlineSessionId('meditor-inline');

    setInlineAiState(current => current ? {
      ...current,
      status: 'submitting',
      response: '',
      error: null,
    } : current);

    let responseText = '';
    let isCleanedUp = false;
    let unlistenChunk: () => void = () => {};
    let unlistenCompleted: () => void = () => {};
    let unlistenFailed: () => void = () => {};

    const cleanup = () => {
      if (isCleanedUp) {
        return;
      }
      isCleanedUp = true;

      try {
        unlistenChunk();
      } catch {
        // Ignore cleanup failures.
      }
      try {
        unlistenCompleted();
      } catch {
        // Ignore cleanup failures.
      }
      try {
        unlistenFailed();
      } catch {
        // Ignore cleanup failures.
      }
    };

    unlistenChunk = editorAiAPI.onTextChunk(event => {
      if (event.requestId !== requestId) {
        return;
      }

      if (!event.text) {
        return;
      }

      responseText += event.text;
      setInlineAiState(current => current ? {
        ...current,
        status: 'streaming',
        response: responseText,
        error: null,
      } : current);
    });

    unlistenCompleted = editorAiAPI.onCompleted(event => {
      if (event.requestId !== requestId) {
        return;
      }

      cleanup();
      inlineRequestRef.current = null;

      const finalText = event.fullText && event.fullText.length >= responseText.length
        ? event.fullText
        : responseText;
      const sanitizedResponse = sanitizeInlineAiMarkdownResponse(finalText);

      if (!sanitizedResponse) {
        setInlineAiState(current => current ? {
          ...current,
          status: 'error',
          error: t('editor.meditor.inlineAi.continueEmptyResult'),
        } : current);
        return;
      }

      setInlineAiState(current => current ? {
        ...current,
        status: 'ready',
        response: sanitizedResponse,
        error: null,
      } : current);
    });

    unlistenFailed = editorAiAPI.onError(event => {
      if (event.requestId !== requestId) {
        return;
      }

      cleanup();
      inlineRequestRef.current = null;
      setInlineAiState(current => current ? {
        ...current,
        status: 'error',
        error: typeof event.error === 'string' ? event.error : t('editor.meditor.inlineAi.continueFailed'),
      } : current);
    });

    inlineRequestRef.current = {
      requestId,
      cancel: () => editorAiAPI.cancel({ requestId }),
      cleanup,
    };

    const resolvedUserInput = options?.userInputOverride ?? inlineAiState.query;
    const promptKind = options?.promptKindOverride ?? inlineAiState.promptKind;
    const instance = editorRef.current;
    const promptParams = {
      userInput: resolvedUserInput,
      markdown: currentMarkdownRef.current,
      blockIndex: inlineAiState.blockIndex,
      filePath,
      topLevelBlocks: instance ? tiptapDocToTopLevelMarkdownBlocks(instance.getJSON()) : undefined,
    };
    const prompt = promptKind === 'summary'
      ? buildInlineSummaryPrompt(promptParams)
      : promptKind === 'todo'
        ? buildInlineTodoPrompt(promptParams)
        : buildInlineContinuePrompt(promptParams);

    try {
      await editorAiAPI.stream({
        requestId,
        modelId: 'primary',
        prompt,
      });
    } catch (error) {
      cleanup();
      inlineRequestRef.current = null;
      log.error('Failed to start inline continuation request', { error });
      setInlineAiState(current => current ? {
        ...current,
        status: 'error',
        error: error instanceof Error ? error.message : t('editor.meditor.inlineAi.continueStartFailed'),
      } : current);
    }
  }, [filePath, inlineAiState, t]);

  const handleAcceptInlineContinue = useCallback(() => {
    const instance = editorRef.current;
    if (!inlineAiState || !instance) {
      return;
    }

    const inserted = insertGeneratedMarkdown(instance, inlineAiState.blockId, inlineAiState.response);
    if (!inserted) {
      setInlineAiState(current => current ? {
        ...current,
        status: 'error',
        error: t('editor.meditor.inlineAi.continueEmptyResult'),
      } : current);
      return;
    }

    notificationService.success(t('editor.meditor.inlineAi.continueInserted'), { duration: 2500 });
    setInlineAiState(null);
    window.setTimeout(() => {
      instance.commands.focus();
    }, 0);
  }, [inlineAiState, insertGeneratedMarkdown, t]);

  const handleRejectInlineContinue = useCallback(() => {
    const shouldCancel = inlineAiState?.status === 'submitting' || inlineAiState?.status === 'streaming';
    closeInlineAi({ cancelRequest: shouldCancel, focusEditor: true });
  }, [closeInlineAi, inlineAiState?.status]);

  const handleRetryInlineContinue = useCallback(() => {
    void handleContinueWriting();
  }, [handleContinueWriting]);

  const handleInlineAiQuickAction = useCallback((promptKind: InlineAiPromptKind, query: string) => {
    setInlineAiState(current => current ? {
      ...current,
      promptKind,
      query,
    } : current);
    void handleContinueWriting({
      userInputOverride: query,
      promptKindOverride: promptKind,
    });
  }, [handleContinueWriting]);

  useEffect(() => {
    if (!editor) {
      return;
    }

    const previewState = inlineAiState &&
      inlineAiState.status !== 'idle'
      ? {
          blockId: inlineAiState.blockId,
          status: inlineAiState.status,
          response: inlineAiState.response,
          error: inlineAiState.error,
          basePath,
          canAccept: (
            inlineAiState.status === 'ready' &&
            !!inlineAiState.response.trim()
          ),
          labels: {
            title: t('editor.meditor.inlineAi.previewTitle'),
            streaming: t('editor.meditor.inlineAi.previewStreaming'),
            ready: t('editor.meditor.inlineAi.previewReady'),
            error: t('editor.meditor.inlineAi.continueFailed'),
            accept: t('editor.meditor.inlineAi.accept'),
            reject: t('editor.meditor.inlineAi.reject'),
            retry: tCommon('retry'),
          },
          onAccept: handleAcceptInlineContinue,
          onReject: handleRejectInlineContinue,
          onRetry: handleRetryInlineContinue,
        }
      : null;

    editor.view.dispatch(
      editor.state.tr
        .setMeta('addToHistory', false)
        .setMeta(inlineAiPreviewPluginKey, previewState)
    );
  }, [
    basePath,
    editor,
    handleAcceptInlineContinue,
    handleRejectInlineContinue,
    handleRetryInlineContinue,
    inlineAiState,
    t,
    tCommon,
  ]);

  useImperativeHandle(ref, () => ({
    scrollToLine: (line: number, highlight = true) => {
      if (!editor) {
        return;
      }

      const blockIndex = getBlockIndexForLine(currentMarkdownRef.current, line);
      if (blockIndex < 0) {
        editor.commands.focus();
        return;
      }

      focusBlockByIndex(editor, blockIndex, highlight);
    },
    undo: () => editor?.commands.undo() ?? false,
    redo: () => editor?.commands.redo() ?? false,
    get canUndo() {
      return editor?.can().undo() ?? false;
    },
    get canRedo() {
      return editor?.can().redo() ?? false;
    },
    focus: () => {
      focusEditorWithoutScroll(editor);
    },
    getContent: () => currentMarkdownRef.current,
    markSaved: () => {
      savedContentRef.current = currentMarkdownRef.current;
      onDirtyChange?.(false);
    },
    setInitialContent: (content: string) => {
      preserveTrailingNewlineRef.current = content.endsWith('\n');
      savedContentRef.current = content;
      currentMarkdownRef.current = content;

      if (!editor) {
        return;
      }

      applyingExternalValueRef.current = true;
      replaceEditorContentWithoutHistory(editor, content);
      onDirtyChange?.(false);
    },
    get isDirty() {
      return currentMarkdownRef.current !== savedContentRef.current;
    },
  }), [editor, focusBlockByIndex, onDirtyChange]);

  const isInlineBusy =
    inlineAiState?.status === 'submitting' || inlineAiState?.status === 'streaming';
  const canSubmitInlinePrompt = !!inlineAiState?.query.trim() && !isInlineBusy;

  return (
    <div ref={rootRef} className="m-editor-tiptap">
      <EditorContent editor={editor} />
      {inlineAiState?.isOpen && inlineAiState.status === 'idle' && (
        <div
          className="m-editor-inline-ai"
          data-testid="md-inline-ai-panel"
          style={{
            top: `${inlineAiState.anchorTop}px`,
            left: `${inlineAiState.anchorLeft}px`,
          }}
          onMouseDown={(event) => {
            event.stopPropagation();
          }}
        >
          <div className="m-editor-inline-ai__surface">
            <div className="m-editor-inline-ai__panel">
              <div className="m-editor-inline-ai__composer">
                <Input
                  ref={inlineAiInputRef}
                  variant="filled"
                  inputSize="medium"
                  className="m-editor-inline-ai__composer-input"
                  data-testid="md-inline-ai-input"
                  prefix={<PenLine size={14} strokeWidth={1.75} />}
                  value={inlineAiState.query}
                  onChange={(event) => {
                    const nextValue = event.target.value;
                    setInlineAiState(current => current ? {
                      ...current,
                      query: nextValue,
                    } : current);
                  }}
                  onCompositionStart={() => {
                    inlineAiInputComposingRef.current = true;
                  }}
                  onCompositionEnd={() => {
                    requestAnimationFrame(() => {
                      inlineAiInputComposingRef.current = false;
                    });
                  }}
                  placeholder={t('editor.meditor.inlineAi.askPlaceholder')}
                  onKeyDown={(event) => {
                    const isComposing =
                      (event.nativeEvent as KeyboardEvent).isComposing ||
                      inlineAiInputComposingRef.current;

                    if (event.key === 'Escape') {
                      event.preventDefault();
                      handleRejectInlineContinue();
                      return;
                    }

                    if (event.key === 'Enter') {
                      if (isComposing) {
                        return;
                      }
                      event.preventDefault();
                      if (!inlineAiState.query.trim()) {
                        return;
                      }
                      void handleContinueWriting();
                    }
                  }}
                  suffix={(
                    <div className="m-editor-inline-ai__composer-actions">
                      <span className="m-editor-inline-ai__page-chip">{t('editor.meditor.inlineAi.currentPage')}</span>
                      <button
                        type="button"
                        className="m-editor-inline-ai__send"
                        onClick={() => {
                          void handleContinueWriting();
                        }}
                        disabled={!canSubmitInlinePrompt}
                        aria-label={t('editor.meditor.inlineAi.askSubmit')}
                      >
                        <ArrowUp size={13} strokeWidth={2.1} />
                      </button>
                    </div>
                  )}
                />
              </div>

              <div className="m-editor-inline-ai__section-title">
                {t('editor.meditor.inlineAi.suggestionSection')}
              </div>

              <div className="m-editor-inline-ai__quick-actions">
                <button
                  type="button"
                  className="m-editor-inline-ai__quick-action m-editor-inline-ai__quick-action--primary"
                  data-testid="md-inline-ai-continue"
                  onClick={() => {
                    handleInlineAiQuickAction('continue', '');
                  }}
                >
                  <span className="m-editor-inline-ai__quick-action-icon">
                    <PenLine size={14} strokeWidth={1.75} />
                  </span>
                  <span>{t('editor.meditor.inlineAi.continueMode')}</span>
                </button>
                <button
                  type="button"
                  className="m-editor-inline-ai__quick-action"
                  data-testid="md-inline-ai-summary"
                  onClick={() => {
                    handleInlineAiQuickAction('summary', t('editor.meditor.inlineAi.summaryDirection'));
                  }}
                >
                  <span className="m-editor-inline-ai__quick-action-icon">
                    <FileText size={14} strokeWidth={1.75} />
                  </span>
                  <span>{t('editor.meditor.inlineAi.summaryAction')}</span>
                </button>
                <button
                  type="button"
                  className="m-editor-inline-ai__quick-action"
                  data-testid="md-inline-ai-todo"
                  onClick={() => {
                    handleInlineAiQuickAction('todo', t('editor.meditor.inlineAi.todoDirection'));
                  }}
                >
                  <span className="m-editor-inline-ai__quick-action-icon">
                    <ListTodo size={14} strokeWidth={1.75} />
                  </span>
                  <span>{t('editor.meditor.inlineAi.todoAction')}</span>
                </button>
              </div>

              <div className="m-editor-inline-ai__footer">
                <button
                  type="button"
                  className="m-editor-inline-ai__footer-dismiss"
                  onClick={handleRejectInlineContinue}
                >
                  {t('editor.meditor.inlineAi.cancel')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});

TiptapEditor.displayName = 'TiptapEditor';
