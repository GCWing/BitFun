/**
 * MiniAppGalleryScene — Mini App gallery scene.
 * Opening an app opens a separate scene tab (miniapp:id).
 */
import React, { Suspense, lazy, useState } from 'react';
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
    <div className="miniapp-gallery-scene">
      <nav className="miniapp-gallery-scene__tabs" aria-label={t('market.tabs.label')}>
        <button
          type="button"
          className={activeTab === 'installed' ? 'is-active' : undefined}
          aria-current={activeTab === 'installed' ? 'page' : undefined}
          onClick={() => setActiveTab('installed')}
        >
          <Download size={14} />
          {t('market.tabs.installed')}
        </button>
        <button
          type="button"
          className={activeTab === 'market' ? 'is-active' : undefined}
          aria-current={activeTab === 'market' ? 'page' : undefined}
          onClick={() => setActiveTab('market')}
        >
          <Store size={14} />
          {t('market.tabs.market')}
        </button>
        <button
          type="button"
          className={activeTab === 'submissions' ? 'is-active' : undefined}
          aria-current={activeTab === 'submissions' ? 'page' : undefined}
          onClick={() => setActiveTab('submissions')}
        >
          <UploadCloud size={14} />
          {t('market.tabs.submissions')}
        </button>
      </nav>
      <Suspense fallback={null}>
        {activeTab === 'installed' ? <MiniAppGalleryView /> : null}
        {activeTab === 'market' ? <MiniAppMarketView /> : null}
        {activeTab === 'submissions' ? <MiniAppSubmissionsView /> : null}
      </Suspense>
    </div>
  );
};

export default MiniAppGalleryScene;
