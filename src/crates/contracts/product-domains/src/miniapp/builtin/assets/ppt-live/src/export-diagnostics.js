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
    box_shadow_removed: 'An unsupported CSS box-shadow was removed.',
    text_shadow_removed: 'An unsupported CSS text-shadow was removed.',
    css_filter_removed: 'A CSS filter was removed.',
    css_mask_removed: 'A CSS mask was removed.',
    animation_removed: 'A CSS/SVG animation was removed.',
    inline_margin_removed: 'An unsupported inline margin was ignored.',
    background_image_removed: 'A CSS background image was removed (solid color kept).',
    svg_paint_server_removed: 'An SVG paint-server fill/stroke was replaced with a solid color.',
    svg_feature_removed: 'An unsupported SVG feature was removed.',
    svg_path_outline: 'A filled SVG path was converted to an editable outline.',
    generated_content_removed: 'Pseudo-element generated content was removed.',
    merge_container_unwrapped: 'A merge container was unwrapped into regular editable content.',
    manual_bullet_removed: 'A manual bullet character was removed.',
    element_removed: 'An element that cannot be represented as an editable object was removed.',
    canvas_size_adjusted: 'The slide canvas was normalized to the editable size.',
    slide_simplified: 'This slide contained unconvertible content and was replaced with a simplified editable version.',
    duplicate_source_id_repaired: 'A duplicate element id was reassigned.',
    nested_paragraph_repaired: 'A nested paragraph structure was split into ordered paragraphs.',
    direct_text_wrapped: 'Direct container text was wrapped in a semantic text block.',
    decorated_inline_promoted: 'Decorated inline text was promoted to shape plus text.',
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
    box_shadow_removed: '不支持的 CSS box-shadow 已移除。',
    text_shadow_removed: '不支持的 CSS text-shadow 已移除。',
    css_filter_removed: 'CSS filter 已移除。',
    css_mask_removed: 'CSS mask 已移除。',
    animation_removed: 'CSS/SVG 动画已移除。',
    inline_margin_removed: '不支持的内联外边距已忽略。',
    background_image_removed: 'CSS 背景图片已移除（保留纯色）。',
    svg_paint_server_removed: 'SVG 渐变/图案填充已替换为纯色。',
    svg_feature_removed: '不支持的 SVG 特性已移除。',
    svg_path_outline: '填充 SVG 路径已转换为可编辑轮廓。',
    generated_content_removed: '伪元素生成内容已移除。',
    merge_container_unwrapped: '合并容器已展开为普通可编辑内容。',
    manual_bullet_removed: '手工项目符号字符已移除。',
    element_removed: '无法表示为可编辑对象的元素已移除。',
    canvas_size_adjusted: '页面画布已归一化为可编辑尺寸。',
    slide_simplified: '该页包含无法转换的内容，已替换为简化可编辑版本。',
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
  const severity = ['blocking', 'degrade'].includes(diagnostic.severity)
    ? diagnostic.severity
    : 'rewrite';
  return {
    slideNumber: diagnostic.slideNumber,
    sourceId: sanitizeDiagnosticSourceId(diagnostic.sourceId),
    severity,
    code: String(diagnostic.code || 'unknown').replace(/[^a-z0-9_-]/gi, '').slice(0, 64),
    reason: reason.slice(0, 120),
  };
}

export function localizeExportDiagnosticLocations(locations = [], locale = 'en-US') {
  return locations.map((location) => formatLocalizedExportDiagnostic(location, locale));
}

export function summarizePptxExportDiagnostics(scenes = [], degradations = []) {
  const counts = { rewritten: 0, blocking: 0, degraded: 0 };
  const locations = [];
  const seen = new Set();
  const add = (slideNumber, diagnostic) => {
    const location = {
      slideNumber,
      sourceId: diagnostic.sourceId || null,
      severity: ['blocking', 'degrade'].includes(diagnostic.severity)
        ? diagnostic.severity
        : 'rewrite',
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
  (Array.isArray(degradations) ? degradations : []).forEach((degradation, index) => {
    counts.degraded += 1;
    add(degradation?.slideNumber || index + 1, {
      sourceId: degradation?.sourceId,
      severity: 'degrade',
      code: degradation?.code,
    });
  });
  return {
    counts,
    locations,
    hasWarnings: counts.rewritten > 0 || counts.degraded > 0,
    hasBlocking: counts.blocking > 0,
  };
}
