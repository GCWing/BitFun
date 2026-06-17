/**
 * Alert component demo
 */

import React from 'react';
import { Alert } from './Alert';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';

const log = createLogger('AlertDemo');

export const AlertDemo: React.FC = () => {
  const { t } = useI18n('components');

  return (
    <div style={{ padding: '32px', maxWidth: '800px', display: 'flex', flexDirection: 'column', gap: '24px' }}>
      <div>
        <h2 style={{ color: 'var(--color-text-primary)', fontSize: '16px', fontWeight: '500', marginBottom: '4px' }}>
          {t('componentLibrary.alertDemo.title')}
        </h2>
        <p style={{ color: 'rgba(var(--color-static-white-rgb), 0.5)', fontSize: '13px' }}>
          {t('componentLibrary.alertDemo.subtitle')}
        </p>
      </div>

      <section>
        <h3 style={{ color: 'var(--color-overlay-white-60)', fontSize: '13px', fontWeight: '500', marginBottom: '12px' }}>
          {t('componentLibrary.alertDemo.sections.basic')}
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <Alert
            type="success"
            message={t('componentLibrary.alertDemo.messages.operationSuccess')}
          />
          <Alert
            type="error"
            message={t('componentLibrary.alertDemo.messages.operationFailed')}
          />
          <Alert
            type="warning"
            message={t('componentLibrary.alertDemo.messages.operationRisk')}
          />
          <Alert
            type="info"
            message={t('componentLibrary.alertDemo.messages.infoMessage')}
          />
        </div>
      </section>

      <section>
        <h3 style={{ color: 'var(--color-overlay-white-60)', fontSize: '13px', fontWeight: '500', marginBottom: '12px' }}>
          {t('componentLibrary.alertDemo.sections.withTitle')}
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <Alert
            type="success"
            title={t('componentLibrary.alertDemo.messages.deploySuccessTitle')}
            message={t('componentLibrary.alertDemo.messages.deploySuccessMessage')}
          />
          <Alert
            type="error"
            title={t('componentLibrary.alertDemo.messages.deployFailedTitle')}
            message={t('componentLibrary.alertDemo.messages.deployFailedMessage')}
          />
        </div>
      </section>

      <section>
        <h3 style={{ color: 'var(--color-overlay-white-60)', fontSize: '13px', fontWeight: '500', marginBottom: '12px' }}>
          {t('componentLibrary.alertDemo.sections.withDescription')}
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <Alert
            type="warning"
            title={t('componentLibrary.alertDemo.messages.expiringTitle')}
            message={t('componentLibrary.alertDemo.messages.expiringMessage')}
            description={t('componentLibrary.alertDemo.messages.expiringDescription')}
          />
          <Alert
            type="info"
            title={t('componentLibrary.alertDemo.messages.updateTitle')}
            message={t('componentLibrary.alertDemo.messages.updateMessage')}
            description={t('componentLibrary.alertDemo.messages.updateDescription')}
          />
        </div>
      </section>

      <section>
        <h3 style={{ color: 'var(--color-overlay-white-60)', fontSize: '13px', fontWeight: '500', marginBottom: '12px' }}>
          {t('componentLibrary.alertDemo.sections.closable')}
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <Alert
            type="success"
            message={t('componentLibrary.alertDemo.messages.closableMessage')}
            closable
            onClose={() => log.debug('Alert closed')}
          />
          <Alert
            type="info"
            title={t('componentLibrary.alertDemo.messages.closableInfoTitle')}
            message={t('componentLibrary.alertDemo.messages.closableInfoMessage')}
            closable
          />
        </div>
      </section>

      <section>
        <h3 style={{ color: 'var(--color-overlay-white-60)', fontSize: '13px', fontWeight: '500', marginBottom: '12px' }}>
          {t('componentLibrary.alertDemo.sections.noIcon')}
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <Alert
            type="info"
            message={t('componentLibrary.alertDemo.messages.noIconMessage')}
            showIcon={false}
          />
        </div>
      </section>
    </div>
  );
};

export default AlertDemo;
