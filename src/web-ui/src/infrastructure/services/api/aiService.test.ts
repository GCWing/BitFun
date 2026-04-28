import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelConfig } from '../../../shared/types';

const mocks = vi.hoisted(() => ({
  initializeAI: vi.fn(),
  notificationWarning: vi.fn(),
  translate: vi.fn((key: string) => {
    const messages: Record<string, string> = {
      'errors:ai.initializeFailedTitle': 'AI 客户端初始化失败',
      'errors:ai.primaryModelNotConfigured': '请先在设置中配置主模型',
    };
    return messages[key] ?? key;
  }),
}));

vi.mock('@/infrastructure/api', () => ({
  aiApi: {
    initializeAI: mocks.initializeAI,
  },
}));

vi.mock('../../../shared/notification-system', () => ({
  notificationService: {
    warning: mocks.notificationWarning,
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: mocks.translate,
  },
}));

import { AIService } from './aiService';

const modelConfig: ModelConfig = {
  id: 'model-1',
  name: 'Test model',
  baseUrl: 'https://example.test',
  apiKey: 'test-key',
  modelName: 'test-model',
  format: 'openai',
};

describe('AIService', () => {
  beforeEach(() => {
    vi.stubGlobal('window', { dispatchEvent: vi.fn() });
    vi.stubGlobal('CustomEvent', class CustomEvent<T = unknown> {
      readonly type: string;
      readonly detail?: T;

      constructor(type: string, eventInitDict?: CustomEventInit<T>) {
        this.type = type;
        this.detail = eventInitDict?.detail;
      }
    });
    AIService.reset();
    mocks.initializeAI.mockReset();
    mocks.notificationWarning.mockReset();
    mocks.translate.mockClear();
  });

  it('localizes primary model initialization failures without replacing the raw thrown error', async () => {
    const rawError = 'Primary model not configured, please configure it in settings';
    mocks.initializeAI.mockRejectedValueOnce(new Error(rawError));

    await expect(AIService.initializeAI(modelConfig)).rejects.toThrow(rawError);

    expect(mocks.notificationWarning).toHaveBeenCalledWith(
      '请先在设置中配置主模型',
      expect.objectContaining({
        title: 'AI 客户端初始化失败',
      }),
    );
  });
});
