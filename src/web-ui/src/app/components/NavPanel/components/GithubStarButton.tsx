import React, { useCallback, useState } from 'react';
import { Star } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import {
  GITHUB_STAR_URL,
  isGithubStarCtaDismissed,
  setGithubStarCtaDismissed,
} from './githubStarCtaStorage';

const log = createLogger('GithubStarButton');

/**
 * One-shot invitation to star the repo on GitHub. Clicking it retires the
 * button for good, so it never becomes a standing nag.
 */
const GithubStarButton: React.FC = () => {
  const { t } = useI18n('common');
  const [retired, setRetired] = useState(() => isGithubStarCtaDismissed());

  const retire = useCallback(() => {
    setGithubStarCtaDismissed();
    setRetired(true);
  }, []);

  const handleStar = useCallback(() => {
    systemAPI.openExternal(GITHUB_STAR_URL).catch((error) => {
      log.error('Failed to open the GitHub repository', { url: GITHUB_STAR_URL, error });
    });
    retire();
  }, [retire]);

  if (retired) return null;

  return (
    <div className="bitfun-nav-panel__footer-star">
      <Tooltip content={t('nav.githubStar.tooltip')} placement="top">
        <button
          type="button"
          className="bitfun-nav-panel__footer-star-btn"
          onClick={handleStar}
          data-testid="nav-footer-github-star-btn"
          data-bf-component="nav-panel"
          data-bf-part="footerButton"
        >
          <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
            <Star size={14} className="bitfun-nav-panel__footer-btn-icon-swap-default" />
            <Star size={14} fill="currentColor" className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
          </span>
          <span className="bitfun-nav-panel__footer-star-label">{t('nav.githubStar.label')}</span>
        </button>
      </Tooltip>
    </div>
  );
};

export default GithubStarButton;
