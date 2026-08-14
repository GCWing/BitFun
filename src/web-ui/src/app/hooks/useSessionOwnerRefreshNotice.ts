import { useEffect, useRef } from 'react';
import { SESSION_OWNER_REFRESH_EVENT } from '@/infrastructure/api/adapters/websocket-adapter';
import { api } from '@/infrastructure/api/service-api/ApiClient';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';

const reloadPage = () => window.location.reload();

export function useSessionOwnerRefreshNotice(
  reload: () => void = reloadPage,
): void {
  const { t } = useI18n('errors');
  const noticeShown = useRef(false);

  useEffect(() => api.listen(SESSION_OWNER_REFRESH_EVENT, () => {
    if (noticeShown.current) {
      return;
    }
    noticeShown.current = true;
    notificationService.persistent({
      type: 'warning',
      title: t('sessionOwnerRefresh.title'),
      message: t('sessionOwnerRefresh.message'),
      closable: false,
      actions: [{
        label: t('sessionOwnerRefresh.reload'),
        variant: 'primary',
        onClick: reload,
      }],
      metadata: { source: 'app-server-session-owner-refresh' },
    });
  }), [reload, t]);
}
