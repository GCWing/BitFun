import { FileSystemNode, DirectoryCacheEntry } from '../types';
interface DirectoryCacheConfig {
  maxEntries: number;
  ttl: number;
}

const DEFAULT_CONFIG: DirectoryCacheConfig = {
  maxEntries: 200,
  ttl: 0,
};

class DirectoryCacheClass {
  private cache: Map<string, DirectoryCacheEntry> = new Map();
  private accessOrder: string[] = [];
  private config: DirectoryCacheConfig;
  private invalidationCallbacks: Set<(path: string) => void> = new Set();

  constructor(config: Partial<DirectoryCacheConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  get(path: string): FileSystemNode[] | null {
    const entry = this.cache.get(path);
    
    if (!entry) {
      return null;
    }

    if (this.config.ttl > 0) {
      const age = Date.now() - entry.timestamp;
      if (age > this.config.ttl) {
        this.delete(path);
        return null;
      }
    }

    this.updateAccessOrder(path);

    return entry.children;
  }

  set(path: string, children: FileSystemNode[], isComplete: boolean = true): void {
    if (this.cache.size >= this.config.maxEntries && !this.cache.has(path)) {
      this.evictLRU();
    }

    const entry: DirectoryCacheEntry = {
      path,
      children,
      timestamp: Date.now(),
      isComplete,
    };

    this.cache.set(path, entry);
    this.updateAccessOrder(path);
  }

  delete(path: string): boolean {
    const deleted = this.cache.delete(path);
    
    if (deleted) {
      const index = this.accessOrder.indexOf(path);
      if (index > -1) {
        this.accessOrder.splice(index, 1);
      }
    }

    return deleted;
  }

  invalidate(path: string): void {
    const normalizedPath = path.replace(/\\/g, '/');
    const parentPath = this.getParentPath(normalizedPath);
    
    if (parentPath) {
      this.delete(parentPath);
    }
    
    const pathsToDelete: string[] = [];
    this.cache.forEach((_, cachedPath) => {
      const normalizedCached = cachedPath.replace(/\\/g, '/');
      if (normalizedCached === normalizedPath || normalizedCached.startsWith(normalizedPath + '/')) {
        pathsToDelete.push(cachedPath);
      }
    });

    pathsToDelete.forEach(p => this.delete(p));

    this.invalidationCallbacks.forEach(callback => callback(path));
  }

  invalidateBatch(paths: string[]): void {
    const affectedParents = new Set<string>();
    
    paths.forEach(path => {
      const normalizedPath = path.replace(/\\/g, '/');
      const parentPath = this.getParentPath(normalizedPath);
      if (parentPath) {
        affectedParents.add(parentPath);
      }
      
      this.delete(path);
    });

    affectedParents.forEach(parent => this.delete(parent));
  }

  clear(): void {
    this.cache.clear();
    this.accessOrder = [];
  }

  has(path: string): boolean {
    const entry = this.cache.get(path);
    if (!entry) return false;

    if (this.config.ttl > 0) {
      const age = Date.now() - entry.timestamp;
      if (age > this.config.ttl) {
        this.delete(path);
        return false;
      }
    }

    return true;
  }

  getStats(): { size: number; maxEntries: number; hitRate?: number } {
    return {
      size: this.cache.size,
      maxEntries: this.config.maxEntries,
    };
  }

  onInvalidate(callback: (path: string) => void): () => void {
    this.invalidationCallbacks.add(callback);
    return () => {
      this.invalidationCallbacks.delete(callback);
    };
  }

  private updateAccessOrder(path: string): void {
    const index = this.accessOrder.indexOf(path);
    if (index > -1) {
      this.accessOrder.splice(index, 1);
    }
    this.accessOrder.push(path);
  }

  private evictLRU(): void {
    if (this.accessOrder.length > 0) {
      const lruPath = this.accessOrder.shift()!;
      this.cache.delete(lruPath);
    }
  }

  private getParentPath(path: string): string | null {
    const lastSlash = path.lastIndexOf('/');
    if (lastSlash <= 0) return null;
    return path.substring(0, lastSlash);
  }
}

export const directoryCache = new DirectoryCacheClass();

export { DirectoryCacheClass };
