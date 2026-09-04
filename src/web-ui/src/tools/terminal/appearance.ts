import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const terminalToolAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'terminal-tool',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'terminal-tool' },
    { id: 'loading', visualRole: 'content' },
    { id: 'error', visualRole: 'content' },
    { id: 'toolbar', visualRole: 'toolbar', continuityGroup: 'terminal-tool' },
    { id: 'statusBar', visualRole: 'toolbar', continuityGroup: 'terminal-tool' },
    { id: 'terminal', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'terminal-tool' },
    { id: 'output', propertyProfile: 'layout', visualRole: 'continuous-surface', continuityGroup: 'terminal-tool' },
    { id: 'screen', propertyProfile: 'layout', visualRole: 'content' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-openbitfun-state~="error"]' } },
    { id: 'running', selector: { kind: 'self', suffix: '[data-openbitfun-state~="running"]' } },
    { id: 'exited', selector: { kind: 'self', suffix: '[data-openbitfun-state~="exited"]' } },
  ],
};
