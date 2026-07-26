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

const SPEECH_RECOGNITION_MODEL_HINTS = [
  'asr',
  'transcribe',
  'transcription',
  'whisper',
  'speech',
];

export function inferModelCategory(
  modelName: string,
  _provider?: string
): ModelCategory {
  const normalized = modelName.trim().toLowerCase();
  if (SPEECH_RECOGNITION_MODEL_HINTS.some(hint => normalized.includes(hint))) {
    return 'speech_recognition';
  }
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

  if (category === 'speech_recognition') {
    return 'speech_recognition';
  }

  if (category === 'general_chat' && inferred === 'multimodal') {
    return 'multimodal';
  }

  if (category === 'general_chat' && inferred === 'speech_recognition') {
    return 'speech_recognition';
  }

  return category ?? inferred;
}

export function getCapabilitiesByCategory(category: ModelCategory): ModelCapability[] {
  switch (category) {
    case 'speech_recognition':
      return ['speech_recognition'];
    case 'multimodal':
      return ['text_chat', 'image_understanding', 'function_calling'];
    case 'general_chat':
    default:
      return ['text_chat', 'function_calling'];
  }
}
