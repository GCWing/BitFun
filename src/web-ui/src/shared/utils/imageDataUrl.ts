const TIFF_EXTENSIONS = new Set(['tif', 'tiff']);

export function isTiffPath(path: string): boolean {
  const pathWithoutQueryOrHash = path.split(/[?#]/, 1)[0];
  const extension = pathWithoutQueryOrHash.toLowerCase().split('.').pop();
  return TIFF_EXTENSIONS.has(extension || '');
}

export function getImageMimeType(path: string): string {
  const pathWithoutQueryOrHash = path.split(/[?#]/, 1)[0];
  const extension = pathWithoutQueryOrHash.toLowerCase().split('.').pop();
  const mimeTypes: Record<string, string> = {
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    png: 'image/png',
    gif: 'image/gif',
    bmp: 'image/bmp',
    webp: 'image/webp',
    svg: 'image/svg+xml',
    ico: 'image/x-icon',
    avif: 'image/avif',
    tif: 'image/tiff',
    tiff: 'image/tiff',
  };

  return mimeTypes[extension || ''] || 'image/jpeg';
}

function base64ToArrayBuffer(base64Content: string): ArrayBuffer {
  const binary = atob(base64Content.replace(/\s/g, ''));
  const bytes = new Uint8Array(binary.length);

  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }

  return bytes.buffer;
}

async function tiffBase64ToPngDataUrl(base64Content: string): Promise<string> {
  const UTIF = await import('utif');
  const buffer = base64ToArrayBuffer(base64Content);
  const ifds = UTIF.decode(buffer);
  const firstPage = ifds[0];

  if (!firstPage) {
    throw new Error('TIFF image contains no pages');
  }

  UTIF.decodeImage(buffer, firstPage);

  if (!firstPage.width || !firstPage.height) {
    throw new Error('TIFF image has invalid dimensions');
  }

  const rgba = UTIF.toRGBA8(firstPage);
  const canvas = document.createElement('canvas');
  canvas.width = firstPage.width;
  canvas.height = firstPage.height;

  const context = canvas.getContext('2d');
  if (!context) {
    throw new Error('Failed to get canvas context');
  }

  context.putImageData(
    new ImageData(new Uint8ClampedArray(rgba), firstPage.width, firstPage.height),
    0,
    0,
  );

  return canvas.toDataURL('image/png');
}

export async function createBrowserImageDataUrl(path: string, base64Content: string): Promise<string> {
  if (isTiffPath(path)) {
    return tiffBase64ToPngDataUrl(base64Content);
  }

  return `data:${getImageMimeType(path)};base64,${base64Content}`;
}
