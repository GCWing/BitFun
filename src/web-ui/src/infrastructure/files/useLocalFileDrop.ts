import { useEffect, useRef, type RefObject } from 'react';
import { isDragPositionOverElement } from '@/tools/file-system/services/workspaceFileTransfer';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('useLocalFileDrop');
const DROP_DEDUPE_MS = 500;

type DragPosition = { x: number; y: number };
type LocalFileDragPayload =
  | { type: 'enter'; paths: string[]; position: DragPosition }
  | { type: 'over'; position: DragPosition }
  | { type: 'drop'; paths: string[]; position: DragPosition }
  | { type: 'leave' };

export interface LocalFileDropControllerOptions {
  getTarget: () => HTMLElement | null;
  getScaleFactor: () => Promise<number>;
  isEnabled: () => boolean;
  onDragOver?: (isOver: boolean) => void;
  onDropPaths: (paths: string[]) => void | Promise<void>;
  now?: () => number;
}

export interface LocalFileDropController {
  handle(payload: LocalFileDragPayload): Promise<void>;
  dispose(): void;
}

export function createLocalFileDropController(
  options: LocalFileDropControllerOptions,
): LocalFileDropController {
  let enterPaths: string[] = [];
  let disposed = false;
  let lastDrop: { signature: string; at: number } | null = null;

  const clear = () => {
    enterPaths = [];
    options.onDragOver?.(false);
  };

  return {
    async handle(payload) {
      const enabled = options.isEnabled();
      if (disposed) return;
      if (payload.type === 'leave') {
        clear();
        return;
      }
      if (!enabled) {
        if (payload.type === 'drop') clear();
        return;
      }
      if (payload.type === 'enter') {
        enterPaths = [...payload.paths];
        return;
      }

      const scaleFactor = await options.getScaleFactor();
      const target = options.getTarget();
      const hit = isDragPositionOverElement(payload.position, scaleFactor, target);
      if (payload.type === 'over') {
        options.onDragOver?.(hit);
        return;
      }

      const shouldHandle = hit;
      const paths = payload.paths.length > 0 ? payload.paths : [...enterPaths];
      clear();
      if (!shouldHandle || paths.length === 0) return;

      const signature = paths.join('\0');
      const now = options.now?.() ?? Date.now();
      if (lastDrop && lastDrop.signature === signature && now - lastDrop.at < DROP_DEDUPE_MS) return;
      lastDrop = { signature, at: now };
      await options.onDropPaths(paths);
    },
    dispose() {
      disposed = true;
      clear();
    },
  };
}

export interface UseLocalFileDropOptions {
  targetRef: RefObject<HTMLElement | null>;
  enabled?: boolean;
  onDragOver?: (isOver: boolean) => void;
  onDropPaths: (paths: string[]) => void | Promise<void>;
}

export function useLocalFileDrop({
  targetRef,
  enabled = true,
  onDragOver,
  onDropPaths,
}: UseLocalFileDropOptions): void {
  const enabledRef = useRef(enabled);
  const onDragOverRef = useRef(onDragOver);
  const onDropPathsRef = useRef(onDropPaths);
  enabledRef.current = enabled;
  onDragOverRef.current = onDragOver;
  onDropPathsRef.current = onDropPaths;

  useEffect(() => {
    const hasWindow = typeof window !== 'undefined';
    const hasGlobalTauri = hasWindow && '__TAURI__' in window;
    if (!hasWindow || !hasGlobalTauri) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let controller: LocalFileDropController | undefined;

    const setup = async () => {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const webview = getCurrentWebview();
        if (cancelled) return;

        controller = createLocalFileDropController({
          getTarget: () => targetRef.current,
          getScaleFactor: () => webview.window.scaleFactor(),
          isEnabled: () => enabledRef.current,
          onDragOver: (isOver) => onDragOverRef.current?.(isOver),
          onDropPaths: (paths) => onDropPathsRef.current(paths),
        });
        unlisten = await webview.onDragDropEvent((event) => {
          if (!controller) return;
          void controller.handle(event.payload).catch((error) => {
            log.warn('Failed to handle local file drag-drop event', error);
          });
        });
        if (cancelled) unlisten();
      } catch (error) {
        log.warn('Local file drag-drop listener not available', error);
      }
    };

    void setup();
    return () => {
      cancelled = true;
      controller?.dispose();
      unlisten?.();
    };
  }, [targetRef]);
}
