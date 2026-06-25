import {
  type MermaidThemeFallbackKey,
  type MermaidThemeMode,
  getMermaidThemeFallback,
} from './mermaidThemeFallbacks';

/**
 * Mermaid theme config builder.
 * Reads CSS variables and supports live theme switching.
 */
export const MERMAID_THEME_CHANGE_EVENT = 'mermaid-theme-changed';

/**
 * Read a CSS variable with a fallback.
 */
function getCSSVar(name: string, fallback: string = ''): string {
  if (typeof document === 'undefined') return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

/**
 * Resolve the current theme type.
 */
export function getThemeType(): 'dark' | 'light' {
  if (typeof document === 'undefined') return 'dark';
  const themeType = document.documentElement.getAttribute('data-theme-type');
  if (themeType === 'light' || themeType === 'dark') {
    return themeType;
  }
  const dataTheme = document.documentElement.getAttribute('data-theme');
  if (dataTheme?.includes('light')) return 'light';
  if (dataTheme?.includes('dark')) return 'dark';
  if (document.documentElement.classList.contains('light')) return 'light';
  if (typeof window.matchMedia !== 'function') return 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/**
 * Build Mermaid themeVariables from CSS variables.
 * Uses theme-aware fallbacks when variables are missing.
 */
function getThemeVariables() {
  const isDark = getThemeType() === 'dark';
  const mode: MermaidThemeMode = isDark ? 'dark' : 'light';
  const fallback = (key: MermaidThemeFallbackKey) => getMermaidThemeFallback(mode, key);

  return {
    primaryColor: getCSSVar('--mermaid-node-fill', fallback('nodeFill')),
    primaryTextColor: getCSSVar('--mermaid-node-text', fallback('nodeText')),
    primaryBorderColor: getCSSVar('--mermaid-node-stroke', fallback('nodeStroke')),
    secondaryColor: getCSSVar('--mermaid-node-fill-hover', fallback('nodeFillHover')),
    secondaryTextColor: getCSSVar('--mermaid-node-text', fallback('nodeText')),
    secondaryBorderColor: getCSSVar('--mermaid-node-stroke-hover', fallback('nodeStrokeHover')),
    tertiaryColor: getCSSVar('--mermaid-cluster-fill', fallback('clusterFill')),
    tertiaryTextColor: getCSSVar('--mermaid-cluster-text', fallback('clusterText')),
    tertiaryBorderColor: getCSSVar('--mermaid-cluster-stroke', fallback('nodeStrokeMuted')),
    background: 'transparent',
    mainBkg: getCSSVar('--mermaid-node-fill', fallback('nodeFill')),
    secondBkg: getCSSVar('--mermaid-node-fill-hover', fallback('nodeFillHover')),
    textColor: getCSSVar('--mermaid-node-text', fallback('nodeText')),
    nodeTextColor: getCSSVar('--mermaid-node-text', fallback('nodeText')),
    lineColor: getCSSVar('--mermaid-edge-stroke', fallback('edgeStroke')),
    border1: getCSSVar('--mermaid-node-stroke', fallback('nodeBorder')),
    border2: getCSSVar('--mermaid-cluster-stroke', fallback('nodeStrokeSubtle')),
    nodeBkg: getCSSVar('--mermaid-node-fill', fallback('nodeFill')),
    nodeBorder: getCSSVar('--mermaid-node-stroke', fallback('nodeBorder')),
    clusterBkg: getCSSVar('--mermaid-cluster-fill', fallback('clusterFillRuntime')),
    clusterBorder: getCSSVar('--mermaid-cluster-stroke', fallback('nodeStrokeMuted')),
    arrowheadColor: getCSSVar('--mermaid-arrow-color', fallback('arrow')),
    edgeLabelBackground: getCSSVar('--mermaid-edge-label-bg', fallback('edgeLabelBg')),
    noteBkgColor: getCSSVar('--mermaid-note-fill', fallback('noteFill')),
    noteTextColor: getCSSVar('--mermaid-note-text', fallback('noteText')),
    noteBorderColor: getCSSVar('--mermaid-note-stroke', fallback('noteStroke')),
    activationBkgColor: getCSSVar('--mermaid-activation-fill', fallback('activationFill')),
    activationBorderColor: getCSSVar('--mermaid-activation-stroke', fallback('activationStroke')),
    actorBkg: getCSSVar('--mermaid-actor-fill', fallback('nodeFill')),
    actorBorder: getCSSVar('--mermaid-actor-stroke', fallback('nodeStroke')),
    actorTextColor: getCSSVar('--mermaid-actor-text', fallback('nodeText')),
    actorLineColor: getCSSVar('--mermaid-signal-stroke', fallback('nodeBorder')),
    signalColor: getCSSVar('--mermaid-signal-stroke', fallback('nodeStrokeHover')),
    signalTextColor: getCSSVar('--mermaid-signal-text', fallback('nodeText')),
    labelBoxBkgColor: getCSSVar('--mermaid-edge-label-bg', fallback('edgeLabelBgStrong')),
    labelBoxBorderColor: getCSSVar('--mermaid-edge-label-border', fallback('nodeStrokeMuted')),
    labelTextColor: getCSSVar('--mermaid-edge-label-text', fallback('clusterText')),
    loopTextColor: getCSSVar('--mermaid-edge-label-text', fallback('clusterText')),
    sectionBkgColor: getCSSVar('--mermaid-section-fill', fallback('sectionFill')),
    altSectionBkgColor: getCSSVar('--mermaid-section-alt-fill', fallback('sectionAltFill')),
    gridColor: getCSSVar('--mermaid-grid-stroke', fallback('gridStroke')),
    doneTaskBkgColor: getCSSVar('--mermaid-done-fill', fallback('doneFill')),
    doneTaskBorderColor: getCSSVar('--mermaid-done-stroke', fallback('doneStroke')),
    activeTaskBkgColor: getCSSVar('--mermaid-active-fill', fallback('activeFill')),
    activeTaskBorderColor: getCSSVar('--mermaid-active-stroke', fallback('activeStroke')),
    critBkgColor: getCSSVar('--mermaid-crit-fill', fallback('critFill')),
    critBorderColor: getCSSVar('--mermaid-crit-stroke', fallback('critStroke')),
    taskTextColor: getCSSVar('--mermaid-task-text', fallback('nodeText')),
    taskTextOutsideColor: getCSSVar('--mermaid-edge-label-text', fallback('clusterText')),
    taskTextClickableColor: getCSSVar('--mermaid-info', fallback('taskClickableInfo')),
    classText: getCSSVar('--mermaid-class-text', fallback('nodeText')),
    labelColor: getCSSVar('--mermaid-node-text', fallback('nodeText')),
    pie1: getCSSVar('--mermaid-pie-1', fallback('taskClickableInfo')),
    pie2: getCSSVar('--mermaid-pie-2', fallback('doneStroke')),
    pie3: getCSSVar('--mermaid-pie-3', fallback('warning')),
    pie4: getCSSVar('--mermaid-pie-4', fallback('critStroke')),
    pie5: getCSSVar('--mermaid-pie-5', fallback('pie5')),
    pie6: getCSSVar('--mermaid-pie-6', fallback('pie6')),
    pie7: getCSSVar('--mermaid-pie-7', fallback('pie7')),
    pie8: getCSSVar('--mermaid-pie-8', fallback('pie8')),
    pieTitleTextSize: '16px',
    pieTitleTextColor: getCSSVar('--mermaid-pie-title-text', fallback('pieTitleText')),
    pieSectionTextSize: '12px',
    pieSectionTextColor: getCSSVar('--mermaid-pie-title-text', fallback('pieTitleText')),
    pieLegendTextSize: '12px',
    pieLegendTextColor: getCSSVar('--mermaid-pie-legend-text', fallback('pieLegendText')),
    pieStrokeColor: getCSSVar('--mermaid-pie-stroke', fallback('pieStroke')),
    pieStrokeWidth: getCSSVar('--mermaid-pie-stroke-width', '2px'),
    pieOuterStrokeWidth: '2px',
    pieOuterStrokeColor: getCSSVar('--mermaid-node-stroke', fallback('nodeBorder')),
    pieOpacity: '0.9',
    errorBkgColor: getCSSVar('--mermaid-error-bg', fallback('errorFill')),
    errorTextColor: getCSSVar('--mermaid-error', fallback('error')),
    fontFamily: '"Inter", "Segoe UI", -apple-system, BlinkMacSystemFont, sans-serif',
  };
}

/**
 * Build the full Mermaid config.
 */
export function getMermaidConfig() {
  const isDark = getThemeType() === 'dark';
  return {
    theme: 'base' as const,
    darkMode: isDark,
    themeVariables: getThemeVariables(),
    flowchart: {
      useMaxWidth: true,
      htmlLabels: true,
      curve: 'basis',
      padding: 16,
      nodeSpacing: 60,
      rankSpacing: 60,
      diagramPadding: 16,
      defaultRenderer: 'dagre-wrapper',
      wrappingWidth: 200,
    },
    sequence: {
      diagramMarginX: 40,
      diagramMarginY: 16,
      actorMargin: 60,
      width: 160,
      height: 60,
      boxMargin: 12,
      boxTextMargin: 8,
      noteMargin: 12,
      messageMargin: 40,
      mirrorActors: true,
      bottomMarginAdj: 1,
      useMaxWidth: true,
      rightAngles: false,
      showSequenceNumbers: false,
      wrap: true,
      wrapPadding: 12,
    },
    gantt: {
      titleTopMargin: 20,
      barHeight: 24,
      barGap: 6,
      topPadding: 40,
      leftPadding: 80,
      gridLineStartPadding: 40,
      fontSize: 12,
      fontFamily: '"Inter", "Segoe UI", sans-serif',
      numberSectionStyles: 4,
      useWidth: 960,
    },
    pie: {
      useWidth: 600,
      useMaxWidth: true,
      textPosition: 0.75,
    },
    state: {
      dividerMargin: 12,
      sizeUnit: 8,
      padding: 10,
      textHeight: 12,
      titleShift: -20,
      noteMargin: 12,
      forkWidth: 80,
      forkHeight: 8,
      miniPadding: 4,
      fontSizeFactor: 5.02,
      fontSize: 20,
      labelHeight: 20,
      edgeLengthFactor: '24',
      compositTitleSize: 40,
      radius: 6,
      defaultRenderer: 'dagre-wrapper',
    },
    class: {
      useMaxWidth: true,
      defaultRenderer: 'dagre-wrapper',
    },
    er: {
      diagramPadding: 24,
      layoutDirection: 'TB',
      minEntityWidth: 120,
      minEntityHeight: 80,
      entityPadding: 16,
      stroke: 'gray',
      fill: 'honeydew',
      fontSize: 13,
      useMaxWidth: true,
    },
    gitGraph: {
      showBranches: true,
      showCommitLabel: true,
      mainBranchName: 'main',
      mainBranchOrder: 0,
      rotateCommitLabel: true,
    },
  };
}

/**
 * Watch for theme changes and run the callback.
 * Returns a cleanup function.
 */
export function setupThemeListener(callback: () => void): () => void {
  if (typeof document === 'undefined') return () => {};
  let lastThemeType = getThemeType();
  const observer = new MutationObserver(() => {
    const currentTheme = getThemeType();
    if (lastThemeType !== currentTheme) {
      lastThemeType = currentTheme;
      window.dispatchEvent(new CustomEvent(MERMAID_THEME_CHANGE_EVENT, {
        detail: { theme: currentTheme }
      }));
      callback();
    }
  });

  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme', 'data-theme-type', 'class']
  });

  return () => observer.disconnect();
}

/**
 * Runtime color overrides for SVG rendering.
 */
export function getRuntimeColors() {
  const isDark = getThemeType() === 'dark';
  const mode: MermaidThemeMode = isDark ? 'dark' : 'light';
  const fallback = (key: MermaidThemeFallbackKey) => getMermaidThemeFallback(mode, key);

  return {
    node: {
      // Use softer light backgrounds to avoid pure white.
      fill: getCSSVar('--mermaid-node-fill', fallback('nodeFillRuntime')),
      fillHover: getCSSVar('--mermaid-node-fill-hover', fallback('nodeFillHoverRuntime')),
      stroke: getCSSVar('--mermaid-node-stroke', fallback('nodeStroke')),
      strokeHover: getCSSVar('--mermaid-node-stroke-hover', fallback('nodeStrokeHoverStrong')),
      // Keep text dark in light theme.
      text: getCSSVar('--mermaid-node-text', fallback('nodeText')),
      dashArray: getCSSVar('--mermaid-node-dash-array', '4 2'),
    },
    cluster: {
      fill: getCSSVar('--mermaid-cluster-fill', fallback('clusterFillRuntime')),
      fillHover: fallback('clusterFillHover'),
      stroke: getCSSVar('--mermaid-cluster-stroke', fallback('nodeStrokeMuted')),
      strokeHover: fallback('nodeStrokeHover'),
      dashArray: getCSSVar('--mermaid-cluster-dash-array', '5 3'),
    },
    edgeLabel: {
      fill: getCSSVar('--mermaid-edge-label-bg', fallback('edgeLabelBgRuntime')),
      fillHover: fallback('edgeLabelBgHover'),
      stroke: getCSSVar('--mermaid-edge-label-border', fallback('nodeStrokeSubtle')),
      strokeHover: fallback('edgeLabelBorderHover'),
    },
    edge: {
      stroke: getCSSVar('--mermaid-edge-stroke', fallback('edgeStroke')),
      strokeHover: getCSSVar('--mermaid-edge-stroke-hover', fallback('nodeStrokeHoverStrong')),
    },
    highlight: {
      stroke: getCSSVar('--mermaid-highlight-stroke', fallback('highlightStroke')),
      glow: getCSSVar('--mermaid-highlight-glow', fallback('highlightGlow')),
      glowStrong: fallback('highlightGlowStrong'),
    },
    status: {
      success: getCSSVar('--mermaid-success', fallback('doneStroke')),
      error: getCSSVar('--mermaid-error', fallback('error')),
      warning: getCSSVar('--mermaid-warning', fallback('warning')),
      info: getCSSVar('--mermaid-info', fallback('info')),
    },
    text: {
      primary: getCSSVar('--mermaid-node-text', fallback('nodeText')),
      secondary: getCSSVar('--mermaid-edge-label-text', fallback('clusterText')),
      muted: fallback('textMuted'),
      highlight: fallback('nodeTextStrong'),
    },
  };
}
