import type { TiptapTopLevelMarkdownBlock } from './tiptapMarkdown';

type InlineAiPromptParams = {
  userInput: string;
  markdown: string;
  blockIndex: number;
  filePath?: string;
  topLevelBlocks?: TiptapTopLevelMarkdownBlock[];
};

type InlineAiPromptKind = 'continue' | 'summary' | 'todo';

const MAX_CONTEXT_CHARS = 12000;
const SURROUNDING_BLOCK_WINDOW = 2;
const MAX_FOCUSED_CONTEXT_CHARS = 8000;
const MAX_BLOCK_SNIPPET_CHARS = 1600;
const MAX_STRUCTURE_SUMMARY_CHARS = 2400;
const MAX_RANGE_SUMMARY_ENTRIES = 6;

function formatDocumentContext(markdown: string, blocks: string[], blockIndex: number): string {
  const normalized = markdown.trim();
  if (!normalized) {
    return '(Document is currently empty)';
  }

  if (normalized.length <= MAX_CONTEXT_CHARS) {
    return '(Omitted because the full document is short and already shown below.)';
  }

  const content = buildPromptDocumentContext(markdown, blocks, blockIndex).trim();
  if (!content) {
    return '(Document is currently empty)';
  }

  return content;
}

function getPromptBlocks(markdown: string, topLevelBlocks?: TiptapTopLevelMarkdownBlock[]): string[] {
  const normalizedMarkdown = markdown.replace(/\r\n/g, '\n').trim();
  const normalizedTopLevelBlocks = (topLevelBlocks ?? [])
    .map(block => block.markdown.replace(/\r\n/g, '\n').trim())
    .filter(Boolean);

  if (normalizedTopLevelBlocks.length > 0) {
    return normalizedTopLevelBlocks;
  }

  if (!normalizedMarkdown) {
    return [];
  }

  return normalizedMarkdown
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

function buildPromptDocumentContext(markdown: string, blocks: string[], blockIndex: number): string {
  const normalized = markdown.trim();
  if (!normalized) {
    return '';
  }

  if (normalized.length <= MAX_CONTEXT_CHARS) {
    return normalized;
  }

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

function formatInsertionContext(blocks: string[], blockIndex: number): string {
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
    `Insertion anchor: after top-level block #${blockIndex} and before top-level block #${blockIndex + 1}.`,
    `Resolved anchor index in current prompt blocks: ${anchorIndex}.`,
    '',
    'Blocks immediately before the insertion point:',
    previousBlocks.length > 0
      ? previousBlocks.map((block, index) => {
          const actualIndex = anchorIndex - previousBlocks.length + index;
          return `[[before_block_${actualIndex + 1}]]\n${block}`;
        }).join('\n\n')
      : '(No previous blocks)',
    '',
    'Blocks immediately after the insertion point:',
    nextBlocks.length > 0
      ? nextBlocks.map((block, index) => `[[after_block_${anchorIndex + index + 1}]]\n${block}`).join('\n\n')
      : '(Insertion point is at the end of the document)',
  ].join('\n');
}

function formatFullDocument(markdown: string): string {
  const normalized = markdown.trim();
  return normalized || '(Document is currently empty)';
}

function buildPromptDocumentSection(markdown: string, blocks: string[], blockIndex: number): string[] {
  const normalizedMarkdown = markdown.trim();
  const shouldIncludeFullDocument = normalizedMarkdown.length > 0 && normalizedMarkdown.length <= MAX_CONTEXT_CHARS;

  return [
    shouldIncludeFullDocument ? 'Current markdown document:' : 'Focused document context beyond the immediate neighbors:',
    '```md',
    shouldIncludeFullDocument ? formatFullDocument(markdown) : formatDocumentContext(markdown, blocks, blockIndex),
    '```',
  ];
}

function buildInlineInsertPrompt(
  params: InlineAiPromptParams,
  kind: InlineAiPromptKind,
): string {
  const { userInput, markdown, blockIndex, filePath, topLevelBlocks } = params;
  const blocks = getPromptBlocks(markdown, topLevelBlocks);
  const locationLine = filePath
    ? `Current file path: ${filePath}`
    : 'Current file path: (not available)';
  const trimmedUserInput = userInput.trim();

  const promptByKind: Record<InlineAiPromptKind, string[]> = {
    continue: [
      'You are continuing an in-editor Markdown document at one exact insertion point.',
      'This is continuation only, not rewriting.',
      'Return only the new Markdown content that should be inserted at that location.',
      'Do not add explanations, analysis, XML tags, or wrapper text.',
      'Do not rewrite, summarize, paraphrase, or restate any existing block in the document.',
      'Do not copy sentences, headings, list items, or bullet points from the surrounding blocks.',
      'Keep the writing consistent with the existing language, tone, structure, heading depth, and list style.',
      'The user is actively editing at the insertion anchor described below, so generate content for that exact location only.',
      'If there are later blocks after the insertion point, make the continuation lead into them naturally without reusing their text.',
      'If no new content is needed, return an empty string.',
      'Prefer a concise continuation unless the user explicitly asks for something longer.',
      '',
      locationLine,
      `Insertion point: the empty paragraph after top-level block #${blockIndex}.`,
      'The generated content will replace that empty paragraph only.',
      'Do not modify or regenerate the blocks before or after the insertion point.',
      trimmedUserInput
        ? `User direction for the continuation: ${trimmedUserInput}`
        : 'User direction for the continuation: continue naturally from the current context.',
    ],
    summary: [
      'You are inserting a short summary into an in-editor Markdown document at one exact insertion point.',
      'Return only the new Markdown content that should be inserted at that location.',
      'Do not add explanations, analysis, XML tags, or wrapper text.',
      'Summarize the key points from the content before the insertion point in a compact way.',
      'Do not copy sentences or headings verbatim unless a short phrase is necessary.',
      'Do not modify or regenerate the surrounding blocks.',
      'If there are later blocks after the insertion point, make sure the summary does not conflict with them.',
      'Keep the writing consistent with the existing language, tone, structure, heading depth, and list style.',
      'Prefer a single short paragraph or a very short bullet list unless the user asks for a different shape.',
      '',
      locationLine,
      `Insertion point: the empty paragraph after top-level block #${blockIndex}.`,
      'The generated content will replace that empty paragraph only.',
      trimmedUserInput
        ? `User direction for the summary: ${trimmedUserInput}`
        : 'User direction for the summary: insert a short summary at the current position.',
    ],
    todo: [
      'You are inserting a concise Markdown todo list into an in-editor Markdown document at one exact insertion point.',
      'Return only the new Markdown content that should be inserted at that location.',
      'Do not add explanations, analysis, XML tags, or wrapper text.',
      'Extract the next concrete action items implied by the content before the insertion point.',
      'Prefer Markdown task list items (`- [ ]`) unless the user explicitly asks for another format.',
      'Do not copy sentences or bullet points verbatim from the surrounding blocks.',
      'Do not modify or regenerate the surrounding blocks.',
      'If there are later blocks after the insertion point, make sure the todo list does not conflict with them.',
      'Keep the writing consistent with the existing language and style.',
      'Prefer a short list focused on the most actionable next steps.',
      '',
      locationLine,
      `Insertion point: the empty paragraph after top-level block #${blockIndex}.`,
      'The generated content will replace that empty paragraph only.',
      trimmedUserInput
        ? `User direction for the todo list: ${trimmedUserInput}`
        : 'User direction for the todo list: insert a concise Markdown todo list at the current position.',
    ],
  };

  return [
    ...promptByKind[kind],
    '',
    formatInsertionContext(blocks, blockIndex),
    '',
    ...buildPromptDocumentSection(markdown, blocks, blockIndex),
  ].join('\n');
}

export function buildInlineContinuePrompt(params: InlineAiPromptParams): string {
  return buildInlineInsertPrompt(params, 'continue');
}

export function buildInlineSummaryPrompt(params: InlineAiPromptParams): string {
  return buildInlineInsertPrompt(params, 'summary');
}

export function buildInlineTodoPrompt(params: InlineAiPromptParams): string {
  return buildInlineInsertPrompt(params, 'todo');
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
