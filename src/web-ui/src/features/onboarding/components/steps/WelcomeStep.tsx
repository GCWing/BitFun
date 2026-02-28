/**
 * Welcome step.
 * WelcomeStep - shows the product intro and three pillars.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Zap, Eye, Puzzle } from 'lucide-react';

export const WelcomeStep: React.FC = () => {
  const { t } = useTranslation('onboarding');

  return (
    <div className="bitfun-onboarding-step bitfun-onboarding-welcome">
      {/* Title */}
      <div className="bitfun-onboarding-step__header">
        <h1 className="bitfun-onboarding-step__title">
          {t('welcome.title')}
        </h1>
        <p className="bitfun-onboarding-step__subtitle">
          {t('welcome.subtitle')}
        </p>
        <p className="bitfun-onboarding-step__description">
          {t('welcome.description')}
        </p>
      </div>

      {/* Three pillars */}
      <div className="bitfun-onboarding-welcome__pillars">
        <div className="bitfun-onboarding-welcome__pillar">
          <div className="bitfun-onboarding-welcome__pillar-icon">
            <Zap />
          </div>
          <div className="bitfun-onboarding-welcome__pillar-title">
            {t('welcome.pillars.agentic.title')}
          </div>
          <div className="bitfun-onboarding-welcome__pillar-desc">
            {t('welcome.pillars.agentic.description')}
          </div>
        </div>

        <div className="bitfun-onboarding-welcome__pillar">
          <div className="bitfun-onboarding-welcome__pillar-icon">
            <Eye />
          </div>
          <div className="bitfun-onboarding-welcome__pillar-title">
            {t('welcome.pillars.visual.title')}
          </div>
          <div className="bitfun-onboarding-welcome__pillar-desc">
            {t('welcome.pillars.visual.description')}
          </div>
        </div>

        <div className="bitfun-onboarding-welcome__pillar">
          <div className="bitfun-onboarding-welcome__pillar-icon">
            <Puzzle />
          </div>
          <div className="bitfun-onboarding-welcome__pillar-title">
            {t('welcome.pillars.extensible.title')}
          </div>
          <div className="bitfun-onboarding-welcome__pillar-desc">
            {t('welcome.pillars.extensible.description')}
          </div>
        </div>
      </div>

    </div>
  );
};

export default WelcomeStep;
