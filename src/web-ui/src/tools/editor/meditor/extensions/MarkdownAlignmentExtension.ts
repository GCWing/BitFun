import { Extension } from '@tiptap/core';

const ALIGNABLE_TYPES = [
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

export const MarkdownAlignmentExtension = Extension.create({
  name: 'markdownAlignment',

  addGlobalAttributes() {
    return [
      {
        types: ALIGNABLE_TYPES,
        attributes: {
          align: {
            default: null,
            parseHTML: (element: HTMLElement) => (
              element.getAttribute('data-align') ??
              element.style.textAlign ??
              null
            ),
            renderHTML: (attributes: Record<string, unknown>) => {
              const align = typeof attributes.align === 'string' && attributes.align
                ? attributes.align
                : null;

              return align
                ? {
                    'data-align': align,
                    style: `text-align: ${align};`,
                  }
                : {};
            },
          },
          alignGroup: {
            default: null,
            parseHTML: (element: HTMLElement) => element.getAttribute('data-align-group'),
            renderHTML: (attributes: Record<string, unknown>) => {
              const alignGroup = attributes.alignGroup;
              return alignGroup !== null && alignGroup !== undefined
                ? { 'data-align-group': String(alignGroup) }
                : {};
            },
          },
        },
      },
    ];
  },
});
