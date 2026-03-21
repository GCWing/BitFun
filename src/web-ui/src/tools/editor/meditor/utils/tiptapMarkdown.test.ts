import { describe, expect, it } from 'vitest';
import {
  canRoundTripMarkdownWithTiptap,
  getUnsupportedTiptapMarkdownFeatures,
  markdownToTiptapDoc,
  tiptapDocToMarkdown,
} from './tiptapMarkdown';

describe('tiptap markdown compatibility', () => {
  it('supports gfm tables without falling back', () => {
    const markdown = [
      '| name | value |',
      '| --- | --- |',
      '| foo | bar |',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
  });

  it('supports escaped markdown literals without losing semantics', () => {
    const markdown = String.raw`Show literal \*asterisks\* here.`;

    const doc = markdownToTiptapDoc(markdown);

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
  });

  it('preserves a preferred trailing newline during serialization', () => {
    const markdown = '# Title\n\nBody\n';
    const doc = markdownToTiptapDoc(markdown);

    expect(tiptapDocToMarkdown(doc, { preserveTrailingNewline: true })).toBe(markdown);
    expect(tiptapDocToMarkdown(doc)).toBe('# Title\n\nBody');
  });

  it('allows simple markdown documents to use the tiptap engine', () => {
    const markdown = [
      '# Title',
      '',
      '- item one',
      '- item two',
      '',
      'Regular paragraph.',
    ].join('\n');

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
  });

  it('supports deep heading levels', () => {
    const markdown = '#### Deep heading';

    const doc = markdownToTiptapDoc(markdown);

    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
    expect(tiptapDocToMarkdown(doc)).toBe(markdown);
  });

  it('supports aligned html wrapper sections used by the project README header', () => {
    const markdown = [
      '<div align="center">',
      '',
      '![BitFun](./png/BitFun_title.png)',
      '',
      '**AI assistant with personality and memory**',
      '',
      'Hand over the work, keep the life',
      '',
      '</div>',
      '<div align="center">',
      '',
      '[![Website](https://img.shields.io/badge/Website-openbitfun.com-6f42c1?style=flat-square)](https://openbitfun.com/)',
      '',
      '</div>',
    ].join('\n');

    const doc = markdownToTiptapDoc(markdown);
    const serialized = tiptapDocToMarkdown(doc);

    expect(serialized).toBe(markdown);
    expect(canRoundTripMarkdownWithTiptap(markdown)).toBe(true);
    expect(getUnsupportedTiptapMarkdownFeatures(markdown)).toEqual([]);
  });
});
