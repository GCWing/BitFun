/**
 * sceneStore — compatibility re-export.
 *
 * @deprecated Use overlayStore directly.
 * This module is kept only so remaining import sites compile without changes.
 * All new code should import from overlayStore.
 */

export { useOverlayStore as useSceneStore } from './overlayStore';
