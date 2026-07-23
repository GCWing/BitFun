import { describe, expect, it } from 'vitest';
import {
  buildInlineContinuePrompt,
  buildInlineSummaryPrompt,
} from './inlineAi';

function createBlock(index: number, repeat = 220): string {
  const heading = `## Section ${String(index).padStart(2, '0')}`;
  const body = Array.from(
    { length: repeat },
    () => `BODY-${String(index).padStart(2, '0')}`
  ).join(' ');

  return `${heading}\n${body}`;
}

describe('inlineAi prompt shaping', () => {
  it('uses the insertion point as the split between previous and next blocks', () => {
    const markdown = [
      '# Intro\nAAA',
      '## Middle\nBBB',
      '## Tail\nCCC',
    ].join('\n\n');

    const prompt = buildInlineContinuePrompt({
      userInput: '',
      markdown,
      blockIndex: 1,
      filePath: '/tmp/doc.md',
    });

    expect(prompt).toContain('[[before_block_1]]\n# Intro\nAAA');
    expect(prompt).toContain('[[after_block_2]]\n## Middle\nBBB');
    expect(prompt).toContain('[[after_block_3]]\n## Tail\nCCC');
    expect(prompt).not.toContain('[[before_block_2]]\n## Middle\nBBB');
  });

  it('prioritizes blocks near the insertion point when the document is long', () => {
    const markdown = Array.from({ length: 24 }, (_, index) => createBlock(index + 1)).join('\n\n');

    const prompt = buildInlineContinuePrompt({
      userInput: '',
      markdown,
      blockIndex: 12,
      filePath: '/tmp/long-doc.md',
    });

    expect(prompt).toContain('Focused document context around the insertion point:');
    expect(prompt).toContain('Document structure outside the focused context:');
    expect(prompt).toContain('BODY-12');
    expect(prompt).toContain('BODY-13');
    expect(prompt).not.toContain(`${'BODY-01 '.repeat(20).trim()}`);
    expect(prompt).not.toContain(`${'BODY-24 '.repeat(20).trim()}`);
    expect(prompt).toContain('- [#1] ## Section 01');
    expect(prompt).toContain('- [#24] ## Section 24');
  });

  it('uses summary-specific instructions without the continuation anti-summary rule', () => {
    const markdown = [
      '# Intro\nAAA',
      '## Middle\nBBB',
    ].join('\n\n');

    const prompt = buildInlineSummaryPrompt({
      userInput: '在当前位置插入一段简短摘要，概括上文的关键内容，并保持当前文档语言和风格。',
      markdown,
      blockIndex: 1,
      filePath: '/tmp/doc.md',
    });

    expect(prompt).toContain('You are inserting a short summary into an in-editor Markdown document at one exact insertion point.');
    expect(prompt).toContain('Summarize the key points from the content before the insertion point in a compact way.');
    expect(prompt).not.toContain('Do not rewrite, summarize, paraphrase, or restate any existing block in the document.');
    expect(prompt).toContain('User direction for the summary: 在当前位置插入一段简短摘要');
  });
});
