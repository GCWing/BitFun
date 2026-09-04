import { Icon, IconButton } from '@openbitfun/ui';
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
    <div data-openbitfun-component="mini-app-card" data-openbitfun-part="root" data-miniapp-id={app.id}
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
        <div className="miniapp-card__header" data-openbitfun-component="mini-app-card" data-openbitfun-part="header">
          <div className="miniapp-card__icon-area" data-openbitfun-component="mini-app-card" data-openbitfun-part="iconArea">
            <div className="miniapp-card__icon" data-openbitfun-component="mini-app-card" data-openbitfun-part="icon">
              {iconAsset ? (
                <img className="miniapp-card__icon-image" src={iconAsset} alt="" aria-hidden="true" />
              ) : renderMiniAppIcon(app.icon || 'box', 40)}
            </div>
          </div>
          <div className="miniapp-card__header-actions">
            {(isRunning || isCustomizing) && (
              <span className="miniapp-card__status-dots" data-openbitfun-component="mini-app-card" data-openbitfun-part="status" aria-hidden="true">
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
          <div className="miniapp-card__title-group" data-openbitfun-component="mini-app-card" data-openbitfun-part="title">
            <span className="miniapp-card__name" data-openbitfun-component="mini-app-card" data-openbitfun-part="name">{localizedName}</span>
          </div>

          <div className="miniapp-card__body" data-openbitfun-component="mini-app-card" data-openbitfun-part="body">
            {localizedDescription ? (
              <div className="miniapp-card__desc" data-openbitfun-component="mini-app-card" data-openbitfun-part="description">
                <span className="miniapp-card__desc-inner">{localizedDescription}</span>
              </div>
            ) : null}
          </div>
        </div>

        <div className="miniapp-card__footer" data-openbitfun-component="mini-app-card" data-openbitfun-part="footer">
          <div className="miniapp-card__tags" data-openbitfun-component="mini-app-card" data-openbitfun-part="tags">
            <span className="miniapp-card__tag" data-openbitfun-component="mini-app-card" data-openbitfun-part="version">
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
          <div className="miniapp-card__actions" data-openbitfun-component="mini-app-card" data-openbitfun-part="actions" onClick={(event) => event.stopPropagation()}>
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
