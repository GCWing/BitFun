/**
 * MiniAppGalleryScene — Mini App gallery scene.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy, useState } from 'react';
import { TabGroup } from '@bitfun/ui';
import { Download, Store, UploadCloud } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import './MiniAppGalleryScene.scss';

const MiniAppGalleryView = lazy(() => import('./views/MiniAppGalleryView'));
const MiniAppMarketView = lazy(() => import('./views/MiniAppMarketView'));
const MiniAppSubmissionsView = lazy(() => import('./views/MiniAppSubmissionsView'));

type MiniAppGalleryTab = 'installed' | 'market' | 'submissions';

const MiniAppGalleryScene: React.FC = () => {
  const { t } = useI18n('scenes/miniapp');
  const [activeTab, setActiveTab] = useState<MiniAppGalleryTab>('installed');

  return (
    <div className="miniapp-gallery-scene" data-bf-scene="miniapp-gallery" data-bf-part="root">
      <div className="miniapp-gallery-scene__nav">
        <TabGroup
          items={[
            { icon: <Download />, id: 'miniapp-gallery-tab-installed', label: t('market.tabs.installed'), panelId: 'miniapp-gallery-panel-installed', value: 'installed' },
            { icon: <Store />, id: 'miniapp-gallery-tab-market', label: t('market.tabs.market'), panelId: 'miniapp-gallery-panel-market', value: 'market' },
            { icon: <UploadCloud />, id: 'miniapp-gallery-tab-submissions', label: t('market.tabs.submissions'), panelId: 'miniapp-gallery-panel-submissions', value: 'submissions' },
          ]}
          value={activeTab}
          onValueChange={(key) => setActiveTab(key as MiniAppGalleryTab)}
        />
      </div>
      <div
        aria-labelledby={`miniapp-gallery-tab-${activeTab}`}
        className="miniapp-gallery-scene__panel"
        id={`miniapp-gallery-panel-${activeTab}`}
        role="tabpanel"
      >
        {activeTab === 'installed' && (
          <Suspense fallback={null}>
            <MiniAppGalleryView />
          </Suspense>
        )}
        {activeTab === 'market' && (
          <Suspense fallback={null}>
            <MiniAppMarketView />
          </Suspense>
        )}
        {activeTab === 'submissions' && (
          <Suspense fallback={null}>
            <MiniAppSubmissionsView />
          </Suspense>
        )}
      </div>
    </div>
  );
};

export default MiniAppGalleryScene;
