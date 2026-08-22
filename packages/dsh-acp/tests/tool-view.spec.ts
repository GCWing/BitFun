import { describe, expect, it } from 'vitest'

import { toolKind } from '../src/tool-view.ts'

describe('harness tool presentation', () => {
  it('renders the external harness lsp tool as a search card', () => {
    expect(toolKind('lsp')).toBe('search')
  })

  it('keeps unknown tools honest instead of guessing by substring', () => {
    expect(toolKind('custom_lsp_wrapper')).toBe('other')
  })
})
