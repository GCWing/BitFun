const DIAGNOSTIC_REASONS = {
  'en-US': {
    active_content_removed: 'Unsafe active content was removed.',
    canvas_overflow: 'Slide content exceeds the editable canvas.',
    canvas_size: 'Slide dimensions do not match the editable canvas.',
    text_out_of_bounds: 'Text exceeds the slide boundary.',
    bottom_safety_margin: 'Text enters the bottom safety margin.',
    css_gradient: 'A CSS gradient was rewritten as editable solid strips.',
    svg_path_rewrite: 'An SVG path was rewritten as editable line segments.',
    css_box_shadow_ring: 'A CSS ring box-shadow was rewritten as a concentric editable shape.',
    box_shadow_unsupported: 'This CSS box-shadow cannot be represented as editable PowerPoint geometry.',
    manual_bullet_list: 'Manual bullets were rewritten as an editable list.',
    unreadable_document: 'The slide document could not be read.',
    unmeasurable_canvas: 'The slide canvas could not be measured.',
    pptx_serialization: 'The slide could not be serialized to PPTX.',
  },
  'zh-CN': {
    active_content_removed: '已移除不安全的活动内容。',
    canvas_overflow: '页面内容超出可编辑幻灯片边界。',
    canvas_size: '页面尺寸与可编辑幻灯片画布不一致。',
    text_out_of_bounds: '文字超出幻灯片边界。',
    bottom_safety_margin: '文字进入底部安全边距。',
    css_gradient: 'CSS 渐变已重写为可编辑纯色条带。',
    svg_path_rewrite: 'SVG 路径已重写为可编辑线段。',
    css_box_shadow_ring: 'CSS 环形 box-shadow 已重写为同心可编辑形状。',
    box_shadow_unsupported: '该 CSS box-shadow 无法表示为可编辑的 PowerPoint 几何。',
    manual_bullet_list: '手工项目符号已重写为可编辑列表。',
    unreadable_document: '无法读取幻灯片文档。',
    unmeasurable_canvas: '无法测量幻灯片画布。',
    pptx_serialization: '无法将幻灯片序列化为 PPTX。',
  },
};

const UNKNOWN_REASON = {
  'en-US': 'Export encountered a protected internal error.',
  'zh-CN': '导出遇到已保护的内部错误。',
};

export function sanitizeDiagnosticSourceId(value) {
  const safe = String(value || '').replace(/[^a-zA-Z0-9_-]/g, '').slice(0, 48);
  return safe || null;
}

export function formatLocalizedExportDiagnostic(diagnostic = {}, locale = 'en-US') {
  const resolvedLocale = locale === 'zh-CN' ? locale : 'en-US';
  const reason = DIAGNOSTIC_REASONS[resolvedLocale][diagnostic.code]
    || UNKNOWN_REASON[resolvedLocale];
  return {
    slideNumber: diagnostic.slideNumber,
    sourceId: sanitizeDiagnosticSourceId(diagnostic.sourceId),
    severity: diagnostic.severity === 'blocking' ? 'blocking' : 'rewrite',
    code: String(diagnostic.code || 'unknown').replace(/[^a-z0-9_-]/gi, '').slice(0, 64),
    reason: reason.slice(0, 120),
  };
}

export function localizeExportDiagnosticLocations(locations = [], locale = 'en-US') {
  return locations.map((location) => formatLocalizedExportDiagnostic(location, locale));
}

export function summarizePptxExportDiagnostics(scenes = []) {
  const counts = { rewritten: 0, blocking: 0 };
  const locations = [];
  const seen = new Set();
  const add = (slideNumber, diagnostic) => {
    const location = {
      slideNumber,
      sourceId: diagnostic.sourceId || null,
      severity: diagnostic.severity === 'blocking' ? 'blocking' : 'rewrite',
      code: diagnostic.code || diagnostic.rewrite || null,
    };
    const key = `${location.slideNumber}:${location.sourceId}:${location.severity}:${location.code}`;
    if (seen.has(key)) return;
    seen.add(key);
    locations.push(location);
  };
  scenes.forEach((scene, index) => {
    const slideNumber = scene?.slideNumber || index + 1;
    (scene?.nodes || []).forEach((node) => {
      if (!node.rewrite) return;
      counts.rewritten += 1;
      add(slideNumber, {
        sourceId: node.sourceId,
        severity: 'rewrite',
        code: node.rewrite,
      });
    });
  });
  return {
    counts,
    locations,
    hasWarnings: counts.rewritten > 0,
    hasBlocking: counts.blocking > 0,
  };
}
