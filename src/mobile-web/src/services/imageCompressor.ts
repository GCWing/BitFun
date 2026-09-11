const MAX_DIMENSION = 1920;
const JPEG_QUALITY = 0.8;
const MAX_OUTPUT_BYTES = 1024 * 1024;

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

function dataUrlByteSize(dataUrl: string): number {
  const base64 = dataUrl.split(',')[1];
  if (!base64) return 0;
  return Math.ceil((base64.length * 3) / 4);
}

/**
 * Compress an image data-URL so it stays within size/resolution limits.
 * Returns a (possibly re-encoded) data-URL and the original file name.
 */
export async function compressImageDataUrl(
  dataUrl: string,
  fileName: string,
): Promise<{ dataUrl: string; name: string }> {
  // Decode even small inputs: HEIC/SVG and corrupt bytes must not pass through
  // as model-ready images just because their payload is short.
  const img = await loadImage(dataUrl);
  if (/^data:image\/(png|jpeg|gif|webp);base64,/.test(dataUrl)
    && dataUrlByteSize(dataUrl) <= MAX_OUTPUT_BYTES
    && Math.max(img.width, img.height) <= MAX_DIMENSION) {
    return { dataUrl, name: fileName };
  }
  let { width, height } = img;

  if (width > MAX_DIMENSION || height > MAX_DIMENSION) {
    const scale = MAX_DIMENSION / Math.max(width, height);
    width = Math.round(width * scale);
    height = Math.round(height * scale);
  }

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('Image conversion is unavailable');
  ctx.drawImage(img, 0, 0, width, height);

  let quality = JPEG_QUALITY;
  let result = canvas.toDataURL('image/jpeg', quality);

  while (dataUrlByteSize(result) > MAX_OUTPUT_BYTES && quality > 0.3) {
    quality -= 0.1;
    result = canvas.toDataURL('image/jpeg', quality);
  }

  while (dataUrlByteSize(result) > MAX_OUTPUT_BYTES) {
    if (canvas.width <= 64 || canvas.height <= 64) {
      throw new Error('Image exceeds the upload size limit');
    }
    canvas.width = Math.max(1, Math.floor(canvas.width * 0.75));
    canvas.height = Math.max(1, Math.floor(canvas.height * 0.75));
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
    result = canvas.toDataURL('image/jpeg', 0.6);
  }

  const ext = fileName.replace(/\.[^.]+$/, '');
  return { dataUrl: result, name: `${ext}.jpg` };
}

/**
 * Read a File as data-URL, compress if needed, return { name, dataUrl }.
 */
export async function compressImageFile(
  file: File,
): Promise<{ name: string; dataUrl: string }> {
  const raw = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });

  return compressImageDataUrl(raw, file.name);
}
