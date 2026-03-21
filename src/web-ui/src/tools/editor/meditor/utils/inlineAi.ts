type InlineAiPromptParams = {
  userInput: string;
  markdown: string;
  blockIndex: number;
  filePath?: string;
};

const MAX_CONTEXT_CHARS = 12000;
const SURROUNDING_BLOCK_WINDOW = 2;
const MAX_FOCUSED_CONTEXT_CHARS = 8000;
const MAX_BLOCK_SNIPPET_CHARS = 1600;
const MAX_STRUCTURE_SUMMARY_CHARS = 2400;
const MAX_RANGE_SUMMARY_ENTRIES = 6;

function formatDocumentContext(markdown: string, blockIndex: number): string {
  const content = buildPromptDocumentContext(markdown, blockIndex).trim();
  if (!content) {
    return '(Document is currently empty)';
  }

  return content;
}

function parseTopLevelBlocks(markdown: string): string[] {
  const normalized = markdown.replace(/\r\n/g, '\n').trim();
  if (!normalized) {
    return [];
  }

  return normalized
    .split(/\n{2,}/)
    .map(block => block.trim())
    .filter(Boolean);
}

function getInsertionAnchorIndex(blocks: string[], blockIndex: number): number {
  if (blocks.length === 0) {
    return 0;
  }

  return Math.max(0, Math.min(blockIndex, blocks.length));
}

function summarizeBlock(block: string, maxChars = 120): string {
  const lines = block
    .split('\n')
    .map(line => line.trim())
    .filter(Boolean);
  const preferredLine = lines.find(line => /^#{1,6}\s+/.test(line)) ?? lines[0] ?? '';
  const normalized = preferredLine.replace(/\s+/g, ' ').trim();

  if (normalized.length <= maxChars) {
    return normalized;
  }

  return `${normalized.slice(0, maxChars - 1)}...`;
}

function clipBlockForPrompt(block: string, relation: 'before' | 'after'): string {
  const normalized = block.trim();
  if (normalized.length <= MAX_BLOCK_SNIPPET_CHARS) {
    return normalized;
  }

  if (relation === 'before') {
    return [
      `...[earlier lines omitted, kept last ${MAX_BLOCK_SNIPPET_CHARS} chars of block]...`,
      normalized.slice(-MAX_BLOCK_SNIPPET_CHARS),
    ].join('\n');
  }

  return [
    normalized.slice(0, MAX_BLOCK_SNIPPET_CHARS),
    `...[later lines omitted, kept first ${MAX_BLOCK_SNIPPET_CHARS} chars of block]...`,
  ].join('\n');
}

function pickRepresentativeIndices(indices: number[]): number[] {
  if (indices.length <= MAX_RANGE_SUMMARY_ENTRIES) {
    return indices;
  }

  const picked = new Set<number>();
  const firstCount = 2;
  const lastCount = 2;
  const middleCount = MAX_RANGE_SUMMARY_ENTRIES - firstCount - lastCount;

  indices.slice(0, firstCount).forEach(index => {
    picked.add(index);
  });

  if (middleCount > 0) {
    const step = (indices.length - 1) / (middleCount + 1);
    for (let i = 1; i <= middleCount; i += 1) {
      picked.add(indices[Math.round(step * i)]);
    }
  }

  indices.slice(-lastCount).forEach(index => {
    picked.add(index);
  });

  return Array.from(picked).sort((a, b) => a - b);
}

function buildOmittedRangeSummary(
  blocks: string[],
  indices: number[],
  label: string,
): string {
  if (indices.length === 0) {
    return `${label}: none`;
  }

  const representativeLines = pickRepresentativeIndices(indices)
    .map(index => `- [#${index + 1}] ${summarizeBlock(blocks[index])}`);

  return [
    `${label}: ${indices.length} blocks omitted`,
    ...representativeLines,
  ].join('\n');
}

function buildPromptDocumentContext(markdown: string, blockIndex: number): string {
  const normalized = markdown.trim();
  if (!normalized) {
    return '';
  }

  if (normalized.length <= MAX_CONTEXT_CHARS) {
    return normalized;
  }

  const blocks = parseTopLevelBlocks(markdown);
  if (blocks.length === 0) {
    return normalized.slice(0, MAX_CONTEXT_CHARS);
  }

  const anchorIndex = getInsertionAnchorIndex(blocks, blockIndex);
  const selectedIndices = new Set<number>();
  let usedChars = 0;

  const trySelect = (index: number): boolean => {
    if (selectedIndices.has(index) || index < 0 || index >= blocks.length) {
      return false;
    }

    const relation = index < anchorIndex ? 'before' : 'after';
    const snippet = clipBlockForPrompt(blocks[index], relation);
    const estimatedLength = snippet.length + 32;

    if (usedChars + estimatedLength > MAX_FOCUSED_CONTEXT_CHARS && selectedIndices.size > 0) {
      return false;
    }

    selectedIndices.add(index);
    usedChars += estimatedLength;
    return true;
  };

  for (let distance = 0; distance < blocks.length && usedChars < MAX_FOCUSED_CONTEXT_CHARS; distance += 1) {
    const leftIndex = anchorIndex - 1 - distance;
    const rightIndex = anchorIndex + distance;

    let progressed = false;

    if (leftIndex >= 0) {
      progressed = trySelect(leftIndex) || progressed;
    }

    if (rightIndex < blocks.length) {
      progressed = trySelect(rightIndex) || progressed;
    }

    if (!progressed && leftIndex < 0 && rightIndex >= blocks.length) {
      break;
    }
  }

  if (selectedIndices.size === 0) {
    trySelect(Math.max(0, Math.min(anchorIndex, blocks.length - 1)));
  }

  const orderedSelectedIndices = Array.from(selectedIndices).sort((a, b) => a - b);
  const focusedBlocks = orderedSelectedIndices.map(index => {
    const relation = index < anchorIndex ? 'before' : 'after';
    return `[[block_${index + 1}_${relation}]]\n${clipBlockForPrompt(blocks[index], relation)}`;
  });

  const omittedBefore = Array.from({ length: Math.max(orderedSelectedIndices[0] ?? 0, 0) }, (_, index) => index)
    .filter(index => !selectedIndices.has(index));
  const omittedAfter = Array.from(
    { length: Math.max(0, blocks.length - ((orderedSelectedIndices[orderedSelectedIndices.length - 1] ?? -1) + 1)) },
    (_, index) => ((orderedSelectedIndices[orderedSelectedIndices.length - 1] ?? -1) + 1 + index),
  ).filter(index => !selectedIndices.has(index));

  const structureSummary = [
    buildOmittedRangeSummary(blocks, omittedBefore, 'Earlier omitted context'),
    buildOmittedRangeSummary(blocks, omittedAfter, 'Later omitted context'),
  ].join('\n\n');

  const boundedStructureSummary = structureSummary.length > MAX_STRUCTURE_SUMMARY_CHARS
    ? `${structureSummary.slice(0, MAX_STRUCTURE_SUMMARY_CHARS - 1)}...`
    : structureSummary;

  return [
    `Document is long (${normalized.length} chars), so the context below prioritizes blocks nearest the insertion point.`,
    '',
    'Focused document context around the insertion point:',
    ...focusedBlocks,
    '',
    'Document structure outside the focused context:',
    boundedStructureSummary,
  ].join('\n');
}

function formatInsertionContext(markdown: string, blockIndex: number): string {
  const blocks = parseTopLevelBlocks(markdown);
  if (blocks.length === 0) {
    return '(No surrounding blocks yet)';
  }

  const anchorIndex = getInsertionAnchorIndex(blocks, blockIndex);
  const previousBlocks = blocks.slice(
    Math.max(0, anchorIndex - SURROUNDING_BLOCK_WINDOW),
    anchorIndex,
  );
  const nextBlocks = blocks.slice(anchorIndex, anchorIndex + SURROUNDING_BLOCK_WINDOW);

  return [
    'Blocks immediately before the insertion point:',
    previousBlocks.length > 0
      ? previousBlocks.map((block, index) => `[[before_${index + 1}]]\n${block}`).join('\n\n')
      : '(No previous blocks)',
    '',
    'Blocks immediately after the insertion point:',
    nextBlocks.length > 0
      ? nextBlocks.map((block, index) => `[[after_${index + 1}]]\n${block}`).join('\n\n')
      : '(Insertion point is at the end of the document)',
  ].join('\n');
}

export function buildInlineAskAiPrompt(params: InlineAiPromptParams): string {
  const { userInput, markdown, blockIndex, filePath } = params;
  const locationLine = filePath
    ? `Current file path: ${filePath}`
    : 'Current file path: (not available)';

  return [
    'You are helping with an in-editor Markdown document.',
    'The document may contain unsaved local edits, so treat the content below as the source of truth.',
    'You may answer the user, suggest edits, or decide which workspace actions to take.',
    'If you reference edits to the current document, remember that the unsaved buffer may differ from the file on disk.',
    '',
    locationLine,
    `Cursor is on an empty paragraph after top-level block #${blockIndex + 1}.`,
    '',
    formatInsertionContext(markdown, blockIndex),
    '',
    'Current markdown document or focused document context:',
    '```md',
    formatDocumentContext(markdown, blockIndex),
    '```',
    '',
    'User request:',
    userInput.trim(),
  ].join('\n');
}

export function buildInlineContinuePrompt(params: InlineAiPromptParams): string {
  const { userInput, markdown, blockIndex, filePath } = params;
  const instruction = userInput.trim()
    ? `User direction for the continuation: ${userInput.trim()}`
    : 'User direction for the continuation: continue naturally from the current context.';
  const locationLine = filePath
    ? `Current file path: ${filePath}`
    : 'Current file path: (not available)';

  return [
    'You are completing an in-editor Markdown document at a specific insertion point.',
    'Return only the Markdown content that should be inserted there.',
    'Do not add explanations, analysis, XML tags, or wrapper text.',
    'Keep the writing consistent with the existing language, tone, structure, heading depth, and list style.',
    'Do not repeat surrounding content that is already in the document.',
    'If there are later blocks after the insertion point, make the continuation flow into them naturally.',
    'Prefer a concise continuation unless the user explicitly asks for something longer.',
    '',
    locationLine,
    `Insertion point: empty paragraph after top-level block #${blockIndex + 1}.`,
    instruction,
    '',
    formatInsertionContext(markdown, blockIndex),
    '',
    'Current markdown document or focused document context:',
    '```md',
    formatDocumentContext(markdown, blockIndex),
    '```',
  ].join('\n');
}

export function sanitizeInlineAiMarkdownResponse(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return '';
  }

  const fencedMatch = trimmed.match(/^```(?:markdown|md)?\s*([\s\S]*?)\s*```$/i);
  if (fencedMatch) {
    return fencedMatch[1].trim();
  }

  return trimmed;
}
