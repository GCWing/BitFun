// ─────────────────────────────────────────────────────────────────────────────
// PPTX / PDF / PNG Export Functions
//
// exportEditablePptx() — the only EditableSlideScene → PPTX serializer entry.
// exportPdfFromBase64Pages() — merges rendered PDF pages.
// exportPngZipFromPages() — zips rendered PNG slides.
//
// All functions return { filename, mimeType, base64 } for browser download.
// ─────────────────────────────────────────────────────────────────────────────
import { PDFDocument } from 'pdf-lib';
import JSZip from 'jszip';
import {
  buildSlideFromScene,
  buildSpeakerNotes,
  createPptxDeck,
} from './pptx-html-build.js';

const MIME_PPTX = 'application/vnd.openxmlformats-officedocument.presentationml.presentation';

function uint8ToBase64(bytes) {
  const chunk = 0x8000;
  let binary = '';
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function exportFileSafe(value) {
  return String(value || 'ppt-live').replace(/[\\/:*?"<>|]+/g, '-').slice(0, 96);
}

async function pptxToExportResult(pptx, deck) {
  const base64 = await pptx.write({ outputType: 'base64' });
  return {
    filename: `${exportFileSafe(deck.title || 'ppt-live')}.pptx`,
    mimeType: MIME_PPTX,
    base64: String(base64 || '').replace(/^data:.*;base64,/, ''),
  };
}

/**
 * Serialize every scene. A slide whose scene fails OOXML serialization is
 * replaced with a blank slide instead of aborting the whole deck export; the
 * degradation is reported through `options.onDegrade`.
 */
export async function exportEditablePptx(deck, scenes, options = {}) {
  const prepared = Array.isArray(scenes) ? scenes : [];
  if (!prepared.length) throw new Error('No editable slide scenes to export');
  const pptx = createPptxDeck(deck);
  const slides = Array.isArray(deck?.slides) ? deck.slides : [];
  for (const [index, scene] of prepared.entries()) {
    const sourceSlide = slides[index] || {};
    let result;
    try {
      result = await buildSlideFromScene(scene, pptx);
    } catch (error) {
      options.onDegrade?.({
        severity: 'degrade',
        slideNumber: scene?.slideNumber || index + 1,
        sourceId: error?.sourceId || error?.diagnostic?.sourceId || `slide-${index + 1}`,
        code: 'slide_simplified',
        message: 'The slide could not be serialized and was replaced with a blank slide.',
      });
      result = await buildSlideFromScene({
        slideNumber: scene?.slideNumber || index + 1,
        width: scene?.width || 13.333,
        height: scene?.height || 7.5,
        nodes: [{
          type: 'shape',
          shapeType: 'rect',
          sourceId: `slide-${index + 1}-blank-background`,
          x: 0,
          y: 0,
          w: scene?.width || 13.333,
          h: scene?.height || 7.5,
          paintOrder: 0,
          subOrder: 0,
          style: { fill: 'FFFFFF', line: null },
        }],
      }, pptx);
    }
    const notes = buildSpeakerNotes(sourceSlide);
    if (notes && result?.slide && typeof result.slide.addNotes === 'function') {
      result.slide.addNotes(notes);
    }
  }
  return pptxToExportResult(pptx, deck);
}

export async function exportPdfFromBase64Pages(deck, pages) {
  const list = Array.isArray(pages) ? pages : [];
  if (!list.length) throw new Error('No rendered PDF pages to export');
  const merged = await PDFDocument.create();
  for (const pageBase64 of list) {
    const raw = String(pageBase64 || '').replace(/^data:.*;base64,/, '');
    const buffer = Uint8Array.from(atob(raw), (c) => c.charCodeAt(0));
    const source = await PDFDocument.load(buffer);
    const copied = await merged.copyPages(source, source.getPageIndices());
    copied.forEach((page) => merged.addPage(page));
  }
  const bytes = await merged.save();
  return {
    filename: `${exportFileSafe(deck?.title || 'ppt-live')}.pdf`,
    mimeType: 'application/pdf',
    base64: uint8ToBase64(bytes),
  };
}

export async function exportPngZipFromPages(deck, pages) {
  const list = Array.isArray(pages) ? pages : [];
  if (!list.length) throw new Error('No rendered PNG pages to export');
  const zip = new JSZip();
  list.forEach((item, index) => {
    const raw = typeof item === 'string'
      ? item
      : String(item?.base64 || '').replace(/^data:.*;base64,/, '');
    const slideIndex = (item?.index ?? index) + 1;
    zip.file(`slide-${String(slideIndex).padStart(2, '0')}.png`, raw, { base64: true });
  });
  const blob = await zip.generateAsync({ type: 'base64', compression: 'DEFLATE' });
  return {
    filename: `${exportFileSafe(deck?.title || 'ppt-live')}-slides.zip`,
    mimeType: 'application/zip',
    base64: String(blob || '').replace(/^data:.*;base64,/, ''),
  };
}
