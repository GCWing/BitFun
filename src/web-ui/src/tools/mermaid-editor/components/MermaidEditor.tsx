import React, { useCallback, useMemo, useState } from 'react';
import { MermaidEditorHeader } from './MermaidEditorHeader';
import { MermaidSourceEditor } from './MermaidSourceEditor';
import { MermaidComponentLibrary } from './MermaidComponentLibrary';
import { MermaidPreview, MermaidPreviewRef } from './MermaidPreview';
import { FloatingToolbar } from './FloatingToolbar';
import { useMermaidEditor } from '../hooks/useMermaidEditor';
import { MermaidEditorProps, LayoutMode, MermaidComponent } from '../types';
import { useContextStore } from '../../../shared/context-system';
import type { MermaidDiagramContext } from '../../../shared/types/context';
import { CubeLoading } from '@/component-library/components/CubeLoading';
import { Sparkles } from 'lucide-react';
import { aiApi } from '../../../infrastructure/api';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';

import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import './MermaidEditor.css';

/** Escape regex special characters. */
const escapeRegExp = (str: string) => str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Detect Mermaid diagram type. */
const detectDiagramType = (code: string): MermaidDiagramContext['diagramType'] => {
  const firstLine = code.trim().split('\n')[0].toLowerCase();
  if (firstLine.includes('graph') || firstLine.includes('flowchart')) return 'flowchart';
  if (firstLine.includes('sequencediagram')) return 'sequence';
  if (firstLine.includes('classdiagram')) return 'class';
  if (firstLine.includes('statediagram')) return 'state';
  if (firstLine.includes('erdiagram')) return 'er';
  if (firstLine.includes('gantt')) return 'gantt';
  return 'other';
};

const SVG_NS = 'http://www.w3.org/2000/svg';
const XHTML_NS = 'http://www.w3.org/1999/xhtml';

const sanitizeFileName = (name: string) => name.replace(/[<>:"/\\|?*\u0000-\u001F]/g, '_').trim() || 'diagram';

const createTimestampSuffix = () => {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}_${pad(now.getHours())}-${pad(now.getMinutes())}-${pad(now.getSeconds())}`;
};

const saveExportBlob = async (blob: Blob, baseName: string, extension: 'svg' | 'png') => {
  const downloadsPath = await downloadDir();
  const fileName = `${sanitizeFileName(baseName)}_${createTimestampSuffix()}.${extension}`;
  const filePath = await join(downloadsPath, fileName);
  const bytes = new Uint8Array(await blob.arrayBuffer());
  await writeFile(filePath, bytes);
  return filePath;
};

/**
 * Prepare a self-contained SVG element for export.
 *
 * cloneNode(true) already preserves:
 *  - Mermaid's internal <style> block (with resolved theme colors)
 *  - All inline styles set by applyBaseStyles (fill, stroke, etc.)
 *
 * We only need to: clean up interactive-only styles, set proper namespaces,
 * add a background rect, and ensure dimensions are correct.
 *
 * IMPORTANT: Do NOT bulk-copy computed styles (getComputedStyle) onto elements —
 * doing so injects hundreds of irrelevant CSS properties (width, display,
 * overflow, …) that break SVG layout and rendering.
 */
const prepareExportSvg = (
  sourceSvg: SVGElement,
  dims: { width: number; height: number }
) => {
  const cloned = sourceSvg.cloneNode(true) as SVGElement;

  const rootStyles = window.getComputedStyle(document.documentElement);
  const previewContainer = sourceSvg.parentElement;
  const previewBg = previewContainer
    ? window.getComputedStyle(previewContainer).backgroundColor
    : '';
  const exportBgColor =
    (previewBg && previewBg !== 'transparent' && previewBg !== 'rgba(0, 0, 0, 0)')
      ? previewBg
      : rootStyles.getPropertyValue('--mermaid-bg').trim()
        || rootStyles.getPropertyValue('--color-bg-primary').trim()
        || '#ffffff';

  // Strip interactive-only styles irrelevant for static export.
  cloned.style.transform = '';
  cloned.style.transition = '';
  cloned.style.transformOrigin = '';
  cloned.style.position = '';
  cloned.style.left = '';
  cloned.style.top = '';
  cloned.style.userSelect = '';
  cloned.style.flexShrink = '';
  cloned.style.width = '';
  cloned.style.height = '';
  cloned.style.minWidth = '';
  cloned.style.minHeight = '';

  cloned.setAttribute('width', String(dims.width));
  cloned.setAttribute('height', String(dims.height));
  cloned.setAttribute('xmlns', SVG_NS);
  cloned.setAttribute('xmlns:xlink', 'http://www.w3.org/1999/xlink');

  if (!cloned.getAttribute('viewBox')) {
    cloned.setAttribute('viewBox', `0 0 ${dims.width} ${dims.height}`);
  }

  // Ensure foreignObject HTML children carry the XHTML namespace
  // (required for standalone SVG rendering / Image data-URI loading).
  cloned.querySelectorAll('foreignObject').forEach(fo => {
    Array.from(fo.children).forEach(child => {
      if (!child.getAttribute('xmlns')) {
        child.setAttribute('xmlns', XHTML_NS);
      }
    });
  });

  // Remove interactive helper attributes & styles from all descendants.
  cloned.querySelectorAll('*').forEach(el => {
    const htmlEl = el as HTMLElement;
    if (htmlEl.style) {
      htmlEl.style.cursor = '';
      htmlEl.style.pointerEvents = '';
      htmlEl.style.transition = '';
    }
    el.removeAttribute('data-original-fill');
    el.removeAttribute('data-original-stroke');
    el.removeAttribute('data-original-color');
  });

  // Insert a background rect that covers the full viewBox (which may have
  // negative x/y offsets for padding), not just 0,0 → width,height.
  const backgroundRect = document.createElementNS(SVG_NS, 'rect');
  const viewBox = cloned.getAttribute('viewBox');
  if (viewBox) {
    const parts = viewBox.trim().split(/[\s,]+/).map(Number);
    if (parts.length >= 4) {
      backgroundRect.setAttribute('x', String(parts[0]));
      backgroundRect.setAttribute('y', String(parts[1]));
      backgroundRect.setAttribute('width', String(parts[2]));
      backgroundRect.setAttribute('height', String(parts[3]));
    }
  }
  if (!backgroundRect.getAttribute('width')) {
    backgroundRect.setAttribute('width', String(dims.width));
    backgroundRect.setAttribute('height', String(dims.height));
  }
  backgroundRect.setAttribute('fill', exportBgColor);
  const firstContent = Array.from(cloned.children).find(
    c => !['style', 'defs'].includes(c.tagName.toLowerCase())
  );
  cloned.insertBefore(backgroundRect, firstContent ?? null);

  return cloned;
};

export const MermaidEditor: React.FC<MermaidEditorProps> = React.memo(({
  initialSourceCode,
  onSave,
  onExport,
  className = '',
  mode,
  nodeMetadata,
  enableTooltips = true,
}) => {
  const { t } = useI18n('mermaid-editor');
  
  
  const {
    actions: { setSourceCode, setShowSourceEditor, setShowComponentLibrary, setLoading, setError },
    sourceCode,
    isDirty,
    isLoading,
    error,
    showSourceEditor,
    showComponentLibrary,
  } = useMermaidEditor({ initialSourceCode, autoValidate: true, autoParseInterval: 300 });

  const previewRef = React.useRef<MermaidPreviewRef>(null);

  const [isFixing, setIsFixing] = useState(false);
  const [fixProgress, setFixProgress] = useState({ current: 0, total: 0 });

  const [zoomLevel, setZoomLevel] = useState(100);

  // Default edit mode depends on interactive/editor mode.
  const [isEditMode, setIsEditMode] = useState(mode !== 'interactive');

  const [floatingToolbar, setFloatingToolbar] = useState<{
    isVisible: boolean;
    position: { x: number; y: number };
    type: 'node' | 'edge';
    data: { id: string; text: string; fromNode?: string; toNode?: string };
  }>({
    isVisible: false,
    position: { x: 0, y: 0 },
    type: 'node',
    data: { id: '', text: '' },
  });

  const addContext = useContextStore(state => state.addContext);

  const layoutMode: LayoutMode = useMemo(() => {
    if (showSourceEditor && showComponentLibrary) return 'source-library';
    if (showSourceEditor) return 'source-only';
    if (showComponentLibrary) return 'library-only';
    return 'preview-only';
  }, [showSourceEditor, showComponentLibrary]);

  const handleSourceChange = useCallback((newCode: string) => {
    setSourceCode(newCode);
    setError(null);
  }, [setSourceCode, setError]);

  const handleToggleComponentLibrary = useCallback(() => {
    setShowComponentLibrary(!showComponentLibrary);
  }, [showComponentLibrary, setShowComponentLibrary]);

  const handleToggleEditMode = useCallback(() => {
    const enteringEditMode = !isEditMode;
    setIsEditMode(enteringEditMode);
    
    if (enteringEditMode) {
      // Auto-open the source editor when entering edit mode.
      setShowSourceEditor(true);
    } else {
      // Close toolbar and source editor when leaving edit mode.
      setFloatingToolbar(prev => ({ ...prev, isVisible: false }));
      setShowSourceEditor(false);
    }
  }, [isEditMode, setShowSourceEditor]);

  const handleSave = useCallback(async () => {
    if (!isDirty || !onSave) return;
    try {
      setLoading(true);
      setError(null);
      await onSave(sourceCode);
    } catch (err) {
      setError(err instanceof Error ? err.message : t('errors.saveFailed'));
    } finally {
      setLoading(false);
    }
  }, [isDirty, onSave, sourceCode, setLoading, setError]);

  const handleExport = useCallback(async (format: string) => {
    const loadingCtrl = notificationService.loading({
      title: t('export.loading.pngTitle'),
      message: t('export.loading.pngMessage'),
    });

    try {
      setError(null);

      const svgEl = previewRef.current?.getSvgElement();
      const dims = previewRef.current?.getSvgDimensions();
      if (!svgEl) throw new Error(t('errors.exportFailed'));

      const w = dims?.width ?? 800;
      const h = dims?.height ?? 600;
      const cloned = prepareExportSvg(svgEl, { width: w, height: h });
      const svgData = new XMLSerializer().serializeToString(cloned);

      const { mermaidService } = await import('../services/MermaidService');
      const blob = await mermaidService.exportAsPNG(sourceCode, 2, svgData, { width: w, height: h });
      const filePath = await saveExportBlob(blob, t('panel.diagramName'), 'png');
      loadingCtrl.complete();
      notificationService.success(filePath, { title: t('export.success.png') });
      await onExport?.(format, '');
    } catch (err) {
      const msg = err instanceof Error ? err.message : t('errors.exportFailed');
      loadingCtrl.fail(`PNG ${t('errors.exportFailed')}: ${msg}`);
      setError(msg);
    }
  }, [onExport, sourceCode, setError, t]);

  const handleComponentSelect = useCallback((component: MermaidComponent) => {
    setSourceCode(sourceCode + '\n' + component.code);
  }, [sourceCode, setSourceCode]);

  // Keep the toolbar within the viewport.
  const calculateToolbarPosition = useCallback((event?: any, type: 'node' | 'edge' = 'node') => {
    if (!event) return { x: window.innerWidth / 2, y: window.innerHeight / 2 };
    
    const clientX = event.clientX ?? event.pageX;
    const clientY = event.clientY ?? event.pageY;
    const toolbarWidth = type === 'edge' ? 400 : 300;
    const toolbarHeight = type === 'edge' ? 100 : 70;
    
    let x = clientX - toolbarWidth / 2;
    let y = clientY + 8;
    
    x = Math.max(10, Math.min(window.innerWidth - toolbarWidth - 10, x));
    y = Math.max(10, Math.min(window.innerHeight - toolbarHeight - 10, y));
    
    return { x, y };
  }, []);

  const handleNodeClick = useCallback((nodeInfo: { id: string; text: string }, event?: MouseEvent) => {
    event?.stopPropagation();
    setFloatingToolbar({
      isVisible: true,
      position: calculateToolbarPosition(event, 'node'),
      type: 'node',
      data: { id: nodeInfo.id, text: nodeInfo.text },
    });
  }, [calculateToolbarPosition]);

  const handleEdgeClick = useCallback((edgeInfo: { id: string; text: string; fromNode?: string; toNode?: string }, event?: MouseEvent) => {
    event?.stopPropagation();
    setFloatingToolbar({
      isVisible: true,
      position: calculateToolbarPosition(event, 'edge'),
      type: 'edge',
      data: edgeInfo,
    });
  }, [calculateToolbarPosition]);

  const handleZoomIn = useCallback(() => previewRef.current?.zoomIn(), []);
  const handleZoomOut = useCallback(() => previewRef.current?.zoomOut(), []);
  const handleResetView = useCallback(() => previewRef.current?.resetView(), []);

  const handleAddToChatInput = useCallback(() => {
    const mermaidContext: MermaidDiagramContext = {
      id: `mermaid-${Date.now()}`,
      type: 'mermaid-diagram',
      diagramCode: sourceCode,
      diagramType: detectDiagramType(sourceCode),
      timestamp: Date.now(),
    };
    addContext(mermaidContext);
    window.dispatchEvent(new CustomEvent('insert-context-tag', { detail: { context: mermaidContext } }));
  }, [sourceCode, addContext]);

  // AI-assisted fix with validation and retries.
  const handleFixError = useCallback(async () => {
    if (!error || isFixing) return;
    
    const MAX_RETRIES = 10;
    let currentError = error;
    
    try {
      setIsFixing(true);
      setError(null);
      
      for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
        setFixProgress({ current: attempt, total: MAX_RETRIES });
        
        try {
          const fixedCode = await aiApi.fixMermaidCode({ sourceCode, errorMessage: currentError });
          
          if (!fixedCode?.trim()) {
            currentError = t('errors.aiEmptyCode');
            continue;
          }
          
          const { mermaidService } = await import('../services/MermaidService');
          const validation = await mermaidService.validateMermaidCode(fixedCode);
          
          if (validation.valid) {
            const trimmedCode = fixedCode.trim();
            setSourceCode(trimmedCode);
            setError(null);
            if (onSave) {
              try {
                await onSave(trimmedCode);
              } catch {
                setError(t('errors.fixSuccessButSaveFailed'));
              }
            }
            return;
          } else {
            currentError = validation.error || t('errors.validationFailed');
            if (attempt === MAX_RETRIES) {
              throw new Error(`${t('errors.fixFailedRetries', { count: MAX_RETRIES })}: ${currentError}`);
            }
          }
        } catch (err) {
          if (attempt === MAX_RETRIES) throw err;
          currentError = err instanceof Error ? err.message : 'Unknown error';
        }
      }
    } catch (err) {
      setError(`${t('errors.fixFailed')}: ${err instanceof Error ? err.message : 'Unknown error'}`);
    } finally {
      setIsFixing(false);
      setFixProgress({ current: 0, total: 0 });
    }
  }, [error, sourceCode, isFixing, setSourceCode, setError, onSave]);

  const handleToolbarSave = useCallback((editedData: any) => {
    if (floatingToolbar.type === 'node') {
      const { text: oldText } = floatingToolbar.data;
      const newText = editedData.text;
      
      if (oldText !== newText && newText.trim()) {
        const patterns = [
          { regex: new RegExp(`\\[${escapeRegExp(oldText)}\\]`, 'g'), replacement: `[${newText}]` },
          { regex: new RegExp(`\\(${escapeRegExp(oldText)}\\)`, 'g'), replacement: `(${newText})` },
          { regex: new RegExp(`\\{${escapeRegExp(oldText)}\\}`, 'g'), replacement: `{${newText}}` },
          { regex: new RegExp(`\\(\\(${escapeRegExp(oldText)}\\)\\)`, 'g'), replacement: `((${newText}))` },
          { regex: new RegExp(`\\[\\[${escapeRegExp(oldText)}\\]\\]`, 'g'), replacement: `[[${newText}]]` },
        ];
        
        let updated = sourceCode;
        for (const { regex, replacement } of patterns) {
          const newCode = updated.replace(regex, replacement);
          if (newCode !== updated) {
            updated = newCode;
            break;
          }
        }
        if (updated !== sourceCode) setSourceCode(updated, true);
      }
    } else if (floatingToolbar.type === 'edge') {
      const { fromNode: oldFrom, toNode: oldTo, text: oldText } = floatingToolbar.data;
      const { fromNode: newFrom, toNode: newTo, text: newText } = editedData;
      
      if ((oldFrom && newFrom && oldFrom !== newFrom) || (oldTo && newTo && oldTo !== newTo)) {
        const lines = sourceCode.split('\n');
        const updated = lines.map(line => {
          const patterns = [
            new RegExp(`^\\s*${escapeRegExp(oldFrom!)}\\s*(-->|->|---|==>|-\\.->)\\s*\\|[^|]*\\|\\s*${escapeRegExp(oldTo!)}`, 'i'),
            new RegExp(`^\\s*${escapeRegExp(oldFrom!)}\\s*(-->|->|---|==>|-\\.->)\\s*${escapeRegExp(oldTo!)}`, 'i'),
          ];
          for (const p of patterns) {
            if (p.test(line.trim())) {
              const conn = line.match(/(-->|->|---|==>|-\.->)/)?.[1] || '-->';
              return newText?.trim() 
                ? `    ${newFrom} ${conn}|${newText}| ${newTo}`
                : `    ${newFrom} ${conn} ${newTo}`;
            }
          }
          return line;
        }).join('\n');
        setSourceCode(updated, true);
      } else if (oldText !== newText) {
        let updated = sourceCode;
        if (oldText?.trim()) {
          updated = updated.replace(
            new RegExp(`(-->|->|---|==>|-\\.->)\\s*\\|${escapeRegExp(oldText)}\\|`, 'g'),
            newText?.trim() ? `$1|${newText}|` : '$1'
          );
        }
        setSourceCode(updated, true);
      }
    }
    
    setFloatingToolbar(prev => ({ ...prev, isVisible: false }));
  }, [floatingToolbar, sourceCode, setSourceCode]);

  const handleToolbarDelete = useCallback(() => {
    const { type, data } = floatingToolbar;
    let updated = sourceCode;
    
    if (type === 'node') {
      const nodeId = data.id;
      const lines = updated.split('\n');
      
      // Check whether the node is a subgraph.
      let subgraphStart = -1, subgraphEnd = -1;
      for (let i = 0; i < lines.length; i++) {
        if (new RegExp(`^subgraph\\s+${escapeRegExp(nodeId)}`, 'i').test(lines[i].trim())) {
          subgraphStart = i;
          for (let j = i + 1; j < lines.length; j++) {
            if (lines[j].trim() === 'end') { subgraphEnd = j; break; }
          }
          break;
        }
      }
      
      if (subgraphStart >= 0 && subgraphEnd >= 0) {
        // Remove the subgraph wrapper but keep its contents.
        updated = lines.filter((_, i) => i !== subgraphStart && i !== subgraphEnd)
          .map((line, _, _arr) => {
            const origIdx = lines.indexOf(line);
            if (origIdx > subgraphStart && origIdx < subgraphEnd) {
              return line.replace(/^(\s{2,4})/, '');
            }
            return line;
          }).join('\n');
      } else {
        // Remove a normal node and its edges.
        updated = lines.filter(line => {
          const trimmed = line.trim();
          if (!trimmed || trimmed.startsWith('//') || trimmed.startsWith('%%')) return true;
          
          const isNodeDef = new RegExp(`^\\s*${escapeRegExp(nodeId)}[\\[\\(\\{>]`).test(trimmed);
          const isConnection = [
            new RegExp(`^\\s*${escapeRegExp(nodeId)}\\s*(-->|->|---|==>|-\\.->)`),
            new RegExp(`(-->|->|---|==>|-\\.->)\\s*(?:\\|[^|]*\\|)?\\s*${escapeRegExp(nodeId)}\\s*$`),
          ].some(p => p.test(trimmed));
          
          return !isNodeDef && !isConnection;
        }).join('\n');
      }
    } else if (type === 'edge') {
      const { fromNode, toNode, text } = data;
      const lines = updated.split('\n');
      updated = lines.filter(line => {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('//') || trimmed.startsWith('%%')) return true;
        
        if (fromNode && toNode) {
          return ![
            new RegExp(`^\\s*${escapeRegExp(fromNode)}\\s*(-->|->|---|==>|-\\.->)\\s*\\|[^|]*\\|\\s*${escapeRegExp(toNode)}\\s*$`),
            new RegExp(`^\\s*${escapeRegExp(fromNode)}\\s*(-->|->|---|==>|-\\.->)\\s*${escapeRegExp(toNode)}\\s*$`),
          ].some(p => p.test(trimmed));
        }
        
        if (text?.trim()) {
          return !new RegExp(`(-->|->|---|==>|-\\.->)\\s*\\|${escapeRegExp(text)}\\|`).test(trimmed);
        }
        
        return true;
      }).join('\n');
    }
    
    updated = updated.replace(/\n\s*\n\s*\n/g, '\n\n');
    setSourceCode(updated, true);
    setFloatingToolbar(prev => ({ ...prev, isVisible: false }));
  }, [floatingToolbar, sourceCode, setSourceCode]);

  const handleContainerClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (!target.closest('.floating-toolbar, [data-node-id], [data-edge-id]') && floatingToolbar.isVisible) {
      setFloatingToolbar(prev => ({ ...prev, isVisible: false }));
    }
  }, [floatingToolbar.isVisible]);

  return (
    <div className={`mermaid-editor ${className}`} onClick={handleContainerClick} data-testid="mermaid-editor">
      <MermaidEditorHeader
        showComponentLibrary={showComponentLibrary}
        isDirty={isDirty}
        onToggleComponentLibrary={handleToggleComponentLibrary}
        onSave={handleSave}
        onExport={handleExport}
        zoomLevel={zoomLevel}
        onZoomIn={handleZoomIn}
        onZoomOut={handleZoomOut}
        onResetView={handleResetView}
        onAddToChatInput={handleAddToChatInput}
        hasError={!!error}
        onFixError={handleFixError}
        isFixing={isFixing}
        fixProgress={fixProgress}
        isEditMode={isEditMode}
        onToggleEditMode={handleToggleEditMode}
      />

      <div className={`editor-content layout-${layoutMode}`}>
        {layoutMode !== 'preview-only' && (
          <div className="left-panel">
            {layoutMode === 'source-only' && (
              <MermaidSourceEditor sourceCode={sourceCode} onChange={handleSourceChange} className="full-height" />
            )}
            {layoutMode === 'library-only' && (
              <MermaidComponentLibrary onComponentSelect={handleComponentSelect} className="full-height" />
            )}
            {layoutMode === 'source-library' && (
              <>
                <div className="top-panel">
                  <MermaidSourceEditor sourceCode={sourceCode} onChange={handleSourceChange} />
                </div>
                <div className="bottom-panel">
                  <MermaidComponentLibrary onComponentSelect={handleComponentSelect} />
                </div>
              </>
            )}
          </div>
        )}

        <div className="right-panel">
          <MermaidPreview
            ref={previewRef}
            sourceCode={sourceCode}
            isEditMode={isEditMode}
            nodeMetadata={nodeMetadata}
            enableTooltips={enableTooltips}
            onNodeClick={handleNodeClick}
            onEdgeClick={handleEdgeClick}
            onError={setError}
            onRender={() => setError(null)}
            onZoomChange={setZoomLevel}
          />
          
          {(error || isFixing) && (
            <div className="error-overlay error-overlay--centered">
              <div className="error-overlay__card">
                {isFixing ? (
                  <>
                    <CubeLoading size="small" />
                    <div className="error-overlay__hint">
                      {t('errors.fixingProgress', { current: fixProgress.current, total: fixProgress.total })}
                    </div>
                    <div className="error-overlay__progress">
                      <div
                        className="error-overlay__progress-bar"
                        style={{ width: `${(fixProgress.current / fixProgress.total) * 100}%` }}
                      />
                    </div>
                  </>
                ) : (
                  <>
                    <div className="error-overlay__hint">{t('errors.syntaxHint')}</div>
                    <button
                      className="error-overlay__fix-btn"
                      onClick={handleFixError}
                    >
                      <Sparkles size={16} />
                      <span>{t('header.oneClickFix')}</span>
                    </button>
                  </>
                )}
              </div>
            </div>
          )}
          
          {isLoading && (
            <div className="loading-overlay">
              <div className="loading-spinner">
                <CubeLoading size="small" />
                <span>{t('loading.processing')}</span>
              </div>
            </div>
          )}
        </div>
      </div>

      <FloatingToolbar
        isVisible={floatingToolbar.isVisible}
        position={floatingToolbar.position}
        type={floatingToolbar.type}
        data={floatingToolbar.data}
        onSave={handleToolbarSave}
        onDelete={handleToolbarDelete}
        onClose={() => setFloatingToolbar(prev => ({ ...prev, isVisible: false }))}
      />
    </div>
  );
}, (prevProps, nextProps) => {
  return prevProps.initialSourceCode === nextProps.initialSourceCode &&
    prevProps.className === nextProps.className;
});

MermaidEditor.displayName = 'MermaidEditor';
