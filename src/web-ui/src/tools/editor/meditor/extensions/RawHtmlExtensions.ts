import React from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { Node } from '@tiptap/core';
import { MarkdownRenderer } from '@/component-library';
import { activeEditTargetService } from '@/tools/editor/services/ActiveEditTargetService';

type SourceBackedBlockOptions = {
  basePath?: string;
};

type RawHtmlInlineOptions = {
  label: string;
};

let sourceBackedBlockTextareaTargetCounter = 0;

function createRawHtmlInlinePreviewNode(
  html: string,
  labelText: string,
): HTMLElement {
  const root = document.createElement('span');
  root.className = 'm-editor-raw-html-inline';

  const label = document.createElement('span');
  label.className = 'm-editor-raw-html-inline__label';
  label.textContent = labelText;

  const source = document.createElement('code');
  source.className = 'm-editor-raw-html-inline__source';
  source.textContent = html;

  root.append(label, source);
  return root;
}

function isSelectAllShortcut(event: KeyboardEvent): boolean {
  return (
    event.key.toLowerCase() === 'a' &&
    (event.ctrlKey || event.metaKey) &&
    !event.altKey
  );
}

function focusElementWithoutScroll(element: HTMLElement): void {
  try {
    element.focus({ preventScroll: true });
  } catch {
    element.focus();
  }
}

function selectElementContent(element: HTMLElement): void {
  const selection = window.getSelection();
  if (!selection) {
    return;
  }

  const range = document.createRange();
  range.selectNodeContents(element);
  selection.removeAllRanges();
  selection.addRange(range);
}

function previewHasVisibleContent(preview: HTMLElement): boolean {
  const text = preview.textContent?.replace(/\u200b/g, '').trim() ?? '';
  if (text.length > 0) {
    return true;
  }

  if (preview.querySelector('img, svg, video, audio, table, pre, code, blockquote, ul, ol, hr')) {
    return true;
  }

  return Array.from(preview.querySelectorAll('details')).some((details) => {
    const detailsText = details.textContent?.replace(/\u200b/g, '').trim() ?? '';
    return detailsText.length > 0 || !!details.querySelector(
      'img, svg, video, audio, table, pre, code, blockquote, ul, ol, hr',
    );
  });
}

function normalizeDetailsBodyMarkdown(markdown: string): string {
  return markdown
    .replace(/^\s*\n/, '')
    .replace(/\n\s*$/, '');
}

function parseDetailsSource(markdown: string): {
  open: boolean;
  summaryHtml: string;
  bodyMarkdown: string;
} | null {
  const trimmed = markdown.trim();
  const match = trimmed.match(/^<details(\s[^>]*)?>\s*<summary>([\s\S]*?)<\/summary>\s*([\s\S]*?)\s*<\/details>$/);
  if (!match) {
    return null;
  }

  const [, attrSource = '', summaryHtml = '', bodyRaw = ''] = match;
  const isOpen = /\bopen\b/i.test(attrSource);

  return {
    open: isOpen,
    summaryHtml,
    bodyMarkdown: normalizeDetailsBodyMarkdown(bodyRaw),
  };
}

function isSafePreviewUrl(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return !normalized.startsWith('javascript:') && !normalized.startsWith('vbscript:');
}

function sanitizeDetailsSummaryHtml(summaryHtml: string): string {
  if (typeof document === 'undefined') {
    return summaryHtml;
  }

  const template = document.createElement('template');
  template.innerHTML = summaryHtml;
  const allowedTags = new Set(['A', 'STRONG', 'B', 'EM', 'I', 'CODE', 'BR', 'IMG']);

  const sanitizeNode = (node: globalThis.Node) => {
    if (!(node instanceof HTMLElement)) {
      return;
    }

    if (!allowedTags.has(node.tagName)) {
      const parent = node.parentNode;
      if (!parent) {
        return;
      }

      while (node.firstChild) {
        parent.insertBefore(node.firstChild, node);
      }
      parent.removeChild(node);
      return;
    }

    Array.from(node.attributes).forEach((attr) => {
      const name = attr.name.toLowerCase();
      const value = attr.value;

      if (name.startsWith('on')) {
        node.removeAttribute(attr.name);
        return;
      }

      if (node.tagName === 'A') {
        if (!['href', 'title'].includes(name)) {
          node.removeAttribute(attr.name);
          return;
        }
        if (name === 'href' && !isSafePreviewUrl(value)) {
          node.removeAttribute(attr.name);
        }
        return;
      }

      if (node.tagName === 'IMG') {
        if (!['src', 'alt', 'title', 'width', 'height', 'align'].includes(name)) {
          node.removeAttribute(attr.name);
          return;
        }
        if (name === 'src' && !isSafePreviewUrl(value)) {
          node.removeAttribute(attr.name);
        }
        return;
      }

      if (name !== 'class') {
        node.removeAttribute(attr.name);
      }
    });

    Array.from(node.children).forEach((child) => sanitizeNode(child));
  };

  Array.from(template.content.children).forEach((child) => sanitizeNode(child));
  return template.innerHTML;
}

function executeTextareaAction(
  textarea: HTMLTextAreaElement | null,
  action: 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'selectAll',
): boolean {
  if (!textarea || textarea.disabled) {
    return false;
  }

  textarea.focus();

  if (textarea.readOnly && action !== 'copy' && action !== 'selectAll') {
    return false;
  }

  if (action === 'selectAll') {
    textarea.select();
    return true;
  }

  return document.execCommand(action);
}

function createSourceBackedBlock(
  name: string,
  valueAttr: 'html' | 'markdown',
  className: string,
) {
  return Node.create<SourceBackedBlockOptions>({
    name,
    group: 'block',
    atom: true,
    isolating: true,
    selectable: true,
    draggable: false,
    defining: true,

    addOptions() {
      return {
        basePath: undefined,
      };
    },

    addAttributes() {
      return {
        [valueAttr]: {
          default: '',
        },
        kind: {
          default: null,
        },
      };
    },

    parseHTML() {
      return [{
        tag: `div[data-type="${name}"]`,
        getAttrs: element => ({
          [valueAttr]: element.getAttribute(`data-${valueAttr}`) ?? '',
          kind: element.getAttribute('data-kind'),
        }),
      }];
    },

    renderHTML({ node }) {
      const value = String(node.attrs[valueAttr] ?? '');
      const kind = typeof node.attrs.kind === 'string' && node.attrs.kind
        ? node.attrs.kind
        : null;

      return [
        'div',
        {
          'data-type': name,
          [`data-${valueAttr}`]: value,
          ...(kind ? { 'data-kind': kind } : {}),
        },
      ];
    },

    addNodeView() {
      return ({ editor, node, getPos }) => {
        let currentNode = node;
        let isEditing = false;
        let lastSyncedValue: string | null = null;
        let lastEditableState = editor.isEditable;
        let previewRoot: Root | null = null;
        let previewCheckTimer: number | null = null;
        const textareaTargetId = `${name}-textarea-${++sourceBackedBlockTextareaTargetCounter}`;
        let unbindEditTarget: (() => void) | null = null;

        const dom = document.createElement('div');
        dom.className = className;
        dom.draggable = false;
        dom.setAttribute('draggable', 'false');

        const body = document.createElement('div');
        body.className = `${className}__body`;
        body.draggable = false;
        body.setAttribute('draggable', 'false');

        const editorPane = document.createElement('div');
        editorPane.className = `${className}__pane ${className}__pane--editor`;

        const textarea = document.createElement('textarea');
        textarea.className = `${className}__textarea`;
        textarea.spellcheck = false;
        textarea.wrap = 'off';
        textarea.draggable = false;
        textarea.setAttribute('draggable', 'false');

        editorPane.append(textarea);

        const previewPane = document.createElement('div');
        previewPane.className = `${className}__pane ${className}__pane--preview`;

        const preview = document.createElement('div');
        preview.className = `${className}__preview markdown-body`;
        preview.draggable = false;
        preview.setAttribute('draggable', 'false');
        preview.tabIndex = 0;
        previewRoot = createRoot(preview);

        const sourceFallback = document.createElement('pre');
        sourceFallback.className = `${className}__source-fallback`;

        previewPane.append(preview, sourceFallback);
        body.append(editorPane, previewPane);
        dom.append(body);

        const applyAttrs = (attrs: Record<string, unknown>) => {
          const pos = typeof getPos === 'function' ? getPos() : null;
          if (typeof pos !== 'number') {
            return;
          }

          editor.view.dispatch(
            editor.view.state.tr.setNodeMarkup(pos, undefined, {
              ...currentNode.attrs,
              ...attrs,
            }),
          );
        };

        const syncEditingState = () => {
          dom.setAttribute('data-editing', isEditing ? 'true' : 'false');
          preview.tabIndex = editor.isEditable ? -1 : 0;
        };

        const setEditing = (nextEditing: boolean, options?: { focus?: boolean }) => {
          const resolvedEditing = editor.isEditable ? nextEditing : false;
          if (isEditing === resolvedEditing) {
            if (resolvedEditing && options?.focus) {
              focusElementWithoutScroll(textarea);
            }
            return;
          }

          isEditing = resolvedEditing;
          syncEditingState();

          if (resolvedEditing && options?.focus) {
            focusElementWithoutScroll(textarea);
          }
        };

        const exitEditing = () => {
          setEditing(false);
        };

        const renderPreview = (markdown: string) => {
          const kind = typeof currentNode.attrs.kind === 'string' ? currentNode.attrs.kind : null;
          const detailsSource = kind === 'details' ? parseDetailsSource(markdown) : null;
          const shouldCheckPreviewVisibility = name === 'rawHtmlBlock' || kind === 'details';

          const syncPreviewVisibility = (fallbackToMarkdownRenderer = false) => {
            if (!shouldCheckPreviewVisibility) {
              dom.setAttribute('data-preview-empty', 'false');
              return;
            }

            if (previewCheckTimer !== null) {
              window.clearTimeout(previewCheckTimer);
            }

            previewCheckTimer = window.setTimeout(() => {
              const hasVisibleContent = previewHasVisibleContent(preview);

              if (!hasVisibleContent && fallbackToMarkdownRenderer) {
                previewRoot?.render(
                  React.createElement(MarkdownRenderer, {
                    content: markdown,
                    basePath: this.options.basePath,
                    className: `${className}__markdown`,
                  }),
                );

                previewCheckTimer = window.setTimeout(() => {
                  dom.setAttribute('data-preview-empty', previewHasVisibleContent(preview) ? 'false' : 'true');
                  previewCheckTimer = null;
                }, 0);
                return;
              }

              dom.setAttribute('data-preview-empty', hasVisibleContent ? 'false' : 'true');
              previewCheckTimer = null;
            }, 0);
          };

          if (detailsSource) {
            previewRoot?.render(
              React.createElement(
                'details',
                {
                  open: detailsSource.open,
                  className: `${className}__details`,
                },
                React.createElement(
                  'summary',
                  {
                    className: `${className}__details-summary`,
                  },
                  React.createElement('span', {
                    className: `${className}__details-summary-content`,
                    dangerouslySetInnerHTML: {
                      __html: sanitizeDetailsSummaryHtml(detailsSource.summaryHtml),
                    },
                  }),
                ),
                detailsSource.bodyMarkdown
                  ? React.createElement(
                      'div',
                      {
                        className: `${className}__details-body`,
                      },
                      React.createElement(MarkdownRenderer, {
                        content: detailsSource.bodyMarkdown,
                        basePath: this.options.basePath,
                        className: `${className}__markdown`,
                      }),
                    )
                  : null,
              ),
            );

            sourceFallback.textContent = markdown;
            dom.setAttribute('data-preview-empty', 'false');
            syncPreviewVisibility(true);
            return;
          }

          previewRoot?.render(
            React.createElement(MarkdownRenderer, {
              content: markdown,
              basePath: this.options.basePath,
              className: `${className}__markdown`,
            }),
          );

          sourceFallback.textContent = markdown;
          dom.setAttribute('data-preview-empty', 'false');
          syncPreviewVisibility();
        };

        const sync = () => {
          const value = String(currentNode.attrs[valueAttr] ?? '');
          const editable = editor.isEditable;
          const valueChanged = lastSyncedValue !== value;
          const editableChanged = lastEditableState !== editable;

          dom.setAttribute('data-readonly', editable ? 'false' : 'true');

          if (valueChanged && textarea.value !== value) {
            textarea.value = value;
          }

          textarea.readOnly = !editable;
          if (!editable && isEditing) {
            isEditing = false;
          }
          syncEditingState();

          if (valueChanged || editableChanged) {
            renderPreview(value);
          }

          lastSyncedValue = value;
          lastEditableState = editable;
        };

        const enterEditing = () => {
          setEditing(true, { focus: true });
          const end = textarea.value.length;
          textarea.setSelectionRange(end, end);
          sync();
        };

        const stopPropagation = (event: Event) => {
          event.stopPropagation();
        };

        const handleTextareaKeyDown = (event: KeyboardEvent) => {
          if (isSelectAllShortcut(event)) {
            event.preventDefault();
            textarea.select();
          }

          event.stopPropagation();
        };

        const handlePreviewKeyDown = (event: KeyboardEvent) => {
          if (isSelectAllShortcut(event)) {
            event.preventDefault();
            selectElementContent(preview);
          }

          event.stopPropagation();
        };

        const handlePreviewClickCapture = (event: Event) => {
          const target = event.target;
          if (!(target instanceof HTMLElement)) {
            return;
          }

          if (target.closest('a')) {
            event.stopPropagation();
          }
        };

        const handlePreviewMouseDown = (event: MouseEvent) => {
          if (!editor.isEditable) {
            focusElementWithoutScroll(preview);
          }

          event.stopPropagation();
        };

        const handlePreviewDoubleClick = (event: MouseEvent) => {
          const target = event.target;
          if (!(target instanceof HTMLElement)) {
            return;
          }

          if (target.closest('a, summary')) {
            event.stopPropagation();
            return;
          }

          event.preventDefault();
          event.stopPropagation();
          enterEditing();
        };

        const preventDrag = (event: Event) => {
          event.preventDefault();
          event.stopPropagation();
        };

        const handleTextareaFocus = () => {
          activeEditTargetService.setActiveTarget(textareaTargetId);
        };

        const handleTextareaBlur = () => {
          window.setTimeout(() => {
            const activeElement = typeof document !== 'undefined' ? document.activeElement : null;
            if (activeElement instanceof HTMLElement && textarea.contains(activeElement)) {
              return;
            }

            activeEditTargetService.clearActiveTarget(textareaTargetId);
          }, 0);
        };

        textarea.addEventListener('mousedown', stopPropagation);
        textarea.addEventListener('click', stopPropagation);
        textarea.addEventListener('keydown', handleTextareaKeyDown);
        textarea.addEventListener('focus', handleTextareaFocus);
        textarea.addEventListener('blur', exitEditing);
        textarea.addEventListener('blur', handleTextareaBlur);

        preview.addEventListener('mousedown', handlePreviewMouseDown);
        preview.addEventListener('click', handlePreviewClickCapture, true);
        preview.addEventListener('click', stopPropagation);
        preview.addEventListener('dblclick', handlePreviewDoubleClick);
        preview.addEventListener('keydown', handlePreviewKeyDown);

        [dom, body, textarea, preview].forEach((element) => {
          element.addEventListener('dragstart', preventDrag);
        });

        textarea.addEventListener('input', () => {
          const nextValue = textarea.value;
          lastSyncedValue = nextValue;
          applyAttrs({ [valueAttr]: nextValue });
          void renderPreview(nextValue);
        });

        unbindEditTarget = activeEditTargetService.bindTarget({
          id: textareaTargetId,
          kind: 'markdown-textarea',
          focus: () => {
            focusElementWithoutScroll(textarea);
          },
          hasTextFocus: () => {
            const activeElement = typeof document !== 'undefined' ? document.activeElement : null;
            return activeElement === textarea;
          },
          undo: () => executeTextareaAction(textarea, 'undo'),
          redo: () => executeTextareaAction(textarea, 'redo'),
          cut: () => executeTextareaAction(textarea, 'cut'),
          copy: () => executeTextareaAction(textarea, 'copy'),
          paste: () => executeTextareaAction(textarea, 'paste'),
          selectAll: () => executeTextareaAction(textarea, 'selectAll'),
          containsElement: (element) => element === textarea,
        });

        sync();

        return {
          dom,
          update: (updatedNode) => {
            if (updatedNode.type.name !== this.name) {
              return false;
            }

            currentNode = updatedNode;
            sync();
            return true;
          },
          stopEvent: (event) => {
            if (event.type === 'dragstart') {
              return true;
            }

            const target = event.target;
            return target instanceof HTMLElement && dom.contains(target) && (
              !!target.closest('textarea') ||
              !!target.closest(`.${className}__preview`)
            );
          },
          ignoreMutation: (mutation) => {
            const target = mutation.target;
            return target instanceof globalThis.Node && dom.contains(target);
          },
          destroy: () => {
            activeEditTargetService.clearActiveTarget(textareaTargetId);
            unbindEditTarget?.();
            unbindEditTarget = null;
            if (previewCheckTimer !== null) {
              window.clearTimeout(previewCheckTimer);
              previewCheckTimer = null;
            }
            previewRoot?.unmount();
            previewRoot = null;
          },
        };
      };
    },
  });
}

export const RenderOnlyBlock = createSourceBackedBlock(
  'renderOnlyBlock',
  'markdown',
  'm-editor-render-only-block',
);

export const RawHtmlBlock = createSourceBackedBlock(
  'rawHtmlBlock',
  'html',
  'm-editor-raw-html-block',
);

export const RawHtmlInline = Node.create<RawHtmlInlineOptions>({
  name: 'rawHtmlInline',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  draggable: false,

  addOptions() {
    return {
      label: 'HTML',
    };
  },

  addAttributes() {
    return {
      html: {
        default: '',
      },
    };
  },

  parseHTML() {
    return [{
      tag: 'span[data-type="raw-html-inline"]',
      getAttrs: element => ({
        html: element.getAttribute('data-html') ?? '',
      }),
    }];
  },

  renderHTML({ node }) {
    return [
      'span',
      {
        'data-type': 'raw-html-inline',
        'data-html': String(node.attrs.html ?? ''),
      },
    ];
  },

  addNodeView() {
    return ({ node }) => ({
      dom: createRawHtmlInlinePreviewNode(
        String(node.attrs.html ?? ''),
        this.options.label,
      ),
    });
  },
});
