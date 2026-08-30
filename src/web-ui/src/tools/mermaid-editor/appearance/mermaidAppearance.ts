import { mermaidAppearanceAdapter } from '@/infrastructure/appearance/adapters/MermaidAppearanceAdapter';
import {
  readActiveTypographyTokenPx,
  readActiveTypographyTokenValue,
} from '@/infrastructure/design-system/typographyRuntime';

export function getMermaidConfig() {
  const { mode, palette: p } = mermaidAppearanceAdapter.getSettings();
  const fontFamily = readActiveTypographyTokenValue('font.family.sans');
  const bodyFontSize = readActiveTypographyTokenPx('font.size.sm');
  const supportFontSize = readActiveTypographyTokenPx('font.size.xs');
  const titleFontSize = readActiveTypographyTokenPx('font.size.xl');
  return {
    theme: 'base' as const,
    darkMode: mode === 'dark',
    fontFamily,
    fontSize: bodyFontSize,
    themeVariables: {
      primaryColor: p.nodeFill, primaryTextColor: p.nodeText, primaryBorderColor: p.nodeStroke,
      secondaryColor: p.nodeFillHover, secondaryTextColor: p.nodeText, secondaryBorderColor: p.nodeStrokeHover,
      tertiaryColor: p.clusterFill, tertiaryTextColor: p.clusterText, tertiaryBorderColor: p.clusterStroke,
      background: 'transparent', mainBkg: p.nodeFill, secondBkg: p.nodeFillHover, textColor: p.nodeText,
      nodeTextColor: p.nodeText, lineColor: p.edgeStroke, border1: p.nodeStroke, border2: p.clusterStroke,
      nodeBkg: p.nodeFill, nodeBorder: p.nodeStroke, clusterBkg: p.clusterFill, clusterBorder: p.clusterStroke,
      arrowheadColor: p.edgeStroke, edgeLabelBackground: p.edgeLabelBackground, noteBkgColor: p.noteFill,
      noteTextColor: p.noteText, noteBorderColor: p.noteStroke, activationBkgColor: p.activationFill,
      activationBorderColor: p.activationStroke, actorBkg: p.nodeFill, actorBorder: p.nodeStroke,
      actorTextColor: p.nodeText, actorLineColor: p.edgeStroke, signalColor: p.nodeStrokeHover,
      signalTextColor: p.nodeText, labelBoxBkgColor: p.edgeLabelBackground, labelBoxBorderColor: p.clusterStroke,
      labelTextColor: p.edgeLabelText, loopTextColor: p.edgeLabelText, doneTaskBkgColor: p.noteFill,
      doneTaskBorderColor: p.success, activeTaskBkgColor: p.nodeFillHover, activeTaskBorderColor: p.highlight,
      critBkgColor: p.errorBackground, critBorderColor: p.error, taskTextColor: p.nodeText,
      taskTextOutsideColor: p.edgeLabelText, taskTextClickableColor: p.info, classText: p.nodeText,
      labelColor: p.nodeText, pie1: p.pieColors[0], pie2: p.pieColors[1], pie3: p.pieColors[2],
      pie4: p.pieColors[3], pie5: p.pieColors[4], pie6: p.pieColors[5], pie7: p.pieColors[6],
      pie8: p.pieColors[7], pieTitleTextSize: `${titleFontSize}px`, pieTitleTextColor: p.nodeText,
      pieSectionTextSize: `${supportFontSize}px`, pieSectionTextColor: p.nodeText, pieLegendTextSize: `${supportFontSize}px`,
      pieLegendTextColor: p.edgeLabelText, pieStrokeColor: p.nodeStroke, pieStrokeWidth: '2px',
      pieOuterStrokeWidth: '2px', pieOuterStrokeColor: p.nodeStroke, pieOpacity: '0.9',
      errorBkgColor: p.errorBackground, errorTextColor: p.error,
      fontFamily,
    },
    flowchart: { useMaxWidth: true, htmlLabels: true, curve: 'basis', padding: 16, nodeSpacing: 60, rankSpacing: 60, diagramPadding: 16, defaultRenderer: 'dagre-wrapper', wrappingWidth: 200 },
    sequence: { diagramMarginX: 40, diagramMarginY: 16, actorMargin: 60, width: 160, height: 60, boxMargin: 12, boxTextMargin: 8, noteMargin: 12, messageMargin: 40, mirrorActors: true, bottomMarginAdj: 1, useMaxWidth: true, rightAngles: false, showSequenceNumbers: false, wrap: true, wrapPadding: 12 },
    gantt: { titleTopMargin: 20, barHeight: 24, barGap: 6, topPadding: 40, leftPadding: 80, gridLineStartPadding: 40, fontSize: supportFontSize, fontFamily, numberSectionStyles: 4, useWidth: 960 },
    pie: { useWidth: 600, useMaxWidth: true, textPosition: 0.75 },
    state: { dividerMargin: 12, sizeUnit: 8, padding: 10, textHeight: supportFontSize, titleShift: -20, noteMargin: 12, forkWidth: 80, forkHeight: 8, miniPadding: 4, fontSizeFactor: 5.02, fontSize: readActiveTypographyTokenPx('font.size.3xl'), labelHeight: readActiveTypographyTokenPx('font.size.2xl'), edgeLengthFactor: '24', compositTitleSize: readActiveTypographyTokenPx('font.size.6xl'), radius: 6, defaultRenderer: 'dagre-wrapper' },
    class: { useMaxWidth: true, defaultRenderer: 'dagre-wrapper' },
    er: { diagramPadding: 24, layoutDirection: 'TB', minEntityWidth: 120, minEntityHeight: 80, entityPadding: 16, stroke: p.edgeStroke, fill: p.nodeFill, fontSize: bodyFontSize, useMaxWidth: true },
    gitGraph: { showBranches: true, showCommitLabel: true, mainBranchName: 'main', mainBranchOrder: 0, rotateCommitLabel: true },
  };
}

export function getRuntimeColors() {
  const p = mermaidAppearanceAdapter.getSettings().palette;
  return {
    node: { fill: p.nodeFill, fillHover: p.nodeFillHover, stroke: p.nodeStroke, strokeHover: p.nodeStrokeHover, text: p.nodeText, dashArray: '4 2' },
    cluster: { fill: p.clusterFill, fillHover: p.nodeFillHover, stroke: p.clusterStroke, strokeHover: p.nodeStrokeHover, dashArray: '5 3' },
    edgeLabel: { fill: p.edgeLabelBackground, fillHover: p.nodeFillHover, stroke: p.clusterStroke, strokeHover: p.nodeStrokeHover },
    edge: { stroke: p.edgeStroke, strokeHover: p.nodeStrokeHover },
    highlight: { stroke: p.highlight, glow: p.highlight, glowStrong: p.highlight },
    status: { success: p.success, error: p.error, warning: p.warning, info: p.info },
    text: { primary: p.nodeText, secondary: p.edgeLabelText, muted: p.clusterText, highlight: p.nodeText },
  };
}
