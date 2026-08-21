/**
 * MiniAppGalleryScene — Mini App gallery scene.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy, useState } from 'react';
import { Download, Store, UploadCloud } from 'lucide-react';
import { Tabs, TabPane } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { isTauriRuntime } from '@/infrastructure/runtime';
import './MiniAppGalleryScene.scss';

const MiniAppGalleryView = lazy(() => import('./views/MiniAppGalleryView'));
const MiniAppMarketView = lazy(() => import('./views/MiniAppMarketView'));
const MiniAppSubmissionsView = lazy(() => import('./views/MiniAppSubmissionsView'));

type MiniAppGalleryTab = 'installed' | 'market' | 'submissions';

const MiniAppGalleryScene: React.FC = () => {
  const { t } = useI18n('scenes/miniapp');
  const [activeTab, setActiveTab] = useState<MiniAppGalleryTab>('installed');

  // MiniApps run inside the desktop host (worker pool + built-in seeding live
  // there). On server/web surfaces the scene degrades loudly instead of
  // rendering an empty gallery.
  if (!isTauriRuntime()) {
    return (
      <div className="miniapp-gallery-scene" data-bf-scene="miniapp-gallery" data-bf-part="root">
        <div className="miniapp-gallery-scene__unsupported">
          <div className="miniapp-gallery-scene__unsupported-card">
            <h2 className="miniapp-gallery-scene__unsupported-title">{t('unsupported.title')}</h2>
            <p className="miniapp-gallery-scene__unsupported-body">{t('unsupported.body')}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="miniapp-gallery-scene" data-bf-scene="miniapp-gallery" data-bf-part="root">
      <Tabs
        className="miniapp-gallery-scene__tabs"
        type="line"
        size="small"
        activeKey={activeTab}
        onChange={(key) => setActiveTab(key as MiniAppGalleryTab)}
      >
        <TabPane
          tabKey="installed"
          icon={<Download size={14} />}
          label={t('market.tabs.installed')}
        >
          <Suspense fallback={null}>
            <MiniAppGalleryView />
          </Suspense>
        </TabPane>
        <TabPane
          tabKey="market"
          icon={<Store size={14} />}
          label={t('market.tabs.market')}
        >
          <Suspense fallback={null}>
            <MiniAppMarketView />
          </Suspense>
        </TabPane>
        <TabPane
          tabKey="submissions"
          icon={<UploadCloud size={14} />}
          label={t('market.tabs.submissions')}
        >
          <Suspense fallback={null}>
            <MiniAppSubmissionsView />
          </Suspense>
        </TabPane>
      </Tabs>
    </div>
  );
};

export default MiniAppGalleryScene;
