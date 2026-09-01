import { useCallback, useContext, useSyncExternalStore } from "react";
import { LayerStackContext } from "../providers/DesignSystemProvider.context";
import type {
  OverlayDismissReason,
  OverlayLayerScope,
} from "./types";

export interface OverlayLayerDescriptor {
  id: symbol;
  onDismiss: (reason: OverlayDismissReason) => void;
  scope?: OverlayLayerScope;
}

export class OverlayLayerStack {
  private layers: OverlayLayerDescriptor[] = [];
  private listeners = new Set<() => void>();
  private version = 0;

  register(layer: OverlayLayerDescriptor): () => void {
    this.layers = [
      ...this.layers.filter((candidate) => candidate.id !== layer.id),
      layer,
    ];
    this.notify();

    return () => {
      const nextLayers = this.layers.filter((candidate) => candidate.id !== layer.id);
      if (nextLayers.length === this.layers.length) return;
      this.layers = nextLayers;
      this.notify();
    };
  }

  isTopLayer(id: symbol): boolean {
    return this.layers[this.layers.length - 1]?.id === id;
  }

  dismissTop(scope?: OverlayLayerScope): boolean {
    const index = this.findTopIndex(scope);
    const layer = index >= 0 ? this.layers[index] : undefined;
    if (!layer) return false;
    layer.onDismiss("programmatic");
    return true;
  }

  dismissAll(): boolean {
    if (this.layers.length === 0) return false;
    [...this.layers].reverse().forEach((layer) => {
      layer.onDismiss("programmatic");
    });
    return true;
  }

  hasLayers(scope?: OverlayLayerScope): boolean {
    return this.findTopIndex(scope) >= 0;
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getSnapshot = (): number => this.version;

  private findTopIndex(scope?: OverlayLayerScope): number {
    if (!scope) return this.layers.length - 1;
    for (let index = this.layers.length - 1; index >= 0; index -= 1) {
      if (this.layers[index]?.scope === scope) return index;
    }
    return -1;
  }

  private notify(): void {
    this.version += 1;
    this.listeners.forEach((listener) => listener());
  }
}

const fallbackLayerStack = new OverlayLayerStack();

export function useOverlayLayerStack(): OverlayLayerStack {
  return useContext(LayerStackContext) ?? fallbackLayerStack;
}

export function useHasOverlayLayers(scope?: OverlayLayerScope): boolean {
  const stack = useOverlayLayerStack();
  useSyncExternalStore(stack.subscribe, stack.getSnapshot, stack.getSnapshot);
  return stack.hasLayers(scope);
}

export function useOverlayLayerActions() {
  const stack = useOverlayLayerStack();
  const dismissTop = useCallback(
    (scope?: OverlayLayerScope) => stack.dismissTop(scope),
    [stack],
  );
  const dismissAll = useCallback(() => stack.dismissAll(), [stack]);
  return { dismissAll, dismissTop };
}
