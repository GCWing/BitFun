import { SnapshotInitializer } from './SnapshotInitializer';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SnapshotLazyLoader');

class SnapshotLazyLoader {
  private static _isInitializing = false;
  private static initPromise: Promise<void> | null = null;

  /**
   * Ensure the snapshot system is initialized.
   *
   * Safe to call multiple times:
   * - If already initialized, returns immediately.
   * - If initialization is in-flight, returns the same promise.
   */
  static async ensureInitialized(): Promise<void> {
    if (SnapshotInitializer.isInitialized()) {
      return;
    }

    if (this.initPromise) {
      return this.initPromise;
    }

    this._isInitializing = true;
    
    this.initPromise = SnapshotInitializer.initialize()
      .then(() => {
        this._isInitializing = false;
        log.info('Snapshot system lazy loaded');
      })
      .catch(error => {
        this._isInitializing = false;
        this.initPromise = null; // allow retry after a failed init
        log.error('Failed to initialize snapshot system', error);
        throw error;
      });

    return this.initPromise;
  }

  static isInitializing(): boolean {
    return this._isInitializing;
  }

  static isInitialized(): boolean {
    return SnapshotInitializer.isInitialized();
  }
}

export default SnapshotLazyLoader;
