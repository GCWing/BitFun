/**
 * SettingsNav — scene-specific left-side navigation for the Settings scene.
 *
 * Layout:
 *   ┌──────────────────────┐
 *   │ ← Settings           │  header: back button + title
 *   ├──────────────────────┤
 *   │  Category            │
 *   │    › Tab item        │  scrollable nav list
 *   │    › Tab item        │
 *   │  ...                 │
 *   └──────────────────────┘
 *
 * Clicking "back" closes the Settings scene (sceneStore.closeScene) which
 * automatically restores the previously-active scene and its nav.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from './settingsStore';
import { SETTINGS_CATEGORIES } from './settingsConfig';
import type { ConfigTab } from './settingsConfig';
import './SettingsNav.scss';

const SettingsNav: React.FC = () => {
  const { t } = useTranslation('settings');
  const { activeTab, setActiveTab } = useSettingsStore();

  const handleTabClick = useCallback((tab: ConfigTab) => {
    setActiveTab(tab);
  }, [setActiveTab]);

  return (
    <div className="bitfun-settings-nav">
      {/* Header: title only — back/forward handled by NavBar */}
      <div className="bitfun-settings-nav__header">
        <span className="bitfun-settings-nav__title">
          {t('configCenter.title', 'Settings')}
        </span>
      </div>

      {/* Scrollable category + tab list */}
      <div className="bitfun-settings-nav__sections">
        {SETTINGS_CATEGORIES.map(category => (
          <div key={category.id} className="bitfun-settings-nav__category">
            <div className="bitfun-settings-nav__category-header">
              <span className="bitfun-settings-nav__category-label">
                {t(category.nameKey, category.id)}
              </span>
            </div>

            <div className="bitfun-settings-nav__items">
              {category.tabs.map(tabDef => (
                <button
                  key={tabDef.id}
                  type="button"
                  className={[
                    'bitfun-settings-nav__item',
                    activeTab === tabDef.id && 'is-active',
                  ].filter(Boolean).join(' ')}
                  onClick={() => handleTabClick(tabDef.id)}
                >
                  {t(tabDef.labelKey, tabDef.id)}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default SettingsNav;
