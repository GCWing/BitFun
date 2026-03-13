import React, { useCallback } from 'react';
import { BarChart3 } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useMyAgentStore } from '@/app/scenes/my-agent/myAgentStore';
import { useInsightsStore } from '@/app/scenes/my-agent/insightsStore';
import './InsightsButton.scss';

interface InsightsButtonProps {
  className?: string;
  tooltipPlacement?: 'top' | 'bottom' | 'left' | 'right';
}

const InsightsButton: React.FC<InsightsButtonProps> = ({ className, tooltipPlacement = 'bottom' }) => {
  const { t } = useI18n('common');
  const generating = useInsightsStore((s) => s.generating);
  const progress = useInsightsStore((s) => s.progress);

  const handleClick = useCallback(() => {
    useMyAgentStore.getState().setActiveView('insights');
    useSceneStore.getState().openScene('my-agent');
  }, []);

  const progressText = generating && progress.total > 0
    ? `${progress.current}/${progress.total}`
    : undefined;

  const tooltipContent = generating
    ? progress.message || t('insights.generating')
    : t('insights.buttonTooltip');

  return (
    <Tooltip
      content={tooltipContent}
      placement={tooltipPlacement}
    >
      <button
        className={[
          'insights-btn',
          generating ? 'insights-btn--generating' : '',
          className || '',
        ].filter(Boolean).join(' ')}
        onClick={handleClick}
        type="button"
        aria-label={t('insights.buttonTooltip')}
      >
        {generating ? (
          <div className="insights-btn__progress">
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              className="insights-btn__spinner"
            >
              <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round" />
            </svg>
            {progressText && (
              <span className="insights-btn__progress-text">{progressText}</span>
            )}
          </div>
        ) : (
          <BarChart3 size={14} />
        )}
      </button>
    </Tooltip>
  );
};

export default InsightsButton;
