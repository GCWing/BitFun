/**
 * Overlay system type definitions.
 *
 * Two-layer scene model:
 *   - Base layer: 'session' (Agentic OS) — always mounted, never navigated away from
 *   - Overlay layer: OverlaySceneId — shown on top of the base; null = base is fully visible
 */

import type { LucideIcon } from 'lucide-react';

/** Scenes that appear as overlays on top of the Agentic OS base session. */
export type OverlaySceneId =
  | 'terminal'
  | 'settings'
  | 'file-viewer'
  | 'profile'
  | 'apps'
  | 'subagents'
  | 'skills'
  | 'tools'
  | 'assistant'
  | 'shell'
  | 'panel-view'
  | 'task-detail'
  | `live-app:${string}`;

/** Full scene identifier — base or overlay. */
export type SceneId = 'session' | OverlaySceneId;

/**
 * @deprecated Use OverlaySceneId or SceneId instead.
 * Kept as a compatibility alias while callers are migrated.
 */
export type SceneTabId = SceneId;

/** Static definition for an overlay scene. */
export interface OverlaySceneDef {
  id: OverlaySceneId;
  label: string;
  /** i18n key under common namespace */
  labelKey?: string;
  Icon?: LucideIcon;
}
