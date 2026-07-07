import { describe, expect, it } from 'vitest';

import { GENERATIVE_WIDGET_SHELL_HTML } from './GenerativeWidgetFrame';

describe('GenerativeWidgetFrame shell', () => {
  it('keeps iframe-local small text aligned with the host default token', () => {
    const values = [...GENERATIVE_WIDGET_SHELL_HTML.matchAll(/--font-size-sm:\s*([^;]+);/g)].map(
      (match) => match[1]?.trim()
    );

    expect(values).toEqual(['13px']);
  });
});
