/**
 * TTS playback hook for AI responses.
 * Watches FlowChatStore for streaming text completion and speaks the content.
 * Supports streaming mode: speaks text incrementally as AI streams tokens.
 */

import { useEffect, useRef, useCallback, useState } from 'react';
import { FlowChatStore } from '../store/FlowChatStore';
import { createTtsProvider, TtsProvider } from '@/infrastructure/voice/TtsProviderFactory';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatState, FlowTextItem } from '../types/flow-chat';

const log = createLogger('TtsPlayback');

interface VoiceSettings {
  sttEnabled: boolean;
  ttsEnabled: boolean;
  sttProvider: 'webspeech' | 'faster-whisper';
  ttsProvider: 'edge' | 'elevenlabs';
  ttsVoice: string;
  language: string;
}

interface UseTtsPlaybackOptions {
  /** Called when TTS starts speaking */
  onSpeakingStart?: () => void;
  /** Called when TTS finishes */
  onSpeakingEnd?: () => void;
}

function stripMarkdown(text: string): string {
  return text
    .replace(/<scratchpad>[\s\S]*?<\/scratchpad>/gi, '')
    .replace(/<thinking>[\s\S]*?<\/thinking>/gi, '')
    .replace(/<反思>[\s\S]*?<\/反思>/gi, '')
    .replace(/<scratch[\s\S]*?<\/scratch>/gi, '')
    .replace(/<noscratch>[\s\S]*?<\/noscratch>/gi, '')
    .replace(/<search_plan>[\s\S]*?<\/search_plan>/gi, '')
    .replace(/<execution_summary>[\s\S]*?<\/execution_summary>/gi, '')
    .replace(/<file_plan>[\s\S]*?<\/file_plan>/gi, '')
    .replace(/<viewplan>[\s\S]*?<\/viewplan>/gi, '')
    .replace(/<CodeOutputBlock>[\s\S]*?<\/CodeOutputBlock>/gi, '[code]')
    .replace(/<[^>]*think[^>]*>[\s\S]*?<\/[^>]*think[^>]*>/gi, '')
    .replace(/<[^>]*reasoning[^>]*>[\s\S]*?<\/[^>]*reasoning[^>]*>/gi, '')
    .replace(/<[^>]*thought[^>]*>[\s\S]*?<\/[^>]*thought[^>]*>/gi, '')
    .replace(/<[^>]*scratchpad[^>]*>[\s\S]*?<\/[^>]*scratchpad[^>]*>/gi, '')
    .replace(/^###?\[.*?\].*?$/gm, '')
    .replace(/#{1,6}\s/g, '')
    .replace(/\*\*(.+?)\*\*/g, '$1')
    .replace(/\*(.+?)\*/g, '$1')
    .replace(/`(.+?)`/g, '$1')
    .replace(/```[\s\S]*?```/gm, '[code]')
    .replace(/\[(.+?)\]\(.+?\)/g, '$1')
    .replace(/\n+/g, ' ')
    .trim();
}

export function useTtsPlayback(options: UseTtsPlaybackOptions = {}) {
  const { onSpeakingStart, onSpeakingEnd } = options;

  const [isSpeaking, setIsSpeaking] = useState(false);
  const [currentTextItemId, setCurrentTextItemId] = useState<string | null>(null);

  const ttsRef = useRef<TtsProvider | null>(null);
  const speakingItemIdRef = useRef<string | null>(null);
  const activeItemIdRef = useRef<string | null>(null);
  const prevRoundIdRef = useRef<string | null>(null);
  const voiceConfigRef = useRef<VoiceSettings | null>(null);

  const loadVoiceConfig = useCallback((): VoiceSettings => {
    try {
      const stored = configManager.getConfig<VoiceSettings>('voice');
      log.debug('loadVoiceConfig', { stored });
      voiceConfigRef.current = {
        sttEnabled: true,
        ttsEnabled: true,
        sttProvider: 'webspeech',
        ttsProvider: 'edge',
        ttsVoice: 'zh-CN-XiaoxiaoNeural',
        language: 'zh-CN',
        ...stored,
      };
    } catch (err) {
      log.warn('loadVoiceConfig failed, using defaults', { err });
      voiceConfigRef.current = {
        sttEnabled: true,
        ttsEnabled: true,
        sttProvider: 'webspeech',
        ttsProvider: 'edge',
        ttsVoice: 'zh-CN-XiaoxiaoNeural',
        language: 'zh-CN',
      };
    }
    return voiceConfigRef.current;
  }, []);

  useEffect(() => {
    const cfg = loadVoiceConfig();
    if (!cfg.ttsEnabled) return;
    if (ttsRef.current) return;
    ttsRef.current = createTtsProvider(cfg.ttsProvider);
  }, [loadVoiceConfig]);

  const speakNext = useCallback(async () => {
    const cfg = loadVoiceConfig();
    if (!cfg.ttsEnabled) return;
    if (speakingItemIdRef.current || !activeItemIdRef.current) return;

    const itemId = activeItemIdRef.current;

    const state = FlowChatStore.getInstance().getState();
    let text: string | null = null;
    for (const [, session] of state.sessions) {
      for (const turn of session.dialogTurns) {
        for (const round of turn.modelRounds) {
          for (const item of round.items) {
            if (item.type === 'text' && item.id === itemId) {
              const markdownText = typeof item.content === 'string' ? item.content : '';
              text = stripMarkdown(markdownText);
              break;
            }
          }
          if (text) break;
        }
        if (text) break;
      }
      if (text) break;
    }

    if (!text || text.length < 3) return;

    speakingItemIdRef.current = itemId;
    setIsSpeaking(true);
    setCurrentTextItemId(itemId);
    onSpeakingStart?.();

    log.debug('TTS speakNext', { itemId, textLength: text.length, voice: cfg.ttsVoice, lang: cfg.language });
    try {
      if (!ttsRef.current) {
        ttsRef.current = createTtsProvider(cfg.ttsProvider);
      }
      ttsRef.current.configure(cfg.ttsVoice, cfg.language);
      log.debug('TTS calling provider.speak()');
      await ttsRef.current.speak(text);
      log.debug('TTS speak() completed');
    } catch (err) {
      log.error('TTS speak error', { err });
    } finally {
      setIsSpeaking(false);
      speakingItemIdRef.current = null;
      setCurrentTextItemId(null);
      onSpeakingEnd?.();
    }
  }, [onSpeakingStart, onSpeakingEnd]);

  const stop = useCallback(() => {
    if (!ttsRef.current) return;
    ttsRef.current.stop();
    setIsSpeaking(false);
    speakingItemIdRef.current = null;
    activeItemIdRef.current = null;
    prevRoundIdRef.current = null;
  }, []);

  const skipCurrent = useCallback(() => {
    stop();
  }, [stop]);

  useEffect(() => {
    const unsubscribe = FlowChatStore.getInstance().subscribe((state: FlowChatState) => {
      const cfg = loadVoiceConfig();
      if (!cfg.ttsEnabled) return;

      const state2 = FlowChatStore.getInstance().getState();
      const activeSessionId = state2.activeSessionId;

      const { sessions } = state;
      let latestCompletedTextItem: { item: FlowTextItem; roundId: string } | null = null;

      for (const [, session] of sessions) {
        if (session.sessionId !== activeSessionId) continue;
        for (const turn of session.dialogTurns) {
          for (const round of turn.modelRounds) {
            for (const item of round.items) {
              if (item.type !== 'text') continue;
              const textItem = item as FlowTextItem;
              if (textItem.status !== 'completed') continue;
              if (round.isStreaming) continue;

              latestCompletedTextItem = { item: textItem, roundId: round.id };
            }
          }
        }
      }

      if (!latestCompletedTextItem) return;

      const { item, roundId } = latestCompletedTextItem;

      if (item.id === speakingItemIdRef.current) return;
      if (item.id === activeItemIdRef.current && roundId === prevRoundIdRef.current) return;

      log.debug('TTS triggered', { textItemId: item.id, roundId, prevRound: prevRoundIdRef.current });

      activeItemIdRef.current = item.id;
      prevRoundIdRef.current = roundId;
      void speakNext();
    });

    return unsubscribe;
  }, [speakNext]);

  return {
    isSpeaking,
    currentTextItemId,
    stop,
    skipCurrent,
  };
}