import { createLogger } from '@/shared/utils/logger';

const log = createLogger('CreationRuntime');

/** Loads user UI code only after the application shell has rendered. */
export interface CreationModule<Api> {
  default?: (api: Api) => void | (() => void) | Promise<void | (() => void)>;
}

export interface CreationRuntimeOptions<Api> {
  api: Api;
  disposeApi: () => void;
  signal: AbortSignal;
  loadModule?: (url: string) => Promise<CreationModule<Api>>;
}

export async function activateCreationRuntime<Api>({
  api, disposeApi, signal,
  loadModule = url => import(/* @vite-ignore */ url),
}: CreationRuntimeOptions<Api>): Promise<() => void> {
  let deactivate: void | (() => void);
  let disposed = false;
  const stylesheet = document.createElement('link');
  stylesheet.rel = 'stylesheet';
  stylesheet.href = new URL('/openbitfun-creation.css', window.location.href).href;
  stylesheet.dataset.openbitfunCreation = 'stylesheet';
  const styleOrder = new MutationObserver(() => {
    if (!disposed && stylesheet.isConnected && document.head.lastElementChild !== stylesheet) {
      document.head.append(stylesheet);
    }
  });
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    signal.removeEventListener('abort', dispose);
    styleOrder.disconnect();
    try { deactivate?.(); } catch (error) {
      log.warn('Failed to clean up UI customization', { error });
    } finally {
      try { disposeApi(); } finally { stylesheet.remove(); }
    }
  };
  signal.addEventListener('abort', dispose, { once: true });
  try {
    signal.throwIfAborted();
    await new Promise<void>((resolve, reject) => {
      const abort = () => reject(new DOMException('Customization cancelled', 'AbortError'));
      const finish = (error?: Error) => {
        signal.removeEventListener('abort', abort);
        stylesheet.onload = null;
        stylesheet.onerror = null;
        if (error) reject(error); else resolve();
      };
      signal.addEventListener('abort', abort, { once: true });
      stylesheet.onload = () => finish();
      stylesheet.onerror = () => finish(new Error('Failed to load UI customization stylesheet'));
      // Last in head: overrides win over the packaged CSS at equal specificity.
      document.head.append(stylesheet);
    });
    signal.throwIfAborted();
    styleOrder.observe(document.head, { childList: true });
    const module = await loadModule(new URL('/openbitfun-creation.js', window.location.href).href);
    signal.throwIfAborted();
    if (module.default !== undefined && typeof module.default !== 'function') {
      throw new Error('UI customization must export a default activation function');
    }
    const cleanup = await module.default?.(api);
    if (cleanup !== undefined && typeof cleanup !== 'function') {
      throw new Error('UI customization activation must return a cleanup function or nothing');
    }
    deactivate = cleanup;
    // Async activation can finish after a surface switch. Dispose its result too.
    if (disposed) { deactivate?.(); signal.throwIfAborted(); }
    return dispose;
  } catch (error) {
    dispose();
    throw error;
  }
}
