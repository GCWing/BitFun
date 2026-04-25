class StorageAdapter {
    private isAvailable: boolean;
    private memoryStorage: Map<string, string> = new Map();

    constructor() {
        this.isAvailable = this.checkAvailability();
    }

    private checkAvailability(): boolean {
        try {
            const storageKey = '__storageKey__';
            localStorage.setItem(storageKey, storageKey);
            localStorage.removeItem(storageKey);
            return true;
        } catch (e) {
            console.error("SessionStorageAdapter check failed for session storage", e);
            return false;
        }
    }

    
    getItem(key: string): string | null {
        if (this.isAvailable) {
            try {
                return localStorage.getItem(key);
            } catch (e) {
                console.error(`Failed to get ${key} for `, e);
            }
        }
        return this.memoryStorage.get(key) || null;
    }

    setItem(key: string, value: string): void {
        if (this.isAvailable) {
            try {
                localStorage.setItem(key, value);
                this.memoryStorage.set(key, value);
                return;
            } catch (e) {
                console.warn(`Failed to set ${key} Local storage not available`, e);
            }
        }
        this.memoryStorage.set(key, value);
    }

    removeItem(key: string): void {
        if (this.isAvailable) {
            try {
                localStorage.removeItem(key);
                this.memoryStorage.delete(key);
                return;
            } catch (e) {
                console.warn(`Failed to delete ${key} Local storage not available`, e);
            }
        }
        this.memoryStorage.delete(key);
    }

    clear(): void {
        if (this.isAvailable) {
            try {
                localStorage.clear();
                this.memoryStorage.clear();
                return;
            } catch (e) {
                console.warn(`Failed to clear storage. Local storage not available`, e);
            }
        }
        this.memoryStorage.clear();
    }

    getKeys(): string[] {
        if (this.isAvailable) {
            try {
                return Object.keys(localStorage);
            } catch (e) {
                console.warn(`Failed to get keys. Local storage not available`, e);
            }
        }
        return Array.from(this.memoryStorage.keys());
    }

    hasKey(key: string): boolean {
        return this.getItem(key) !== null;
    }
}

export const storage = new StorageAdapter();