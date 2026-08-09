/**
 * Shared ResizeObserver.
 *
 * A single module-level ResizeObserver multiplexed across many elements.
 * Prefer this over registering one `window.addEventListener('resize', ...)`
 * per component instance: observer callbacks run after layout in a single
 * batch, so per-element measurement reads do not force synchronous layout,
 * and long lists do not accumulate N window listeners.
 */

import { createLogger } from './logger';

const log = createLogger('sharedResizeObserver');

type ResizeCallback = (entry: ResizeObserverEntry) => void;

const callbacksByElement = new Map<Element, ResizeCallback>();

let sharedObserver: ResizeObserver | null = null;

function ensureObserver(): ResizeObserver | null {
  if (typeof ResizeObserver === 'undefined') {
    return null;
  }
  if (!sharedObserver) {
    sharedObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        // Isolate subscribers: this observer is shared, so a throwing callback
        // must not abort the batch and silently stop resize handling for every
        // other observed element.
        try {
          callbacksByElement.get(entry.target)?.(entry);
        } catch (error) {
          // Report but keep going: swallowing silently would hide real
          // subscriber bugs behind the shared observer.
          log.error('Subscriber callback threw', error);
        }
      }
    });
  }
  return sharedObserver;
}

/**
 * Observe size changes of `element` via the shared observer.
 * Only one callback per element is supported; a second call for the same
 * element replaces the previous callback.
 *
 * @returns cleanup function that stops observing the element.
 */
export function observeElementResize(
  element: Element,
  callback: ResizeCallback,
): () => void {
  const observer = ensureObserver();
  if (!observer) {
    return () => {};
  }

  callbacksByElement.set(element, callback);
  observer.observe(element);

  return () => {
    if (callbacksByElement.get(element) === callback) {
      callbacksByElement.delete(element);
      observer.unobserve(element);
    }
  };
}
