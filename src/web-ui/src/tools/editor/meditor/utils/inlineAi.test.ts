import { describe, expect, it } from 'vitest';
import { buildInlineContinuePrompt } from './inlineAi';

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

    expect(prompt).toContain('[[before_1]]\n# Intro\nAAA');
    expect(prompt).toContain('[[after_1]]\n## Middle\nBBB');
    expect(prompt).toContain('[[after_2]]\n## Tail\nCCC');
    expect(prompt).not.toContain('[[before_2]]\n## Middle\nBBB');
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
});
