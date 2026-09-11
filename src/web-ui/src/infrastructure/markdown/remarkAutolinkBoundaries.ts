type MarkdownNode = {
  type: string;
  value?: string;
  url?: string;
  title?: string | null;
  position?: {
    start: { line: number; column: number; offset?: number };
    end: { line: number; column: number; offset?: number };
  };
  children?: MarkdownNode[];
};

// CJK prose punctuation delimits bare URLs. Keep Chinese letters, ASCII URL
// punctuation (including balanced parentheses), and percent escapes intact.
const PROSE_BOUNDARY = /([\u3001\u3002\u3008-\u3011\u3014-\u301f\uff01\uff08\uff09\uff0c\uff1a\uff1b\uff1f\u2018-\u201d]+)/;
const URL_PREFIX = /^(https?:\/\/|www\.)/i;

function splitBareLink(node: MarkdownNode, source: string): MarkdownNode[] | undefined {
  if (node.type !== 'link' || !node.url || node.children?.length !== 1) return;
  const label = node.children[0];
  if (label.type !== 'text' || !label.value || !URL_PREFIX.test(label.value)) return;
  if (!PROSE_BOUNDARY.test(label.value)) return;

  // GFM's fallback transform produces positionless links after Markdown
  // escapes are decoded. Authored inline/angle links retain source positions.
  // Eight characters cover the longest supported prefix, https://.
  const offset = node.position?.start.offset;
  if (offset !== undefined && !URL_PREFIX.test(source.slice(offset, offset + 8))) return;

  return label.value.split(PROSE_BOUNDARY).filter(Boolean).map(part => {
    if (!URL_PREFIX.test(part)) return { type: 'text', value: part };
    return {
      type: 'link',
      title: null,
      url: /^www\./i.test(part) ? `http://${part}` : part,
      children: [{ type: 'text', value: part }],
    };
  });
}

/** Apply prose boundaries only to GFM-generated links, never authored links. */
export function remarkAutolinkBoundaries() {
  return (tree: MarkdownNode, file: { value: unknown }) => {
    const source = String(file.value);
    const pending = [tree];
    while (pending.length > 0) {
      const parent = pending.pop()!;
      const children = parent.children;
      if (!children) continue;
      let nextChildren: MarkdownNode[] | undefined;
      for (let index = 0; index < children.length; index += 1) {
        const child = children[index];
        const replacements = splitBareLink(child, source);
        if (replacements) {
          // Copy the prefix only on the first change. Append each remaining
          // sibling once instead of shifting the array for every split link.
          nextChildren ??= children.slice(0, index);
          for (const replacement of replacements) nextChildren.push(replacement);
        } else {
          nextChildren?.push(child);
          // Traverse only original children. Split links are already final;
          // visiting their newly generated nodes would repeat the work.
          if (child.children?.length) pending.push(child);
        }
      }
      if (nextChildren) parent.children = nextChildren;
    }
  };
}
