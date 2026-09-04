import React from 'react';

type CanvasRuntimeHooks = {
  useHostAppearance?: () => unknown;
  useCanvasState?: <T>(key: string, defaultValue: T, options?: CanvasStateOptions<T>) => [T, (value: T | ((previous: T) => T)) => void];
  useCanvasAction?: () => (action: unknown) => Promise<unknown>;
};

export interface CanvasStateOptions<T> {
  version?: number;
  validate?: (value: unknown) => value is T;
  migrate?: (value: unknown, fromVersion: number) => T;
}

declare global {
  interface Window {
    OpenBitFunCanvasRuntimeHooks?: CanvasRuntimeHooks;
  }
}

function runtimeHooks(): CanvasRuntimeHooks {
  if (typeof window === 'undefined') return {};
  return window.OpenBitFunCanvasRuntimeHooks || {};
}

export function useHostAppearance() {
  const hook = runtimeHooks().useHostAppearance;
  return hook ? hook() : {};
}

export function useCanvasState<T>(key: string, defaultValue: T, options?: CanvasStateOptions<T>): [T, (value: T | ((previous: T) => T)) => void] {
  const fallbackState = React.useState(defaultValue);
  const hook = runtimeHooks().useCanvasState;
  return hook ? hook(key, defaultValue, options) : fallbackState;
}

export function useCanvasAction(): (action: unknown) => Promise<unknown> {
  const hook = runtimeHooks().useCanvasAction;
  return hook ? hook() : async () => null;
}

export const useState = React.useState;
export const useRef = React.useRef;
export const useEffect = React.useEffect;
export const useCallback = React.useCallback;
export const useMemo = React.useMemo;
