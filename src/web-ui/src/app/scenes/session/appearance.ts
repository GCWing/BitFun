import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance/types';

export const sessionAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'session',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'workspace', continuityGroup: 'session-workspace' },
    { id: 'main', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'chat', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'session-workspace' },
    { id: 'auxiliary', propertyProfile: 'layout', visualRole: 'panel', continuityGroup: 'session-workspace' },
    { id: 'terminal', propertyProfile: 'layout', visualRole: 'panel', continuityGroup: 'session-workspace' },
  ],
  states: [
    { id: 'dragging', selector: { kind: 'self', suffix: '[data-openbitfun-state~="dragging"]' } },
    { id: 'terminalBottom', selector: { kind: 'self', suffix: '[data-openbitfun-state~="terminal-bottom"]' } },
  ],
};
