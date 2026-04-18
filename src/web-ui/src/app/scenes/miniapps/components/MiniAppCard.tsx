import React from 'react';
import { Play, Square, Trash2 } from 'lucide-react';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { renderMiniAppIcon } from '../utils/miniAppIcons';
import { pickLocalizedString, pickLocalizedTags } from '../utils/pickLocalizedString';
import { useI18n } from '@/infrastructure/i18n';
import './MiniAppCard.scss';

interface MiniAppCardProps {
  app: MiniAppMeta;
  index?: number;
  isRunning?: boolean;
  onOpenDetails: (app: MiniAppMeta) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onStop?: (id: string) => void;
}

const MiniAppCard: React.FC<MiniAppCardProps> = ({
  app,
  index = 0,
  isRunning = false,
  onOpenDetails,
  onOpen,
  onDelete,
  onStop,
}) => {
  const { t, currentLanguage } = useI18n('scenes/miniapp');
  const localizedName = pickLocalizedString(app, currentLanguage, 'name');
  const localizedDescription = pickLocalizedString(app, currentLanguage, 'description');
  const localizedTags = pickLocalizedTags(app, currentLanguage);
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
        'miniapp-card',
        isRunning && 'miniapp-card--running',
      ]
        .filter(Boolean)
        .join(' ')}
      style={{
        '--card-index': index,
        '--miniapp-card-gradient': isRunning
          ? 'linear-gradient(135deg, rgba(52, 211, 153, 0.28) 0%, rgba(16, 185, 129, 0.18) 100%)'
          : 'linear-gradient(135deg, rgba(59, 130, 246, 0.28) 0%, rgba(139, 92, 246, 0.18) 100%)',
      } as React.CSSProperties}
      onClick={handleOpenDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && handleOpenDetails()}
      aria-label={localizedName}
    >
      {/* Header with icon and title */}
      <div className="miniapp-card__header">
        <div className="miniapp-card__icon-area">
          <div className="miniapp-card__icon">
            {renderMiniAppIcon(app.icon || 'box', 20)}
          </div>
        </div>
        <div className="miniapp-card__title-group">
          <span className="miniapp-card__name">{localizedName}</span>
          <span className="miniapp-card__version">v{app.version}</span>
        </div>
        {isRunning && <span className="miniapp-card__run-dot" />}
      </div>

      {/* Body: description + tags */}
      <div className="miniapp-card__body">
        {localizedDescription ? (
          <div className="miniapp-card__desc">
            <span className="miniapp-card__desc-inner">{localizedDescription}</span>
          </div>
        ) : null}
        {localizedTags.length > 0 ? (
          <div className="miniapp-card__tags">
            {localizedTags.slice(0, 3).map((tag) => (
              <span key={tag} className="miniapp-card__tag">{tag}</span>
            ))}
          </div>
        ) : null}
      </div>

      {/* Footer with actions */}
      <div className="miniapp-card__footer">
        <div className="miniapp-card__actions" onClick={(e) => e.stopPropagation()}>
          <button
            className="miniapp-card__action-btn miniapp-card__action-btn--primary"
            onClick={handleOpenClick}
            aria-label={t('card.start')}
            title={t('card.start')}
          >
            <Play size={15} fill="currentColor" strokeWidth={0} />
          </button>
          {isRunning && onStop ? (
            <button
              className="miniapp-card__action-btn miniapp-card__action-btn--stop"
              onClick={handleStopClick}
              aria-label={t('card.stop')}
              title={t('card.stop')}
            >
              <Square size={13} />
            </button>
          ) : (
            <button
              className="miniapp-card__action-btn miniapp-card__action-btn--danger"
              onClick={handleDeleteClick}
              aria-label={t('card.delete')}
              title={t('card.delete')}
            >
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default MiniAppCard;
