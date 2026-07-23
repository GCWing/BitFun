import { useRef, useCallback, useState } from 'react'
import type { EditorInstance, EditorMode, EditorTheme } from '../types'

/**
 * Core editor hook.
 */
export function useEditor(
  initialValue: string = '',
  onChange?: (value: string) => void
) {
  const [value, setValue] = useState(initialValue)
  const [mode, setMode] = useState<EditorMode>('ir')
  const [theme, setTheme] = useState<EditorTheme>('dark')
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const handleChange = useCallback((newValue: string) => {
    setValue(newValue)
    onChange?.(newValue)
  }, [onChange])

  const getValue = useCallback(() => {
    return value
  }, [value])

  const handleSetValue = useCallback((newValue: string) => {
    setValue(newValue)
    if (textareaRef.current) {
      textareaRef.current.value = newValue
    }
  }, [])

  const insertValue = useCallback((
    text: string,
    start?: number,
    end?: number
  ) => {
    const textarea = textareaRef.current
    if (!textarea) return

    const selStart = start ?? textarea.selectionStart
    const selEnd = end ?? textarea.selectionEnd
    const before = value.substring(0, selStart)
    const after = value.substring(selEnd)
    const newValue = before + text + after

    handleChange(newValue)

    setTimeout(() => {
      textarea.focus()
      const newCursorPos = selStart + text.length
      textarea.setSelectionRange(newCursorPos, newCursorPos)
    }, 0)
  }, [value, handleChange])

  const focus = useCallback(() => {
    textareaRef.current?.focus()
  }, [])

  const blur = useCallback(() => {
    textareaRef.current?.blur()
  }, [])

  const getSelection = useCallback(() => {
    const textarea = textareaRef.current
    if (!textarea) {
      return { start: 0, end: 0, text: '' }
    }

    return {
      start: textarea.selectionStart,
      end: textarea.selectionEnd,
      text: value.substring(textarea.selectionStart, textarea.selectionEnd)
    }
  }, [value])

  const destroy = useCallback(() => {
  }, [])

  const editorInstance: EditorInstance = {
    getValue,
    setValue: handleSetValue,
    insertValue,
    focus,
    blur,
    setMode,
    setTheme,
    getSelection,
    destroy
  }

  return {
    value,
    setValue: handleChange,
    mode,
    setMode,
    theme,
    setTheme,
    textareaRef,
    editorInstance
  }
}

