/** Status bar for cursor position, language, encoding, and LSP status. */

import React from 'react';
import { 
  AlertCircle,
  Loader2,
  Zap
} from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './EditorStatusBar.scss';

export interface EditorStatusBarProps {
  /** Current line number */
  line: number;
  /** Current column number */
  column: number;
  /** Number of selected characters */
  selectedChars?: number;
  /** Number of selected lines */
  selectedLines?: number;
  /** Programming language */
  language: string;
  /** File encoding */
  encoding?: string;
  /** Tab size */
  tabSize?: number;
  /** Whether to use spaces instead of tabs */
  insertSpaces?: boolean;
  /** Whether file has unsaved changes (reserved for extension) */
  hasChanges?: boolean;
  /** Whether file is being saved (reserved for extension) */
  isSaving?: boolean;
  /** Whether file is read-only */
  isReadOnly?: boolean;
  /** LSP connection status */
  lspStatus?: 'connected' | 'disconnected' | 'connecting';
  /** Language click callback */
  onLanguageClick?: (e: React.MouseEvent) => void;
  /** Encoding click callback */
  onEncodingClick?: (e: React.MouseEvent) => void;
  /** Indent click callback */
  onIndentClick?: (e: React.MouseEvent) => void;
  /** Position click callback */
  onPositionClick?: (e: React.MouseEvent) => void;
}

/** Get friendly display name for language */
const getLanguageDisplayName = (language: string): string => {
  const languageMap: Record<string, string> = {
    'typescript': 'TypeScript',
    'javascript': 'JavaScript',
    'typescriptreact': 'TypeScript React',
    'javascriptreact': 'JavaScript React',
    'python': 'Python',
    'rust': 'Rust',
    'go': 'Go',
    'java': 'Java',
    'csharp': 'C#',
    'cpp': 'C++',
    'c': 'C',
    'html': 'HTML',
    'css': 'CSS',
    'scss': 'SCSS',
    'less': 'Less',
    'json': 'JSON',
    'yaml': 'YAML',
    'xml': 'XML',
    'markdown': 'Markdown',
    'sql': 'SQL',
    'shell': 'Shell',
    'bash': 'Bash',
    'powershell': 'PowerShell',
    'dockerfile': 'Dockerfile',
    'plaintext': 'Plain Text',
    'toml': 'TOML',
    'ini': 'INI',
    'vue': 'Vue',
    'svelte': 'Svelte',
    'graphql': 'GraphQL',
    'php': 'PHP',
    'ruby': 'Ruby',
    'swift': 'Swift',
    'kotlin': 'Kotlin',
    'scala': 'Scala',
    'lua': 'Lua',
    'perl': 'Perl',
    'r': 'R',
  };
  return languageMap[language.toLowerCase()] || language;
};

const getLspStatusInfo = (
  status: 'connected' | 'disconnected' | 'connecting' | undefined,
  t: (key: string) => string
) => {
  switch (status) {
    case 'connected':
      return { 
        icon: <Zap size={12} />, 
        className: 'editor-status-bar__lsp--connected',
        title: t('editor.statusBar.lspConnected')
      };
    case 'connecting':
      return { 
        icon: <Loader2 size={12} className="editor-status-bar__lsp-spinner" />, 
        className: 'editor-status-bar__lsp--connecting',
        title: t('editor.statusBar.lspConnecting')
      };
    case 'disconnected':
    default:
      return { 
        icon: <AlertCircle size={12} />, 
        className: 'editor-status-bar__lsp--disconnected',
        title: t('editor.statusBar.lspDisconnected')
      };
  }
};

export const EditorStatusBar: React.FC<EditorStatusBarProps> = ({
  line,
  column,
  selectedChars = 0,
  selectedLines = 0,
  language,
  encoding = 'UTF-8',
  tabSize = 2,
  insertSpaces = true,
  isReadOnly = false,
  lspStatus,
  onLanguageClick,
  onEncodingClick,
  onIndentClick,
  onPositionClick,
}) => {
  const { t } = useI18n('tools');
  const lspInfo = getLspStatusInfo(lspStatus, t);

  // Build selection info text (updates with language changes).
  const getSelectionText = () => {
    if (selectedLines > 1) {
      return `(${t('editor.statusBar.selectionLinesChars', { lines: selectedLines, chars: selectedChars })})`;
    }
    if (selectedChars > 0) {
      return `(${t('editor.statusBar.selectionChars', { count: selectedChars })})`;
    }
    return '';
  };

  return (
    <div className="editor-status-bar">
      <div className="editor-status-bar__left">
        {isReadOnly && (
          <div className="editor-status-bar__item editor-status-bar__readonly">
            {t('editor.statusBar.readOnly')}
          </div>
        )}
      </div>

      <div className="editor-status-bar__right">
        <Tooltip content={t('editor.statusBar.goToLine')} placement="top">
          <div 
            className={`editor-status-bar__item ${onPositionClick ? 'editor-status-bar__item--clickable' : ''}`}
            onClick={onPositionClick}
          >
            <span>{t('editor.statusBar.ln')} {line}, {t('editor.statusBar.col')} {column}</span>
            {getSelectionText() && (
              <span className="editor-status-bar__selection">{getSelectionText()}</span>
            )}
          </div>
        </Tooltip>

        <div className="editor-status-bar__separator" />

        <Tooltip content={t('editor.statusBar.indentSettings')} placement="top">
          <div 
            className={`editor-status-bar__item ${onIndentClick ? 'editor-status-bar__item--clickable' : ''}`}
            onClick={onIndentClick}
          >
            {insertSpaces ? t('editor.statusBar.indentSpaces', { n: tabSize }) : t('editor.statusBar.indentTab', { n: tabSize })}
          </div>
        </Tooltip>

        <div className="editor-status-bar__separator" />

        <Tooltip content={t('editor.statusBar.fileEncoding')} placement="top">
          <div 
            className={`editor-status-bar__item ${onEncodingClick ? 'editor-status-bar__item--clickable' : ''}`}
            onClick={onEncodingClick}
          >
            {encoding}
          </div>
        </Tooltip>

        <div className="editor-status-bar__separator" />

        <Tooltip content={t('editor.statusBar.selectLanguageMode')} placement="top">
          <div 
            className={`editor-status-bar__item ${onLanguageClick ? 'editor-status-bar__item--clickable' : ''}`}
            onClick={onLanguageClick}
          >
            {getLanguageDisplayName(language)}
          </div>
        </Tooltip>

        {lspStatus && (
          <>
            <div className="editor-status-bar__separator" />
            <div 
              className={`editor-status-bar__item editor-status-bar__lsp ${lspInfo.className}`}
              title={lspInfo.title}
            >
              {lspInfo.icon}
            </div>
          </>
        )}
      </div>
    </div>
  );
};

export default EditorStatusBar;
