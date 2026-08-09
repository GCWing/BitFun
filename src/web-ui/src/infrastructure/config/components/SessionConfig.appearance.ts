import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const sessionConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'session-config',
  parts: [
    { id: 'root' },
    { id: 'content' },
    { id: 'control' },
    { id: 'petPicker' },
    { id: 'petChooser' },
    { id: 'petTrigger' },
    { id: 'petList' },
    { id: 'petGroup' },
    { id: 'petOption' },
    { id: 'petOptionMain' },
    { id: 'petActions' },
    { id: 'platformNote' },
    { id: 'debugInputs' },
    { id: 'debugActions' },
    { id: 'templateModal' },
    { id: 'templateCard' },
    { id: 'templateHeader' },
    { id: 'templateInfo' },
    { id: 'templateContent' },
    { id: 'templateField' },
    { id: 'templateNotes' },
    { id: 'modalFooter' },
    { id: 'restartModal' },
  ],
  facets: [
    { id: 'view', attribute: 'data-bf-view', values: ['personalization', 'permissions'] },
  ],
  states: [
    { id: 'selected', selector: { kind: 'self', suffix: '[data-bf-state~="selected"]' } },
    { id: 'expanded', selector: { kind: 'self', suffix: '[data-bf-state~="expanded"]' } },
  ],
};
