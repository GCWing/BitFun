import React from 'react';
import {
  BITFUN_ICON_SIZE,
  BitFunIcon,
  bitFunIconMetadata,
  bitFunIconNames,
} from '../icons';
import './IconCatalogPreview.scss';

const previewSizes = [
  BITFUN_ICON_SIZE.compact,
  BITFUN_ICON_SIZE.navigation,
  BITFUN_ICON_SIZE.regular,
  BITFUN_ICON_SIZE.display,
] as const;

export const IconCatalogPreview: React.FC = () => (
  <div
    className="bitfun-icon-catalog-preview"
    data-bf-component="icon-catalog"
    data-bf-part="root"
    data-testid="bitfun-icon-catalog"
  >
    <section
      className="bitfun-icon-catalog-preview__navigation-context"
      aria-labelledby="bitfun-icon-navigation-context-title"
    >
      <div className="bitfun-icon-catalog-preview__navigation-context-copy">
        <strong id="bitfun-icon-navigation-context-title">14px navigation context</strong>
        <span>Semantic row color controls contrast; the icon is not dimmed again.</span>
      </div>
      <div className="bitfun-icon-catalog-preview__navigation-rows">
        {bitFunIconNames.map((name, index) => (
          <div
            className={`bitfun-icon-catalog-preview__navigation-row${index === 0 ? ' is-active' : ''}`}
            key={name}
            data-state={index === 0 ? 'active' : 'resting'}
          >
            <BitFunIcon name={name} size={BITFUN_ICON_SIZE.compact} />
            <span>{bitFunIconMetadata[name].displayName}</span>
          </div>
        ))}
      </div>
    </section>
    {bitFunIconNames.map(name => {
      const metadata = bitFunIconMetadata[name];
      return (
        <article
          className="bitfun-icon-catalog-preview__item"
          key={name}
          data-bf-part="item"
          data-bf-icon={name}
        >
          <div className="bitfun-icon-catalog-preview__glyph" aria-hidden="true">
            <BitFunIcon
              name={name}
              size={104}
            />
          </div>
          <div className="bitfun-icon-catalog-preview__copy">
            <strong>{metadata.displayName}</strong>
            <code>{name}</code>
          </div>
          <div className="bitfun-icon-catalog-preview__size-samples" aria-label="Actual icon sizes">
            {previewSizes.map(size => (
              <span className="bitfun-icon-catalog-preview__size-sample" key={size} title={`${size}px`}>
                <BitFunIcon name={name} size={size} />
                <small>{size}</small>
              </span>
            ))}
          </div>
        </article>
      );
    })}
  </div>
);
