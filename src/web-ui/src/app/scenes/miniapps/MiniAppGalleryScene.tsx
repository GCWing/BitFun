/**
 * MiniAppGalleryScene — Mini App gallery scene.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy, useState } from 'react';

import { SegmentedControl } from '@bitfun/ui';
import { useI18n } from '@/infrastructure/i18n';
import './MiniAppGalleryScene.scss';

const MiniAppLibraryView = lazy(() => import('./views/MiniAppLibraryView'));
const MiniAppSubmissionsView = lazy(() => import('./views/MiniAppSubmissionsView'));

type MiniAppGalleryTab = 'apps' | 'submissions';

const MiniAppGalleryScene: React.FC = () => {
  const { t } = useI18n('scenes/miniapp');
  const [activeTab, setActiveTab] = useState<MiniAppGalleryTab>('apps');
  const tabs = (
    <SegmentedControl
      aria-label={t('title')}
      className="miniapp-gallery-tabs"
      distribution="fill"
      options={[
        {
          value: 'apps',
          label: t('market.tabs.apps'),
        },
        {
          value: 'submissions',
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
          {activeTab === 'apps' && <MiniAppLibraryView tabs={tabs} />}
          {activeTab === 'submissions' && <MiniAppSubmissionsView tabs={tabs} />}
        </Suspense>
      </div>
    </div>
  );
};

export default MiniAppGalleryScene;
