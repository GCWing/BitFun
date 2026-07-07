// Direct re-export from source — no intermediate vendor bundle needed.
// build-bitfun.mjs (esbuild, bundle:true) resolves all npm dependencies
// (pptxgenjs, pdf-lib, jszip) at final bundle time.
export {
  exportPptxFromDeck,
  exportPptxPrepared,
  exportPdfFromBase64Pages,
  exportPngZipFromPages,
} from './export-deck-browser.js';
