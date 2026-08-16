const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';
const POINTER_INTENT_WINDOW_MS = 1500;

export type MotionAwareScrollBehavior = 'auto' | 'smooth';
export type InteractionModality = 'keyboard' | 'pointer' | 'programmatic';
export type InteractionMotion = 'instant' | 'pointer';

let lastInteractionModality: InteractionModality = 'programmatic';
let lastInteractionAt = Number.NEGATIVE_INFINITY;
let removeInteractionModalityListeners: (() => void) | null = null;

function interactionNow(): number {
  return globalThis.performance?.now() ?? Date.now();
}

/**
 * Records the input source that should own the next state transition.
 *
 * Pointer intent is deliberately short-lived: a click-triggered async preload
 * may resolve on a later task, while unrelated background updates must not
 * inherit motion from an old click.
 */
export function recordInteractionModality(modality: InteractionModality): void {
  lastInteractionModality = modality;
  lastInteractionAt = interactionNow();
}

export function getInteractionMotion(): InteractionMotion {
  const pointerIsCurrent = lastInteractionModality === 'pointer'
    && interactionNow() - lastInteractionAt <= POINTER_INTENT_WINDOW_MS;
  return pointerIsCurrent ? 'pointer' : 'instant';
}

/**
 * Installs one capture-phase modality tracker for the application shell.
 * Keyboard activation stays instant; mouse, pen, and touch clicks can opt into
 * the subtle spatial transitions exposed by `getInteractionMotion`.
 */
export function installInteractionModalityTracking(
  target: Document | undefined = globalThis.document,
): () => void {
  if (!target) return () => {};
  if (removeInteractionModalityListeners) return removeInteractionModalityListeners;

  const handlePointerDown = () => recordInteractionModality('pointer');
  const handleKeyDown = () => recordInteractionModality('keyboard');
  target.addEventListener('pointerdown', handlePointerDown, true);
  target.addEventListener('keydown', handleKeyDown, true);

  removeInteractionModalityListeners = () => {
    target.removeEventListener('pointerdown', handlePointerDown, true);
    target.removeEventListener('keydown', handleKeyDown, true);
    removeInteractionModalityListeners = null;
  };
  return removeInteractionModalityListeners;
}

export function isReducedMotionPreferred(): boolean {
  return globalThis.matchMedia?.(REDUCED_MOTION_QUERY).matches ?? false;
}

/**
 * Preserve occasional spatial navigation while respecting reduced motion.
 * Automatic, keyboard-repeated, and other high-frequency paths should pass
 * `auto` directly instead of opting into smooth scrolling.
 */
export function getMotionAwareScrollBehavior(
  preferredBehavior: MotionAwareScrollBehavior,
): MotionAwareScrollBehavior {
  if (preferredBehavior !== 'smooth') {
    return preferredBehavior;
  }

  return isReducedMotionPreferred()
    ? 'auto'
    : preferredBehavior;
}
