/**
 * Status bar popovers: go to line, indent, encoding, language mode.
 * Styled to match Cursor/VS Code popover interactions.
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import {
  FileCode,
  FileText,
  Braces,
  Code2,
  Code,
  FileJson,
  type LucideIcon,
} from 'lucide-react';
import { Button, Input } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './StatusBarPopovers.scss';

export type StatusBarPopoverType = 'position' | 'indent' | 'encoding' | 'language';

export interface AnchorRect {
  top: number;
  left: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

export interface GoToLinePopoverProps {
  anchorRect: AnchorRect;
  currentLine: number;
  currentColumn: number;
  onConfirm: (line: number, column: number) => void;
  onClose: () => void;
}

export const GoToLinePopover: React.FC<GoToLinePopoverProps> = ({
  anchorRect,
  currentLine,
  currentColumn,
  onConfirm,
  onClose,
}) => {
  const { t } = useI18n('tools');
  const [value, setValue] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setValue(`${currentLine}:${currentColumn}`);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 50);
  }, [currentLine, currentColumn]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onClose();
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const trimmed = value.trim();
      if (!trimmed) {
        onClose();
        return;
      }
      const part = trimmed.split(':');
      const line = Math.max(1, parseInt(part[0], 10) || 1);
      const column = part[1] !== undefined ? Math.max(1, parseInt(part[1], 10) || 1) : 1;
      onConfirm(line, column);
      onClose();
    }
  };

  const top = anchorRect.top - 4;
  const left = Math.max(8, Math.min(anchorRect.right - 200, anchorRect.left));

  return createPortal(
    <div
      className="status-bar-popover"
      style={{ top, left }}
      role="dialog"
      aria-label={t('editor.statusBar.goToLine')}
    >
      <div className="status-bar-popover__hint">{t('editor.statusBar.goToLineHint')}</div>
      <div className="status-bar-popover__input-wrap">
        <Input
          ref={inputRef}
          type="text"
          className="status-bar-popover__input"
          variant="outlined"
          inputSize="small"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('editor.statusBar.goToLinePlaceholder')}
        />
      </div>
    </div>,
    document.body
  );
};

export interface IndentOption {
  label: string;
  tabSize: number;
  insertSpaces: boolean;
}

const INDENT_OPTIONS: Array<{ tabSize: number; insertSpaces: boolean }> = [
  { tabSize: 1, insertSpaces: true },
  { tabSize: 2, insertSpaces: true },
  { tabSize: 4, insertSpaces: true },
  { tabSize: 8, insertSpaces: true },
  { tabSize: 1, insertSpaces: false },
  { tabSize: 2, insertSpaces: false },
  { tabSize: 4, insertSpaces: false },
  { tabSize: 8, insertSpaces: false },
];

export interface IndentPopoverProps {
  anchorRect: AnchorRect;
  currentTabSize: number;
  currentInsertSpaces: boolean;
  onConfirm: (tabSize: number, insertSpaces: boolean) => void;
  onClose: () => void;
}

export const IndentPopover: React.FC<IndentPopoverProps> = ({
  anchorRect,
  currentTabSize,
  currentInsertSpaces,
  onConfirm,
  onClose,
}) => {
  const { t } = useI18n('tools');
  const handleSelect = useCallback(
    (opt: { tabSize: number; insertSpaces: boolean }) => {
      onConfirm(opt.tabSize, opt.insertSpaces);
      onClose();
    },
    [onConfirm, onClose]
  );

  const top = anchorRect.top - 4;
  const left = Math.max(8, Math.min(anchorRect.right - 160, anchorRect.left));

  return createPortal(
    <div
      className="status-bar-popover"
      style={{ top, left }}
      role="dialog"
      aria-label={t('editor.statusBar.indentSettings')}
    >
      <div className="status-bar-popover__hint">{t('editor.statusBar.selectIndent')}</div>
      <div className="status-bar-popover__list">
        {INDENT_OPTIONS.map((opt) => {
          const label = opt.insertSpaces
            ? t('editor.statusBar.indentOptionSpaces', { n: opt.tabSize })
            : t('editor.statusBar.indentOptionTab', { n: opt.tabSize });
          return (
            <Button
              key={`${opt.insertSpaces ? 's' : 't'}-${opt.tabSize}`}
              className={`status-bar-popover__item ${
                opt.tabSize === currentTabSize && opt.insertSpaces === currentInsertSpaces
                  ? 'status-bar-popover__item--active'
                  : ''
              }`}
              variant="ghost"
              size="small"
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleSelect(opt);
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  handleSelect(opt);
                }
                if (e.key === 'Escape') onClose();
              }}
              role="option"
              tabIndex={0}
            >
              {label}
            </Button>
          );
        })}
      </div>
    </div>,
    document.body
  );
};

const ENCODING_OPTIONS = ['UTF-8', 'UTF-8 with BOM', 'GBK', 'GB2312', 'UTF-16LE', 'UTF-16BE', 'ISO-8859-1'];

export interface EncodingPopoverProps {
  anchorRect: AnchorRect;
  currentEncoding: string;
  onConfirm: (encoding: string) => void;
  onClose: () => void;
}

export const EncodingPopover: React.FC<EncodingPopoverProps> = ({
  anchorRect,
  currentEncoding,
  onConfirm,
  onClose,
}) => {
  const { t } = useI18n('tools');
  const top = anchorRect.top - 4;
  const left = Math.max(8, Math.min(anchorRect.right - 160, anchorRect.left));

  return createPortal(
    <div
      className="status-bar-popover"
      style={{ top, left }}
      role="dialog"
      aria-label={t('editor.statusBar.fileEncoding')}
    >
      <div className="status-bar-popover__hint">{t('editor.statusBar.selectEncoding')}</div>
      <div className="status-bar-popover__list">
        {ENCODING_OPTIONS.map((enc) => (
          <Button
            key={enc}
            className={`status-bar-popover__item ${
              enc === currentEncoding ? 'status-bar-popover__item--active' : ''
            }`}
            variant="ghost"
            size="small"
            type="button"
            onClick={() => {
              onConfirm(enc);
              onClose();
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') onClose();
            }}
            role="option"
            tabIndex={0}
          >
            {enc}
          </Button>
        ))}
      </div>
    </div>,
    document.body
  );
};

// Language mode (with Cursor-style small icons)
const getLanguageDisplayName = (id: string, aliases?: string[]): string => {
  const map: Record<string, string> = {
    typescript: 'TypeScript',
    javascript: 'JavaScript',
    typescriptreact: 'TypeScript React',
    javascriptreact: 'JavaScript React',
    python: 'Python',
    rust: 'Rust',
    go: 'Go',
    java: 'Java',
    csharp: 'C#',
    cpp: 'C++',
    c: 'C',
    html: 'HTML',
    css: 'CSS',
    scss: 'SCSS',
    less: 'Less',
    json: 'JSON',
    yaml: 'YAML',
    xml: 'XML',
    markdown: 'Markdown',
    sql: 'SQL',
    shell: 'Shell',
    bash: 'Bash',
    plaintext: 'Plain Text',
    toml: 'TOML',
    vue: 'Vue',
    svelte: 'Svelte',
    graphql: 'GraphQL',
    php: 'PHP',
    ruby: 'Ruby',
    swift: 'Swift',
    kotlin: 'Kotlin',
    lua: 'Lua',
  };
  return map[id.toLowerCase()] || (aliases?.[0] ?? id);
};

const getLanguageIcon = (id: string): LucideIcon => {
  const key = id.toLowerCase();
  if (key === 'json' || key === 'jsonc') return Braces;
  if (key === 'markdown' || key === 'md') return FileText;
  if (key === 'html' || key === 'xml') return FileCode;
  if (key === 'plaintext' || key === 'txt') return FileText;
  if (key === 'typescript' || key === 'typescriptreact' || key === 'javascript' || key === 'javascriptreact') return Code2;
  if (key === 'python' || key === 'rust' || key === 'go' || key === 'java' || key === 'csharp' || key === 'cpp' || key === 'c') return Code;
  if (key === 'css' || key === 'scss' || key === 'less') return FileCode;
  if (key === 'yaml' || key === 'yml' || key === 'toml') return FileCode;
  if (key === 'sql' || key === 'shell' || key === 'bash') return FileCode;
  if (key === 'vue' || key === 'svelte' || key === 'graphql') return FileJson;
  return FileCode;
};

export interface LanguagePopoverProps {
  anchorRect: AnchorRect;
  currentLanguageId: string;
  languages: Array<{ id: string; aliases?: string[] }>;
  onConfirm: (languageId: string) => void;
  onClose: () => void;
}

export const LanguagePopover: React.FC<LanguagePopoverProps> = ({
  anchorRect,
  currentLanguageId,
  languages,
  onConfirm,
  onClose,
}) => {
  const { t } = useI18n('tools');
  const top = anchorRect.top - 4;
  const left = Math.max(8, Math.min(anchorRect.right - 180, anchorRect.left));

  return createPortal(
    <div
      className="status-bar-popover"
      style={{ top, left, maxHeight: 320 }}
      role="dialog"
      aria-label={t('editor.statusBar.selectLanguageMode')}
    >
      <div className="status-bar-popover__hint">{t('editor.statusBar.selectLanguageModeHint')}</div>
      <div className="status-bar-popover__list">
        {languages.map((lang) => {
          const Icon = getLanguageIcon(lang.id);
          return (
            <Button
              key={lang.id}
              className={`status-bar-popover__item ${
                lang.id === currentLanguageId ? 'status-bar-popover__item--active' : ''
              }`}
              variant="ghost"
              size="small"
              type="button"
              onClick={() => {
                onConfirm(lang.id);
                onClose();
              }}
              onKeyDown={(e) => {
                if (e.key === 'Escape') onClose();
              }}
              role="option"
              tabIndex={0}
            >
              <span className="status-bar-popover__item-icon" aria-hidden>
                <Icon size={14} strokeWidth={2} />
              </span>
              {getLanguageDisplayName(lang.id, lang.aliases)}
            </Button>
          );
        })}
      </div>
    </div>,
    document.body
  );
};