import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const createPlanDisplayAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'create-plan-display',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'headerMain' }, { id: 'content' },
    { id: 'overview' }, { id: 'todos' }, { id: 'todo' }, { id: 'footer' },
    { id: 'loading' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-openbitfun-state~="loading"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-openbitfun-state~="expanded"]' } },
  ],
};
