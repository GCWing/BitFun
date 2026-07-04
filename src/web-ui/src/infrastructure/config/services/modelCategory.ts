import type { ModelCapability, ModelCategory } from '../types';

const MULTIMODAL_MODEL_HINTS = [
  'vision',
  'gpt-4o',
  'gpt-4-turbo',
  'claude-3',
  'gemini-pro-vision',
  'gemini-1.5',
  'kimi',
];

export function inferModelCategory(
  modelName: string,
  _provider?: string
): ModelCategory {
  const normalized = modelName.trim().toLowerCase();
  if (MULTIMODAL_MODEL_HINTS.some(hint => normalized.includes(hint))) {
    return 'multimodal';
  }
  return 'general_chat';
}

export function resolveModelCategory(
  modelName: string,
  category?: ModelCategory,
  provider?: string
): ModelCategory {
  const inferred = inferModelCategory(modelName, provider);

  if (category === 'multimodal') {
    return 'multimodal';
  }

  if (category === 'general_chat' && inferred === 'multimodal') {
    return 'multimodal';
  }

  return category ?? inferred;
}

export function getCapabilitiesByCategory(category: ModelCategory): ModelCapability[] {
  switch (category) {
    case 'multimodal':
      return ['text_chat', 'image_understanding', 'function_calling'];
    case 'general_chat':
    default:
      return ['text_chat', 'function_calling'];
  }
}
