import { describe, expect, it } from 'vitest';
import { shouldFillPreviewViewport, takeNextMarkdownChunk } from './Preview';

function collectChunks(markdown: string): string[] {
  const chunks: string[] = [];
  let offset = 0;
  while (offset < markdown.length) {
    const chunk = takeNextMarkdownChunk(markdown, offset);
    chunks.push(chunk.content);
    expect(chunk.nextOffset).toBeGreaterThan(offset);
    offset = chunk.nextOffset;
  }
  return chunks;
}

describe('shouldFillPreviewViewport', () => {
  it('fills a maximized viewport until content becomes scrollable', () => {
    expect(shouldFillPreviewViewport(700, 1000)).toBe(true);
    expect(shouldFillPreviewViewport(1000, 1000)).toBe(true);
    expect(shouldFillPreviewViewport(1001, 1000)).toBe(false);
  });

  it('does not auto-load before the container has a measurable height', () => {
    expect(shouldFillPreviewViewport(0, 0)).toBe(false);
  });
});

describe('takeNextMarkdownChunk', () => {
  it('keeps small Markdown in one chunk', () => {
    const markdown = '# Title\n\nBody';
    expect(takeNextMarkdownChunk(markdown, 0)).toEqual({
      content: markdown,
      nextOffset: markdown.length,
    });
  });

  it('splits large Markdown at block boundaries without losing content', () => {
    const markdown = Array.from(
      { length: 4000 },
      (_, index) => `## Section ${index}\n\nParagraph ${index}.\n\n`,
    ).join('');
    const chunks = collectChunks(markdown);

    expect(chunks.length).toBeGreaterThan(1);
    expect(chunks.join('')).toBe(markdown);
    expect(chunks.every(chunk => chunk.length <= 64 * 1024)).toBe(true);
  });

  it('only materializes the first 64 KiB of a two MiB single line', () => {
    const markdown = 'x'.repeat(2 * 1024 * 1024);
    const firstChunk = takeNextMarkdownChunk(markdown, 0);

    expect(firstChunk.content.length).toBe(64 * 1024);
    expect(firstChunk.nextOffset).toBe(64 * 1024);
  });

  it('continues from the previous offset without losing a long line', () => {
    const markdown = 'x'.repeat(2 * 1024 * 1024);
    const chunks = collectChunks(markdown);

    expect(chunks.join('')).toBe(markdown);
    expect(Math.max(...chunks.map(chunk => chunk.length))).toBeLessThanOrEqual(64 * 1024);
  });

  it('prefers a completed block boundary after the target size', () => {
    const paragraph = `${'line\n'.repeat(7000)}\n`;
    const markdown = `${paragraph}Next paragraph.\n`;
    const firstChunk = takeNextMarkdownChunk(markdown, 0);

    expect(firstChunk.content).toBe(paragraph);
    expect(firstChunk.nextOffset).toBe(paragraph.length);
  });
});
