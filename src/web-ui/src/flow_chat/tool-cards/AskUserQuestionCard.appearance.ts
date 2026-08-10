import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const askUserQuestionCardAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'ask-user-question-card',
  parts: [
    { id: 'root' }, { id: 'header' }, { id: 'questions' }, { id: 'question' },
    { id: 'options' }, { id: 'option' }, { id: 'customInput' }, { id: 'footer' },
    { id: 'status' }, { id: 'summary' }, { id: 'loading' }, { id: 'error' },
  ],
  states: [
    { id: 'loading', selector: { kind: 'self', suffix: '[data-bf-state~="loading"]' } },
    { id: 'error', selector: { kind: 'self', suffix: '[data-bf-state~="error"]' } },
    { id: 'completed', selector: { kind: 'self', suffix: '[data-bf-state~="completed"]' } },
  ],
};
