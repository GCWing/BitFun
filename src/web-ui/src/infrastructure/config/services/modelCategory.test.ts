import { describe, expect, it } from 'vitest';
import {
  filterSelectableTextChatModels,
  getEffectiveModelCapabilities,
  isSelectableModelForCapability,
  isSelectableTextChatModel,
} from './modelCategory';

describe('model capability selection', () => {
  it('uses category defaults when a legacy model has no capabilities', () => {
    const legacyChat = {
      id: 'legacy-chat',
      enabled: true,
      category: 'general_chat',
      capabilities: [],
    };

    expect(getEffectiveModelCapabilities(legacyChat)).toContain('text_chat');
    expect(isSelectableTextChatModel(legacyChat)).toBe(true);
    expect(filterSelectableTextChatModels([legacyChat])).toEqual([legacyChat]);
  });

  it('does not infer text chat from an explicit non-chat capability', () => {
    const imageOnlyModel = {
      id: 'image-only',
      enabled: true,
      category: 'multimodal',
      capabilities: ['image_understanding'],
    };

    expect(getEffectiveModelCapabilities(imageOnlyModel)).toEqual(['image_understanding']);
    expect(isSelectableTextChatModel(imageOnlyModel)).toBe(false);
    expect(isSelectableModelForCapability(imageOnlyModel, 'image_understanding')).toBe(true);
  });

  it('requires an enabled model with a stable id for every selectable capability', () => {
    expect(isSelectableTextChatModel({
      enabled: true,
      category: 'general_chat',
      capabilities: ['text_chat'],
    })).toBe(false);
    expect(isSelectableTextChatModel({
      id: 'disabled-chat',
      enabled: false,
      category: 'general_chat',
      capabilities: ['text_chat'],
    })).toBe(false);
    expect(isSelectableTextChatModel({
      id: 'speech-model',
      enabled: true,
      category: 'speech_recognition',
      capabilities: [],
    })).toBe(false);
  });
});
