import type { ShortcutScope } from '@/shared/types/shortcut';

export interface DismissibleLayerDescriptor {
  id: string;
  scope: ShortcutScope;
  onDismiss: () => void;
}

type DismissibleLayerListener = () => void;

class DismissibleLayerManager {
  private layers: DismissibleLayerDescriptor[] = [];

  private listeners = new Set<DismissibleLayerListener>();

  private version = 0;

  register(layer: DismissibleLayerDescriptor): () => void {
    this.layers = [...this.layers.filter(existing => existing.id !== layer.id), layer];
    this.bumpVersion();

    return () => {
      const nextLayers = this.layers.filter(existing => existing.id !== layer.id);
      if (nextLayers.length === this.layers.length) {
        return;
      }

      this.layers = nextLayers;
      this.bumpVersion();
    };
  }

  dismissTop(scope?: ShortcutScope): boolean {
    const index = this.findTopIndex(scope);
    if (index === -1) {
      return false;
    }

    const [layer] = this.layers.splice(index, 1);
    this.bumpVersion();
    layer.onDismiss();
    return true;
  }

  dismissAll(): boolean {
    if (this.layers.length === 0) {
      return false;
    }

    const layers = [...this.layers].reverse();
    this.layers = [];
    this.bumpVersion();

    for (const layer of layers) {
      layer.onDismiss();
    }

    return true;
  }

  hasLayers(scope?: ShortcutScope): boolean {
    return this.findTopIndex(scope) !== -1;
  }

  subscribe(listener: DismissibleLayerListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  getVersion(): number {
    return this.version;
  }

  private findTopIndex(scope?: ShortcutScope): number {
    if (!scope) {
      return this.layers.length - 1;
    }

    for (let index = this.layers.length - 1; index >= 0; index -= 1) {
      if (this.layers[index]?.scope === scope) {
        return index;
      }
    }

    return -1;
  }

  private bumpVersion(): void {
    this.version += 1;
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const dismissibleLayerManager = new DismissibleLayerManager();
