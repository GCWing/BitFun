export { MEditor } from './components/MEditor'
export type { MEditorProps } from './components/MEditor'

export { EditArea } from './components/EditArea'
export { Preview } from './components/Preview'
export { TiptapEditor } from './components/TiptapEditor'
export type { TiptapEditorHandle } from './components/TiptapEditor'

// Hooks
export { useEditor } from './hooks/useEditor'
export { useMarkdown } from './hooks/useMarkdown'
export { useEditorHistory } from './hooks/useEditorHistory'
export type { UseEditorHistoryOptions, UseEditorHistoryReturn } from './hooks/useEditorHistory'

export { MarkdownParser } from './utils/markdown'
export * from './utils/keyboardShortcuts'
export * from './utils/tiptapMarkdown'

export type {
  EditorMode,
  EditorTheme,
  EditorOptions,
  EditorInstance,
  ToolbarButton,
  ToolbarConfig,
  Plugin,
  UploadConfig,
  RenderOptions
} from './types'
