import type {
  LearningProposalSelection,
  LearningProposalSourceKind,
} from '@/infrastructure/api/service-api/LearningProposalAPI';

export interface ConversationSelectionSnapshot extends LearningProposalSelection {
  anchor: {
    left: number;
    top: number;
  };
}

const SOURCE_SELECTOR = '[data-learning-source-kind]';
const VIRTUAL_ITEM_SELECTOR = '.virtual-item-wrapper[data-turn-id]';

function nodeElement(node: Node | null): HTMLElement | null {
  if (!node) {
    return null;
  }
  if (node.nodeType === 1) {
    return node as HTMLElement;
  }
  return node.parentElement;
}

function isSourceKind(value: string | undefined): value is LearningProposalSourceKind {
  return value === 'user_message'
    || value === 'assistant_text'
    || value === 'assistant_thinking'
    || value === 'tool'
    || value === 'unknown';
}

function fallbackSourceKind(itemType: string | undefined): LearningProposalSourceKind {
  if (itemType === 'user-message') {
    return 'user_message';
  }
  return 'unknown';
}

function selectionAnchor(range: Range): ConversationSelectionSnapshot['anchor'] {
  const rangeWithRects = range as Range & {
    getBoundingClientRect?: () => DOMRect;
    getClientRects?: () => DOMRectList;
  };
  const rects = typeof rangeWithRects.getClientRects === 'function'
    ? Array.from(rangeWithRects.getClientRects())
    : [];
  const rect = rects[rects.length - 1]
    ?? (typeof rangeWithRects.getBoundingClientRect === 'function'
      ? rangeWithRects.getBoundingClientRect()
      : null);

  return {
    left: rect ? rect.left + rect.width / 2 : 0,
    top: rect ? rect.bottom + 8 : 0,
  };
}

export function resolveConversationSelection(
  selection: Selection | null,
  scope: HTMLElement | null,
): ConversationSelectionSnapshot | null {
  if (!selection || selection.isCollapsed || selection.rangeCount === 0 || !scope) {
    return null;
  }

  const selectedText = selection.toString().trim();
  if (!selectedText) {
    return null;
  }

  const range = selection.getRangeAt(0);
  const startElement = nodeElement(range.startContainer);
  const endElement = nodeElement(range.endContainer);
  if (!startElement || !endElement || !scope.contains(startElement) || !scope.contains(endElement)) {
    return null;
  }

  const startWrapper = startElement.closest<HTMLElement>(VIRTUAL_ITEM_SELECTOR);
  const endWrapper = endElement.closest<HTMLElement>(VIRTUAL_ITEM_SELECTOR);
  if (!startWrapper || startWrapper !== endWrapper) {
    return null;
  }
  if (
    startElement.closest<HTMLElement>('[data-round-id][data-streaming="true"]')
    || endElement.closest<HTMLElement>('[data-round-id][data-streaming="true"]')
  ) {
    return null;
  }

  const startSource = startElement.closest<HTMLElement>(SOURCE_SELECTOR);
  const endSource = endElement.closest<HTMLElement>(SOURCE_SELECTOR);
  if (startSource !== endSource) {
    return null;
  }
  const sourceElement = startSource && startWrapper.contains(startSource) ? startSource : startWrapper;

  const turnId = sourceElement.dataset.turnId || startWrapper.dataset.turnId;
  if (!turnId) {
    return null;
  }

  const roundElement = sourceElement.closest<HTMLElement>('[data-round-id]');
  const sourceKindValue = sourceElement.dataset.learningSourceKind;
  const itemId = sourceElement.dataset.learningItemId
    || sourceElement.dataset.flowItemId
    || startWrapper.dataset.learningItemId
    || startWrapper.dataset.flowItemId;
  const sourceKind = isSourceKind(sourceKindValue)
    ? sourceKindValue
    : fallbackSourceKind(startWrapper.dataset.itemType);
  if (sourceKind === 'unknown') {
    return null;
  }

  return {
    selectedText,
    turnId,
    roundId: sourceElement.dataset.roundId || roundElement?.dataset.roundId || undefined,
    itemId: itemId || undefined,
    sourceKind,
    anchor: selectionAnchor(range),
  };
}
