/**
 * MermaidBlock component
 * Renders Mermaid diagrams in Markdown
 */

import React, { useEffect, useState, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { MermaidService, MERMAID_THEME_CHANGE_EVENT } from '../../../tools/mermaid-editor/services/MermaidService';
import { Loader2, AlertCircle, Code2, Copy, Check, Edit } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import './MermaidBlock.scss';

const log = createLogger('MermaidBlock');

const svgCache = new Map<string, string>();

let themeVersion = 0;

const getThemeType = (): 'dark' | 'light' => {
  const themeType = document.documentElement.getAttribute('data-theme-type');
  if (themeType === 'light' || themeType === 'dark') return themeType;
  return 'dark';
};

const getCacheKey = (code: string): string => {
  return `${getThemeType()}:${code.trim()}`;
};

const clearCache = () => {
  svgCache.clear();
  themeVersion++;
  log.debug('Cache cleared', { version: themeVersion });
};

if (typeof window !== 'undefined') {
  window.addEventListener(MERMAID_THEME_CHANGE_EVENT, clearCache);
}

export interface MermaidBlockProps {
  code: string;
  isStreaming?: boolean;
  className?: string;
}

type RenderState = 'streaming' | 'incomplete' | 'loading' | 'rendered' | 'error';

const isCodeComplete = (code: string): boolean => {
  const trimmed = code.trim();
  if (!trimmed) return false;
  return /^(graph|flowchart|sequenceDiagram|classDiagram|stateDiagram|erDiagram|gantt|pie|journey|gitGraph|mindmap|timeline|quadrantChart)/m.test(trimmed);
};

export const MermaidBlock: React.FC<MermaidBlockProps> = ({
  code,
  isStreaming = false,
  className = ''
}) => {
  const { t } = useI18n('components');
  const { t: tMermaid } = useI18n('mermaid-editor');
  
  const cacheKey = getCacheKey(code);
  const cachedSvg = svgCache.get(cacheKey);
  
  const [state, setState] = useState<RenderState>(() => {
    if (cachedSvg) return 'rendered';
    if (isStreaming) return 'streaming';
    if (!code.trim() || !isCodeComplete(code)) return 'incomplete';
    return 'loading';
  });
  const [svgContent, setSvgContent] = useState<string>(cachedSvg || '');
  const [error, setError] = useState<string>('');
  const [showCode, setShowCode] = useState(false);
  const [copied, setCopied] = useState(false);
  
  const [currentThemeVersion, setCurrentThemeVersion] = useState(themeVersion);
  
  const mermaidService = useRef(MermaidService.getInstance());
  const renderTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const currentCodeRef = useRef<string>('');

  const renderDiagram = useCallback(async (codeToRender: string) => {
    const trimmedCode = codeToRender.trim();
    const key = getCacheKey(trimmedCode);
    
    if (!isCodeComplete(trimmedCode)) {
      setState('incomplete');
      return;
    }

    const cached = svgCache.get(key);
    if (cached) {
      setSvgContent(cached);
      setState('rendered');
      return;
    }

    setState('loading');
    setError('');

    try {
      const svg = await mermaidService.current.renderDiagram(trimmedCode);
      if (currentCodeRef.current === trimmedCode) {
        svgCache.set(key, svg);
        setSvgContent(svg);
        setState('rendered');
      }
    } catch (err) {
      if (currentCodeRef.current === trimmedCode) {
        setError(err instanceof Error ? err.message : t('mermaidBlock.renderFailed'));
        setState('error');
      }
    }
  }, []);

  useEffect(() => {
    const trimmedCode = code.trim();
    currentCodeRef.current = trimmedCode;

    if (renderTimeoutRef.current) {
      clearTimeout(renderTimeoutRef.current);
      renderTimeoutRef.current = null;
    }

    if (isStreaming) {
      setState('streaming');
      return;
    }

    if (!trimmedCode || !isCodeComplete(trimmedCode)) {
      setState('incomplete');
      return;
    }

    const key = getCacheKey(trimmedCode);
    const cached = svgCache.get(key);
    if (cached) {
      setSvgContent(cached);
      setState('rendered');
      return;
    }

    renderTimeoutRef.current = setTimeout(() => {
      renderDiagram(trimmedCode);
    }, 200);

    return () => {
      if (renderTimeoutRef.current) {
        clearTimeout(renderTimeoutRef.current);
      }
    };
  }, [code, isStreaming, renderDiagram, currentThemeVersion]);

  useEffect(() => {
    const handleThemeChange = () => {
      log.debug('Theme changed, triggering re-render');
      setCurrentThemeVersion(themeVersion);
      setSvgContent('');
      setState('loading');
    };

    window.addEventListener(MERMAID_THEME_CHANGE_EVENT, handleThemeChange);
    return () => {
      window.removeEventListener(MERMAID_THEME_CHANGE_EVENT, handleThemeChange);
    };
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      log.error('Failed to copy code', err);
    }
  }, [code]);

  const handleOpenInEditor = useCallback(() => {
    const duplicateCheckKey = `mermaid-md-${code.length}-${code.slice(0, 64).replace(/\s+/g, '')}`;
    window.dispatchEvent(new CustomEvent('expand-right-panel'));
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent('agent-create-tab', {
        detail: {
          type: 'mermaid-editor',
          title: tMermaid('tabs.chartEdit'),
          data: {
            mermaid_code: code,
            title: tMermaid('tabs.chartEdit'),
            mode: 'editor',
            allow_mode_switch: true,
            editor_config: {
              readonly: false,
              show_preview: true,
              auto_format: false
            }
          },
          metadata: { duplicateCheckKey, fromMarkdown: true },
          checkDuplicate: true,
          duplicateCheckKey,
          replaceExisting: true
        }
      }));
    }, 100);
  }, [code]);

  const renderContent = () => {
    switch (state) {
      case 'streaming':
        return (
          <div className="mermaid-block__streaming">
            <div className="mermaid-block__code-preview">
              <pre className="mermaid-code">
                <code>{code}</code>
                <span className="streaming-cursor">█</span>
              </pre>
            </div>
          </div>
        );

      case 'incomplete':
        return (
          <div className="mermaid-block__incomplete">
            <div className="mermaid-block__code-preview">
              <pre className="mermaid-code">
                <code>{code}</code>
              </pre>
            </div>
            <div className="mermaid-block__hint">
              <AlertCircle size={14} />
              <span>{t('mermaidBlock.codeIncomplete')}</span>
            </div>
          </div>
        );

      case 'loading':
        return (
          <div className="mermaid-block__loading">
            <div className="mermaid-block__loading-indicator">
              <Loader2 size={20} className="spinning" />
              <span>{t('mermaidBlock.rendering')}</span>
            </div>
          </div>
        );

      case 'rendered':
        return (
          <div className="mermaid-block__rendered">
            <div 
              className="mermaid-block__diagram"
              dangerouslySetInnerHTML={{ __html: svgContent }}
            />
            
            <div className="mermaid-block__actions">
              <button
                className="mermaid-icon-btn"
                onClick={handleOpenInEditor}
                title={t('mermaidBlock.openInEditor')}
              >
                <Edit size={14} />
              </button>
              <button
                className="mermaid-icon-btn"
                onClick={() => setShowCode(!showCode)}
                title={showCode ? t('mermaidBlock.hideCode') : t('mermaidBlock.showCode')}
              >
                <Code2 size={14} />
              </button>
              <button
                className={`mermaid-icon-btn ${copied ? 'copied' : ''}`}
                onClick={handleCopy}
                title={t('mermaidBlock.copyCode')}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
            </div>

            {showCode && (
              <div className="mermaid-block__source">
                <pre className="mermaid-code">
                  <code>{code}</code>
                </pre>
              </div>
            )}
          </div>
        );

      case 'error':
        return (
          <div className="mermaid-block__error">
            <div className="mermaid-block__error-message">
              <AlertCircle size={16} />
              <span>{t('mermaidBlock.renderFailed')}: {error}</span>
            </div>
            <div className="mermaid-block__code-preview">
              <pre className="mermaid-code">
                <code>{code}</code>
              </pre>
            </div>
            <div className="mermaid-block__actions">
              <button
                className={`mermaid-icon-btn ${copied ? 'copied' : ''}`}
                onClick={handleCopy}
                title={t('mermaidBlock.copyCode')}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
            </div>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className={`mermaid-block mermaid-block--${state} ${className}`}>
      {renderContent()}
    </div>
  );
};

export default MermaidBlock;
