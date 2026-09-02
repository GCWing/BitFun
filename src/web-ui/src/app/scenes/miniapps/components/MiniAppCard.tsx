import { Icon, IconButton } from '@bitfun/ui';
import React from 'react';
import { Play, Square } from 'lucide-react';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { getMiniAppIconAsset, renderMiniAppIcon } from '../utils/miniAppIcons';
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
  onStop,
}) => {
  const { t, currentLanguage } = useI18n('scenes/miniapp');
  const localizedName = pickLocalizedString(app, currentLanguage, 'name');
  const localizedDescription = pickLocalizedString(app, currentLanguage, 'description');
  const localizedTags = pickLocalizedTags(app, currentLanguage);
  const displayedTags = localizedTags.slice(0, 4);
  const overflowTags = localizedTags.slice(4);
  const iconAsset = getMiniAppIconAsset(app.id);

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

  const handleMoreClick = (event: React.MouseEvent) => {
    event.stopPropagation();
    handleOpenDetails();
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
        <div className="miniapp-card__header" data-bf-component="mini-app-card" data-bf-part="header">
          <div className="miniapp-card__icon-area" data-bf-component="mini-app-card" data-bf-part="iconArea">
            <div className="miniapp-card__icon" data-bf-component="mini-app-card" data-bf-part="icon">
              {iconAsset ? (
                <img className="miniapp-card__icon-image" src={iconAsset} alt="" aria-hidden="true" />
              ) : renderMiniAppIcon(app.icon || 'box', 40)}
            </div>
          </div>
          <div className="miniapp-card__header-actions">
            {(isRunning || isCustomizing) && (
              <span className="miniapp-card__status-dots" data-bf-component="mini-app-card" data-bf-part="status" aria-hidden="true">
                {isRunning && <span className="miniapp-card__run-dot" />}
                {isCustomizing && <span className="miniapp-card__customize-dot" />}
              </span>
            )}
            <IconButton
              aria-label={localizedName}
              icon={<Icon name="more" size="sm" />}
              onClick={handleMoreClick}
              size="xs"
              title={localizedName}
            />
          </div>
        </div>

        <div className="miniapp-card__content">
          <div className="miniapp-card__title-group" data-bf-component="mini-app-card" data-bf-part="title">
            <span className="miniapp-card__name" data-bf-component="mini-app-card" data-bf-part="name">{localizedName}</span>
          </div>

          <div className="miniapp-card__body" data-bf-component="mini-app-card" data-bf-part="body">
            {localizedDescription ? (
              <div className="miniapp-card__desc" data-bf-component="mini-app-card" data-bf-part="description">
                <span className="miniapp-card__desc-inner">{localizedDescription}</span>
              </div>
            ) : null}
          </div>
        </div>

        <div className="miniapp-card__footer" data-bf-component="mini-app-card" data-bf-part="footer">
          <div className="miniapp-card__tags" data-bf-component="mini-app-card" data-bf-part="tags">
            <span className="miniapp-card__tag" data-bf-component="mini-app-card" data-bf-part="version">
              V{marketReleaseNumber ?? app.version}
            </span>
            {displayedTags.map((tag) => (
              <span key={tag} className="miniapp-card__tag" title={tag}>{tag}</span>
            ))}
            {overflowTags.length > 0 ? (
              <span
                className="miniapp-card__tag miniapp-card__tag-overflow"
                title={overflowTags.join(', ')}
                aria-label={overflowTags.join(', ')}
              >
                +{overflowTags.length}
              </span>
            ) : null}
          </div>
          <div className="miniapp-card__actions" data-bf-component="mini-app-card" data-bf-part="actions" onClick={(event) => event.stopPropagation()}>
            {isRunning && onStop ? (
              <IconButton
                aria-label={t('card.stop')}
                icon={<Square size={10} fill="currentColor" />}
                onClick={handleStopClick}
                shape="circle"
                size="xs"
                title={t('card.stop')}
                variant="primary"
              />
            ) : (
              <IconButton
                aria-label={t('card.start')}
                icon={<Play size={10} fill="currentColor" strokeWidth={0} />}
                onClick={handleOpenClick}
                shape="circle"
                size="xs"
                title={t('card.start')}
                variant="primary"
              />
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default MiniAppCard;
