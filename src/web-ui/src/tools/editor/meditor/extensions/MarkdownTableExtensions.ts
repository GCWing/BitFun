import { mergeAttributes, Node } from '@tiptap/core';

function parseAlignments(value: string | null): Array<string | null> {
  if (!value) {
    return [];
  }

  return value
    .split(',')
    .map(item => item.trim() || null);
}

function serializeAlignments(value: unknown): string | null {
  if (!Array.isArray(value) || value.length === 0) {
    return null;
  }

  return value
    .map(item => (typeof item === 'string' ? item : ''))
    .join(',');
}

export const MarkdownTable = Node.create({
  name: 'markdownTable',
  group: 'block',
  content: 'markdownTableRow+',
  isolating: true,

  addAttributes() {
    return {
      align: {
        default: [],
        parseHTML: (element: HTMLElement) => parseAlignments(element.getAttribute('data-column-align')),
        renderHTML: (attributes: Record<string, unknown>) => {
          const align = serializeAlignments(attributes.align);
          return align ? { 'data-column-align': align } : {};
        },
      },
    };
  },

  parseHTML() {
    return [{ tag: 'table[data-type="markdown-table"]' }, { tag: 'table' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['table', mergeAttributes(HTMLAttributes, { 'data-type': 'markdown-table' }), ['tbody', 0]];
  },
});

export const MarkdownTableRow = Node.create({
  name: 'markdownTableRow',
  content: '(markdownTableHeader|markdownTableCell)+',

  parseHTML() {
    return [{ tag: 'tr' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['tr', mergeAttributes(HTMLAttributes), 0];
  },
});

export const MarkdownTableHeader = Node.create({
  name: 'markdownTableHeader',
  content: 'inline*',

  parseHTML() {
    return [{ tag: 'th' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['th', mergeAttributes(HTMLAttributes), 0] as const;
  },
});

export const MarkdownTableCell = Node.create({
  name: 'markdownTableCell',
  content: 'inline*',

  parseHTML() {
    return [{ tag: 'td' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ['td', mergeAttributes(HTMLAttributes), 0] as const;
  },
});
