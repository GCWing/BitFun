import { describe, expect, it } from 'vitest';
import { embeddedSource } from './embeddedSource';

describe('embedded source payloads', () => {
  it.each([
    ['```mermaid\ngraph TD\n  A-->B\n```', 'code', 'graph TD\n  A-->B'],
    ['~~~js title="sample"\r\nconst x = 1;\r\n~~~~', 'code', 'const x = 1;'],
    ['$$\nx^2\n$$', 'math', 'x^2'],
    ['$x^2$', 'inlineMath', 'x^2'],
    ['<div>HTML</div>', 'html', '<div>HTML</div>'],
  ])('retains wrappers and metadata: %s', (markdown, kind, source) => {
    const payload = embeddedSource(markdown, kind);
    expect(payload.source).toBe(source);
    expect(payload.wrap(source)).toBe(markdown);
  });

  it('lengthens a code fence when the edited content contains a closing fence', () => {
    const payload = embeddedSource('```md\nold\n```', 'code');
    expect(payload.wrap('```\nnew')).toBe('````md\n```\nnew\n````');
  });
});
