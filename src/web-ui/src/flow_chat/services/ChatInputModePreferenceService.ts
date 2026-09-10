import type { AppFlowChatConfig } from '@/infrastructure/config/types';
import { configManager } from '@/infrastructure/config/services/ConfigManager';

import { normalizeUserDefaultChatInputModeId } from '../utils/chatInputMode';

export const CHAT_INPUT_MODE_PREFERENCE_CONFIG_PATH = 'app.flow_chat';

export type ChatInputDefaultModeStrategy = 'follow_last' | 'fixed';

export interface ChatInputModePreference {
  strategy: ChatInputDefaultModeStrategy;
  fixedModeId: string | null;
  lastModeId: string | null;
}

const INELIGIBLE_DEFAULT_MODE_IDS = new Set(['claw', 'multitask', 'plan']);

function isEligibleDefaultModeId(modeId: string | null): modeId is string {
  return Boolean(modeId && !INELIGIBLE_DEFAULT_MODE_IDS.has(modeId.toLowerCase()));
}

/**
 * Missing strategy is an upgrade-compatibility state:
 * - an existing `default_mode_id` keeps its historical fixed-default meaning;
 * - an untouched config adopts the new follow-last behavior.
 */
export function normalizeChatInputModePreference(
  config: AppFlowChatConfig | null | undefined,
): ChatInputModePreference {
  const storedFixedModeId = normalizeUserDefaultChatInputModeId(config?.default_mode_id);
  const storedLastModeId = normalizeUserDefaultChatInputModeId(config?.last_mode_id);
  const fixedModeId = isEligibleDefaultModeId(storedFixedModeId) ? storedFixedModeId : null;
  const lastModeId = isEligibleDefaultModeId(storedLastModeId) ? storedLastModeId : null;
  const configuredStrategy = config?.default_mode_strategy;
  const strategy: ChatInputDefaultModeStrategy =
    configuredStrategy === 'follow_last' || configuredStrategy === 'fixed'
      ? configuredStrategy
      : storedFixedModeId
        ? 'fixed'
        : 'follow_last';

  return {
    strategy,
    fixedModeId,
    lastModeId,
  };
}

/** Returns null when the Standard Harness fallback should be used. */
export function resolveConfiguredChatInputDefaultModeId(
  preference: ChatInputModePreference,
): string | null {
  const candidate = preference.strategy === 'fixed'
    ? preference.fixedModeId
    : preference.lastModeId;
  return isEligibleDefaultModeId(candidate) ? candidate : null;
}

export function withChatInputDefaultModeStrategy(
  preference: ChatInputModePreference,
  strategy: ChatInputDefaultModeStrategy,
): ChatInputModePreference {
  return {
    ...preference,
    strategy,
    fixedModeId:
      strategy === 'fixed' && !preference.fixedModeId
        ? preference.lastModeId ?? 'Standard'
        : preference.fixedModeId,
  };
}

export function withFixedChatInputDefaultMode(
  preference: ChatInputModePreference,
  modeId: string,
): ChatInputModePreference {
  const normalizedModeId = normalizeUserDefaultChatInputModeId(modeId);
  if (!isEligibleDefaultModeId(normalizedModeId)) {
    throw new Error('A supported default ChatInput mode id is required');
  }
  return {
    ...preference,
    strategy: 'fixed',
    fixedModeId: normalizedModeId,
  };
}

class ChatInputModePreferenceService {
  async getPreference(): Promise<ChatInputModePreference> {
    const config = await configManager.getConfig<AppFlowChatConfig>(
      CHAT_INPUT_MODE_PREFERENCE_CONFIG_PATH,
    );
    return normalizeChatInputModePreference(config);
  }

  subscribe(
    listener: (preference: ChatInputModePreference) => void,
    onError?: (error: unknown) => void,
  ): () => void {
    return configManager.watch(CHAT_INPUT_MODE_PREFERENCE_CONFIG_PATH, () => {
      void this.getPreference()
        .then(listener)
        .catch(error => onError?.(error));
    });
  }

  async setStrategy(strategy: ChatInputDefaultModeStrategy): Promise<ChatInputModePreference> {
    const nextConfig = await configManager.updateConfig<AppFlowChatConfig>(
      CHAT_INPUT_MODE_PREFERENCE_CONFIG_PATH,
      currentConfig => {
        const nextPreference = withChatInputDefaultModeStrategy(
          normalizeChatInputModePreference(currentConfig),
          strategy,
        );
        return {
          ...currentConfig,
          default_mode_strategy: nextPreference.strategy,
          default_mode_id: nextPreference.fixedModeId,
        };
      },
    );
    return normalizeChatInputModePreference(nextConfig);
  }

  async setFixedMode(modeId: string): Promise<ChatInputModePreference> {
    const nextConfig = await configManager.updateConfig<AppFlowChatConfig>(
      CHAT_INPUT_MODE_PREFERENCE_CONFIG_PATH,
      currentConfig => {
        const nextPreference = withFixedChatInputDefaultMode(
          normalizeChatInputModePreference(currentConfig),
          modeId,
        );
        return {
          ...currentConfig,
          default_mode_strategy: nextPreference.strategy,
          default_mode_id: nextPreference.fixedModeId,
        };
      },
    );
    return normalizeChatInputModePreference(nextConfig);
  }

  async rememberMode(modeId: string): Promise<ChatInputModePreference> {
    const normalizedModeId = normalizeUserDefaultChatInputModeId(modeId);
    if (!isEligibleDefaultModeId(normalizedModeId)) {
      return this.getPreference();
    }

    const nextConfig = await configManager.updateConfig<AppFlowChatConfig>(
      CHAT_INPUT_MODE_PREFERENCE_CONFIG_PATH,
      currentConfig => ({
        ...currentConfig,
        last_mode_id: normalizedModeId,
      }),
    );
    return normalizeChatInputModePreference(nextConfig);
  }
}

export const chatInputModePreferenceService = new ChatInputModePreferenceService();
