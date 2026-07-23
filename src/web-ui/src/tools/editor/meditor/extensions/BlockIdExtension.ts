import { Extension } from '@tiptap/core';
import { Plugin } from '@tiptap/pm/state';
import type { EditorState, Transaction } from '@tiptap/pm/state';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { createBlockId } from '../utils/blockId';

const TOP_LEVEL_TYPES = [
  'paragraph',
  'heading',
  'bulletList',
  'orderedList',
  'taskList',
  'blockquote',
  'codeBlock',
  'horizontalRule',
  'markdownTable',
  'details',
  'renderOnlyBlock',
  'rawHtmlBlock',
];

export const BlockIdExtension = Extension.create({
  name: 'blockId',

  addGlobalAttributes() {
    return [
      {
        types: TOP_LEVEL_TYPES,
        attributes: {
          blockId: {
            default: null,
            parseHTML: (element: HTMLElement) => element.getAttribute('data-block-id'),
            renderHTML: (attributes: Record<string, unknown>) => (
              attributes.blockId
                ? { 'data-block-id': attributes.blockId }
                : {}
            ),
          },
        },
      },
    ];
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        appendTransaction: (
          _transactions: readonly Transaction[],
          _oldState: EditorState,
          newState: EditorState,
        ) => {
          let transaction = newState.tr;
          let hasChanges = false;

          newState.doc.forEach((node: ProseMirrorNode, offset: number) => {
            if (!TOP_LEVEL_TYPES.includes(node.type.name) || node.attrs.blockId) {
              return;
            }

            transaction = transaction.setNodeMarkup(offset, undefined, {
              ...node.attrs,
              blockId: createBlockId(),
            });
            hasChanges = true;
          });

          return hasChanges ? transaction : null;
        },
      }),
    ];
  },
});
