import React from 'react';

type CanvasRuntimeHooks = {
  useHostTheme?: () => unknown;
  useCanvasState?: <T>(key: string, defaultValue: T) => [T, (value: T | ((previous: T) => T)) => void];
  useCanvasAction?: () => (action: unknown) => Promise<unknown>;
};

declare global {
  interface Window {
    BitfunCanvasRuntimeHooks?: CanvasRuntimeHooks;
  }
}

function runtimeHooks(): CanvasRuntimeHooks {
  if (typeof window === 'undefined') return {};
  return window.BitfunCanvasRuntimeHooks || {};
}

export function useHostTheme() {
  const hook = runtimeHooks().useHostTheme;
  return hook ? hook() : {};
}

export function useCanvasState<T>(key: string, defaultValue: T): [T, (value: T | ((previous: T) => T)) => void] {
  const fallbackState = React.useState(defaultValue);
  const hook = runtimeHooks().useCanvasState;
  return hook ? hook(key, defaultValue) : fallbackState;
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
