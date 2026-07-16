export function buildDomPaintOrderMap(doc) {
  const map = new Map();
  if (!doc?.body) return map;
  [doc.body, ...doc.body.querySelectorAll('*')].forEach((element, index) => {
    const sourceId = element.dataset?.pptxSourceId || element.id || null;
    if (sourceId && !map.has(sourceId)) map.set(sourceId, index);
  });
  return map;
}
