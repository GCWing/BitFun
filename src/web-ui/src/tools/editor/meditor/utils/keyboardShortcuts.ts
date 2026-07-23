/**
 * IR editor keyboard shortcut helpers.
 * Provides shortcut handlers for Markdown formatting and editing.
 */

import { i18nService } from '@/infrastructure/i18n';

/**
 * Check whether current platform is macOS.
 */
export const isMacPlatform = (): boolean => {
  return typeof navigator !== 'undefined' && 
    navigator.platform.toUpperCase().indexOf('MAC') >= 0
}

/**
 * Check whether the modifier key is pressed (Cmd on macOS, Ctrl elsewhere).
 */
export const isModKey = (e: KeyboardEvent | React.KeyboardEvent): boolean => {
  return isMacPlatform() ? e.metaKey : e.ctrlKey
}

/**
 * Shortcut definition
 */
export interface ShortcutDefinition {
  /** Shortcut description */
  description: string | (() => string)
  /** Key (case-insensitive) */
  key: string
  /** Whether Ctrl/Cmd is required */
  modKey?: boolean
  /** Whether Shift is required */
  shiftKey?: boolean
  /** Whether Alt is required */
  altKey?: boolean
}

/**
 * Predefined shortcuts
 */
export const SHORTCUTS = {
  UNDO: { description: () => i18nService.t('tools:editor.meditor.shortcuts.undo'), key: 'z', modKey: true },
  REDO_SHIFT: { description: () => i18nService.t('tools:editor.meditor.shortcuts.redo'), key: 'z', modKey: true, shiftKey: true },
  REDO_Y: { description: () => i18nService.t('tools:editor.meditor.shortcuts.redo'), key: 'y', modKey: true },
  SAVE: { description: () => i18nService.t('tools:editor.meditor.shortcuts.save'), key: 's', modKey: true },
  ESCAPE: { description: () => i18nService.t('tools:editor.meditor.shortcuts.escape'), key: 'Escape' },
  
  INDENT: { description: () => i18nService.t('tools:editor.meditor.shortcuts.indent'), key: 'Tab' },
  OUTDENT: { description: () => i18nService.t('tools:editor.meditor.shortcuts.outdent'), key: 'Tab', shiftKey: true },
  
  BOLD: { description: () => i18nService.t('tools:editor.meditor.shortcuts.bold'), key: 'b', modKey: true },
  ITALIC: { description: () => i18nService.t('tools:editor.meditor.shortcuts.italic'), key: 'i', modKey: true },
  LINK: { description: () => i18nService.t('tools:editor.meditor.shortcuts.link'), key: 'k', modKey: true },
  NEW_BLOCK: { description: () => i18nService.t('tools:editor.meditor.shortcuts.newBlock'), key: 'Enter', modKey: true },
} as const

export const getShortcutDescription = (shortcut: ShortcutDefinition): string => {
  return typeof shortcut.description === 'function' ? shortcut.description() : shortcut.description
}

/**
 * Check whether event matches a specific shortcut.
 */
export const matchShortcut = (
  e: KeyboardEvent | React.KeyboardEvent, 
  shortcut: ShortcutDefinition
): boolean => {
  const keyMatch = e.key.toLowerCase() === shortcut.key.toLowerCase()
  const modMatch = shortcut.modKey ? isModKey(e) : !isModKey(e)
  const shiftMatch = shortcut.shiftKey ? e.shiftKey : !e.shiftKey
  const altMatch = shortcut.altKey ? e.altKey : !e.altKey
  
  return keyMatch && modMatch && shiftMatch && altMatch
}

/**
 * Wrap selection in the given wrapper (e.g. `**` for bold).
 *
 * @param text Original text
 * @param selectionStart Selection start index
 * @param selectionEnd Selection end index
 * @param wrapper Wrapper string (e.g. `**`)
 * @returns New text and updated selection positions
 */
export const wrapSelection = (
  text: string,
  selectionStart: number,
  selectionEnd: number,
  wrapper: string
): { text: string; selectionStart: number; selectionEnd: number } => {
  const before = text.substring(0, selectionStart)
  const selected = text.substring(selectionStart, selectionEnd)
  const after = text.substring(selectionEnd)
  
  const isWrapped = 
    before.endsWith(wrapper) && 
    after.startsWith(wrapper)
  
  if (isWrapped && selected.length > 0) {
    const newBefore = before.slice(0, -wrapper.length)
    const newAfter = after.slice(wrapper.length)
    return {
      text: newBefore + selected + newAfter,
      selectionStart: selectionStart - wrapper.length,
      selectionEnd: selectionEnd - wrapper.length
    }
  } else {
    return {
      text: before + wrapper + selected + wrapper + after,
      selectionStart: selectionStart + wrapper.length,
      selectionEnd: selectionEnd + wrapper.length
    }
  }
}

/**
 * Toggle bold for selected text.
 */
export const toggleBold = (
  text: string,
  selectionStart: number,
  selectionEnd: number
): { text: string; selectionStart: number; selectionEnd: number } => {
  return wrapSelection(text, selectionStart, selectionEnd, '**')
}

/**
 * Toggle italic for selected text.
 */
export const toggleItalic = (
  text: string,
  selectionStart: number,
  selectionEnd: number
): { text: string; selectionStart: number; selectionEnd: number } => {
  return wrapSelection(text, selectionStart, selectionEnd, '*')
}

/**
 * Insert a link.
 */
export const insertLink = (
  text: string,
  selectionStart: number,
  selectionEnd: number
): { text: string; selectionStart: number; selectionEnd: number } => {
  const before = text.substring(0, selectionStart)
  const selected = text.substring(selectionStart, selectionEnd)
  const after = text.substring(selectionEnd)
  
  if (selected.length > 0) {
    const newText = before + '[' + selected + '](url)' + after
    return {
      text: newText,
      selectionStart: selectionEnd + 3,
      selectionEnd: selectionEnd + 6 // select "url"
    }
  } else {
    const linkText = i18nService.t('tools:editor.meditor.insertLinkTextPlaceholder')
    const newText = before + `[${linkText}](url)` + after
    return {
      text: newText,
      selectionStart: selectionStart + 1,
      selectionEnd: selectionStart + 1 + linkText.length // select link text
    }
  }
}

/**
 * Indent lines (add leading spaces or Tab).
 */
export const indentLines = (
  text: string,
  selectionStart: number,
  selectionEnd: number,
  indentString: string = '  ' // default: two spaces
): { text: string; selectionStart: number; selectionEnd: number } => {
  const lines = text.split('\n')
  let currentPos = 0
  let startLine = 0
  let endLine = 0
  
  for (let i = 0; i < lines.length; i++) {
    const lineEnd = currentPos + lines[i].length
    if (currentPos <= selectionStart && selectionStart <= lineEnd + 1) {
      startLine = i
    }
    if (currentPos <= selectionEnd && selectionEnd <= lineEnd + 1) {
      endLine = i
      break
    }
    currentPos = lineEnd + 1 // +1 for newline
  }
  
  const newLines = lines.map((line, i) => {
    if (i >= startLine && i <= endLine) {
      return indentString + line
    }
    return line
  })
  
  const indentedCount = endLine - startLine + 1
  
  return {
    text: newLines.join('\n'),
    selectionStart: selectionStart + indentString.length,
    selectionEnd: selectionEnd + indentString.length * indentedCount
  }
}

/**
 * Outdent lines (remove leading spaces or Tab).
 */
export const outdentLines = (
  text: string,
  selectionStart: number,
  selectionEnd: number,
  indentString: string = '  '
): { text: string; selectionStart: number; selectionEnd: number } => {
  const lines = text.split('\n')
  let currentPos = 0
  let startLine = 0
  let endLine = 0
  
  for (let i = 0; i < lines.length; i++) {
    const lineEnd = currentPos + lines[i].length
    if (currentPos <= selectionStart && selectionStart <= lineEnd + 1) {
      startLine = i
    }
    if (currentPos <= selectionEnd && selectionEnd <= lineEnd + 1) {
      endLine = i
      break
    }
    currentPos = lineEnd + 1
  }
  
  let removedBefore = 0
  let removedTotal = 0
  
  const newLines = lines.map((line, i) => {
    if (i >= startLine && i <= endLine) {
      if (line.startsWith(indentString)) {
        if (i === startLine) {
          removedBefore = indentString.length
        }
        removedTotal += indentString.length
        return line.slice(indentString.length)
      } else if (line.startsWith('\t')) {
        if (i === startLine) {
          removedBefore = 1
        }
        removedTotal += 1
        return line.slice(1)
      }
    }
    return line
  })
  
  return {
    text: newLines.join('\n'),
    selectionStart: Math.max(0, selectionStart - removedBefore),
    selectionEnd: Math.max(0, selectionEnd - removedTotal)
  }
}

/**
 * Get display text for a shortcut.
 */
export const getShortcutDisplay = (shortcut: ShortcutDefinition): string => {
  const parts: string[] = []
  const isMac = isMacPlatform()
  
  if (shortcut.modKey) {
    parts.push(isMac ? '⌘' : 'Ctrl')
  }
  if (shortcut.shiftKey) {
    parts.push(isMac ? '⇧' : 'Shift')
  }
  if (shortcut.altKey) {
    parts.push(isMac ? '⌥' : 'Alt')
  }
  
  let keyDisplay = shortcut.key.toUpperCase()
  if (shortcut.key === 'Escape') keyDisplay = 'Esc'
  if (shortcut.key === 'Enter') keyDisplay = '↵'
  if (shortcut.key === 'Tab') keyDisplay = '⇥'
  
  parts.push(keyDisplay)
  
  return parts.join(isMac ? '' : '+')
}
