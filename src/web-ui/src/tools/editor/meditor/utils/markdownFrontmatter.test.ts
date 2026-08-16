import { describe, expect, it } from 'vitest';
import { splitMarkdownFrontmatter } from './markdownFrontmatter';

describe('splitMarkdownFrontmatter', () => {
  it('keeps the complete frontmatter envelope and body boundary', () => {
    const markdown = '---\nname: Demo\n---\n\n# Body';

    expect(splitMarkdownFrontmatter(markdown)).toEqual({
      raw: '---\nname: Demo\n---\n',
      body: '\n# Body',
      delimiter: '---',
      yaml: 'name: Demo',
      openingLineEnding: '\n',
      contentLineEnding: '\n',
      closingLineEnding: '\n',
    });
  });

  it('does not treat a thematic break in the body as frontmatter', () => {
    expect(splitMarkdownFrontmatter('---\n\n# Body')).toBeNull();
  });

  it('supports an empty frontmatter envelope', () => {
    expect(splitMarkdownFrontmatter('---\n---\n# Body')).toEqual({
      raw: '---\n---\n',
      body: '# Body',
      delimiter: '---',
      yaml: '',
      openingLineEnding: '\n',
      contentLineEnding: '',
      closingLineEnding: '\n',
    });
  });

  it('keeps every line ending after the closing delimiter in the body', () => {
    expect(splitMarkdownFrontmatter('---\r\nname: Demo\r\n---\r\n\r\n\r\n# Body')).toMatchObject({
      raw: '---\r\nname: Demo\r\n---\r\n',
      body: '\r\n\r\n# Body',
      yaml: 'name: Demo',
      openingLineEnding: '\r\n',
      contentLineEnding: '\r\n',
      closingLineEnding: '\r\n',
    });
  });
});
