import React, { useCallback, useState } from 'react';
;

import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createLogger } from '@/shared/utils/logger';
import { Button, Icon, Tooltip } from '@bitfun/ui';
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
        <Button
          variant="text"
          size="xs"
          className="bitfun-nav-panel__footer-star-btn"
          onClick={handleStar}
          data-testid="nav-footer-github-star-btn"
          data-bf-component="nav-panel"
          data-bf-part="footerButton"
          leadingIcon={(
            <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
              <Icon name="star" size="sm" className="bitfun-nav-panel__footer-btn-icon-swap-default" />
              <Icon name="star" size="sm" className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
            </span>
          )}
        >
          <span className="bitfun-nav-panel__footer-star-label">{t('nav.githubStar.label')}</span>
        </Button>
      </Tooltip>
    </div>
  );
};

export default GithubStarButton;
