/**
 * WelcomeScene — the lightweight, tabless landing surface shown by
 * SceneViewport until the user opens a scene.
 */

import React, { useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import './WelcomeScene.scss';

type GreetingPeriod = 'morning' | 'afternoon' | 'evening';

function getGreetingPeriod(hour: number): GreetingPeriod {
  if (hour < 12) return 'morning';
  if (hour < 18) return 'afternoon';
  return 'evening';
}

const WelcomeScene: React.FC = () => {
  const { t } = useI18n('common');
  const [greetingPeriod] = useState<GreetingPeriod>(
    () => getGreetingPeriod(new Date().getHours()),
  );
  const greeting = greetingPeriod === 'morning'
    ? t('welcomeScene.greeting.morning')
    : greetingPeriod === 'afternoon'
      ? t('welcomeScene.greeting.afternoon')
      : t('welcomeScene.greeting.evening');

  return (
    <section
      className="welcome-scene"
      data-testid="welcome-scene"
      data-bf-scene="welcome"
      data-bf-part="root"
      aria-labelledby="welcome-scene-title"
    >
      <div className="welcome-scene__content" data-bf-scene="welcome" data-bf-part="content">
        <div className="welcome-scene__greeting" data-bf-scene="welcome" data-bf-part="greeting">
          <h1
            id="welcome-scene-title"
            className="welcome-scene__title"
            data-bf-scene="welcome"
            data-bf-part="title"
          >
            {greeting}
          </h1>
          <p
            className="welcome-scene__greeting-label"
            data-bf-scene="welcome"
            data-bf-part="subtitle"
          >
            {t('welcomeScene.greeting.subtitle')}
          </p>
        </div>
      </div>
    </section>
  );
};

export default WelcomeScene;
