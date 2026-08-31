import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { Preview } from './Preview';

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key === 'editor.meditor.frontmatter.label'
      ? 'YAML Frontmatter'
      : key,
  }),
}));

vi.mock('@/infrastructure/markdown', () => ({
  MarkdownRenderer: ({ content }: { content: string }) => (
    <div data-testid="markdown-renderer">{content}</div>
  ),
}));

describe('Preview', () => {
  it('shows frontmatter separately and renders only the markdown body', () => {
    const html = renderToStaticMarkup(
      <Preview value={'---\nname: Demo\n---\n\n# Body'} />,
    );

    expect(html).toContain('m-editor-preview-frontmatter');
    expect(html).toContain('YAML Frontmatter');
    expect(html).toContain('name: Demo');
    expect(html).not.toContain('---');
    expect(html).toContain('data-testid="markdown-renderer"');
    expect(html).toContain('# Body');
  });
});
