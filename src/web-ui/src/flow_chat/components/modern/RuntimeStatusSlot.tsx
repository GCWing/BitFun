import React from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@openbitfun/ui';
import { useRuntimeStatusStore } from '../../store/runtimeStatusStore';
import './RuntimeStatusSlot.scss';

interface RuntimeStatusSlotProps {
  sessionId?: string | null;
  placement?: 'footer' | 'inline';
  className?: string;
}

function stableHintIndex(seed: string, hintCount: number): number {
  if (hintCount === 0) return 0;
  const hash = seed.split('').reduce((value, character) => value + character.charCodeAt(0), 0);
  return Math.abs(hash) % hintCount;
}

export const RuntimeStatusSlot: React.FC<RuntimeStatusSlotProps> = ({
  sessionId,
  placement = 'inline',
  className = '',
}) => {
  const status = useRuntimeStatusStore(state => (
    sessionId ? state.bySessionId.get(sessionId) : undefined
  ));
  const { t } = useTranslation('flow-chat/processing-hints');
  const rawHints = t('items', { returnObjects: true });
  const hints = Array.isArray(rawHints)
    ? rawHints.filter((item): item is string => typeof item === 'string')
    : [];
  const hint = status
    ? status.label
      || hints[stableHintIndex(`${status.turnId}:${status.roundId}`, hints.length)]
      || ''
    : '';
  const visible = Boolean(status && hint);

  return (
    <div
      className={`runtime-status-slot runtime-status-slot--${placement} ${visible ? 'runtime-status-slot--visible' : ''} ${className}`.trim()}
      data-openbitfun-component="runtime-status-slot"
      data-openbitfun-part="root"
      aria-hidden={!visible}
      data-runtime-status-visible={visible ? 'true' : 'false'}
    >
      <div
        className="runtime-status-slot__content"
        data-openbitfun-component="runtime-status-slot"
        data-openbitfun-part="content"
      >
        <Spinner size="sm" />
        <span
          className="runtime-status-slot__hint"
          data-openbitfun-component="runtime-status-slot"
          data-openbitfun-part="hint"
        >
          {hint}
        </span>
      </div>
    </div>
  );
};
