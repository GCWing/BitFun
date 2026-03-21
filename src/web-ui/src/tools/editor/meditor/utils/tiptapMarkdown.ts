import { unified } from 'unified';
import remarkGfm from 'remark-gfm';
import remarkParse from 'remark-parse';
import type { JSONContent } from '@tiptap/core';
import { createBlockId } from './blockId';

type MdastNode = {
  type?: string;
  value?: string;
  depth?: number;
  lang?: string | null;
  url?: string;
  alt?: string | null;
  title?: string | null;
  ordered?: boolean;
  start?: number;
  checked?: boolean | null;
  align?: Array<string | null>;
  children?: MdastNode[];
};

type Mark = {
  type: string;
  attrs?: Record<string, unknown>;
};

type TiptapMarkdownOptions = {
  preserveTrailingNewline?: boolean;
};

type AlignmentStackEntry = {
  align: string | null;
  groupId: number | null;
};

type AlignmentState = {
  activeAlign: string | null;
  activeGroupId: number | null;
  nextGroupId: number;
  stack: AlignmentStackEntry[];
};

const TOP_LEVEL_BLOCK_TYPES = new Set([
  'paragraph',
  'heading',
  'bulletList',
  'orderedList',
  'taskList',
  'blockquote',
  'codeBlock',
  'horizontalRule',
  'markdownTable',
]);

function createTextNode(text: string, marks: Mark[] = []): JSONContent[] {
  if (!text) {
    return [];
  }

  return [{
    type: 'text',
    text,
    ...(marks.length > 0 ? { marks } : {}),
  }];
}

function createParagraph(content: JSONContent[] = []): JSONContent {
  return {
    type: 'paragraph',
    ...(content.length > 0 ? { content } : {}),
  };
}

function withBlockAttrs(node: JSONContent, align: string | null, alignGroup: number | null): JSONContent {
  if (!node.type || !TOP_LEVEL_BLOCK_TYPES.has(node.type)) {
    return node;
  }

  return {
    ...node,
    attrs: {
      ...node.attrs,
      ...(align ? { align } : {}),
      ...(alignGroup !== null ? { alignGroup } : {}),
    },
  };
}

function withTopLevelBlockIds(content: JSONContent[]): JSONContent[] {
  return content.map(node => {
    if (!node.type || !TOP_LEVEL_BLOCK_TYPES.has(node.type)) {
      return node;
    }

    return {
      ...node,
      attrs: {
        ...node.attrs,
        blockId: typeof node.attrs?.blockId === 'string' ? node.attrs.blockId : createBlockId(),
      },
    };
  });
}

function flattenText(node: MdastNode | null | undefined): string {
  if (!node) {
    return '';
  }

  if (node.type === 'text' || node.type === 'inlineCode') {
    return node.value ?? '';
  }

  return (node.children ?? []).map(child => flattenText(child)).join('');
}

function convertInline(node: MdastNode, marks: Mark[] = []): JSONContent[] {
  switch (node.type) {
    case 'text':
      return createTextNode(node.value ?? '', marks);
    case 'inlineCode':
      return createTextNode(node.value ?? '', [...marks, { type: 'code' }]);
    case 'image':
      return [{
        type: 'markdownImage',
        attrs: {
          src: node.url ?? '',
          alt: node.alt ?? '',
          title: node.title ?? null,
        },
        ...(marks.length > 0 ? { marks } : {}),
      }];
    case 'strong':
      return (node.children ?? []).flatMap(child => convertInline(child, [...marks, { type: 'bold' }]));
    case 'emphasis':
      return (node.children ?? []).flatMap(child => convertInline(child, [...marks, { type: 'italic' }]));
    case 'delete':
      return (node.children ?? []).flatMap(child => convertInline(child, [...marks, { type: 'strike' }]));
    case 'link':
      return (node.children ?? []).flatMap(child => convertInline(child, [
        ...marks,
        { type: 'link', attrs: { href: node.url ?? '' } },
      ]));
    case 'break':
      return [{ type: 'hardBreak' }];
    default:
      return (node.children ?? []).flatMap(child => convertInline(child, marks));
  }
}

function convertListItemContent(node: MdastNode): JSONContent[] {
  const content = (node.children ?? []).flatMap(child => convertBlock(child));
  return content.length > 0 ? content : [createParagraph()];
}

function isTaskList(node: MdastNode): boolean {
  const items = node.children ?? [];
  return items.length > 0 && items.every(item => typeof item.checked === 'boolean');
}

function convertList(node: MdastNode): JSONContent[] {
  if (isTaskList(node)) {
    return [{
      type: 'taskList',
      content: (node.children ?? []).map(item => ({
        type: 'taskItem',
        attrs: {
          checked: !!item.checked,
        },
        content: convertListItemContent(item),
      })),
    }];
  }

  return [{
    type: node.ordered ? 'orderedList' : 'bulletList',
    ...(node.ordered ? { attrs: { start: node.start ?? 1 } } : {}),
    content: (node.children ?? []).map(item => ({
      type: 'listItem',
      content: convertListItemContent(item),
    })),
  }];
}

function convertTableCell(node: MdastNode, type: 'markdownTableHeader' | 'markdownTableCell'): JSONContent {
  const inline = (node.children ?? []).flatMap(child => convertInline(child));
  return {
    type,
    ...(inline.length > 0 ? { content: inline } : {}),
  };
}

function convertTable(node: MdastNode): JSONContent[] {
  const rows = node.children ?? [];

  return [{
    type: 'markdownTable',
    attrs: {
      align: node.align ?? [],
    },
    content: rows.map((row, rowIndex) => ({
      type: 'markdownTableRow',
      content: (row.children ?? []).map(cell => convertTableCell(
        cell,
        rowIndex === 0 ? 'markdownTableHeader' : 'markdownTableCell',
      )),
    })),
  }];
}

function convertBlock(node: MdastNode): JSONContent[] {
  switch (node.type) {
    case 'paragraph': {
      const inline = (node.children ?? []).flatMap(child => convertInline(child));
      return [createParagraph(inline)];
    }
    case 'heading': {
      const inline = (node.children ?? []).flatMap(child => convertInline(child));
      return [{
        type: 'heading',
        attrs: { level: Math.min(Math.max(node.depth ?? 1, 1), 6) },
        ...(inline.length > 0 ? { content: inline } : {}),
      }];
    }
    case 'blockquote':
      return [{
        type: 'blockquote',
        content: (node.children ?? []).flatMap(child => convertBlock(child)),
      }];
    case 'list':
      return convertList(node);
    case 'table':
      return convertTable(node);
    case 'code':
      return [{
        type: 'codeBlock',
        attrs: {
          language: node.lang ?? null,
        },
        content: createTextNode(node.value ?? ''),
      }];
    case 'thematicBreak':
      return [{ type: 'horizontalRule' }];
    case 'image':
      return [createParagraph(createTextNode(`![${flattenText(node)}](${node.url ?? ''})`))];
    case 'html':
      return [];
    default:
      if (node.children?.length) {
        return node.children.flatMap(child => convertBlock(child));
      }

      return node.value ? [createParagraph(createTextNode(node.value))] : [];
  }
}

function wrapTextWithMarks(text: string, marks?: Mark[]): string {
  if (!text) {
    return '';
  }

  const hasCodeMark = (marks ?? []).some(mark => mark.type === 'code');
  const escapedText = hasCodeMark
    ? text
    : text
        .replace(/\\/g, '\\\\')
        .replace(/([`*_[\]|])/g, '\\$1');

  return (marks ?? []).reduce((result, mark) => {
    switch (mark.type) {
      case 'bold':
        return `**${result}**`;
      case 'italic':
        return `*${result}*`;
      case 'strike':
        return `~~${result}~~`;
      case 'code':
        return `\`${result}\``;
      case 'link':
        return `[${result}](${String(mark.attrs?.href ?? '')})`;
      default:
        return result;
    }
  }, escapedText);
}

function wrapInlineMarkdownWithMarks(markdown: string, marks?: Mark[]): string {
  if (!markdown) {
    return '';
  }

  return (marks ?? []).reduce((result, mark) => {
    switch (mark.type) {
      case 'bold':
        return `**${result}**`;
      case 'italic':
        return `*${result}*`;
      case 'strike':
        return `~~${result}~~`;
      case 'code':
        return `\`${result}\``;
      case 'link':
        return `[${result}](${String(mark.attrs?.href ?? '')})`;
      default:
        return result;
    }
  }, markdown);
}

function escapeMarkdownImageText(value: string): string {
  return value
    .replace(/\\/g, '\\\\')
    .replace(/([\[\]])/g, '\\$1');
}

function escapeMarkdownUrl(value: string): string {
  return value
    .replace(/\\/g, '\\\\')
    .replace(/([()])/g, '\\$1');
}

function renderMarkdownImage(node: JSONContent): string {
  const alt = escapeMarkdownImageText(String(node.attrs?.alt ?? ''));
  const src = escapeMarkdownUrl(String(node.attrs?.src ?? ''));
  const title = typeof node.attrs?.title === 'string' && node.attrs.title
    ? ` "${node.attrs.title.replace(/"/g, '\\"')}"`
    : '';

  return wrapInlineMarkdownWithMarks(`![${alt}](${src}${title})`, node.marks as Mark[] | undefined);
}

function walkMdast(node: MdastNode | null | undefined, visit: (current: MdastNode) => void): void {
  if (!node) {
    return;
  }

  visit(node);
  (node.children ?? []).forEach(child => {
    walkMdast(child, visit);
  });
}

export function getUnsupportedTiptapMarkdownFeatures(markdown: string): string[] {
  if (!markdown) {
    return [];
  }

  const issues = new Set<string>();

  if (/^\n+/.test(markdown)) {
    issues.add('leadingBlankLines');
  }

  if (/\n{2,}$/.test(markdown)) {
    issues.add('multipleTrailingBlankLines');
  }

  const tree = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .parse(markdown) as MdastNode;

  walkMdast(tree, (node) => {
    if (node.type === 'footnoteDefinition' || node.type === 'footnoteReference') {
      issues.add('footnote');
    }
  });

  const roundTripped = tiptapDocToMarkdown(markdownToTiptapDoc(markdown), {
    preserveTrailingNewline: markdown.endsWith('\n'),
  });
  if (roundTripped !== markdown) {
    issues.add('roundTripMismatch');
  }

  return Array.from(issues);
}

export function canRoundTripMarkdownWithTiptap(markdown: string): boolean {
  return getUnsupportedTiptapMarkdownFeatures(markdown).length === 0;
}

function renderInline(content: JSONContent[] = []): string {
  return content.map((node: JSONContent) => {
    if (node.type === 'text') {
      return wrapTextWithMarks(node.text ?? '', node.marks as Mark[] | undefined);
    }

    if (node.type === 'hardBreak') {
      return '  \n';
    }

    if (node.type === 'markdownImage') {
      return renderMarkdownImage(node);
    }

    return renderInline(node.content ?? []);
  }).join('');
}

function parseAlignmentDirective(html: string, state: AlignmentState): boolean {
  const tagPattern = /<\/?div\b[^>]*>/gi;
  let matched = false;
  let cursor = 0;
  let match: RegExpExecArray | null;

  while ((match = tagPattern.exec(html)) !== null) {
    const before = html.slice(cursor, match.index);
    if (before.trim()) {
      return false;
    }

    const token = match[0];
    cursor = match.index + token.length;
    matched = true;

    if (/^<\s*\/\s*div/i.test(token)) {
      state.stack.pop();
      const nextState = state.stack.at(-1) ?? null;
      state.activeAlign = nextState?.align ?? null;
      state.activeGroupId = nextState?.groupId ?? null;
      continue;
    }

    const alignMatch = token.match(/\balign\s*=\s*["']?([a-zA-Z-]+)["']?/i);
    const align = alignMatch?.[1]?.toLowerCase() ?? null;
    const groupId = state.nextGroupId++;
    state.stack.push({ align, groupId });
    state.activeAlign = align;
    state.activeGroupId = groupId;
  }

  return matched && html.slice(cursor).trim().length === 0;
}

function convertRootMarkdownChildren(children: MdastNode[]): JSONContent[] {
  const alignmentState: AlignmentState = {
    activeAlign: null,
    activeGroupId: null,
    nextGroupId: 1,
    stack: [],
  };

  const content: JSONContent[] = [];

  children.forEach((child) => {
    if (child.type === 'html' && child.value && parseAlignmentDirective(child.value, alignmentState)) {
      return;
    }

    const nextNodes = convertBlock(child).map(node => withBlockAttrs(
      node,
      alignmentState.activeAlign,
      alignmentState.activeGroupId,
    ));
    content.push(...nextNodes);
  });

  return content;
}

function normalizeTableCellMarkdown(markdown: string): string {
  return markdown
    .replace(/\n+/g, '<br>')
    .replace(/\|/g, '\\|')
    .trim();
}

function renderTableRow(row: JSONContent): string {
  const cells = (row.content ?? []).map((cell: JSONContent) => {
    const value = renderInline(cell.content ?? []);
    return normalizeTableCellMarkdown(value);
  });

  return `| ${cells.join(' | ')} |`;
}

function renderTableSeparator(alignments: unknown[], columnCount: number): string {
  const cells = Array.from({ length: columnCount }, (_, index) => {
    const align = typeof alignments[index] === 'string' ? alignments[index] : null;

    switch (align) {
      case 'left':
        return ':---';
      case 'center':
        return ':---:';
      case 'right':
        return '---:';
      default:
        return '---';
    }
  });

  return `| ${cells.join(' | ')} |`;
}

function indentMarkdown(markdown: string, depth: number): string {
  const indent = '  '.repeat(depth);
  return markdown
    .split('\n')
    .map((line: string) => (line ? `${indent}${line}` : line))
    .join('\n');
}

function renderListItem(
  item: JSONContent,
  prefix: string,
  depth: number,
  taskChecked?: boolean,
): string {
  const children = item.content ?? [];
  const indent = '  '.repeat(depth);
  const marker = taskChecked === undefined ? prefix : `- [${taskChecked ? 'x' : ' '}] `;

  if (children.length === 0) {
    return `${indent}${marker}`;
  }

  const [first, ...rest] = children;
  const firstRendered = first.type === 'paragraph'
    ? renderInline(first.content ?? [])
    : renderBlock(first, depth + 1);

  const lines: string[] = [`${indent}${marker}${firstRendered}`];

  rest.forEach((child: JSONContent) => {
    lines.push(indentMarkdown(renderBlock(child, depth + 1), depth + 1));
  });

  return lines.join('\n');
}

function renderBlock(node: JSONContent, depth = 0): string {
  switch (node.type) {
    case 'paragraph':
      return renderInline(node.content ?? []);
    case 'heading':
      return `${'#'.repeat(Number(node.attrs?.level ?? 1))} ${renderInline(node.content ?? [])}`.trimEnd();
    case 'blockquote': {
      const body = (node.content ?? []).map((child: JSONContent) => renderBlock(child, depth)).join('\n\n');
      return body
        .split('\n')
        .map((line: string) => `> ${line}`)
        .join('\n');
    }
    case 'bulletList':
      return (node.content ?? []).map((item: JSONContent) => renderListItem(item, '- ', depth)).join('\n');
    case 'orderedList': {
      const start = Number(node.attrs?.start ?? 1);
      return (node.content ?? []).map((item: JSONContent, index: number) => renderListItem(item, `${start + index}. `, depth)).join('\n');
    }
    case 'taskList':
      return (node.content ?? []).map((item: JSONContent) => renderListItem(item, '- ', depth, !!item.attrs?.checked)).join('\n');
    case 'codeBlock': {
      const language = String(node.attrs?.language ?? '').trim();
      const text = (node.content ?? [])
        .map((child: JSONContent) => (child.type === 'text' ? child.text ?? '' : renderInline(child.content ?? [])))
        .join('');
      return `\`\`\`${language}\n${text}\n\`\`\``;
    }
    case 'horizontalRule':
      return '---';
    case 'markdownTable': {
      const rows = node.content ?? [];
      if (rows.length === 0) {
        return '';
      }

      const headerRow = rows[0];
      const headerCellCount = headerRow.content?.length ?? 0;
      const alignments = Array.isArray(node.attrs?.align) ? node.attrs.align as unknown[] : [];
      const bodyRows = rows.slice(1);

      return [
        renderTableRow(headerRow),
        renderTableSeparator(alignments, headerCellCount),
        ...bodyRows.map(row => renderTableRow(row)),
      ].join('\n');
    }
    default:
      return (node.content ?? []).map((child: JSONContent) => renderBlock(child, depth)).join('\n\n');
  }
}

function wrapAlignedMarkdownBlocks(markdownBlocks: string[], align: string | null): string {
  if (markdownBlocks.length === 0) {
    return '';
  }

  if (!align) {
    return markdownBlocks.join('\n\n');
  }

  return [
    `<div align="${align}">`,
    '',
    markdownBlocks.join('\n\n'),
    '',
    '</div>',
  ].join('\n');
}

export function markdownToTiptapDoc(markdown: string): JSONContent {
  const tree = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .parse(markdown) as MdastNode;

  const content = withTopLevelBlockIds(convertRootMarkdownChildren(tree.children ?? []));

  return {
    type: 'doc',
    content: content.length > 0 ? content : [createParagraph()],
  };
}

export function tiptapDocToMarkdown(
  doc: JSONContent | null | undefined,
  options: TiptapMarkdownOptions = {},
): string {
  const content = doc?.content ?? [];
  const chunks: string[] = [];
  let group: string[] = [];
  let groupAlign: string | null = null;
  let groupAlignId: string | null = null;

  const flushGroup = () => {
    if (group.length === 0) {
      return;
    }

    chunks.push(wrapAlignedMarkdownBlocks(group, groupAlign));
    group = [];
    groupAlign = null;
    groupAlignId = null;
  };

  content.forEach((node: JSONContent) => {
    const rendered = renderBlock(node);
    if (!rendered) {
      return;
    }

    const align = typeof node.attrs?.align === 'string' && node.attrs.align
      ? node.attrs.align
      : null;
    const alignId = node.attrs?.alignGroup !== null && node.attrs?.alignGroup !== undefined
      ? String(node.attrs.alignGroup)
      : null;

    if (group.length === 0) {
      group = [rendered];
      groupAlign = align;
      groupAlignId = alignId;
      return;
    }

    if (groupAlign === align && groupAlignId === alignId) {
      group.push(rendered);
      return;
    }

    flushGroup();
    group = [rendered];
    groupAlign = align;
    groupAlignId = alignId;
  });

  flushGroup();

  const markdown = chunks
    .filter(Boolean)
    .join('\n\n')
    .replace(/<\/div>\n\n<div\b/g, '</div>\n<div')
    .replace(/\n{3,}/g, '\n\n');

  if (!markdown) {
    return '';
  }

  return options.preserveTrailingNewline ? `${markdown}\n` : markdown;
}
