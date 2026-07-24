const SEARCH_HIGHLIGHT_NAME = 'bitfun-flowchat-search-current';

type HighlightRegistryLike = {
  set: (name: string, highlight: unknown) => void;
  delete: (name: string) => void;
};

type HighlightConstructorLike = new (...ranges: Range[]) => unknown;

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

  return !parent.closest('script, style, [aria-hidden="true"]');
}

function foldTextWithOriginalOffsets(text: string): {
  text: string;
  offsets: FoldedTextOffset[];
} {
  const foldedText = text.toLowerCase();
  const offsets: FoldedTextOffset[] = [];
  let originalOffset = 0;
  let foldedOffset = 0;

  for (const character of text) {
    const start = originalOffset;
    originalOffset += character.length;
    const foldedLength = character.toLowerCase().length;

    for (let index = 0; index < foldedLength; index += 1) {
      offsets[foldedOffset + index] = {
        start,
        end: originalOffset,
      };
    }
    foldedOffset += foldedLength;
  }

  return {
    text: foldedText,
    offsets,
  };
}

/**
 * Finds a case-insensitive query even when Markdown splits it across adjacent
 * text nodes (for example, around inline emphasis or code spans).
 */
export function findFlowChatSearchTextRange(root: HTMLElement, query: string): Range | null {
  const trimmedQuery = query.trim();
  if (!trimmedQuery) {
    return null;
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
  const foldedMatchStart = folded.text.indexOf(foldedQuery);
  if (foldedMatchStart < 0) {
    return null;
  }

  const foldedMatchEnd = foldedMatchStart + foldedQuery.length;
  const matchStart = folded.offsets[foldedMatchStart]?.start;
  const matchEnd = folded.offsets[foldedMatchEnd - 1]?.end;
  if (matchStart === undefined || matchEnd === undefined) {
    return null;
  }

  const startEntry = textNodes.find(entry => matchStart >= entry.start && matchStart < entry.end);
  const endEntry = textNodes.find(entry => matchEnd > entry.start && matchEnd <= entry.end);
  if (!startEntry || !endEntry) {
    return null;
  }

  const range = ownerDocument.createRange();
  range.setStart(startEntry.node, matchStart - startEntry.start);
  range.setEnd(endEntry.node, matchEnd - endEntry.start);
  return range;
}

export function setFlowChatSearchHighlight(range: Range | null): void {
  const cssWithHighlights = globalThis.CSS as (typeof CSS & {
    highlights?: HighlightRegistryLike;
  }) | undefined;
  const HighlightConstructor = (globalThis as typeof globalThis & {
    Highlight?: HighlightConstructorLike;
  }).Highlight;

  if (!cssWithHighlights?.highlights) {
    return;
  }

  cssWithHighlights.highlights.delete(SEARCH_HIGHLIGHT_NAME);
  if (range && HighlightConstructor) {
    cssWithHighlights.highlights.set(
      SEARCH_HIGHLIGHT_NAME,
      new HighlightConstructor(range),
    );
  }
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
): HTMLElement {
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
  }

  return wrapper.querySelector<HTMLElement>('.user-message-item__content') ?? wrapper;
}
