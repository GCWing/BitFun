import type {
  AppearanceStorage,
  StoredAppearanceCatalogEntry,
  StoredAppearancePackage,
} from '../types';

const DATABASE_NAME = 'openbitfun-appearance';
const DATABASE_VERSION = 1;
const PACKAGE_STORE = 'packages';
const CATALOG_STORE = 'catalog';

function catalogEntry(value: StoredAppearancePackage): StoredAppearanceCatalogEntry {
  return {
    id: value.manifest.id,
    name: value.manifest.name,
    author: value.manifest.author,
    description: value.manifest.description,
    version: value.manifest.version,
    mode: value.manifest.mode,
    importedAt: value.importedAt,
    marketOrigin: value.marketOrigin,
    localOverride: value.localOverride,
    schemaVersion: value.manifest.schemaVersion,
    archiveSchemaVersion: value.archiveSchemaVersion,
  };
}

export class MemoryAppearanceStorage implements AppearanceStorage {
  private readonly packages = new Map<string, StoredAppearancePackage>();

  async listCatalog(): Promise<StoredAppearanceCatalogEntry[]> {
    return [...this.packages.values()].map(catalogEntry);
  }

  async get(id: string): Promise<StoredAppearancePackage | null> {
    return this.packages.get(id) ?? null;
  }

  async put(value: StoredAppearancePackage): Promise<void> {
    this.packages.set(value.manifest.id, value);
  }

  async delete(id: string): Promise<void> {
    this.packages.delete(id);
  }
}

export class IndexedDbAppearanceStorage implements AppearanceStorage {
  private databasePromise: Promise<IDBDatabase> | null = null;

  private open(): Promise<IDBDatabase> {
    if (this.databasePromise) return this.databasePromise;
    this.databasePromise = new Promise((resolve, reject) => {
      const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
      request.onupgradeneeded = () => {
        const database = request.result;
        if (!database.objectStoreNames.contains(PACKAGE_STORE)) database.createObjectStore(PACKAGE_STORE);
        if (!database.objectStoreNames.contains(CATALOG_STORE)) database.createObjectStore(CATALOG_STORE);
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error('Failed to open appearance storage'));
      request.onblocked = () => reject(new Error('Appearance storage upgrade is blocked'));
    });
    return this.databasePromise;
  }

  private async read<T>(storeName: string, key?: IDBValidKey): Promise<T> {
    const database = await this.open();
    return new Promise<T>((resolve, reject) => {
      const transaction = database.transaction(storeName, 'readonly');
      const store = transaction.objectStore(storeName);
      const request = key === undefined ? store.getAll() : store.get(key);
      request.onsuccess = () => resolve(request.result as T);
      request.onerror = () => reject(request.error ?? new Error('Appearance storage request failed'));
      transaction.onabort = () => reject(transaction.error ?? new Error('Appearance storage transaction aborted'));
    });
  }

  listCatalog(): Promise<StoredAppearanceCatalogEntry[]> {
    return this.read(CATALOG_STORE);
  }

  async get(id: string): Promise<StoredAppearancePackage | null> {
    return (await this.read<StoredAppearancePackage | undefined>(PACKAGE_STORE, id)) ?? null;
  }

  async put(value: StoredAppearancePackage): Promise<void> {
    const database = await this.open();
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction([PACKAGE_STORE, CATALOG_STORE], 'readwrite');
      transaction.objectStore(PACKAGE_STORE).put(value, value.manifest.id);
      transaction.objectStore(CATALOG_STORE).put(catalogEntry(value), value.manifest.id);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error('Failed to store appearance package'));
      transaction.onabort = () => reject(transaction.error ?? new Error('Appearance storage transaction aborted'));
    });
  }

  async delete(id: string): Promise<void> {
    const database = await this.open();
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction([PACKAGE_STORE, CATALOG_STORE], 'readwrite');
      transaction.objectStore(PACKAGE_STORE).delete(id);
      transaction.objectStore(CATALOG_STORE).delete(id);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error('Failed to delete appearance package'));
      transaction.onabort = () => reject(transaction.error ?? new Error('Appearance storage transaction aborted'));
    });
  }

}

export function createAppearanceStorage(): AppearanceStorage {
  return typeof indexedDB === 'undefined' ? new MemoryAppearanceStorage() : new IndexedDbAppearanceStorage();
}
