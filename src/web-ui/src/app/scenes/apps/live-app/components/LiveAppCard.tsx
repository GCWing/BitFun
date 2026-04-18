import React from 'react';
import { Play, Square, Trash2 } from 'lucide-react';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { renderLiveAppIcon } from '../liveAppIcons';
import { useI18n } from '@/infrastructure/i18n';
import './LiveAppCard.scss';

interface LiveAppCardProps {
  app: MiniAppMeta;
  index?: number;
  isRunning?: boolean;
  onOpenDetails: (app: MiniAppMeta) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onStop?: (id: string) => void;
}

const LiveAppCard: React.FC<LiveAppCardProps> = ({
  app,
  index = 0,
  isRunning = false,
  onOpenDetails,
  onOpen,
  onDelete,
  onStop,
}) => {
  const { t } = useI18n('scenes/apps');
  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete(app.id);
  };

  const handleStopClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onStop?.(app.id);
  };

  const handleOpenClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onOpen(app.id);
  };

  const handleOpenDetails = () => {
    onOpenDetails(app);
  };

  return (
    <div
      className={[
        'live-app-card',
        isRunning && 'live-app-card--running',
      ]
        .filter(Boolean)
        .join(' ')}
      style={{
        '--card-index': index,
        '--live-app-card-gradient': isRunning
          ? 'linear-gradient(135deg, rgba(52, 211, 153, 0.28) 0%, rgba(16, 185, 129, 0.18) 100%)'
          : 'linear-gradient(135deg, rgba(59, 130, 246, 0.28) 0%, rgba(139, 92, 246, 0.18) 100%)',
      } as React.CSSProperties}
      onClick={handleOpenDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && handleOpenDetails()}
      aria-label={app.name}
    >
      <div className="live-app-card__header">
        <div className="live-app-card__icon-area">
          <div className="live-app-card__icon">
            {renderLiveAppIcon(app.icon || 'box', 20)}
          </div>
        </div>
        <div className="live-app-card__title-group">
          <span className="live-app-card__name">{app.name}</span>
          <span className="live-app-card__version">v{app.version}</span>
        </div>
        {isRunning && <span className="live-app-card__run-dot" />}
      </div>

      <div className="live-app-card__body">
        {app.description ? (
          <div className="live-app-card__desc">
            <span className="live-app-card__desc-inner">{app.description}</span>
          </div>
        ) : null}
        {app.tags.length > 0 ? (
          <div className="live-app-card__tags">
            {app.tags.slice(0, 3).map((tag) => (
              <span key={tag} className="live-app-card__tag">{tag}</span>
            ))}
          </div>
        ) : null}
      </div>

      <div className="live-app-card__footer">
        <div className="live-app-card__actions" onClick={(e) => e.stopPropagation()}>
          <button
            className="live-app-card__action-btn live-app-card__action-btn--primary"
            onClick={handleOpenClick}
            aria-label={t('liveApp.card.start')}
            title={t('liveApp.card.start')}
          >
            <Play size={15} fill="currentColor" strokeWidth={0} />
          </button>
          {isRunning && onStop ? (
            <button
              className="live-app-card__action-btn live-app-card__action-btn--stop"
              onClick={handleStopClick}
              aria-label={t('liveApp.card.stop')}
              title={t('liveApp.card.stop')}
            >
              <Square size={13} />
            </button>
          ) : (
            <button
              className="live-app-card__action-btn live-app-card__action-btn--danger"
              onClick={handleDeleteClick}
              aria-label={t('liveApp.card.delete')}
              title={t('liveApp.card.delete')}
            >
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default LiveAppCard;
