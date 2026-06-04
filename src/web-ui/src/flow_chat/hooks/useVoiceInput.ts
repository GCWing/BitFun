/**
 * Voice input hook using Web Speech API for STT.
 * Manages voice recording state and dispatches recognized text to chat input.
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { globalEventBus } from '@/infrastructure/event-bus';
import { createLogger } from '@/shared/utils/logger';
import type {
  SpeechRecognitionEvent,
  SpeechRecognitionErrorEvent,
} from '@/infrastructure/voice/SttProviderFactory';

const log = createLogger('VoiceInput');

interface UseVoiceInputOptions {
  /** Callback when speech is recognized (returns final text) */
  onResult?: (text: string) => void;
  /** Callback for interim results */
  onInterimResult?: (text: string) => void;
  /** Language BCP-47 tag, defaults to zh-CN */
  language?: string;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type SpeechRecognition = any;

export function useVoiceInput(options: UseVoiceInputOptions = {}) {
  const { onResult, onInterimResult, language = 'zh-CN' } = options;
  
  const [isListening, setIsListening] = useState(false);
  const [isSupported, setIsSupported] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const recognitionRef = useRef<SpeechRecognition | null>(null);
  const interimTranscriptRef = useRef('');

  useEffect(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const win = window as any;
    const SpeechRecognition = win.SpeechRecognition || win.webkitSpeechRecognition;
    
    if (SpeechRecognition) {
      setIsSupported(true);
      const recognition = new SpeechRecognition();
      recognition.continuous = true;
      recognition.interimResults = true;
      recognition.lang = language;
      
      recognition.onresult = (event: SpeechRecognitionEvent) => {
        let finalTranscript = '';
        let interimTranscript = '';
        
        for (let i = event.resultIndex; i < event.results.length; i++) {
          const result = event.results[i];
          if (result.isFinal) {
            finalTranscript += result[0].transcript;
          } else {
            interimTranscript += result[0].transcript;
          }
        }
        
        interimTranscriptRef.current = interimTranscript;
        
        if (interimTranscript) {
          onInterimResult?.(interimTranscript);
        }
        
        if (finalTranscript) {
          log.debug('Speech recognized:', { finalTranscript });
          onResult?.(finalTranscript);
        }
      };
      
      recognition.onerror = (event: SpeechRecognitionErrorEvent) => {
        log.warn('Speech recognition error:', { error: event.error });
        if (event.error !== 'no-speech' && event.error !== 'aborted') {
          setError(event.error);
        }
        setIsListening(false);
      };
      
      recognition.onend = () => {
        setIsListening(false);
        interimTranscriptRef.current = '';
      };
      
      recognition.onstart = () => {
        setIsListening(true);
        setError(null);
      };
      
      recognitionRef.current = recognition;
    } else {
      setIsSupported(false);
      log.warn('Speech recognition not supported');
    }
    
    return () => {
      if (recognitionRef.current) {
        recognitionRef.current.abort();
      }
    };
  }, [language, onResult, onInterimResult]);

  const startListening = useCallback(() => {
    if (!recognitionRef.current || isListening) return;
    
    try {
      interimTranscriptRef.current = '';
      recognitionRef.current.lang = language;
      recognitionRef.current.start();
    } catch (err) {
      log.error('Failed to start speech recognition', { err });
      setError('Failed to start');
    }
  }, [isListening, language]);

  const stopListening = useCallback(() => {
    if (!recognitionRef.current || !isListening) return;
    
    try {
      recognitionRef.current.stop();
    } catch (err) {
      log.error('Failed to stop speech recognition', { err });
    }
  }, [isListening]);

  const toggleListening = useCallback(() => {
    if (isListening) {
      stopListening();
    } else {
      startListening();
    }
  }, [isListening, startListening, stopListening]);

  return {
    isListening,
    isSupported,
    error,
    startListening,
    stopListening,
    toggleListening,
  };
}

/**
 * Hook for filling chat input with voice transcription.
 * Integrates voice input with the chat input's fill-chat-input event.
 */
export function useVoiceInputToChat(options: UseVoiceInputOptions = {}) {
  const { onResult } = options;
  const [interimText, setInterimText] = useState('');
  
  const handleResult = useCallback((text: string) => {
    setInterimText('');
    
    globalEventBus.emit('fill-chat-input', {
      content: text,
      mode: 'append',
      separator: ' ',
    });
    
    onResult?.(text);
  }, [onResult]);

  const handleInterimResult = useCallback((text: string) => {
    setInterimText(text);
  }, []);

  const voice = useVoiceInput({
    onResult: handleResult,
    onInterimResult: handleInterimResult,
    language: options.language,
  });

  return {
    ...voice,
    interimText,
  };
}