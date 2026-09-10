/**
 * WelcomeScene — the lightweight, tabless landing surface shown by
 * SceneViewport until the user opens a scene.
 */

import React, { useEffect, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { isReducedMotionPreferred } from '@/shared/utils/motionPreference';
import './WelcomeScene.scss';

const WORD_HOLD_MS = 3000;
const YOUR_WORD_INDEX = 2;

const WelcomeScene: React.FC = () => {
  const { t } = useI18n('common');
  const [wordIndex, setWordIndex] = useState(() => isReducedMotionPreferred() ? YOUR_WORD_INDEX : 0);
  const [reducedMotion, setReducedMotion] = useState(isReducedMotionPreferred);
  const [isVisible, setIsVisible] = useState(() => !document.hidden);
  const [isHovered, setIsHovered] = useState(false);
  const words = [
    t('welcomeScene.space.work'),
    t('welcomeScene.space.play'),
    t('welcomeScene.space.your'),
  ];
  const suffix = t('welcomeScene.space.suffix');

  useEffect(() => {
    const media = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    const handleMotionChange = () => {
      const prefersReducedMotion = isReducedMotionPreferred();
      setReducedMotion(prefersReducedMotion);
      if (prefersReducedMotion) setWordIndex(YOUR_WORD_INDEX);
    };
    const handleVisibilityChange = () => setIsVisible(!document.hidden);
    media?.addEventListener('change', handleMotionChange);
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      media?.removeEventListener('change', handleMotionChange);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (reducedMotion || !isVisible || isHovered || wordIndex >= YOUR_WORD_INDEX) return;
    const timer = window.setTimeout(() => {
      setWordIndex(index => Math.min(index + 1, YOUR_WORD_INDEX));
    }, WORD_HOLD_MS);
    return () => window.clearTimeout(timer);
  }, [wordIndex, reducedMotion, isVisible, isHovered]);

  const displayedWordIndex = reducedMotion ? YOUR_WORD_INDEX : wordIndex;

  return (
    <section
      className="welcome-scene"
      data-testid="welcome-scene"
      data-openbitfun-scene="welcome"
      data-openbitfun-part="root"
      aria-labelledby="welcome-scene-title"
    >
      <div className="welcome-scene__content" data-openbitfun-scene="welcome" data-openbitfun-part="content">
        <div
          className="welcome-scene__greeting"
          data-openbitfun-scene="welcome"
          data-openbitfun-part="greeting"
        >
          <h1
            id="welcome-scene-title"
            className="welcome-scene__brand"
            data-openbitfun-scene="welcome"
            data-openbitfun-part="title"
          >
            <span
              className="welcome-scene__logo"
              data-openbitfun-scene="welcome"
              data-openbitfun-part="logo"
              aria-hidden="true"
            />
            <span className="welcome-scene__brand-name">
              OpenBitFun{t('welcomeScene.space.separator')}
            </span>
          </h1>
          <h2
            className="welcome-scene__tagline"
            data-openbitfun-scene="welcome"
            data-openbitfun-part="subtitle"
            aria-label={`${words[YOUR_WORD_INDEX]}${suffix}`}
            onPointerEnter={() => setIsHovered(true)}
            onPointerLeave={() => setIsHovered(false)}
          >
            <span className="welcome-scene__word-slot" aria-hidden="true">
              {/* Keep every phrase in the same grid cell so the suffix stays still. */}
              {words.map((word, index) => (
                <span
                  className="welcome-scene__word"
                  data-active={index === displayedWordIndex ? 'true' : 'false'}
                  key={index}
                >
                  {word}
                </span>
              ))}
            </span>
            <span className="welcome-scene__suffix" aria-hidden="true">{suffix}</span>
          </h2>
        </div>
      </div>
    </section>
  );
};

export default WelcomeScene;
