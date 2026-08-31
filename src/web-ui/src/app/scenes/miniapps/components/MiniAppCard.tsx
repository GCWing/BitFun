import { Icon, Button, IconButton } from '@bitfun/ui';
import React from 'react';
import { Play, Square } from 'lucide-react';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { renderMiniAppIcon } from '../utils/miniAppIcons';
import { pickLocalizedString, pickLocalizedTags } from '../utils/pickLocalizedString';
import { useI18n } from '@/infrastructure/i18n';
import './MiniAppCard.scss';

interface MiniAppCardProps {
  app: MiniAppMeta;
  index?: number;
  isRunning?: boolean;
  isCustomizing?: boolean;
  /**
   * Marketplace release this copy was installed from, when it came from the
   * marketplace. It takes over the version label because `app.version` is a
   * local edit counter that always starts at 1 — showing it made a freshly
   * installed v2 read as "v1".
   */
  marketReleaseNumber?: number;
  onOpenDetails: (app: MiniAppMeta) => void;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onStop?: (id: string) => void;
}

const MiniAppCard: React.FC<MiniAppCardProps> = ({
  app,
  index = 0,
  isRunning = false,
  isCustomizing = false,
  marketReleaseNumber,
  onOpenDetails,
  onOpen,
  onDelete,
  onStop,
}) => {
  const { t, currentLanguage } = useI18n('scenes/miniapp');
  const localizedName = pickLocalizedString(app, currentLanguage, 'name');
  const localizedDescription = pickLocalizedString(app, currentLanguage, 'description');
  const localizedTags = pickLocalizedTags(app, currentLanguage);
  const displayedTags = localizedTags.slice(0, 3);
  const compactOverflowTags = localizedTags.slice(2);
  const wideOverflowTags = localizedTags.slice(3);
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

  const handleCardKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleOpenDetails();
    }
  };

  return (
    <div data-bf-component="mini-app-card" data-bf-part="root" data-miniapp-id={app.id}
      className={[
        'miniapp-card',
        isRunning && 'miniapp-card--running',
        isCustomizing && 'miniapp-card--customizing',
      ]
        .filter(Boolean)
        .join(' ')}
      style={{
        '--surface-stagger-index': index,
      } as React.CSSProperties}
      onClick={handleOpenDetails}
      role="button"
      tabIndex={0}
      onKeyDown={handleCardKeyDown}
      aria-label={localizedName}
    >
      <div className="miniapp-card__main">
        <div className="miniapp-card__icon-area" data-bf-component="mini-app-card" data-bf-part="iconArea">
          <div className="miniapp-card__icon" data-bf-component="mini-app-card" data-bf-part="icon">
            {renderMiniAppIcon(app.icon || 'box', 34)}
          </div>
        </div>

        <div className="miniapp-card__content">
          <div className="miniapp-card__header" data-bf-component="mini-app-card" data-bf-part="header">
            <div className="miniapp-card__title-group" data-bf-component="mini-app-card" data-bf-part="title">
              <span className="miniapp-card__name" data-bf-component="mini-app-card" data-bf-part="name">{localizedName}</span>
            </div>
            <div className="miniapp-card__meta">
              {(isRunning || isCustomizing) && (
                <span className="miniapp-card__status-dots" data-bf-component="mini-app-card" data-bf-part="status" aria-hidden="true">
                  {isRunning && <span className="miniapp-card__run-dot" />}
                  {isCustomizing && <span className="miniapp-card__customize-dot" />}
                </span>
              )}
              <span className="miniapp-card__version" data-bf-component="mini-app-card" data-bf-part="version">v{marketReleaseNumber ?? app.version}</span>
            </div>
          </div>

          <div className="miniapp-card__body" data-bf-component="mini-app-card" data-bf-part="body">
            {localizedDescription ? (
              <div className="miniapp-card__desc" data-bf-component="mini-app-card" data-bf-part="description">
                <span className="miniapp-card__desc-inner">{localizedDescription}</span>
              </div>
            ) : null}
            <div className="miniapp-card__footer" data-bf-component="mini-app-card" data-bf-part="footer">
              {localizedTags.length > 0 ? (
                <div className="miniapp-card__tags" data-bf-component="mini-app-card" data-bf-part="tags">
                  {displayedTags.map((tag, tagIndex) => (
                    <span
                      key={tag}
                      className={[
                        'miniapp-card__tag',
                        tagIndex >= 2 && 'miniapp-card__tag--compact-hidden',
                      ].filter(Boolean).join(' ')}
                      title={tag}
                    >
                      {tag}
                    </span>
                  ))}
                  {wideOverflowTags.length > 0 && (
                    <span
                      className="miniapp-card__tag-overflow miniapp-card__tag-overflow--wide"
                      title={wideOverflowTags.join(', ')}
                      aria-label={wideOverflowTags.join(', ')}
                    >
                      +{wideOverflowTags.length}
                    </span>
                  )}
                  {compactOverflowTags.length > 0 && (
                    <span
                      className="miniapp-card__tag-overflow miniapp-card__tag-overflow--compact"
                      title={compactOverflowTags.join(', ')}
                      aria-label={compactOverflowTags.join(', ')}
                    >
                      +{compactOverflowTags.length}
                    </span>
                  )}
                </div>
              ) : null}
              <div className="miniapp-card__actions" data-bf-component="mini-app-card" data-bf-part="actions" onClick={(e) => e.stopPropagation()}>
                <Button
                  variant="fill"
                  size="sm"
                  leadingIcon={<Play size={14} fill="currentColor" strokeWidth={0} />}
                  onClick={handleOpenClick}
                  aria-label={t('card.start')}
                  title={t('card.start')}
                >
                  {t('card.start')}
                </Button>
                {isRunning && onStop ? (
                  <IconButton
                    size="sm"
                    onClick={handleStopClick}
                    aria-label={t('card.stop')}
                    icon={<Square />}
                    title={t('card.stop')}
                  />
                ) : (
                  <IconButton
                    size="sm"
                    tone="danger"
                    onClick={handleDeleteClick}
                    aria-label={t('card.delete')}
                    icon={<Icon name="delete" size="lg" />}
                    title={t('card.delete')}
                  />
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default MiniAppCard;
