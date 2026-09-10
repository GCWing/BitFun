const SEARCH_HIGHLIGHT_CURRENT_NAME = 'openbitfun-flowchat-search-current';
const SEARCH_HIGHLIGHT_MATCH_NAME = 'openbitfun-flowchat-search-match';

type HighlightRegistryLike = {
  set: (name: string, highlight: unknown) => void;
  delete: (name: string) => void;
};

type HighlightConstructorLike = new (...ranges: Range[]) => unknown;

interface SearchHighlightRanges {
  current: Range | null;
  matches: readonly Range[];
}

const documentHighlights = new WeakMap<Document, Map<object, SearchHighlightRanges>>();

interface FoldedTextOffset {
  start: number;
  end: number;
}

function isSearchableTextNode(node: Node): node is Text {
  if (node.nodeType !== Node.TEXT_NODE || !node.textContent) {
    return false;
  }

  const parent = node.parentElement;
  if (!parent) {
    return false;
  }

  return !parent.closest('script, style, button, input, textarea, [contenteditable="true"], [hidden], [aria-hidden="true"]');
}

function foldTextWithOriginalOffsets(text: string): {
  text: string;
  offsets: FoldedTextOffset[];
} {
  // Built from the same per-character foldings the offsets are built from.
  // Lowercasing the whole string separately is not guaranteed to produce the
  // same length as concatenating per-character results, and any divergence
  // leaves holes in `offsets` — the lookup then returns undefined and a real
  // match is silently dropped.
  const foldedParts: string[] = [];
  const offsets: FoldedTextOffset[] = [];
  let originalOffset = 0;
  let foldedOffset = 0;

  for (const character of text) {
    const start = originalOffset;
    originalOffset += character.length;
    const folded = character.toLowerCase();
    foldedParts.push(folded);

    for (let index = 0; index < folded.length; index += 1) {
      offsets[foldedOffset + index] = {
        start,
        end: originalOffset,
      };
    }
    foldedOffset += folded.length;
  }

  return {
    text: foldedParts.join(''),
    offsets,
  };
}

/**
 * Finds every non-overlapping case-insensitive occurrence of the query, even
 * when Markdown splits it across adjacent text nodes (for example, around
 * inline emphasis or code spans). Ranges are returned in document order.
 */
export function findFlowChatSearchTextRanges(root: HTMLElement, query: string): Range[] {
  const trimmedQuery = query.trim();
  if (!trimmedQuery) {
    return [];
  }

  const ownerDocument = root.ownerDocument;
  const walker = ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const textNodes: Array<{ node: Text; start: number; end: number }> = [];
  let combinedText = '';
  let currentNode = walker.nextNode();

  while (currentNode) {
    if (isSearchableTextNode(currentNode)) {
      const start = combinedText.length;
      combinedText += currentNode.textContent;
      textNodes.push({
        node: currentNode,
        start,
        end: combinedText.length,
      });
    }
    currentNode = walker.nextNode();
  }

  const folded = foldTextWithOriginalOffsets(combinedText);
  const foldedQuery = trimmedQuery.toLowerCase();
  const ranges: Range[] = [];
  let searchFrom = 0;

  for (;;) {
    const foldedMatchStart = folded.text.indexOf(foldedQuery, searchFrom);
    if (foldedMatchStart < 0) {
      return ranges;
    }
    searchFrom = foldedMatchStart + foldedQuery.length;

    const foldedMatchEnd = foldedMatchStart + foldedQuery.length;
    const matchStart = folded.offsets[foldedMatchStart]?.start;
    const matchEnd = folded.offsets[foldedMatchEnd - 1]?.end;
    if (matchStart === undefined || matchEnd === undefined) {
      continue;
    }

    const startEntry = textNodes.find(entry => matchStart >= entry.start && matchStart < entry.end);
    const endEntry = textNodes.find(entry => matchEnd > entry.start && matchEnd <= entry.end);
    if (!startEntry || !endEntry) {
      continue;
    }

    const range = ownerDocument.createRange();
    range.setStart(startEntry.node, matchStart - startEntry.start);
    range.setEnd(endEntry.node, matchEnd - endEntry.start);
    ranges.push(range);
  }
}

export function findFlowChatSearchTextRange(root: HTMLElement, query: string): Range | null {
  return findFlowChatSearchTextRanges(root, query)[0] ?? null;
}

function publishSearchHighlights(ownerDocument: Document): void {
  const view = ownerDocument.defaultView;
  const cssWithHighlights = view?.CSS as (typeof CSS & {
    highlights?: HighlightRegistryLike;
  }) | undefined;
  const HighlightConstructor = (view as (Window & {
    Highlight?: HighlightConstructorLike;
  }) | null)?.Highlight;

  if (!cssWithHighlights?.highlights) {
    return;
  }

  cssWithHighlights.highlights.delete(SEARCH_HIGHLIGHT_CURRENT_NAME);
  cssWithHighlights.highlights.delete(SEARCH_HIGHLIGHT_MATCH_NAME);
  if (!HighlightConstructor) {
    return;
  }
  const owners = documentHighlights.get(ownerDocument)?.values() ?? [];
  const current: Range[] = [];
  const matches: Range[] = [];
  for (const ranges of owners) {
    if (ranges.current?.startContainer.isConnected) current.push(ranges.current);
    matches.push(...ranges.matches.filter(range => range.startContainer.isConnected));
  }
  if (matches.length > 0) {
    cssWithHighlights.highlights.set(
      SEARCH_HIGHLIGHT_MATCH_NAME,
      new HighlightConstructor(...matches),
    );
  }
  // Register current last so it wins if two presentations share a text range.
  if (current.length > 0) {
    cssWithHighlights.highlights.set(SEARCH_HIGHLIGHT_CURRENT_NAME, new HighlightConstructor(...current));
  }
}

/** Each mounted row releases only its own ranges, including across chat panes. */
export function createFlowChatSearchHighlightOwner(ownerDocument: Document) {
  const owner = {};
  let disposed = false;
  return {
    update(current: Range | null, matches: readonly Range[]) {
      if (disposed) return;
      let owners = documentHighlights.get(ownerDocument);
      if (!owners) {
        owners = new Map();
        documentHighlights.set(ownerDocument, owners);
      }
      owners.set(owner, { current, matches });
      publishSearchHighlights(ownerDocument);
    },
    dispose() {
      disposed = true;
      documentHighlights.get(ownerDocument)?.delete(owner);
      publishSearchHighlights(ownerDocument);
    },
  };
}

export function findElementWithDataValue(
  root: HTMLElement,
  attributeName: 'data-flow-item-id' | 'data-tool-card-id',
  value: string,
): HTMLElement | null {
  return Array.from(root.querySelectorAll<HTMLElement>(`[${attributeName}]`))
    .find(element => element.getAttribute(attributeName) === value) ?? null;
}

export function getFlowChatSearchTextRoot(
  wrapper: HTMLElement,
  flowItemId?: string,
): HTMLElement | null {
  if (flowItemId) {
    const flowItem = findElementWithDataValue(wrapper, 'data-flow-item-id', flowItemId);
    if (flowItem) {
      return flowItem;
    }

    const thinkingItem = findElementWithDataValue(wrapper, 'data-tool-card-id', flowItemId);
    const thinkingText = thinkingItem?.querySelector<HTMLElement>('.thinking-markdown');
    if (thinkingText) {
      return thinkingText;
    }

    // A collapsed or unmounted source must not highlight unrelated card labels.
    return null;
  }

  return wrapper.querySelector<HTMLElement>('.user-message-item__content') ?? wrapper;
}
