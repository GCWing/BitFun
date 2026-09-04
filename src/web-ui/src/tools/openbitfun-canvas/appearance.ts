import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
import { canvasAppearanceAdapter } from '@/infrastructure/appearance/adapters/CanvasAppearanceAdapter';
import { readWidgetAppearancePayload } from '@/tools/generative-widget/appearancePayload';

export interface CanvasHostAppearancePayload {
  type: 'light' | 'dark' | 'auto';
  id?: string;
  vars?: Record<string, string>;
  bg: string;
  panel: string;
  fg: string;
  muted: string;
  border: string;
  accent: string;
  success: string;
  warning: string;
  danger: string;
  info: string;
}

export function readHostAppearancePayload(): CanvasHostAppearancePayload {
  const settings = canvasAppearanceAdapter.getSettings();
  const widgetAppearance = readWidgetAppearancePayload();
  return {
    type: settings.mode,
    id: settings.id,
    vars: widgetAppearance?.vars,
    bg: settings.bg,
    panel: settings.panel,
    fg: settings.fg,
    muted: settings.muted,
    border: settings.border,
    accent: settings.accent,
    success: settings.success,
    warning: settings.warning,
    danger: settings.danger,
    info: settings.info,
  };
}

export const canvasToolAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'canvas-tool',
  parts: [
    { id: 'root' },
    { id: 'empty' },
    { id: 'message' },
    { id: 'diagnostics' },
    { id: 'source' },
    { id: 'toolbar' },
    { id: 'frame' },
    { id: 'sourceOverlay' },
    { id: 'sourceDialog' },
  ],
  states: [
    { id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } },
  ],
};
