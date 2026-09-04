import { Button, StatusPill, type StatusPillTone } from '@bitfun/ui';
import { GalleryHorizontalEnd } from 'lucide-react';
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
  meta?: string;
  name: string;
  onOpenDetails: () => void;
  onPrimaryAction: () => void;
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
  meta,
  name,
  onOpenDetails,
  onPrimaryAction,
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
      data-bf-component="miniapp-gallery-view"
      data-bf-part="item"
    >
      <button
        type="button"
        className="miniapp-library-row__details"
        aria-label={detailsLabel}
        onClick={onOpenDetails}
      >
        <span
          className="miniapp-library-row__showcase"
          data-bf-component="miniapp-gallery-view"
          data-bf-part="showcase"
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
          data-bf-component="miniapp-gallery-view"
          data-bf-part="summary"
        >
          <span className="miniapp-library-row__status-rail">
            <span className="miniapp-library-row__category">{category}</span>
            {statuses.map((status) => (
              <StatusPill key={`${status.tone}:${status.label}`} tone={status.tone}>
                {status.label}
              </StatusPill>
            ))}
          </span>
          <strong className="miniapp-library-row__name">{name}</strong>
          <span className="miniapp-library-row__description">{description}</span>
          <span
            className="miniapp-library-row__meta"
            data-bf-component="miniapp-gallery-view"
            data-bf-part="meta"
          >
            <span>{version}</span>
            {meta ? <span>{meta}</span> : null}
          </span>
        </span>
      </button>

      <div
        className="miniapp-library-row__actions"
        data-bf-component="miniapp-gallery-view"
        data-bf-part="actions"
      >
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
