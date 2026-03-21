import React, { useCallback, useEffect, useImperativeHandle, forwardRef, useRef } from 'react'
import { createLogger } from '@/shared/utils/logger'
import { activeEditTargetService } from '@/tools/editor/services/ActiveEditTargetService'
import { useEditor } from '../hooks/useEditor'
import { EditArea } from './EditArea'
import { TiptapEditor, TiptapEditorHandle } from './TiptapEditor'
import { Preview } from './Preview'
import type { EditorOptions, EditorInstance } from '../types'
import { useI18n } from '@/infrastructure/i18n'
import './MEditor.scss'

void createLogger('MEditor')
let markdownTextareaTargetCounter = 0

export interface MEditorProps extends EditorOptions {}

function executeTextareaAction(
  textarea: HTMLTextAreaElement | null,
  action: 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'selectAll',
): boolean {
  if (!textarea || textarea.disabled) {
    return false
  }

  textarea.focus()

  if (textarea.readOnly && action !== 'copy' && action !== 'selectAll') {
    return false
  }

  if (action === 'selectAll') {
    textarea.select()
    return true
  }

  return document.execCommand(action)
}

export const MEditor = forwardRef<EditorInstance, MEditorProps>((props, ref) => {
  const {
    value: controlledValue,
    defaultValue = '',
    height = '500px',
    width = '100%',
    mode: initialMode = 'ir',
    theme: initialTheme = 'dark',
    toolbar = false,
    placeholder: placeholderProp,
    readonly = false,
    autofocus = false,
    onChange,
    onSave,
    onFocus,
    onBlur,
    onDirtyChange,
    className = '',
    style = {},
    filePath,
    basePath
  } = props

  const { t } = useI18n('tools')
  const placeholder = placeholderProp ?? t('editor.meditor.placeholder')
  const containerRef = useRef<HTMLDivElement>(null)
  const textareaTargetIdRef = useRef(`markdown-textarea-${++markdownTextareaTargetCounter}`)

  const {
    value,
    setValue,
    mode,
    setMode,
    theme,
    setTheme,
    textareaRef,
    editorInstance
  } = useEditor(controlledValue ?? defaultValue, onChange)

  const tiptapEditorRef = useRef<TiptapEditorHandle>(null)

  useEffect(() => {
    if (mode === 'ir' || mode === 'preview') {
      return
    }

    const targetId = textareaTargetIdRef.current

    return activeEditTargetService.bindTarget({
      id: targetId,
      kind: 'markdown-textarea',
      focus: () => {
        textareaRef.current?.focus()
      },
      hasTextFocus: () => {
        const textarea = textareaRef.current
        const activeElement = typeof document !== 'undefined' ? document.activeElement : null
        return !!textarea && activeElement === textarea
      },
      undo: () => executeTextareaAction(textareaRef.current, 'undo'),
      redo: () => executeTextareaAction(textareaRef.current, 'redo'),
      cut: () => executeTextareaAction(textareaRef.current, 'cut'),
      copy: () => executeTextareaAction(textareaRef.current, 'copy'),
      paste: () => executeTextareaAction(textareaRef.current, 'paste'),
      selectAll: () => executeTextareaAction(textareaRef.current, 'selectAll'),
      containsElement: (element) => {
        const root = containerRef.current
        return !!root && !!element && root.contains(element)
      }
    })
  }, [mode, textareaRef])

  useEffect(() => {
    if (controlledValue !== undefined && controlledValue !== value) {
      setValue(controlledValue)
    }
  }, [controlledValue, value, setValue])

  useEffect(() => {
    if (initialMode) {
      setMode(initialMode)
    }
  }, [initialMode, setMode])

  useEffect(() => {
    if (initialTheme) {
      setTheme(initialTheme)
    }
  }, [initialTheme, setTheme])

  useImperativeHandle(ref, () => ({
    ...editorInstance,
    scrollToLine: (line: number, highlight?: boolean) => {
      if (mode === 'ir' && tiptapEditorRef.current) {
        tiptapEditorRef.current.scrollToLine(line, highlight)
      }
    },
    undo: () => {
      if (mode === 'ir' && tiptapEditorRef.current) {
        return tiptapEditorRef.current.undo()
      }
      if (mode === 'edit' || mode === 'split') {
        return executeTextareaAction(textareaRef.current, 'undo')
      }
      return false
    },
    redo: () => {
      if (mode === 'ir' && tiptapEditorRef.current) {
        return tiptapEditorRef.current.redo()
      }
      if (mode === 'edit' || mode === 'split') {
        return executeTextareaAction(textareaRef.current, 'redo')
      }
      return false
    },
    get canUndo() {
      if (mode === 'ir' && tiptapEditorRef.current) {
        return tiptapEditorRef.current.canUndo
      }
      return false
    },
    get canRedo() {
      if (mode === 'ir' && tiptapEditorRef.current) {
        return tiptapEditorRef.current.canRedo
      }
      return false
    },
    markSaved: () => {
      if (mode === 'ir' && tiptapEditorRef.current) {
        tiptapEditorRef.current.markSaved()
      }
    },
    setInitialContent: (content: string) => {
      if (mode === 'ir' && tiptapEditorRef.current) {
        tiptapEditorRef.current.setInitialContent(content)
      }
    },
    get isDirty() {
      if (mode === 'ir' && tiptapEditorRef.current) {
        return tiptapEditorRef.current.isDirty
      }
      return false
    }
  }), [editorInstance, mode, textareaRef])

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLDivElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault()
      e.stopPropagation()  // Prevent event bubbling; avoids other listeners handling it.
      onSave?.(value)
    }
  }, [value, onSave])

  const handleFocusCapture = useCallback(() => {
    if (mode === 'ir' || mode === 'preview') {
      return
    }

    activeEditTargetService.setActiveTarget(textareaTargetIdRef.current)
  }, [mode])

  const handleBlurCapture = useCallback(() => {
    if (mode === 'ir' || mode === 'preview') {
      return
    }

    window.setTimeout(() => {
      const root = containerRef.current
      const activeElement = typeof document !== 'undefined' ? document.activeElement : null
      if (root && activeElement && root.contains(activeElement)) {
        return
      }

      activeEditTargetService.clearActiveTarget(textareaTargetIdRef.current)
    }, 0)
  }, [mode])

  const containerStyle: React.CSSProperties = {
    ...style,
    height: typeof height === 'number' ? `${height}px` : height,
    width: typeof width === 'number' ? `${width}px` : width
  }

  const themeClass = theme === 'dark' ? 'm-editor-dark' : 'm-editor-light'
  const modeClass = `m-editor-mode-${mode}`

  return (
    <div
      ref={containerRef}
      className={`m-editor ${themeClass} ${modeClass} ${className}`}
      style={containerStyle}
      onKeyDown={handleKeyDown}
      onFocusCapture={handleFocusCapture}
      onBlurCapture={handleBlurCapture}
      tabIndex={-1}
    >
      {toolbar && <div className="m-editor-toolbar">{t('editor.meditor.toolbarPlaceholder')}</div>}
      
      <div className="m-editor-content">
        {mode === 'preview' && (
          <Preview value={value} basePath={basePath} />
        )}

        {mode === 'edit' && (
          <div className="m-editor-edit-panel">
            <EditArea
              ref={textareaRef}
              value={value}
              onChange={setValue}
              onFocus={onFocus}
              onBlur={onBlur}
              placeholder={placeholder}
              readonly={readonly}
              autofocus={autofocus}
            />
          </div>
        )}

        {mode === 'split' && (
          <>
            <div className="m-editor-edit-panel">
              <EditArea
                ref={textareaRef}
                value={value}
                onChange={setValue}
                onFocus={onFocus}
                onBlur={onBlur}
                placeholder={placeholder}
                readonly={readonly}
                autofocus={autofocus}
              />
            </div>
            <div className="m-editor-preview-panel">
              <Preview value={value} basePath={basePath} />
            </div>
          </>
        )}

        {mode === 'ir' && (
          <div className="m-editor-ir-panel">
            <TiptapEditor
              ref={tiptapEditorRef}
              value={value}
              onChange={setValue}
              onFocus={onFocus}
              onBlur={onBlur}
              onDirtyChange={onDirtyChange}
              placeholder={placeholder}
              readonly={readonly}
              autofocus={autofocus}
              filePath={filePath}
              basePath={basePath}
            />
          </div>
        )}
      </div>
    </div>
  )
})

MEditor.displayName = 'MEditor'
