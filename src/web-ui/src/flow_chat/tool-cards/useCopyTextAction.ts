import { useCallback, useEffect, useRef, useState } from 'react';
import type { MouseEvent } from 'react';
import { notificationService } from '@/shared/notification-system';
import { copyTextToClipboard } from '@/shared/utils/textSelection';

interface UseCopyTextActionOptions {
  getText: () => string;
  successMessage: string;
  failureMessage: string;
  resetMs?: number;
}

export function useCopyTextAction({
  getText,
  successMessage,
  failureMessage,
  resetMs = 1600,
}: UseCopyTextActionOptions) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (resetTimerRef.current !== null) {
        window.clearTimeout(resetTimerRef.current);
      }
    };
  }, []);

  const copy = useCallback(async (event?: MouseEvent) => {
    event?.stopPropagation();

    const text = getText();
    if (!text.trim()) {
      return;
    }

    const didCopy = await copyTextToClipboard(text);
    if (!didCopy) {
      notificationService.error(failureMessage);
      return;
    }

    setCopied(true);
    notificationService.success(successMessage, { duration: resetMs });

    if (resetTimerRef.current !== null) {
      window.clearTimeout(resetTimerRef.current);
    }
    resetTimerRef.current = window.setTimeout(() => {
      setCopied(false);
      resetTimerRef.current = null;
    }, resetMs);
  }, [failureMessage, getText, resetMs, successMessage]);

  return { copied, copy };
}
