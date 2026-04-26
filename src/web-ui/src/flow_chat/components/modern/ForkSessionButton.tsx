import React, { useState, useCallback } from 'react';
import { GitFork, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { flowChatManager } from '../../services/FlowChatManager';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';

const log = createLogger('ForkSessionButton');

interface ForkSessionButtonProps {
  sessionId?: string;
  turnId: string;
}

export const ForkSessionButton: React.FC<ForkSessionButtonProps> = ({
  sessionId,
  turnId,
}) => {
  const { t } = useTranslation('flow-chat');
  const [isForking, setIsForking] = useState(false);

  const handleFork = useCallback(async () => {
    if (!sessionId || isForking) {
      return;
    }

    setIsForking(true);
    try {
      await flowChatManager.forkChatSession(sessionId, turnId);
    } catch (error) {
      log.error('Failed to fork session', { sessionId, turnId, error });
      notificationService.error(
        t('modelRound.forkFailed', { defaultValue: 'Failed to fork session' }),
        { duration: 3500 }
      );
    } finally {
      setIsForking(false);
    }
  }, [isForking, sessionId, t, turnId]);

  if (!sessionId) {
    return null;
  }

  return (
    <Tooltip
      content={t('modelRound.forkDialog', { defaultValue: 'Fork session from here' })}
      placement="top"
    >
      <button
        className="model-round-item__action-btn model-round-item__fork-btn"
        onClick={handleFork}
        disabled={isForking}
      >
        {isForking ? <Loader2 size={14} className="spinning" /> : <GitFork size={14} />}
      </button>
    </Tooltip>
  );
};

ForkSessionButton.displayName = 'ForkSessionButton';
