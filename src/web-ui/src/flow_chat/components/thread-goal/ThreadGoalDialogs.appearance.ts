import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';
export const threadGoalDialogsAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'thread-goal-dialogs',
  parts: [
    { id: 'menu', propertyProfile: 'overlay', visualRole: 'dialog' },
    { id: 'header', visualRole: 'toolbar', continuityGroup: 'thread-goal-dialog' },
    { id: 'section', visualRole: 'content' },
    { id: 'objective', visualRole: 'content' },
    { id: 'workflow', visualRole: 'content' },
    { id: 'workflowItem', propertyProfile: 'control', visualRole: 'control' },
    { id: 'footer', visualRole: 'toolbar', continuityGroup: 'thread-goal-dialog' },
    { id: 'actions', visualRole: 'toolbar' },
    { id: 'hint', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'edit', propertyProfile: 'control', visualRole: 'control' },
    { id: 'resume', propertyProfile: 'control', visualRole: 'control' },
  ],
  states: [{ id: 'empty', selector: { kind: 'self', suffix: '[data-openbitfun-state~="empty"]' } }],
};
