/** Git commit graph view (branch graph). */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch, ChevronUp, ChevronDown } from 'lucide-react';
import { Search } from '@/component-library';
import { gitAPI } from '@/infrastructure/api';
import type { GitGraph, GitGraphNode } from '@/infrastructure/api/service-api/GitAPI';
import { 
  GitGraphViewProps, 
  GitGraphViewConfig,
  GitGraphInteractionState 
} from '../../types/graph';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import './GitGraphView.scss';

const log = createLogger('GitGraphView');


const DEFAULT_CONFIG: GitGraphViewConfig = {
  laneWidth: 24,
  rowHeight: 40,
  nodeSize: 5,
  lineWidth: 1.5,
  colors: [
    '#60a5fa',
    '#10b981',
    '#f59e0b',
    '#8b5cf6',
    '#ef4444',
    '#06b6d4',
    '#ec4899',
    '#f97316',
  ],
  showAvatar: false,
  showRelativeTime: true,
};

export const GitGraphView: React.FC<GitGraphViewProps> = ({
  repositoryPath,
  maxCount = 1000,
  config = {},
  onCommitSelect,
  className = ''
}) => {
  const { t } = useTranslation('panels/git');
  const viewConfig = useMemo(() => ({ ...DEFAULT_CONFIG, ...config }), [config]);


  const [graphData, setGraphData] = useState<GitGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [interactionState, setInteractionState] = useState<GitGraphInteractionState>({});
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState('');
  const [currentSearchIndex, setCurrentSearchIndex] = useState(0);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const searchDebounceRef = useRef<NodeJS.Timeout | null>(null);


  const loadGraphData = useCallback(async () => {
    setLoading(true);
    setError(null);
    
    try {
      const data = await gitAPI.getGraph(repositoryPath, maxCount);
      log.debug('Graph data loaded', { nodeCount: data.nodes.length, maxLane: data.maxLane, currentBranch: data.currentBranch });
      setGraphData(data);
    } catch (err) {
      log.error('Failed to load graph data', { repositoryPath, maxCount, error: err });
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [repositoryPath, maxCount]);


  useEffect(() => {
    loadGraphData();
  }, [loadGraphData]);


  useEffect(() => {
    return () => {
      if (searchDebounceRef.current) {
        clearTimeout(searchDebounceRef.current);
      }
    };
  }, []);


  useEffect(() => {
    if (searchDebounceRef.current) {
      clearTimeout(searchDebounceRef.current);
    }

    searchDebounceRef.current = setTimeout(() => {
      setDebouncedSearchQuery(searchQuery);
    }, 200);

    return () => {
      if (searchDebounceRef.current) {
        clearTimeout(searchDebounceRef.current);
      }
    };
  }, [searchQuery]);


  const allNodesData = useMemo(() => {
    if (!graphData) {
      return { nodes: [], startIndex: 0 };
    }

    return { nodes: graphData.nodes, startIndex: 0 };
  }, [graphData]);


  const searchFilter = useMemo(() => {
    if (!debouncedSearchQuery || !graphData) return null;

    const query = debouncedSearchQuery.toLowerCase();
    const matchedIndices: number[] = [];
    const matchedIndicesSet = new Set<number>();

    graphData.nodes.forEach((node, index) => {
      if (
        node.message.toLowerCase().includes(query) ||
        node.authorName.toLowerCase().includes(query) ||
        node.hash.toLowerCase().includes(query)
      ) {
        matchedIndices.push(index);
        matchedIndicesSet.add(index);
      }
    });

    return {
      query: debouncedSearchQuery,
      matchedIndices,
      matchedIndicesSet,
      totalMatches: matchedIndices.length,
    };
  }, [debouncedSearchQuery, graphData]);


  useEffect(() => {
    setCurrentSearchIndex(0);
  }, [debouncedSearchQuery]);


  useEffect(() => {
    if (!canvasRef.current || !graphData) {
      return;
    }

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      log.error('Failed to get canvas context');
      return;
    }


    const dpr = window.devicePixelRatio || 1;
    const totalHeight = graphData.nodes.length * viewConfig.rowHeight!;
    const totalWidth = Math.max(800, (graphData.maxLane + 2) * viewConfig.laneWidth! + 600);
    
    canvas.width = totalWidth * dpr;
    canvas.height = totalHeight * dpr;
    ctx.scale(dpr, dpr);


    ctx.clearRect(0, 0, totalWidth, totalHeight);


    graphData.nodes.forEach((node, index) => {
      const y = index * viewConfig.rowHeight!;
      drawNodeWithInfo(ctx, node, y, viewConfig, { isSelected: false, isHovered: false });
    });
  }, [graphData, viewConfig]);




  const handleCommitClick = useCallback((hash: string) => {
    setInteractionState(prev => ({ ...prev, selectedHash: hash }));
    onCommitSelect?.(hash);
  }, [onCommitSelect]);


  const handleSearch = useCallback((query: string) => {
    setSearchQuery(query);
  }, []);


  const scrollToCommit = useCallback((index: number) => {
    if (!containerRef.current || !graphData) return;
    
    const rowHeight = viewConfig.rowHeight!;
    const targetScrollTop = index * rowHeight;
    const containerHeight = containerRef.current.clientHeight || 600;
    

    const centeredScrollTop = targetScrollTop - containerHeight / 2 + rowHeight / 2;
    
    containerRef.current.scrollTo({
      top: Math.max(0, centeredScrollTop),
      behavior: 'smooth'
    });


    const targetNode = graphData.nodes[index];
    if (targetNode) {
      setInteractionState(prev => ({ ...prev, selectedHash: targetNode.hash }));
    }
  }, [viewConfig.rowHeight, graphData]);


  const goToPreviousMatch = useCallback(() => {
    if (!searchFilter || searchFilter.totalMatches === 0) return;
    
    const newIndex = currentSearchIndex > 0 
      ? currentSearchIndex - 1 
      : searchFilter.totalMatches - 1;
    
    setCurrentSearchIndex(newIndex);
    scrollToCommit(searchFilter.matchedIndices[newIndex]);
  }, [searchFilter, currentSearchIndex, scrollToCommit]);


  const goToNextMatch = useCallback(() => {
    if (!searchFilter || searchFilter.totalMatches === 0) return;
    
    const newIndex = currentSearchIndex < searchFilter.totalMatches - 1 
      ? currentSearchIndex + 1 
      : 0;
    
    setCurrentSearchIndex(newIndex);
    scrollToCommit(searchFilter.matchedIndices[newIndex]);
  }, [searchFilter, currentSearchIndex, scrollToCommit]);


  if (loading) {
    return (
      <div className={`git-graph-view git-graph-view--loading ${className}`}>
        <div className="git-graph-view__loading">
          <div className="git-graph-view__spinner" />
          <p>{t('graph.loading')}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`git-graph-view git-graph-view--error ${className}`}>
        <div className="git-graph-view__error">
          <p>{t('graph.loadFailedWithMessage', { error })}</p>
          <button onClick={loadGraphData}>{t('common.retry')}</button>
        </div>
      </div>
    );
  }

  if (!graphData || graphData.nodes.length === 0) {
    return (
      <div className={`git-graph-view git-graph-view--empty ${className}`}>
        <div className="git-graph-view__empty">
          <p>{t('graph.empty')}</p>
        </div>
      </div>
    );
  }

  const graphWidth = Math.max(800, (graphData.maxLane + 2) * viewConfig.laneWidth! + 600);
  const totalHeight = graphData.nodes.length * viewConfig.rowHeight!;

  return (
    <div className={`git-graph-view ${className}`}>
      <div className="git-graph-view__header">
        <div className="git-graph-view__header-left">
          <h3>{t('graph.title')}</h3>
          {graphData.currentBranch && (
            <span className="git-graph-view__current-branch">
              <GitBranch size={14} />
              {graphData.currentBranch}
            </span>
          )}
        </div>
        
        <div 
          className="git-graph-view__search"
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              if (e.shiftKey) {
                goToPreviousMatch();
              } else {
                goToNextMatch();
              }
            }
          }}
        >
          <Search
            value={searchQuery}
            onChange={handleSearch}
            onSearch={goToNextMatch}
            placeholder={t('graph.searchPlaceholder')}
            size="small"
            clearable
            enterToSearch={false}
            loading={searchQuery !== debouncedSearchQuery}
            suffixContent={
              searchFilter && debouncedSearchQuery && searchFilter.totalMatches > 0 ? (
                <div className="git-graph-view__search-navigation">
                  <span className="git-graph-view__search-count">
                    {currentSearchIndex + 1} / {searchFilter.totalMatches}
                  </span>
                  <button
                    className="git-graph-view__search-nav-btn"
                    onClick={goToPreviousMatch}
                    title={t('graph.searchPrevious')}
                    disabled={searchFilter.totalMatches === 0}
                  >
                    <ChevronUp size={14} />
                  </button>
                  <button
                    className="git-graph-view__search-nav-btn"
                    onClick={goToNextMatch}
                    title={t('graph.searchNext')}
                    disabled={searchFilter.totalMatches === 0}
                  >
                    <ChevronDown size={14} />
                  </button>
                </div>
              ) : searchFilter && debouncedSearchQuery && searchFilter.totalMatches === 0 ? (
                <span className="git-graph-view__search-count git-graph-view__search-count--no-results">
                  {t('graph.noResults')}
                </span>
              ) : null
            }
          />
        </div>
      </div>

      <div 
        className="git-graph-view__content" 
        ref={containerRef}
      >
        <div style={{ height: totalHeight, width: graphWidth, position: 'relative' }}>
          <canvas
            ref={canvasRef}
            className="git-graph-view__canvas-full"
            style={{ 
              width: graphWidth,
              height: totalHeight,
              position: 'absolute',
              top: 0,
              left: 0,
              pointerEvents: 'none'
            }}
          />
          
          {allNodesData.nodes.map((node, index) => {

            const absoluteIndex = allNodesData.startIndex + index;
            const isSelected = interactionState.selectedHash === node.hash;

            const isSearchMatch = searchFilter ? searchFilter.matchedIndicesSet.has(absoluteIndex) : false;
            const isCurrentSearchMatch = searchFilter ? 
              searchFilter.matchedIndices[currentSearchIndex] === absoluteIndex : false;
            
            return (
              <HitArea
                key={node.hash}
                node={node}
                top={absoluteIndex * viewConfig.rowHeight!}
                height={viewConfig.rowHeight!}
                width={graphWidth}
                isSelected={isSelected}
                isSearchMatch={isSearchMatch}
                isCurrentSearchMatch={isCurrentSearchMatch}
                onClick={handleCommitClick}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
};

/** Draws a rounded rectangle (fallback for browsers without `ctx.roundRect`). */
function drawRoundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number
) {

  if (typeof ctx.roundRect === 'function') {
    ctx.roundRect(x, y, width, height, radius);
    return;
  }
  

  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.lineTo(x + width - radius, y);
  ctx.arcTo(x + width, y, x + width, y + radius, radius);
  ctx.lineTo(x + width, y + height - radius);
  ctx.arcTo(x + width, y + height, x + width - radius, y + height, radius);
  ctx.lineTo(x + radius, y + height);
  ctx.arcTo(x, y + height, x, y + height - radius, radius);
  ctx.lineTo(x, y + radius);
  ctx.arcTo(x, y, x + radius, y, radius);
  ctx.closePath();
}

/** Draws a graph node and its associated metadata row. */
function drawNodeWithInfo(
  ctx: CanvasRenderingContext2D,
  node: GitGraphNode,
  y: number,
  config: GitGraphViewConfig,
  state: { isSelected: boolean; isHovered: boolean }
) {
  const laneWidth = config.laneWidth!;
  const nodeSize = config.nodeSize!;
  const lineWidth = config.lineWidth!;
  const colors = config.colors!;
  const rowHeight = config.rowHeight!;

  const x = (node.lane + 1) * laneWidth;
  const centerY = y + rowHeight / 2;
  const color = colors[node.lane % colors.length];




  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';


  if (node.parents.length > 0) {
    ctx.strokeStyle = color;
    ctx.lineWidth = lineWidth;
    ctx.beginPath();
    ctx.moveTo(x, centerY);
    ctx.lineTo(x, y + rowHeight);
    ctx.stroke();
  }


  if (node.children.length > 0) {
    ctx.strokeStyle = color;
    ctx.lineWidth = lineWidth;
    ctx.beginPath();
    ctx.moveTo(x, centerY);
    ctx.lineTo(x, y);
    ctx.stroke();
  }


  node.forkingLanes.forEach(forkLane => {
    const forkX = (forkLane + 1) * laneWidth;
    const forkColor = colors[forkLane % colors.length];
    
    ctx.strokeStyle = forkColor;
    ctx.lineWidth = lineWidth;
    ctx.beginPath();
    ctx.moveTo(x, centerY);
    

    const controlY = centerY - (centerY - y) / 2;
    ctx.bezierCurveTo(x, controlY, forkX, controlY, forkX, y);
    ctx.stroke();
  });


  node.mergingLanes.forEach(mergeLane => {
    const mergeX = (mergeLane + 1) * laneWidth;
    const mergeColor = colors[mergeLane % colors.length];
    
    ctx.strokeStyle = mergeColor;
    ctx.lineWidth = lineWidth;
    ctx.beginPath();
    ctx.moveTo(mergeX, y + rowHeight);
    

    const controlY = centerY + (y + rowHeight - centerY) / 2;
    ctx.bezierCurveTo(mergeX, controlY, x, controlY, x, centerY);
    ctx.stroke();
  });


  node.passingLanes.forEach(passLane => {
    const passX = (passLane + 1) * laneWidth;
    const passColor = colors[passLane % colors.length];
    
    ctx.strokeStyle = passColor;
    ctx.lineWidth = lineWidth;
    ctx.beginPath();
    ctx.moveTo(passX, y);
    ctx.lineTo(passX, y + rowHeight);
    ctx.stroke();
  });


  ctx.save();
  

  ctx.fillStyle = color;
  ctx.beginPath();
  ctx.arc(x, centerY, nodeSize, 0, Math.PI * 2);
  ctx.fill();
  

  if (state.isSelected) {
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(x, centerY, nodeSize + 1.5, 0, Math.PI * 2);
    ctx.stroke();
  }
  
  ctx.restore();


  let refX = x + nodeSize + 8;

  const seenRefs = new Set<string>();
  
  node.refs.forEach((ref) => {

    if (!ref.name || ref.name.trim().length === 0) {
      return;
    }
    

    let displayName = ref.name.trim();
    

    if (displayName.startsWith('HEAD -> ')) {
      displayName = displayName.substring('HEAD -> '.length).trim();
    }
    

    if (displayName.startsWith('refs/heads/')) {
      displayName = displayName.substring('refs/heads/'.length);
    }
    

    if (displayName.startsWith('refs/tags/')) {
      displayName = displayName.substring('refs/tags/'.length);
    }
    

    if (displayName.startsWith('refs/remotes/')) {
      displayName = displayName.substring('refs/remotes/'.length);
    }
    

    if (!displayName || displayName.length === 0) {
      return;
    }
    

    const refKey = `${ref.refType}:${displayName}`;
    if (seenRefs.has(refKey)) {
      return;
    }
    seenRefs.add(refKey);
    

    const bgColor = ref.refType === 'branch' ? '#60a5fa' : ref.refType === 'tag' ? '#f59e0b' : '#8b5cf6';
    const text = displayName;
    
    ctx.font = '11px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
    const textWidth = ctx.measureText(text).width;
    const padding = 5;
    const refWidth = textWidth + padding * 2;
    const refHeight = 18;
    

    ctx.save();
    ctx.fillStyle = bgColor;
    drawRoundRect(ctx, refX, centerY - refHeight / 2, refWidth, refHeight, 3);
    ctx.fill();
    ctx.restore();
    

    if (ref.isCurrent) {
      ctx.save();
      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.arc(refX + refWidth - 4, centerY - refHeight / 2 + 4, 2, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }
    

    ctx.save();
    ctx.fillStyle = '#ffffff';
    ctx.font = '500 11px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
    ctx.textBaseline = 'middle';
    ctx.textAlign = 'left';
    ctx.fillText(text, refX + padding, centerY);
    ctx.restore();
    
    refX += refWidth + 6;
  });


  const textX = refX + 12;
  const maxTextWidth = 240;
  
  ctx.save();
  

  ctx.fillStyle = '#d1d5db';
  ctx.font = '13px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'left';
  
  let displayText = node.message;
  const textWidth = ctx.measureText(displayText).width;
  
  if (textWidth > maxTextWidth) {
    while (ctx.measureText(displayText + '…').width > maxTextWidth && displayText.length > 0) {
      displayText = displayText.substring(0, displayText.length - 1);
    }
    displayText += '…';
  }
  
  ctx.fillText(displayText, textX, centerY);
  

  const metaX = textX + ctx.measureText(displayText).width + 20;


  ctx.fillStyle = '#6b7280';
  ctx.font = '12px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
  
  let authorName = node.authorName;
  const maxAuthorWidth = 120;
  if (ctx.measureText(authorName).width > maxAuthorWidth) {
    while (ctx.measureText(authorName + '…').width > maxAuthorWidth && authorName.length > 0) {
      authorName = authorName.substring(0, authorName.length - 1);
    }
    authorName += '…';
  }
  
  const timeText = formatRelativeTime(node.timestamp);
  const metaText = `${authorName} · ${timeText}`;
  ctx.fillText(metaText, metaX, centerY);
  

  const hashX = metaX + ctx.measureText(metaText).width + 16;
  const hashText = node.hash.substring(0, 7);
  
  ctx.fillStyle = '#4b5563';
  ctx.font = '11px "SF Mono", "Monaco", "Courier New", monospace';
  ctx.fillText(hashText, hashX, centerY);
  
  ctx.restore();
}

function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000;
  const diff = now - timestamp;
  
  if (diff < 60) return i18nService.t('panels/git:relativeTime.justNow');
  if (diff < 3600) {
    return i18nService.t('panels/git:relativeTime.minutesAgo', { count: Math.floor(diff / 60) });
  }
  if (diff < 86400) {
    return i18nService.t('panels/git:relativeTime.hoursAgo', { count: Math.floor(diff / 3600) });
  }
  if (diff < 604800) {
    return i18nService.t('panels/git:relativeTime.daysAgo', { count: Math.floor(diff / 86400) });
  }
  if (diff < 2592000) {
    return i18nService.t('panels/git:relativeTime.weeksAgo', { count: Math.floor(diff / 604800) });
  }
  if (diff < 31536000) {
    return i18nService.t('panels/git:relativeTime.monthsAgo', { count: Math.floor(diff / 2592000) });
  }
  return i18nService.t('panels/git:relativeTime.yearsAgo', { count: Math.floor(diff / 31536000) });
}

/**
 * Memoized hit area to avoid unnecessary re-renders.
 * Hover styling is handled via CSS (no React hover state).
 */
interface HitAreaProps {
  node: GitGraphNode;
  top: number;
  height: number;
  width: number;
  isSelected: boolean;
  isSearchMatch?: boolean;
  isCurrentSearchMatch?: boolean;
  onClick: (hash: string) => void;
}

const HitArea = React.memo<HitAreaProps>(({ 
  node, 
  top, 
  height, 
  width, 
  isSelected,
  isSearchMatch = false,
  isCurrentSearchMatch = false,
  onClick
}) => {
  const handleClick = useCallback(() => onClick(node.hash), [onClick, node.hash]);
  
  const tooltipText = useMemo(() => {
    const refsLabel = i18nService.t('panels/git:graph.tooltip.refs');
    const currentSuffix = i18nService.t('panels/git:graph.tooltip.currentRefSuffix');
    const refsText = node.refs.length > 0
      ? `${refsLabel}\n` + node.refs.map(r => `  - ${r.refType}: ${r.name}${r.isCurrent ? ` ${currentSuffix}` : ''}`).join('\n')
      : '';

    const lines = [
      node.message,
      '',
      `${i18nService.t('panels/git:graph.tooltip.author')}: ${node.authorName} <${node.authorEmail}>`,
      `${i18nService.t('panels/git:graph.tooltip.hash')}: ${node.hash}`,
      `${i18nService.t('panels/git:graph.tooltip.time')}: ${i18nService.formatDate(node.timestamp * 1000)}`
    ];

    if (refsText) {
      lines.push('', refsText);
    }

    return lines.join('\n');
  }, [node]);
  
  const className = [
    'git-graph-view__hit-area',
    isSelected && 'git-graph-view__hit-area--selected',
    isSearchMatch && 'git-graph-view__hit-area--search-match',
    isCurrentSearchMatch && 'git-graph-view__hit-area--current-search-match'
  ].filter(Boolean).join(' ');
  
  return (
    <div
      className={className}
      style={{
        position: 'absolute',
        top,
        height,
        left: 0,
        width,
        cursor: 'pointer',
        zIndex: isCurrentSearchMatch ? 3 : isSelected ? 2 : 1,
      }}
      onClick={handleClick}
      title={tooltipText}
    />
  );
}, (prev, next) => {

  return (
    prev.node.hash === next.node.hash &&
    prev.top === next.top &&
    prev.isSelected === next.isSelected &&
    prev.isSearchMatch === next.isSearchMatch &&
    prev.isCurrentSearchMatch === next.isCurrentSearchMatch
  );
});

HitArea.displayName = 'HitArea';

export default GitGraphView;
