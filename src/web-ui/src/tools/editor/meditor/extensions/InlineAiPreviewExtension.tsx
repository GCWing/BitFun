import { Extension } from '@tiptap/core';
import { Plugin } from '@tiptap/pm/state';
import type { EditorState } from '@tiptap/pm/state';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { EditorView } from '@tiptap/pm/view';
import { createRoot, type Root } from 'react-dom/client';
import { InlineAiPreviewBlock } from '../components/InlineAiPreviewBlock';
import { inlineAiPreviewPluginKey, type InlineAiPreviewWidgetState } from './InlineAiPreviewPluginKey';

function getWidgetPosition(doc: ProseMirrorNode, blockId: string): number | null {
  let position: number | null = null;

  doc.forEach((node, offset) => {
    if (typeof node.attrs?.blockId !== 'string' || node.attrs.blockId !== blockId) {
      return;
    }

    position = offset + node.nodeSize;
  });

  return position;
}

class InlineAiPreviewPluginView {
  private container: HTMLElement | null = null;
  private root: Root | null = null;

  constructor(private view: EditorView) {
    this.render();
  }

  update(view: EditorView): void {
    this.view = view;
    this.render();
  }

  destroy(): void {
    this.cleanup();
  }

  private render(): void {
    const previewState = inlineAiPreviewPluginKey.getState(this.view.state);
    const blockId = previewState?.blockId;
    const nextContainer = blockId
      ? this.view.dom.querySelector<HTMLElement>(`[data-inline-ai-preview-widget="${blockId}"]`)
      : null;

    if (!previewState || !nextContainer) {
      this.cleanup();
      return;
    }

    if (!this.container || this.container !== nextContainer) {
      this.cleanup();
      this.container = nextContainer;
      this.root = createRoot(nextContainer);
    }

    if (!this.root) {
      return;
    }

    this.root.render(
      <InlineAiPreviewBlock
        status={previewState.status}
        response={previewState.response}
        error={previewState.error}
        basePath={previewState.basePath}
        canAccept={previewState.canAccept}
        labels={previewState.labels}
        onAccept={previewState.onAccept}
        onReject={previewState.onReject}
        onRetry={previewState.onRetry}
      />
    );
  }

  private cleanup(): void {
    this.root?.unmount();
    this.root = null;
    this.container = null;
  }
}

export const InlineAiPreviewExtension = Extension.create({
  name: 'inlineAiPreview',

  addProseMirrorPlugins() {
    return [
      new Plugin<InlineAiPreviewWidgetState | null>({
        key: inlineAiPreviewPluginKey,
        state: {
          init: () => null,
          apply: (
            transaction,
            value,
            _oldState: EditorState,
            newState: EditorState,
          ) => {
            const meta = transaction.getMeta(inlineAiPreviewPluginKey);

            if (meta !== undefined) {
              return meta as InlineAiPreviewWidgetState | null;
            }

            if (value && getWidgetPosition(newState.doc, value.blockId) === null) {
              return null;
            }

            return value;
          },
        },
        props: {
          decorations: (state) => {
            const previewState = inlineAiPreviewPluginKey.getState(state);
            if (!previewState) {
              return null;
            }

            const position = getWidgetPosition(state.doc, previewState.blockId);
            if (position === null) {
              return null;
            }

            return DecorationSet.create(state.doc, [
              Decoration.widget(
                position,
                () => {
                  const widget = document.createElement('div');
                  widget.className = 'm-editor-inline-ai-widget';
                  widget.dataset.inlineAiPreviewWidget = previewState.blockId;
                  widget.setAttribute('contenteditable', 'false');
                  return widget;
                },
                {
                  key: `inline-ai-preview-${previewState.blockId}`,
                  side: 1,
                  ignoreSelection: true,
                  stopEvent: () => true,
                },
              ),
            ]);
          },
        },
        view: (view) => new InlineAiPreviewPluginView(view),
      }),
    ];
  },
});
