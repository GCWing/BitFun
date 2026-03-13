/**
 * Mermaid diagram preview with pan/zoom and tooltip support.
 */

import React, { useEffect, useRef, useState, useCallback, useImperativeHandle, forwardRef } from 'react';
import { createPortal } from 'react-dom';
import { mermaidService, MERMAID_THEME_CHANGE_EVENT } from '../services/MermaidService';
import { usePanZoom } from '../hooks/usePanZoom';
import { useSvgInteraction, NodeInfo, EdgeInfo } from '../hooks/useSvgInteraction';
import { CubeLoading } from '@/component-library/components/CubeLoading';
import { useI18n } from '@/infrastructure/i18n';
import type { NodeMetadata, TooltipData } from '../types/MermaidPanelTypes';
import './MermaidPreview.css';

export interface MermaidPreviewProps {
  sourceCode: string;
  className?: string;
  /** Edit mode opens the editor on node click; otherwise it navigates to a file. */
  isEditMode?: boolean;
  /** Node metadata for navigation and tooltips. */
  nodeMetadata?: Record<string, NodeMetadata>;
  /** Whether to show tooltips. */
  enableTooltips?: boolean;
  onRender?: () => void;
  onError?: (error: string) => void;
  /** Node click callback in edit mode. */
  onNodeClick?: (nodeInfo: NodeInfo, event?: MouseEvent) => void;
  /** Edge click callback in edit mode. */
  onEdgeClick?: (edgeInfo: EdgeInfo, event?: MouseEvent) => void;
  onZoomChange?: (zoomLevel: number) => void;
}

export interface MermaidPreviewRef {
  zoomIn: () => void;
  zoomOut: () => void;
  resetView: () => void;
  fitToContainer: () => void;
  getZoomLevel: () => number;
  getSvgElement: () => SVGElement | null;
  getSvgDimensions: () => { width: number; height: number } | null;
}

const RENDER_DEBOUNCE_DELAY = 200;
const TOOLTIP_OFFSET = 16;
const TOOLTIP_MARGIN = 12;

interface MermaidTooltipProps {
  data: TooltipData;
  position: { x: number; y: number };
  capturedVarsLabel: string;
}

/**
 * Tooltip that keeps itself within the viewport.
 */
const MermaidTooltip: React.FC<MermaidTooltipProps> = ({ data, position, capturedVarsLabel }) => {
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [adjustedPosition, setAdjustedPosition] = useState({ x: 0, y: 0 });
  const [isPositioned, setIsPositioned] = useState(false);

  useEffect(() => {
    if (!tooltipRef.current) return;

    const tooltip = tooltipRef.current;
    const rect = tooltip.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let x = position.x + TOOLTIP_OFFSET;
    let y = position.y + TOOLTIP_OFFSET;

    if (x + rect.width + TOOLTIP_MARGIN > viewportWidth) {
      x = position.x - rect.width - TOOLTIP_OFFSET;
    }

    if (y + rect.height + TOOLTIP_MARGIN > viewportHeight) {
      y = position.y - rect.height - TOOLTIP_OFFSET;
    }

    if (x < TOOLTIP_MARGIN) {
      x = TOOLTIP_MARGIN;
    }

    if (y < TOOLTIP_MARGIN) {
      y = TOOLTIP_MARGIN;
    }

    setAdjustedPosition({ x, y });
    setIsPositioned(true);
  }, [position]);

  return (
    <div 
      ref={tooltipRef}
      className="mermaid-tooltip mermaid-tooltip--portal"
      style={{
        position: 'fixed',
        left: adjustedPosition.x,
        top: adjustedPosition.y,
        opacity: isPositioned ? 1 : 0,
        zIndex: 99999,
      }}
    >
      <div className="tooltip-header">
        <span className="tooltip-title">{data.title}</span>
      </div>
      {data.file_location && (
        <div className="tooltip-location">{data.file_location}</div>
      )}
      {data.description && (
        <div className="tooltip-description">{data.description}</div>
      )}
      {data.captured_vars && data.captured_vars.length > 0 && (
        <div className="tooltip-vars">
          <span className="tooltip-vars-label">{capturedVarsLabel}:</span>
          <span className="tooltip-vars-list">{data.captured_vars.join(', ')}</span>
        </div>
      )}
    </div>
  );
};

export const MermaidPreview = React.memo(forwardRef<MermaidPreviewRef, MermaidPreviewProps>(({
  sourceCode,
  className = '',
  isEditMode = true,
  nodeMetadata,
  enableTooltips = true,
  onRender,
  onError,
  onNodeClick,
  onEdgeClick,
  onZoomChange,
}, ref) => {
  const { t } = useI18n('mermaid-editor');
  
  const svgRef = useRef<SVGElement | null>(null);
  const renderTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cleanupInteractionRef = useRef<(() => void) | null>(null);
  const lastSourceCodeRef = useRef<string | null>(null);
  const isFirstRenderRef = useRef(true);
  const svgDimensionsRef = useRef<{ width: number; height: number } | null>(null);
  
  // Keep latest callbacks to avoid stale closures.
  const onRenderRef = useRef(onRender);
  const onErrorRef = useRef(onError);
  useEffect(() => {
    onRenderRef.current = onRender;
    onErrorRef.current = onError;
  });

  const [isLoading, setIsLoading] = useState(false);
  const [tooltipData, setTooltipData] = useState<TooltipData | null>(null);
  const [tooltipPosition, setTooltipPosition] = useState({ x: 0, y: 0 });

  const { 
    transform, 
    isDragging, 
    hasDragged,
    handlers, 
    controls, 
    containerRef,
    resetDragState,
  } = usePanZoom({ onZoomChange });

  const { setupInteraction, applyBaseStyles } = useSvgInteraction({
    sourceCode,
    isEditMode,
    nodeMetadata,
    enableTooltips,
    onNodeClick,
    onEdgeClick,
    onTooltipShow: enableTooltips ? (data: TooltipData, position: { x: number; y: number }) => {
      setTooltipData(data);
      setTooltipPosition(position);
    } : undefined,
    onTooltipUpdate: enableTooltips ? (position: { x: number; y: number }) => {
      setTooltipPosition(position);
    } : undefined,
    onTooltipHide: enableTooltips ? () => {
      setTooltipData(null);
    } : undefined,
    hasDragged,
    resetDragState,
  });

  // Use cached SVG size for fit-to-container.
  const handleFitToContainer = useCallback(() => {
    if (svgDimensionsRef.current) {
      controls.fitToContainer(svgDimensionsRef.current.width, svgDimensionsRef.current.height);
    }
  }, [controls]);

  useImperativeHandle(ref, () => ({
    ...controls,
    fitToContainer: handleFitToContainer,
    getSvgElement: () => svgRef.current,
    getSvgDimensions: () => svgDimensionsRef.current,
  }), [controls, handleFitToContainer]);

  const renderDiagram = useCallback(async () => {
    // Clear previous interaction handlers.
    if (cleanupInteractionRef.current) {
      cleanupInteractionRef.current();
      cleanupInteractionRef.current = null;
    }

    if (!containerRef.current || !sourceCode.trim()) {
      if (containerRef.current) {
        containerRef.current.innerHTML = '';
        svgRef.current = null;
      }
      return;
    }

    // Show loading only on first render.
    if (isFirstRenderRef.current) {
      setIsLoading(true);
    }

    try {
      const svg = await mermaidService.renderDiagram(sourceCode);
      
      if (containerRef.current) {
        containerRef.current.innerHTML = svg;
        
        const svgElement = containerRef.current.querySelector('svg');
        if (svgElement) {
          // Resolve original SVG size from viewBox/attributes/getBBox.
          const viewBox = svgElement.getAttribute('viewBox');
          let originalWidth = 0;
          let originalHeight = 0;
          
          if (viewBox) {
            const parts = viewBox.split(/\s+|,/).map(Number);
            if (parts.length >= 4) {
              originalWidth = parts[2];
              originalHeight = parts[3];
            }
          }
          
          if (originalWidth === 0 || originalHeight === 0) {
            const widthAttr = svgElement.getAttribute('width');
            const heightAttr = svgElement.getAttribute('height');
            if (widthAttr && heightAttr) {
              originalWidth = parseFloat(widthAttr) || 0;
              originalHeight = parseFloat(heightAttr) || 0;
            }
          }
          
          if (originalWidth === 0 || originalHeight === 0) {
            try {
              const bbox = svgElement.getBBox();
              originalWidth = bbox.width || 400;
              originalHeight = bbox.height || 300;
            } catch {
              originalWidth = 400;
              originalHeight = 300;
            }
          }
          
          svgDimensionsRef.current = { width: originalWidth, height: originalHeight };
          
          // Use fixed SVG size instead of 100%.
          svgElement.style.width = `${originalWidth}px`;
          svgElement.style.height = `${originalHeight}px`;
          svgElement.style.minWidth = `${originalWidth}px`;
          svgElement.style.minHeight = `${originalHeight}px`;
          svgElement.style.transformOrigin = '0 0';
          svgElement.style.userSelect = 'none';
          svgElement.style.flexShrink = '0';
          
          svgRef.current = svgElement;
          
          applyBaseStyles(svgElement);
          cleanupInteractionRef.current = setupInteraction(svgElement);
          
          if (isFirstRenderRef.current) {
            // Defer until container layout settles.
            requestAnimationFrame(() => {
              controls.fitToContainer(originalWidth, originalHeight);
            });
          } else {
            svgElement.style.transform = `translate(${transform.translateX}px, ${transform.translateY}px) scale(${transform.scale})`;
            svgElement.style.transition = isDragging ? 'none' : 'transform 0.1s ease-out';
          }
        }
        
        onRenderRef.current?.();
      }
      
      isFirstRenderRef.current = false;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : t('preview.renderFailed');
      onErrorRef.current?.(errorMessage);
      
      if (containerRef.current) {
        containerRef.current.innerHTML = '';
      }
      
      isFirstRenderRef.current = false;
    } finally {
      setIsLoading(false);
    }
  }, [sourceCode, transform.translateX, transform.translateY, transform.scale, isDragging, containerRef, applyBaseStyles, setupInteraction, controls]);

  useEffect(() => {
    if (lastSourceCodeRef.current !== null && lastSourceCodeRef.current === sourceCode) {
      return;
    }
    
    if (renderTimeoutRef.current) {
      clearTimeout(renderTimeoutRef.current);
    }

    renderTimeoutRef.current = setTimeout(() => {
      renderDiagram().finally(() => {
        lastSourceCodeRef.current = sourceCode;
      });
    }, RENDER_DEBOUNCE_DELAY);

    return () => {
      if (renderTimeoutRef.current) {
        clearTimeout(renderTimeoutRef.current);
      }
    };
  }, [sourceCode, renderDiagram]);

  useEffect(() => {
    if (svgRef.current) {
      svgRef.current.style.transform = `translate(${transform.translateX}px, ${transform.translateY}px) scale(${transform.scale})`;
      svgRef.current.style.transition = isDragging ? 'none' : 'transform 0.1s ease-out';
    }
  }, [transform, isDragging]);

  useEffect(() => {
    const handleThemeChange = () => {
      lastSourceCodeRef.current = null; // Force a rerender.
      renderDiagram();
    };

    window.addEventListener(MERMAID_THEME_CHANGE_EVENT, handleThemeChange);
    return () => window.removeEventListener(MERMAID_THEME_CHANGE_EVENT, handleThemeChange);
  }, [renderDiagram]);

  useEffect(() => {
    return () => {
      if (cleanupInteractionRef.current) {
        cleanupInteractionRef.current();
      }
    };
  }, []);

  return (
    <div className={`mermaid-preview ${className}`}>
      {isLoading && (
        <div className="loading-overlay">
          <CubeLoading size="small" />
          <span>{t('preview.rendering')}</span>
        </div>
      )}
      
      <div
        ref={containerRef}
        className="preview-container"
        onMouseDown={handlers.onMouseDown}
        onMouseMove={handlers.onMouseMove}
        onMouseUp={handlers.onMouseUp}
        onDoubleClick={handleFitToContainer}
        onContextMenu={(e) => e.preventDefault()}
        style={{
          opacity: isLoading ? 0.5 : 1,
          transition: 'opacity 0.2s ease',
          cursor: isDragging ? 'grabbing' : 'grab',
          overflow: 'hidden',
          position: 'relative',
        }}
      />
      
      {!sourceCode.trim() && !isLoading && (
        <div className="empty-state">
          <div className="empty-icon">Diagram</div>
          <p>{t('preview.emptyState')}</p>
        </div>
      )}

      {/* Tooltip rendered in a portal to avoid clipping. */}
      {tooltipData && createPortal(
        <MermaidTooltip
          data={tooltipData}
          position={tooltipPosition}
          capturedVarsLabel={t('preview.capturedVars')}
        />,
        document.body
      )}
    </div>
  );
}), (prevProps, nextProps) => {
  return prevProps.sourceCode === nextProps.sourceCode && 
    prevProps.isEditMode === nextProps.isEditMode &&
    prevProps.nodeMetadata === nextProps.nodeMetadata;
});

MermaidPreview.displayName = 'MermaidPreview';
