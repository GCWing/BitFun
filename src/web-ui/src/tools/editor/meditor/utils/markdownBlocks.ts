export interface MarkdownLineBlock {
  startLine: number;
  endLine: number;
}

export function parseMarkdownLineBlocks(content: string): MarkdownLineBlock[] {
  const lines = content.split('\n');
  const blocks: MarkdownLineBlock[] = [];
  let blockStart: number | null = null;
  let inCodeBlock = false;

  lines.forEach((line, index) => {
    if (line.trim().startsWith('```')) {
      if (!inCodeBlock) {
        inCodeBlock = true;
        blockStart = index;
      } else {
        blocks.push({
          startLine: blockStart ?? index,
          endLine: index,
        });
        inCodeBlock = false;
        blockStart = null;
      }
      return;
    }

    if (inCodeBlock) {
      return;
    }

    if (line.trim() === '') {
      if (blockStart !== null) {
        blocks.push({
          startLine: blockStart,
          endLine: index - 1,
        });
        blockStart = null;
      }
      return;
    }

    if (blockStart === null) {
      blockStart = index;
    }
  });

  if (blockStart !== null) {
    blocks.push({
      startLine: blockStart,
      endLine: lines.length - 1,
    });
  }

  return blocks;
}

export function getBlockIndexForLine(content: string, line: number): number {
  const blocks = parseMarkdownLineBlocks(content);

  if (blocks.length === 0) {
    return -1;
  }

  const matchIndex = blocks.findIndex(
    block => line >= block.startLine + 1 && line <= block.endLine + 1,
  );

  if (matchIndex >= 0) {
    return matchIndex;
  }

  let closestIndex = 0;
  let closestDistance = Number.POSITIVE_INFINITY;

  blocks.forEach((block, index) => {
    const distance = Math.min(
      Math.abs(block.startLine + 1 - line),
      Math.abs(block.endLine + 1 - line),
    );

    if (distance < closestDistance) {
      closestDistance = distance;
      closestIndex = index;
    }
  });

  return closestIndex;
}

