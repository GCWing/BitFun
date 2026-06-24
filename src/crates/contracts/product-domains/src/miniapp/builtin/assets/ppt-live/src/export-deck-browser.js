import { PDFDocument } from 'pdf-lib';
import JSZip from 'jszip';
import {
  buildSlideFromExtracted,
  buildSpeakerNotes,
  createPptxDeck,
} from './pptx-html-build.js';
import { exportElementDeckToPptx } from './pptx-element-export.js';

const MIME_PPTX = 'application/vnd.openxmlformats-officedocument.presentationml.presentation';

const RASTER_TEXT_TYPES = new Set(['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'text', 'list', 'merged-text']);

function filterSlideDataForRasterBackdrop(slideData) {
  return {
    ...slideData,
    elements: (slideData.elements || []).filter((el) => RASTER_TEXT_TYPES.has(el.type)),
  };
}

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

export async function exportPptxFromDeck(deck) {
  const slides = Array.isArray(deck?.slides) ? deck.slides : [];
  if (slides.some((slide) => slide?.html)) {
    throw new Error('HTML slides must use the WebView prepare export path.');
  }
  const pptx = await exportElementDeckToPptx(deck);
  return pptxToExportResult(pptx, deck);
}

export async function exportPptxPrepared(deck, preparedSlides) {
  const prepared = Array.isArray(preparedSlides) ? preparedSlides : [];
  if (!prepared.length) throw new Error('No prepared slides to export');
  const pptx = createPptxDeck(deck);
  const slides = Array.isArray(deck?.slides) ? deck.slides : [];
  for (const item of prepared) {
    const sourceSlide = slides[item.index] || item.notes || {};
    let slideData = item.slideData;
    if (item.rasterBase64) {
      const raw = String(item.rasterBase64).replace(/^data:.*;base64,/, '');
      if (item.rasterOnly) {
        slideData = {
          ...slideData,
          elements: (slideData.elements || []).filter((el) => !RASTER_TEXT_TYPES.has(el.type)),
          background: { type: 'image', path: `data:image/png;base64,${raw}` },
        };
      } else {
        slideData = filterSlideDataForRasterBackdrop(slideData);
        slideData = {
          ...slideData,
          background: { type: 'image', path: `data:image/png;base64,${raw}` },
        };
      }
    }
    const result = await buildSlideFromExtracted(
      slideData,
      item.bodyDimensions,
      pptx,
    );
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
