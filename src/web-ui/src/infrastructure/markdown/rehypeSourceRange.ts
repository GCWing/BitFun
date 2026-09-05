/** Render one source region after resolving references against the whole document. */
export interface MarkdownSourceRange {
  start: number;
  end: number;
  idPrefix: string;
}

type HtmlNode = {
  type: string;
  tagName?: string;
  properties?: Record<string, unknown>;
  position?: { start: { offset?: number }; end: { offset?: number } };
  children?: HtmlNode[];
};

export function rehypeSourceRange(range?: MarkdownSourceRange) {
  return (tree: HtmlNode) => {
    if (!range) return;
    const inside = (node: HtmlNode) => {
      const start = node.position?.start.offset;
      const end = node.position?.end.offset;
      return start !== undefined && end !== undefined && start >= range.start && end <= range.end;
    };
    const walk = (node: HtmlNode, visit: (node: HtmlNode) => void) => {
      visit(node);
      node.children?.forEach(child => walk(child, visit));
    };

    // Namespace all anchors consistently across independent render roots. Run
    // after sanitization and account for its user-content- clobber protection.
    const ids = new Map<string, string>();
    walk(tree, node => {
      const id = node.properties?.id;
      if (typeof id === 'string') ids.set(id, `${range.idPrefix}${id}`);
    });
    walk(tree, node => {
      const props = node.properties;
      if (!props) return;
      if (typeof props.id === 'string') props.id = ids.get(props.id);
      if (typeof props.href === 'string' && props.href.startsWith('#')) {
        const id = props.href.slice(1);
        const target = ids.get(id) ?? ids.get(`user-content-${id}`);
        if (target) props.href = `#${target}`;
      }
      if (Array.isArray(props.ariaDescribedBy)) {
        props.ariaDescribedBy = props.ariaDescribedBy.map(id =>
          ids.get(String(id)) ?? ids.get(`user-content-${id}`) ?? id);
      }
    });

    tree.children = (tree.children ?? []).flatMap(node => {
      if (inside(node)) return [node];
      if (node.tagName !== 'section' || node.properties?.dataFootnotes === undefined) return [];
      const list = node.children?.find(child => child.tagName === 'ol');
      const items = list?.children?.filter(child => child.tagName === 'li') ?? [];
      const selected = items.filter(inside);
      if (!list || selected.length === 0) return [];
      selected.forEach(item => {
        item.properties = { ...item.properties, value: items.indexOf(item) + 1 };
      });
      list.children = selected;
      // Each definition is rendered once, with one shared heading and globally
      // ordered numbers, even when definitions live in separate editor blocks.
      if (!selected.includes(items[0])) node.children = [list];
      return [node];
    });
  };
}
