/**
 * MiniAppGalleryScene — Mini App gallery scene.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy, useState } from 'react';

import { Icon, SegmentedControl } from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import './MiniAppGalleryScene.scss';

const MiniAppGalleryView = lazy(() => import('./views/MiniAppGalleryView'));
const MiniAppMarketView = lazy(() => import('./views/MiniAppMarketView'));
const MiniAppSubmissionsView = lazy(() => import('./views/MiniAppSubmissionsView'));

type MiniAppGalleryTab = 'installed' | 'market' | 'submissions';

const MiniAppGalleryScene: React.FC = () => {
  const { t } = useI18n('scenes/miniapp');
  const [activeTab, setActiveTab] = useState<MiniAppGalleryTab>('installed');
  const tabs = (
    <SegmentedControl
      aria-label={t('title')}
      className="miniapp-gallery-tabs"
      distribution="fill"
      options={[
        {
          value: 'installed',
          icon: <Icon name="arrow-down" size="sm" />,
          label: t('market.tabs.installed'),
        },
        {
          value: 'market',
          icon: <Icon name="store" size="sm" />,
          label: t('market.tabs.market'),
        },
        {
          value: 'submissions',
          icon: <Icon name="upload" size="sm" />,
          label: t('market.tabs.submissions'),
        },
      ]}
      size="md"
      value={activeTab}
      onValueChange={(value) => setActiveTab(value as MiniAppGalleryTab)}
    />
  );

  return (
    <div className="miniapp-gallery-scene" data-bf-scene="miniapp-gallery" data-bf-part="root">
      <div className="miniapp-gallery-scene__content">
        <Suspense fallback={null}>
          {activeTab === 'installed' && <MiniAppGalleryView tabs={tabs} />}
          {activeTab === 'market' && <MiniAppMarketView tabs={tabs} />}
          {activeTab === 'submissions' && <MiniAppSubmissionsView tabs={tabs} />}
        </Suspense>
      </div>
    </div>
  );
};

export default MiniAppGalleryScene;
