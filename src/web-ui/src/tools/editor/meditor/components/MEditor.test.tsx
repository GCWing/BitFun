// @vitest-environment jsdom

import React, { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mocks = vi.hoisted(() => ({
  logError: vi.fn(),
}))

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: mocks.logError,
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}))

import { MEditor, MEditorErrorBoundary } from './MEditor'

function UnsupportedMarkdownRenderer(): never {
  throw new SyntaxError('Invalid regular expression: invalid group specifier name')
}

describe('MEditorErrorBoundary', () => {
  let container: HTMLDivElement
  let root: Root
  let consoleError: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
    consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    mocks.logError.mockReset()
  })

  afterEach(() => {
    act(() => root.unmount())
    container.remove()
    consoleError.mockRestore()
  })

  it('keeps the original markdown visible when the rich editor cannot render', () => {
    act(() => {
      root.render(
        <MEditorErrorBoundary
          editorProps={{ value: '# Recovery plan', readonly: true }}
          forwardedRef={null}
        >
          <UnsupportedMarkdownRenderer />
        </MEditorErrorBoundary>
      )
    })

    const fallback = container.querySelector<HTMLTextAreaElement>(
      '[data-openbitfun-component="m-editor"][data-m-editor-fallback="true"] textarea'
    )
    expect(fallback?.value).toBe('# Recovery plan')
    expect(fallback?.readOnly).toBe(true)
    expect(mocks.logError).toHaveBeenCalledWith(
      'Markdown editor render failed, showing source fallback',
      expect.objectContaining({
        message: 'Invalid regular expression: invalid group specifier name',
      })
    )
  })

  it('shows the compatibility warning only when IR mode requests a fallback', () => {
    act(() => {
      root.render(
        <MEditor value={'Mix <span data-x="1">inline</span> HTML.'} mode="preview" />
      )
    })
    expect(container.textContent).not.toContain('IR fallback warning')

    act(() => {
      root.render(
        <MEditor value={'Mix <span data-x="1">inline</span> HTML.'} mode="ir" />
      )
    })
    expect(container.textContent).toContain('IR fallback warning')
  })
})

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
  useI18n: () => ({
    t: (key: string) => key === 'editor.markdownEditor.notice.sourcePreviewFallback'
      ? 'IR fallback warning'
      : key,
  }),
}))
