const DIAGNOSTIC_REASONS = {
  'en-US': {
    active_content_removed: 'Unsafe active content was removed.',
    canvas_overflow: 'Slide content exceeded the canvas; visual fallback was used.',
    canvas_size: 'Slide dimensions could not preserve native fidelity; visual fallback was used.',
    text_out_of_bounds: 'Text exceeded the slide boundary; visual fallback was used.',
    bottom_safety_margin: 'Text entered the bottom safety margin; visual fallback was used.',
    css_gradient: 'A CSS gradient required visual fallback.',
    css_filter: 'A CSS filter required visual fallback.',
    generated_content: 'Generated CSS content required visual fallback.',
    page_visual_fallback: 'A page visual fallback preserved the slide appearance.',
    full_page_fallback: 'A full-page visual fallback preserved the slide appearance.',
    unreadable_document: 'The slide document could not be read.',
    unmeasurable_canvas: 'The slide canvas could not be measured.',
    pptx_serialization: 'The slide could not be serialized to PPTX.',
    full_page_raster_failed: 'The final visual fallback could not be rendered.',
  },
  'zh-CN': {
    active_content_removed: '已移除不安全的活动内容。',
    canvas_overflow: '页面内容超出幻灯片边界，已切换视觉兜底。',
    canvas_size: '页面尺寸无法保证原生保真，已切换视觉兜底。',
    text_out_of_bounds: '文字超出幻灯片边界，已切换视觉兜底。',
    bottom_safety_margin: '文字进入底部安全边距，已切换视觉兜底。',
    css_gradient: 'CSS 渐变已使用视觉兜底。',
    css_filter: 'CSS 滤镜已使用视觉兜底。',
    generated_content: 'CSS 生成内容已使用视觉兜底。',
    page_visual_fallback: '已使用页面视觉兜底保留外观。',
    full_page_fallback: '已使用整页视觉兜底保留外观。',
    unreadable_document: '无法读取幻灯片文档。',
    unmeasurable_canvas: '无法测量幻灯片画布。',
    pptx_serialization: '无法将幻灯片序列化为 PPTX。',
    full_page_raster_failed: '最终视觉兜底渲染失败。',
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
    slideNumber: Number.isFinite(diagnostic.slideNumber) ? diagnostic.slideNumber : diagnostic.slideNumber,
    sourceId: sanitizeDiagnosticSourceId(diagnostic.sourceId),
    phase: String(diagnostic.phase || '').replace(/[^a-z-]/g, '').slice(0, 32) || null,
    severity: diagnostic.severity === 'blocking' ? 'blocking' : diagnostic.severity,
    code: String(diagnostic.code || 'unknown').replace(/[^a-z0-9_-]/gi, '').slice(0, 64),
    reason: reason.slice(0, 120),
  };
}

export function localizeExportDiagnosticLocations(locations = [], locale = 'en-US') {
  return locations.map((location) => formatLocalizedExportDiagnostic(location, locale));
}

export function summarizePptxExportDiagnostics(preparedSlides = []) {
  const counts = {
    repaired: 0,
    svgImage: 0,
    localPng: 0,
    pageVisual: 0,
    fullPage: 0,
    blocking: 0,
  };
  const locations = [];
  const seen = new Set();
  const addLocation = (slideNumber, item, defaults = {}) => {
    const location = {
      slideNumber,
      sourceId: item?.sourceId || defaults.sourceId || null,
      phase: item?.phase || defaults.phase || null,
      severity: item?.severity || defaults.severity || 'fallback',
      code: item?.code || defaults.code || null,
      reason: item?.reason || item?.message || defaults.reason || null,
    };
    const key = [
      location.slideNumber,
      location.sourceId,
      location.phase,
      location.severity,
      location.code,
    ].join(':');
    if (seen.has(key)) return;
    seen.add(key);
    locations.push(location);
  };

  preparedSlides.forEach((prepared, arrayIndex) => {
    const slideNumber = (prepared?.index ?? arrayIndex) + 1;
    const slideData = prepared?.slideData || {};
    (slideData.diagnostics || []).forEach((diagnostic) => {
      if (diagnostic.severity === 'repaired') counts.repaired += 1;
      if (diagnostic.severity === 'blocking') counts.blocking += 1;
      if (diagnostic.severity === 'repaired' || diagnostic.severity === 'blocking'
        || diagnostic.phase || diagnostic.sourceId) {
        addLocation(slideNumber, diagnostic);
      }
    });
    (slideData.fallbackLayers || []).forEach((layer) => {
      if (layer.kind === 'svg-image') counts.svgImage += 1;
      if (layer.kind === 'raster' && layer.phase === 'local-visual') counts.localPng += 1;
      if (layer.kind === 'raster' && layer.phase === 'page-visual') counts.pageVisual += 1;
      addLocation(slideNumber, layer, {
        severity: 'fallback',
        code: layer.kind === 'svg-image' ? 'svg_image_fallback' : 'png_visual_fallback',
      });
    });
    if (slideData.fullPageFallback) {
      counts.fullPage += 1;
      addLocation(slideNumber, slideData.fullPageFallback, {
        severity: 'fallback',
        code: 'full_page_fallback',
        phase: 'full-page',
      });
    }
  });

  return {
    counts,
    locations,
    hasWarnings: counts.repaired + counts.svgImage + counts.localPng
      + counts.pageVisual + counts.fullPage > 0,
    hasBlocking: counts.blocking > 0,
  };
}
