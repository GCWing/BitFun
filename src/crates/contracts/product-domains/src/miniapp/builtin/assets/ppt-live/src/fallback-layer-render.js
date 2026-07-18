import { buildDomPaintOrderMap } from './paint-order.js';

const EXPORT_WIDTH = 1280;
const EXPORT_HEIGHT = 720;
const LOCAL_RASTER_CODES = new Set([
  'css_gradient',
  'css_filter',
  'computed_gradient',
  'generated_content',
  'container_background_image',
  'merge_background_image',
  'complex_svg_raster',
]);

function rasterLayerZIndex(element, view) {
  let current = element;
  while (current) {
    const raw = view.getComputedStyle(current).zIndex;
    if (raw && raw !== 'auto') {
      const parsed = parseInt(raw, 10);
      if (Number.isFinite(parsed)) return parsed;
    }
    current = current.parentElement;
  }
  return 0;
}

function escapedAttributeValue(value) {
  return String(value).replace(/["\\]/g, '\\$&');
}

function serializeRasterTargetDocument(doc, targetSpecs) {
  const targets = (Array.isArray(targetSpecs) ? targetSpecs : [targetSpecs]).map((target) => (
    typeof target === 'string'
      ? { sourceId: target, captureStrategy: 'visual-subtree' }
      : target
  ));
  const sourceRoot = doc._exportRoot || doc.documentElement;
  const sourceBody = doc.body;
  const bodyClone = sourceBody.cloneNode(true);
  let targetCount = 0;
  targets.forEach(({ sourceId, captureStrategy }) => {
    const target = bodyClone.querySelector(
      `[data-pptx-source-id="${escapedAttributeValue(sourceId)}"]`,
    ) || (bodyClone.dataset?.pptxSourceId === sourceId ? bodyClone : null);
    if (!target) return;
    target.setAttribute('data-pptx-raster-target', '1');
    target.setAttribute('data-pptx-capture-strategy', captureStrategy);
    targetCount += 1;
  });
  if (!targetCount) return null;

  const authorStyles = [...sourceRoot.querySelectorAll('style')]
    .map((style) => style.textContent || '')
    .join('\n');
  const bodyAttributes = [...bodyClone.attributes]
    .filter((attribute) => !['class', 'style'].includes(attribute.name))
    .map((attribute) => ` ${attribute.name}="${String(attribute.value).replace(/"/g, '&quot;')}"`)
    .join('');
  const isolationCss = `
html, body, .ppt-export-root, .ppt-export-body {
  margin: 0 !important;
  padding: 0 !important;
  width: ${EXPORT_WIDTH}px !important;
  height: ${EXPORT_HEIGHT}px !important;
  overflow: hidden !important;
  background: transparent !important;
  background-color: transparent !important;
}
.ppt-export-body [data-pptx-source-id] {
  visibility: hidden !important;
}
.ppt-export-body [data-pptx-raster-target="1"],
.ppt-export-body [data-pptx-capture-strategy="visual-subtree"] * {
  visibility: visible !important;
}
.ppt-export-body [data-pptx-capture-strategy="self-decoration"] > *,
.ppt-export-body [data-pptx-capture-strategy="pseudo-only"] > * {
  visibility: hidden !important;
}
.ppt-export-body [data-pptx-capture-strategy="pseudo-only"] {
  background: none !important;
  background-image: none !important;
  border-color: transparent !important;
  box-shadow: none !important;
  filter: none !important;
}
.ppt-export-body [data-pptx-raster-target="1"] :is(p,h1,h2,h3,h4,h5,h6,li,span,a,small,label,code,b,strong,i,em,u,mark,sub,sup),
.ppt-export-body [data-pptx-raster-target="1"]:is(p,h1,h2,h3,h4,h5,h6,li,span,a,small,label,code,b,strong,i,em,u,mark,sub,sup) {
  color: transparent !important;
  -webkit-text-fill-color: transparent !important;
  text-shadow: none !important;
}
.ppt-export-body [data-pptx-raster-target="1"] ::marker {
  color: transparent !important;
  -webkit-text-fill-color: transparent !important;
}
.ppt-export-body [data-pptx-raster-target="1"] svg text,
.ppt-export-body svg[data-pptx-raster-target="1"] text {
  visibility: hidden !important;
}
`;
  return `<!doctype html><html><head><meta charset="utf-8"><style>${authorStyles}</style><style id="pptx-raster-hide-editable-text">${isolationCss}</style></head><body><div class="ppt-export-root"><div class="ppt-export-body"${bodyAttributes} style="${bodyClone.getAttribute('style') || ''}">${bodyClone.innerHTML}</div></div></body></html>`;
}

export function buildRasterFallbackRequests(doc, diagnostics = []) {
  const bodyRect = doc.body.getBoundingClientRect();
  const view = doc.defaultView || globalThis.window;
  const sourceOrder = buildDomPaintOrderMap(doc);
  const grouped = new Map();
  diagnostics.forEach((diagnostic) => {
    if (!LOCAL_RASTER_CODES.has(diagnostic.code) || !diagnostic.sourceId) return;
    const captureStrategy = diagnostic.code === 'generated_content'
      ? 'pseudo-only'
      : (['css_filter', 'complex_svg_raster'].includes(diagnostic.code)
        ? 'visual-subtree'
        : 'self-decoration');
    const key = `${diagnostic.sourceId}:${captureStrategy}`;
    if (!grouped.has(key)) {
      grouped.set(key, {
        sourceId: diagnostic.sourceId,
        captureStrategy,
        diagnostics: [],
      });
    }
    grouped.get(key).diagnostics.push(diagnostic);
  });
  const requests = [];
  grouped.forEach(({ sourceId, captureStrategy, diagnostics: sourceDiagnostics }) => {
    const element = doc.body.dataset?.pptxSourceId === sourceId
      ? doc.body
      : doc.body.querySelector(
        `[data-pptx-source-id="${escapedAttributeValue(sourceId)}"]`,
      );
    const failureRequest = (code, reason, details = {}) => ({
      sourceId,
      zIndex: details.zIndex ?? 0,
      paintOrder: sourceOrder.get(sourceId) ?? requests.length,
      subOrder: 0,
      kind: 'raster',
      phase: 'local-visual',
      canvas: 'full-page',
      captureStrategy,
      suppressedNativeVisualIds: [],
      bbox: details.bbox || { x: 0, y: 0, w: 0, h: 0 },
      diagnostics: sourceDiagnostics,
      buildFailure: {
        code,
        stage: 'request-build',
        reason,
      },
    });
    if (!element) {
      requests.push(failureRequest(
        'local_raster_target_missing',
        `Raster fallback source "${sourceId}" is not present in the export document.`,
      ));
      return;
    }
    let rect;
    try {
      rect = element.getBoundingClientRect();
    } catch (error) {
      requests.push(failureRequest(
        'local_raster_serialize_failed',
        `Raster fallback source "${sourceId}" could not be measured: ${String(error?.message || error)}`,
        { zIndex: rasterLayerZIndex(element, view) },
      ));
      return;
    }
    const bbox = {
      x: (rect.left - bodyRect.left) / 96,
      y: (rect.top - bodyRect.top) / 96,
      w: rect.width / 96,
      h: rect.height / 96,
    };
    if (!(rect.width > 0) || !(rect.height > 0)) {
      requests.push(failureRequest(
        'local_raster_unmeasurable',
        `Raster fallback source "${sourceId}" has no measurable width or height.`,
        { zIndex: rasterLayerZIndex(element, view), bbox },
      ));
      return;
    }
    try {
      if (!serializeRasterTargetDocument(doc, { sourceId, captureStrategy })) {
        throw new Error('Raster target was not retained in the export document.');
      }
    } catch (error) {
      requests.push(failureRequest(
        'local_raster_serialize_failed',
        `Raster fallback source "${sourceId}" could not be serialized: ${String(error?.message || error)}`,
        { zIndex: rasterLayerZIndex(element, view), bbox },
      ));
      return;
    }
    const suppressedNativeVisualIds = captureStrategy === 'pseudo-only'
      ? []
      : (captureStrategy === 'self-decoration'
        ? [sourceId]
        : [element, ...element.querySelectorAll('[data-pptx-source-id]')]
          .map((item) => item.dataset?.pptxSourceId)
          .filter(Boolean));
    requests.push({
      sourceId,
      zIndex: rasterLayerZIndex(element, view),
      paintOrder: sourceOrder.get(sourceId) ?? requests.length,
      subOrder: 0,
      kind: 'raster',
      phase: 'local-visual',
      canvas: 'full-page',
      captureStrategy,
      suppressedNativeVisualIds: [...new Set(suppressedNativeVisualIds)],
      bbox,
      buildHtml: () => serializeRasterTargetDocument(doc, { sourceId, captureStrategy }),
      diagnostics: sourceDiagnostics,
    });
  });
  return requests.sort((a, b) => a.zIndex - b.zIndex || a.paintOrder - b.paintOrder);
}

export function buildPageVisualFallbackRequest(doc, localRequests = []) {
  const sourceIds = [...new Set(localRequests.map((request) => request.sourceId).filter(Boolean))];
  if (!sourceIds.length) return null;
  const targetSpecs = localRequests.map((request) => ({
    sourceId: request.sourceId,
    captureStrategy: request.captureStrategy,
  }));
  const request = {
    sourceId: 'slide-visuals',
    sourceIds,
    captureStrategy: 'page-visual',
    suppressedNativeVisualIds: [...new Set(
      localRequests.flatMap((request) => request.suppressedNativeVisualIds || []),
    )],
    zIndex: Math.min(...localRequests.map((request) => request.zIndex ?? 0)),
    paintOrder: Math.min(...localRequests.map((request) => request.paintOrder ?? 0)),
    subOrder: Math.min(...localRequests.map((request) => request.subOrder ?? 0)),
    kind: 'raster',
    phase: 'page-visual',
    canvas: 'full-page',
    bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
    diagnostics: localRequests.flatMap((request) => request.diagnostics || []),
  };
  const missingSourceIds = localRequests
    .filter((localRequest) => localRequest.buildFailure?.code === 'local_raster_target_missing')
    .map((localRequest) => localRequest.sourceId);
  if (missingSourceIds.length) {
    return {
      ...request,
      buildFailure: {
        code: 'page_visual_target_missing',
        stage: 'request-build',
        reason: `Page-visual fallback cannot cover missing sources: ${missingSourceIds.join(', ')}.`,
      },
    };
  }
  try {
    if (!serializeRasterTargetDocument(doc, targetSpecs)) {
      return {
        ...request,
        buildFailure: {
          code: 'page_visual_target_missing',
          stage: 'request-build',
          reason: `Page-visual fallback could not locate any requested sources: ${sourceIds.join(', ')}.`,
        },
      };
    }
    return {
      ...request,
      buildHtml: () => serializeRasterTargetDocument(doc, targetSpecs),
    };
  } catch (error) {
    return {
      ...request,
      buildFailure: {
        code: 'page_visual_serialize_failed',
        stage: 'request-build',
        reason: `Page-visual fallback could not be serialized: ${String(error?.message || error)}`,
      },
    };
  }
}

export function buildWholePageVisualFallbackRequest(
  doc,
  diagnostics = [],
  suppressedNativeVisualIds = [],
) {
  const bodySourceId = doc?.body?.dataset?.pptxSourceId;
  if (!bodySourceId) {
    return {
      sourceId: 'slide-visuals',
      phase: 'page-visual',
      kind: 'raster',
      bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
      diagnostics,
      suppressedNativeVisualIds: [...new Set(suppressedNativeVisualIds)],
      buildFailure: {
        code: 'page_visual_target_missing',
        stage: 'request-build',
        reason: 'Whole-page visual source is unavailable.',
      },
    };
  }
  const request = buildPageVisualFallbackRequest(doc, [{
    sourceId: bodySourceId,
    captureStrategy: 'visual-subtree',
    suppressedNativeVisualIds: [...new Set(suppressedNativeVisualIds)],
    zIndex: 0,
    paintOrder: 0,
    subOrder: 0,
    diagnostics,
  }]);
  return {
    ...request,
    sourceId: 'slide-visuals',
    sourceIds: [bodySourceId],
    captureStrategy: 'whole-page-visual',
  };
}

export async function renderRasterFallbackLayers(requests, renderRaster, slideIndex) {
  const layers = [];
  const failures = [];
  if (typeof renderRaster !== 'function') {
    return { layers, failures: [...requests] };
  }
  for (const request of requests) {
    if (request.buildFailure) {
      failures.push({
        ...request,
        error: request.buildFailure.reason,
      });
      continue;
    }
    try {
      const html = typeof request.buildHtml === 'function' ? request.buildHtml() : request.html;
      if (!html) throw new Error('Raster fallback HTML could not be generated');
      const rendered = await renderRaster(html, slideIndex, {
        sourceId: request.sourceId,
        bbox: request.bbox,
        zIndex: request.zIndex,
        paintOrder: request.paintOrder,
        subOrder: request.subOrder,
        phase: request.phase,
        captureStrategy: request.captureStrategy,
        suppressedNativeVisualIds: request.suppressedNativeVisualIds || [],
      });
      const raw = String(rendered || '').replace(/^data:.*;base64,/, '');
      if (!raw) throw new Error('Raster renderer returned no PNG data');
      layers.push({
        ...request,
        buildHtml: undefined,
        html: undefined,
        data: `data:image/png;base64,${raw}`,
      });
    } catch (error) {
      failures.push({ ...request, error: String(error?.message || error) });
    }
  }
  return { layers, failures };
}

function failureDiagnostic(code, failure, slideIndex, severity = 'fallback') {
  const reason = failure.buildFailure?.reason || failure.error;
  return {
    severity,
    kind: severity === 'blocking' ? 'blocking' : undefined,
    code: failure.buildFailure?.code || code,
    message: `${failure.phase || 'raster'} fallback failed for ${failure.sourceId || 'slide visual'}: ${reason}`,
    sourceId: failure.sourceId || null,
    slideNumber: slideIndex + 1,
    phase: failure.phase || null,
    stage: failure.buildFailure?.stage || 'render',
    reason,
  };
}

export async function renderRasterFallbackPlan(plan, renderRaster, slideIndex) {
  const diagnostics = [];
  if (plan.pageVisualRequest?.captureStrategy === 'whole-page-visual') {
    const pageResult = await renderRasterFallbackLayers(
      [plan.pageVisualRequest],
      renderRaster,
      slideIndex,
    );
    if (pageResult.layers.length) {
      return {
        layers: pageResult.layers,
        fullPageFallback: null,
        diagnostics: [{
          severity: 'fallback',
          code: 'page_visual_fallback',
          message: `Slide ${slideIndex + 1} used a page visual fallback.`,
          sourceId: 'slide-visuals',
          slideNumber: slideIndex + 1,
          phase: 'page-visual',
        }],
        blocking: false,
      };
    }
    diagnostics.push(...pageResult.failures.map((failure) => (
      failureDiagnostic('page_visual_raster_failed', failure, slideIndex)
    )));
    const fullResult = await renderRasterFallbackLayers(
      plan.fullPageRequest ? [plan.fullPageRequest] : [],
      renderRaster,
      slideIndex,
    );
    if (fullResult.layers.length) {
      return {
        layers: [],
        fullPageFallback: fullResult.layers[0],
        diagnostics: [
          ...diagnostics,
          {
            severity: 'fallback',
            code: 'full_page_fallback',
            message: `Slide ${slideIndex + 1} used a full-page fallback.`,
            sourceId: fullResult.layers[0].sourceId,
            slideNumber: slideIndex + 1,
            phase: 'full-page',
          },
        ],
        blocking: false,
      };
    }
    diagnostics.push(...fullResult.failures.map((failure) => (
      failureDiagnostic('full_page_raster_failed', failure, slideIndex, 'blocking')
    )));
    return { layers: [], fullPageFallback: null, diagnostics, blocking: true };
  }
  const localResult = await renderRasterFallbackLayers(
    plan.localRequests || [],
    renderRaster,
    slideIndex,
  );
  if (!localResult.failures.length) {
    return {
      layers: localResult.layers,
      fullPageFallback: null,
      diagnostics,
      blocking: false,
    };
  }

  diagnostics.push(...localResult.failures.map((failure) => (
    failureDiagnostic('local_raster_failed', failure, slideIndex)
  )));

  if (plan.pageVisualRequest) {
    const pageResult = await renderRasterFallbackLayers(
      [plan.pageVisualRequest],
      renderRaster,
      slideIndex,
    );
    if (pageResult.layers.length) {
      const layer = pageResult.layers[0];
      diagnostics.push({
        severity: 'fallback',
        code: 'page_visual_fallback',
        message: `Slide ${slideIndex + 1} used a transparent page visual fallback.`,
        sourceId: layer.sourceId,
        slideNumber: slideIndex + 1,
        phase: 'page-visual',
        reason: 'One or more local visual layers could not be rendered.',
      });
      return {
        layers: [layer],
        fullPageFallback: null,
        diagnostics,
        blocking: false,
      };
    }
    diagnostics.push(...pageResult.failures.map((failure) => (
      failureDiagnostic('page_visual_raster_failed', failure, slideIndex)
    )));
  }

  if (plan.fullPageRequest) {
    const fullResult = await renderRasterFallbackLayers(
      [plan.fullPageRequest],
      renderRaster,
      slideIndex,
    );
    if (fullResult.layers.length) {
      const fullPageFallback = fullResult.layers[0];
      diagnostics.push({
        severity: 'fallback',
        code: 'full_page_fallback',
        message: `Slide ${slideIndex + 1} was exported as a full-page PNG fallback.`,
        sourceId: fullPageFallback.sourceId,
        slideNumber: slideIndex + 1,
        phase: 'full-page',
        reason: 'Local and transparent page visual fallback rendering failed.',
      });
      return {
        layers: [],
        fullPageFallback,
        diagnostics,
        blocking: false,
      };
    }
    diagnostics.push(...fullResult.failures.map((failure) => (
      failureDiagnostic('full_page_raster_failed', failure, slideIndex, 'blocking')
    )));
  }

  return {
    layers: [],
    fullPageFallback: null,
    diagnostics,
    blocking: true,
  };
}
