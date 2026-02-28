/**
 * SkillsNav — scene-specific left-side navigation for the Skills scene.
 *
 * Layout:
 *   ┌──────────────────────┐
 *   │  技能（Skills）       │  header: title
 *   ├──────────────────────┤
 *   │  DISCOVER            │
 *   │    › 市场            │  scrollable nav list
 *   │  INSTALLED           │
 *   │    › 全部            │
 *   │    › 用户级           │
 *   │    › 项目级           │
 *   └──────────────────────┘
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useSkillsSceneStore } from './skillsSceneStore';
import { SKILLS_NAV_CATEGORIES } from './skillsConfig';
import type { SkillsView } from './skillsSceneStore';
import './SkillsNav.scss';

const SkillsNav: React.FC = () => {
  const { t } = useTranslation('scenes/skills');
  const { activeView, setActiveView } = useSkillsSceneStore();

  const handleItemClick = useCallback(
    (view: SkillsView) => {
      setActiveView(view);
    },
    [setActiveView]
  );

  return (
    <div className="bitfun-skills-nav">
      <div className="bitfun-skills-nav__header">
        <span className="bitfun-skills-nav__title">{t('nav.title')}</span>
      </div>

      <div className="bitfun-skills-nav__sections">
        {SKILLS_NAV_CATEGORIES.map((category) => (
          <div key={category.id} className="bitfun-skills-nav__category">
            <div className="bitfun-skills-nav__category-header">
              <span className="bitfun-skills-nav__category-label">
                {t(category.nameKey)}
              </span>
            </div>
            <div className="bitfun-skills-nav__items">
              {category.items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={[
                    'bitfun-skills-nav__item',
                    activeView === item.id && 'is-active',
                  ].filter(Boolean).join(' ')}
                  onClick={() => handleItemClick(item.id)}
                >
                  {t(item.labelKey)}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default SkillsNav;
