/**
 * Rehype plugin: local image paths.
 * Marks local image paths for async post-processing after render.
 */
import { visit } from 'unist-util-visit';
import { createLogger } from '@/shared/utils/logger';

type Root = any;
type Element = any;

const log = createLogger('rehypeLocalImages');

export interface RehypeLocalImagesOptions {
  /**
   * Directory path of the Markdown file.
   * Used to resolve relative image paths.
   */
  basePath?: string;
}

/**
 * Check whether path is a local file path (not an HTTP/HTTPS URL).
 */
export function isLocalPath(src: string): boolean {
  if (!src) return false;
  
  if (src.startsWith('http://') || src.startsWith('https://')) {
    return false;
  }
  
  if (src.startsWith('data:')) {
    return false;
  }
  
  if (src.startsWith('asset://') || src.startsWith('tauri://')) {
    return false;
  }
  
  return true;
}

/**
 * Normalize path separators (use forward slashes).
 */
export function normalizePath(path: string): string {
  return path.replace(/\\/g, '/');
}

/**
 * Check whether a path is absolute.
 */
export function isAbsolutePath(path: string): boolean {
  const normalized = normalizePath(path);
  if (/^[A-Za-z]:/.test(normalized) || /^\/[A-Za-z]:/.test(normalized)) {
    return true;
  }
  if (normalized.startsWith('/') && !normalized.startsWith('//')) {
    return true;
  }
  return false;
}

/**
 * Resolve image path to an absolute path.
 */
export function resolveImagePath(src: string, basePath?: string): string {
  const normalizedSrc = normalizePath(src);
  
  if (isAbsolutePath(normalizedSrc)) {
    return normalizedSrc.replace(/^\/([A-Za-z]:)/, '$1');
  }
  
  if (!basePath) {
    log.warn('Relative path image requires basePath but not provided', { src });
    return normalizedSrc;
  }
  
  const normalizedBase = normalizePath(basePath);
  
  let resultPath = normalizedSrc;
  if (resultPath.startsWith('./')) {
    resultPath = resultPath.slice(2);
  }
  
  const fullPath = `${normalizedBase}/${resultPath}`.replace(/\/+/g, '/');
  
  return fullPath;
}

/**
 * Get image MIME type.
 */
export function getMimeType(path: string): string {
  const ext = path.toLowerCase().split('.').pop();
  const mimeTypes: Record<string, string> = {
    'jpg': 'image/jpeg',
    'jpeg': 'image/jpeg',
    'png': 'image/png',
    'gif': 'image/gif',
    'bmp': 'image/bmp',
    'webp': 'image/webp',
    'svg': 'image/svg+xml',
    'ico': 'image/x-icon',
    'avif': 'image/avif'
  };
  return mimeTypes[ext || ''] || 'image/jpeg';
}

/**
 * Rehype plugin: mark local image paths.
 * Converts local image paths to absolute paths and marks `data-local-image`.
 * Actual image loading happens asynchronously after render.
 */
export function rehypeLocalImages(options: RehypeLocalImagesOptions = {}) {
  const { basePath } = options;
  
  return (tree: Root) => {
    visit(tree, 'element', (node: Element) => {
      if (node.tagName === 'img' && node.properties) {
        const src = node.properties.src as string | undefined;
        
        if (src && isLocalPath(src)) {
          const absolutePath = resolveImagePath(src, basePath);
          
          node.properties['data-local-image'] = 'true';
          node.properties['data-local-path'] = absolutePath;
          node.properties['data-original-src'] = src;
          
          node.properties.src = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
          
          const existingClass = node.properties.className as string[] | undefined;
          node.properties.className = [...(existingClass || []), 'local-image-loading'];
        }
      }
    });
  };
}
