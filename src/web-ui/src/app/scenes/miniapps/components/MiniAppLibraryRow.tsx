import { Button, Icon, StatusPill, type StatusPillTone } from '@openbitfun/ui';
import {
  GalleryHorizontalEnd,
  HardDrive,
  Package,
  Star,
  UserRound,
} from 'lucide-react';
import React, { useEffect, useState } from 'react';

import {
  marketImageSrcSet,
  marketImageUrl,
  retryOriginalMarketImage,
} from '@/infrastructure/api/service-api/MarketImage';
import type { MiniAppLibraryAction } from '../views/miniAppLibraryItems';

interface MiniAppLibraryStatus {
  label: string;
  tone: StatusPillTone;
}

interface MiniAppLibraryRowProps {
  action: MiniAppLibraryAction;
  actionDisabled?: boolean;
  actionLabel: string;
  actionTitle?: string;
  busy?: boolean;
  category: string;
  description: string;
  detailsLabel: string;
  downloadCount?: string;
  localMeta?: string;
  metaLabel?: string;
  name: string;
  onOpenDetails: () => void;
  onPrimaryAction: () => void;
  owner?: string;
  rating?: string;
  showcaseAlt: string;
  showcaseFallbackLabel: string;
  showcaseUrl?: string;
  statuses: MiniAppLibraryStatus[];
  version: string;
}

const MiniAppLibraryRow: React.FC<MiniAppLibraryRowProps> = ({
  action,
  actionDisabled = false,
  actionLabel,
  actionTitle,
  busy = false,
  category,
  description,
  detailsLabel,
  downloadCount,
  localMeta,
  metaLabel,
  name,
  onOpenDetails,
  onPrimaryAction,
  owner,
  rating,
  showcaseAlt,
  showcaseFallbackLabel,
  showcaseUrl,
  statuses,
  version,
}) => {
  const [showcaseUnavailable, setShowcaseUnavailable] = useState(!showcaseUrl);

  useEffect(() => {
    setShowcaseUnavailable(!showcaseUrl);
  }, [showcaseUrl]);

  return (
    <article
      className="miniapp-library-row"
      role="listitem"
      data-action={action}
      data-openbitfun-component="miniapp-gallery-view"
      data-openbitfun-part="item"
    >
      <button
        type="button"
        className="miniapp-library-row__details"
        aria-label={detailsLabel}
        onClick={onOpenDetails}
      >
        <span
          className="miniapp-library-row__showcase"
          data-openbitfun-component="miniapp-gallery-view"
          data-openbitfun-part="showcase"
        >
          {!showcaseUnavailable && showcaseUrl ? (
            <img
              src={marketImageUrl(showcaseUrl, 'compact-v1')}
              srcSet={marketImageSrcSet(showcaseUrl)}
              sizes="(min-width: 64rem) 224px, 100vw"
              width={640}
              height={360}
              alt={showcaseAlt}
              loading="lazy"
              decoding="async"
              onError={(event) => {
                if (!retryOriginalMarketImage(event.currentTarget, showcaseUrl)) {
                  setShowcaseUnavailable(true);
                }
              }}
            />
          ) : (
            <span
              className="miniapp-library-row__showcase-fallback"
              aria-label={showcaseFallbackLabel}
              role="img"
            >
              <GalleryHorizontalEnd size={34} strokeWidth={1.35} aria-hidden="true" />
            </span>
          )}
        </span>

        <span
          className="miniapp-library-row__summary"
          data-openbitfun-component="miniapp-gallery-view"
          data-openbitfun-part="summary"
        >
          <span
            className="miniapp-library-row__title-row"
            data-openbitfun-component="miniapp-gallery-view"
            data-openbitfun-part="title"
          >
            <strong className="miniapp-library-row__name">{name}</strong>
            <StatusPill className="miniapp-library-row__category" tone="neutral">
              {category}
            </StatusPill>
          </span>
          <span className="miniapp-library-row__description">{description}</span>
          <span
            className="miniapp-library-row__meta"
            data-openbitfun-component="miniapp-gallery-view"
            data-openbitfun-part="meta"
            role="group"
            aria-label={[version, metaLabel].filter(Boolean).join(', ')}
          >
            <span className="miniapp-library-row__meta-item">
              <Package size={13} strokeWidth={1.8} aria-hidden="true" />
              <span>{version}</span>
            </span>
            {owner ? (
              <span className="miniapp-library-row__meta-item miniapp-library-row__meta-item--owner">
                <UserRound size={13} strokeWidth={1.8} aria-hidden="true" />
                <span>@{owner}</span>
              </span>
            ) : null}
            {rating ? (
              <span className="miniapp-library-row__meta-item">
                <Star size={13} strokeWidth={1.8} aria-hidden="true" />
                <span>{rating}</span>
              </span>
            ) : null}
            {downloadCount ? (
              <span className="miniapp-library-row__meta-item">
                <Icon name="arrow-down" size="xs" aria-hidden />
                <span>{downloadCount}</span>
              </span>
            ) : null}
            {localMeta ? (
              <span className="miniapp-library-row__meta-item miniapp-library-row__meta-item--local">
                <HardDrive size={13} strokeWidth={1.8} aria-hidden="true" />
                <span>{localMeta}</span>
              </span>
            ) : null}
          </span>
        </span>
      </button>

      <div
        className="miniapp-library-row__actions"
        data-openbitfun-component="miniapp-gallery-view"
        data-openbitfun-part="actions"
      >
        {statuses.length > 0 ? (
          <div
            className="miniapp-library-row__status-rail"
            data-openbitfun-component="miniapp-gallery-view"
            data-openbitfun-part="status"
          >
            {statuses.map((status) => (
              <StatusPill
                className="miniapp-library-row__status"
                key={`${status.tone}:${status.label}`}
                tone={status.tone}
              >
                {status.label}
              </StatusPill>
            ))}
          </div>
        ) : null}
        <Button
          className="miniapp-library-row__primary"
          size="sm"
          variant={action === 'open' ? 'outline' : 'primary'}
          disabled={actionDisabled}
          loading={busy}
          title={actionTitle}
          onClick={onPrimaryAction}
        >
          {actionLabel}
        </Button>
      </div>
    </article>
  );
};

export default MiniAppLibraryRow;
export type { MiniAppLibraryRowProps, MiniAppLibraryStatus };
