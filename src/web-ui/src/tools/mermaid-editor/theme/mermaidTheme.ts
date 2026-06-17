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
  return {
    primaryColor: getCSSVar('--mermaid-node-fill', isDark ? '#1c1e23' : '#e8eaef'),
    primaryTextColor: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
    primaryBorderColor: getCSSVar('--mermaid-node-stroke', isDark ? '#5a5e6a' : '#9ca3af'),
    secondaryColor: getCSSVar('--mermaid-node-fill-hover', isDark ? '#262830' : '#dfe2e8'),
    secondaryTextColor: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
    secondaryBorderColor: getCSSVar('--mermaid-node-stroke-hover', isDark ? '#6a6e7a' : '#6b7280'),
    tertiaryColor: getCSSVar('--mermaid-cluster-fill', isDark ? '#16181c' : 'rgba(229, 231, 235, 0.7)'),
    tertiaryTextColor: getCSSVar('--mermaid-cluster-text', isDark ? '#9a9ea8' : '#4b5563'),
    tertiaryBorderColor: getCSSVar('--mermaid-cluster-stroke', isDark ? '#4a4e58' : '#d1d5db'),
    background: 'transparent',
    mainBkg: getCSSVar('--mermaid-node-fill', isDark ? '#1c1e23' : '#e8eaef'),
    secondBkg: getCSSVar('--mermaid-node-fill-hover', isDark ? '#262830' : '#dfe2e8'),
    textColor: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
    nodeTextColor: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
    lineColor: getCSSVar('--mermaid-edge-stroke', isDark ? '#5a5e6a' : '#9ca3af'),
    border1: getCSSVar('--mermaid-node-stroke', isDark ? '#4a4e58' : '#9ca3af'),
    border2: getCSSVar('--mermaid-cluster-stroke', isDark ? '#3a3e48' : '#d1d5db'),
    nodeBkg: getCSSVar('--mermaid-node-fill', isDark ? '#1c1e23' : '#e8eaef'),
    nodeBorder: getCSSVar('--mermaid-node-stroke', isDark ? '#4a4e58' : '#9ca3af'),
    clusterBkg: getCSSVar('--mermaid-cluster-fill', isDark ? 'rgba(24, 26, 30, 0.6)' : 'rgba(229, 231, 235, 0.7)'),
    clusterBorder: getCSSVar('--mermaid-cluster-stroke', isDark ? '#4a4e58' : '#d1d5db'),
    arrowheadColor: getCSSVar('--mermaid-arrow-color', isDark ? '#7a7e8a' : '#6b7280'),
    edgeLabelBackground: getCSSVar('--mermaid-edge-label-bg', isDark ? '#1a1c20' : '#f3f4f6'),
    noteBkgColor: getCSSVar('--mermaid-note-fill', isDark ? '#222428' : '#fef3c7'),
    noteTextColor: getCSSVar('--mermaid-note-text', isDark ? '#9a9ea8' : '#92400e'),
    noteBorderColor: getCSSVar('--mermaid-note-stroke', isDark ? '#4a4e58' : '#f59e0b'),
    activationBkgColor: getCSSVar('--mermaid-activation-fill', isDark ? '#2a2c32' : 'rgba(147, 197, 253, 0.25)'),
    activationBorderColor: getCSSVar('--mermaid-activation-stroke', isDark ? '#5a5e6a' : '#93c5fd'),
    actorBkg: getCSSVar('--mermaid-actor-fill', isDark ? '#1c1e23' : '#e8eaef'),
    actorBorder: getCSSVar('--mermaid-actor-stroke', isDark ? '#5a5e6a' : '#9ca3af'),
    actorTextColor: getCSSVar('--mermaid-actor-text', isDark ? '#e0e2e8' : '#1f2937'),
    actorLineColor: getCSSVar('--mermaid-signal-stroke', isDark ? '#4a4e58' : '#9ca3af'),
    signalColor: getCSSVar('--mermaid-signal-stroke', isDark ? '#6a6e7a' : '#9ca3af'),
    signalTextColor: getCSSVar('--mermaid-signal-text', isDark ? '#e0e2e8' : '#1f2937'),
    labelBoxBkgColor: getCSSVar('--mermaid-edge-label-bg', isDark ? '#262830' : '#f3f4f6'),
    labelBoxBorderColor: getCSSVar('--mermaid-edge-label-border', isDark ? '#4a4e58' : '#d1d5db'),
    labelTextColor: getCSSVar('--mermaid-edge-label-text', isDark ? '#9a9ea8' : '#4b5563'),
    loopTextColor: getCSSVar('--mermaid-edge-label-text', isDark ? '#9a9ea8' : '#4b5563'),
    sectionBkgColor: getCSSVar('--mermaid-section-fill', isDark ? '#1c1e23' : '#f3f4f6'),
    altSectionBkgColor: getCSSVar('--mermaid-section-alt-fill', isDark ? '#262830' : '#e5e7eb'),
    gridColor: getCSSVar('--mermaid-grid-stroke', isDark ? '#3a3e48' : 'rgba(156, 163, 175, 0.3)'),
    doneTaskBkgColor: getCSSVar('--mermaid-done-fill', isDark ? 'rgba(109, 212, 160, 0.15)' : 'rgba(34, 197, 94, 0.2)'),
    doneTaskBorderColor: getCSSVar('--mermaid-done-stroke', isDark ? '#6dd4a0' : '#16a34a'),
    activeTaskBkgColor: getCSSVar('--mermaid-active-fill', isDark ? 'rgba(120, 168, 216, 0.15)' : 'rgba(15, 23, 42, 0.08)'),
    activeTaskBorderColor: getCSSVar('--mermaid-active-stroke', isDark ? '#78a8d8' : '#334155'),
    critBkgColor: getCSSVar('--mermaid-crit-fill', isDark ? 'rgba(232, 120, 120, 0.15)' : 'rgba(239, 68, 68, 0.2)'),
    critBorderColor: getCSSVar('--mermaid-crit-stroke', isDark ? '#e87878' : '#dc2626'),
    taskTextColor: getCSSVar('--mermaid-task-text', isDark ? '#e0e2e8' : '#1f2937'),
    taskTextOutsideColor: getCSSVar('--mermaid-edge-label-text', isDark ? '#9a9ea8' : '#4b5563'),
    taskTextClickableColor: getCSSVar('--mermaid-info', isDark ? '#78a8d8' : '#475569'),
    classText: getCSSVar('--mermaid-class-text', isDark ? '#e0e2e8' : '#1f2937'),
    labelColor: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
    pie1: getCSSVar('--mermaid-pie-1', isDark ? '#78a8d8' : '#475569'),
    pie2: getCSSVar('--mermaid-pie-2', isDark ? '#6dd4a0' : '#16a34a'),
    pie3: getCSSVar('--mermaid-pie-3', isDark ? '#e8b060' : '#f59e0b'),
    pie4: getCSSVar('--mermaid-pie-4', isDark ? '#e87878' : '#dc2626'),
    pie5: getCSSVar('--mermaid-pie-5', isDark ? '#a090d8' : '#8b5cf6'),
    pie6: getCSSVar('--mermaid-pie-6', isDark ? '#d890b8' : '#ec4899'),
    pie7: getCSSVar('--mermaid-pie-7', isDark ? '#68c8d8' : '#06b6d4'),
    pie8: getCSSVar('--mermaid-pie-8', isDark ? '#a8d868' : '#84cc16'),
    pieTitleTextSize: '16px',
    pieTitleTextColor: getCSSVar('--mermaid-pie-title-text', isDark ? '#e0e2e8' : '#111827'),
    pieSectionTextSize: '12px',
    pieSectionTextColor: getCSSVar('--mermaid-pie-title-text', isDark ? '#e0e2e8' : '#111827'),
    pieLegendTextSize: '12px',
    pieLegendTextColor: getCSSVar('--mermaid-pie-legend-text', isDark ? '#9a9ea8' : '#374151'),
    pieStrokeColor: getCSSVar('--mermaid-pie-stroke', isDark ? '#1c1e23' : '#f9fafb'),
    pieStrokeWidth: getCSSVar('--mermaid-pie-stroke-width', '2px'),
    pieOuterStrokeWidth: '2px',
    pieOuterStrokeColor: getCSSVar('--mermaid-node-stroke', isDark ? '#4a4e58' : '#9ca3af'),
    pieOpacity: '0.9',
    errorBkgColor: getCSSVar('--mermaid-error-bg', isDark ? 'rgba(232, 120, 120, 0.12)' : 'rgba(239, 68, 68, 0.15)'),
    errorTextColor: getCSSVar('--mermaid-error', isDark ? '#e87878' : '#dc2626'),
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
  return {
    node: {
      // Use softer light backgrounds to avoid pure white.
      fill: getCSSVar('--mermaid-node-fill', isDark ? 'rgba(28, 30, 35, 0.9)' : '#e8eaef'),
      fillHover: getCSSVar('--mermaid-node-fill-hover', isDark ? 'rgba(38, 40, 48, 0.95)' : '#dfe2e8'),
      stroke: getCSSVar('--mermaid-node-stroke', isDark ? '#5a5e6a' : '#9ca3af'),
      strokeHover: getCSSVar('--mermaid-node-stroke-hover', isDark ? '#8a8e9a' : '#6b7280'),
      // Keep text dark in light theme.
      text: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
      dashArray: getCSSVar('--mermaid-node-dash-array', '4 2'),
    },
    cluster: {
      fill: getCSSVar('--mermaid-cluster-fill', isDark ? 'rgba(24, 26, 30, 0.6)' : 'rgba(229, 231, 235, 0.7)'),
      fillHover: isDark ? 'rgba(34, 36, 42, 0.7)' : 'rgba(209, 213, 219, 0.8)',
      stroke: getCSSVar('--mermaid-cluster-stroke', isDark ? '#4a4e58' : '#d1d5db'),
      strokeHover: isDark ? '#6a6e7a' : '#9ca3af',
      dashArray: getCSSVar('--mermaid-cluster-dash-array', '5 3'),
    },
    edgeLabel: {
      fill: getCSSVar('--mermaid-edge-label-bg', isDark ? 'rgba(26, 28, 32, 0.95)' : '#f3f4f6'),
      fillHover: isDark ? 'rgba(36, 38, 45, 0.98)' : '#e5e7eb',
      stroke: getCSSVar('--mermaid-edge-label-border', isDark ? '#3a3e48' : '#d1d5db'),
      strokeHover: isDark ? '#5a5e68' : '#9ca3af',
    },
    edge: {
      stroke: getCSSVar('--mermaid-edge-stroke', isDark ? '#5a5e6a' : '#9ca3af'),
      strokeHover: getCSSVar('--mermaid-edge-stroke-hover', isDark ? '#8a8e9a' : '#6b7280'),
    },
    highlight: {
      stroke: getCSSVar('--mermaid-highlight-stroke', isDark ? '#a8acb8' : '#334155'),
      glow: getCSSVar('--mermaid-highlight-glow', isDark 
        ? 'drop-shadow(0 0 6px rgba(168, 172, 184, 0.4))' 
        : 'drop-shadow(0 0 6px rgba(15, 23, 42, 0.18))'),
      glowStrong: isDark 
        ? 'drop-shadow(0 0 10px rgba(168, 172, 184, 0.5))'
        : 'drop-shadow(0 0 10px rgba(15, 23, 42, 0.22))',
    },
    status: {
      success: getCSSVar('--mermaid-success', isDark ? '#6dd4a0' : '#16a34a'),
      error: getCSSVar('--mermaid-error', isDark ? '#e87878' : '#dc2626'),
      warning: getCSSVar('--mermaid-warning', isDark ? '#e8b060' : '#f59e0b'),
      info: getCSSVar('--mermaid-info', isDark ? '#78a8d8' : '#64748b'),
    },
    text: {
      primary: getCSSVar('--mermaid-node-text', isDark ? '#e0e2e8' : '#1f2937'),
      secondary: getCSSVar('--mermaid-edge-label-text', isDark ? '#9a9ea8' : '#4b5563'),
      muted: isDark ? '#6a6e78' : '#6b7280',
      highlight: isDark ? '#f0f2f8' : '#111827',
    },
  };
}
