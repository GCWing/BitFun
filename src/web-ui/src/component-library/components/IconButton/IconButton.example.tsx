/**
 * IconButton component usage examples
 * Show various variants and states
 */

import React from 'react';
import { IconButton } from './IconButton';
import { useI18n } from '@/infrastructure/i18n';

const PlusIcon = () => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
    <path d="M8 3v10M3 8h10" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
  </svg>
);

const HeartIcon = () => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
    <path d="M8 14s-6-4-6-8c0-2.5 2-4 4-4 1.5 0 2.5 1 2 2 0 0-.5-1 0-2 1.5 0 4 1.5 4 4 0 4-4 8-4 8z" />
  </svg>
);

export const IconButtonExample: React.FC = () => {
  const { t } = useI18n('components');

  return (
    <div style={{ padding: '24px', background: 'var(--preview-bg)', minHeight: '100vh' }}>
      <h2 style={{ color: 'var(--color-static-white)', marginBottom: '24px' }}>
        {t('componentLibrary.iconButtonExample.title')}
      </h2>
      
      <div style={{ marginBottom: '32px' }}>
        <h3 style={{ color: 'var(--preview-text-muted)', marginBottom: '16px' }}>
          {t('componentLibrary.iconButtonExample.sections.basic')}
        </h3>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <IconButton variant="default" size="small">
            <PlusIcon />
          </IconButton>
          <IconButton variant="default" size="medium">
            <PlusIcon />
          </IconButton>
          <IconButton variant="default" size="large">
            <PlusIcon />
          </IconButton>
          <IconButton variant="default" size="medium" disabled>
            <PlusIcon />
          </IconButton>
        </div>
      </div>

      <div style={{ marginBottom: '32px' }}>
        <h3 style={{ color: 'var(--preview-text-muted)', marginBottom: '16px' }}>
          {t('componentLibrary.iconButtonExample.sections.ghost')}
        </h3>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <IconButton variant="ghost" size="small">
            <HeartIcon />
          </IconButton>
          <IconButton variant="ghost" size="medium">
            <HeartIcon />
          </IconButton>
          <IconButton variant="ghost" size="large">
            <HeartIcon />
          </IconButton>
        </div>
      </div>

      <div style={{ marginBottom: '32px' }}>
        <h3 style={{ color: 'var(--preview-text-muted)', marginBottom: '16px' }}>
          {t('componentLibrary.iconButtonExample.sections.primary')}
        </h3>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <IconButton variant="primary" size="small">
            <PlusIcon />
          </IconButton>
          <IconButton variant="primary" size="medium">
            <PlusIcon />
          </IconButton>
          <IconButton variant="primary" size="large">
            <PlusIcon />
          </IconButton>
          <IconButton variant="primary" size="medium" disabled>
            <PlusIcon />
          </IconButton>
        </div>
      </div>

      <div style={{ marginBottom: '32px' }}>
        <h3 style={{ color: 'var(--preview-text-muted)', marginBottom: '16px' }}>
          {t('componentLibrary.iconButtonExample.sections.shape')}
        </h3>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <IconButton variant="default" shape="circle" size="medium">
            <PlusIcon />
          </IconButton>
          <IconButton variant="primary" shape="circle" size="medium">
            <HeartIcon />
          </IconButton>
        </div>
      </div>

      <div style={{ marginBottom: '32px' }}>
        <h3 style={{ color: 'var(--preview-text-muted)', marginBottom: '16px' }}>
          {t('componentLibrary.iconButtonExample.sections.other')}
        </h3>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <IconButton variant="danger" size="medium">
            <PlusIcon />
          </IconButton>
          <IconButton variant="success" size="medium">
            <PlusIcon />
          </IconButton>
          <IconButton variant="warning" size="medium">
            <PlusIcon />
          </IconButton>
          <IconButton variant="ai" size="medium">
            <PlusIcon />
          </IconButton>
        </div>
      </div>

      <div style={{ marginTop: '48px', padding: '16px', background: 'var(--color-overlay-white-08)', borderRadius: '8px' }}>
        <h3 style={{ color: 'var(--color-static-white)', marginBottom: '12px' }}>
          {t('componentLibrary.iconButtonExample.sections.usage')}
        </h3>
        <ul style={{ color: 'var(--preview-text-muted)', lineHeight: '1.8' }}>
          <li>
            <strong>{t('componentLibrary.iconButtonExample.usage.defaultGhost.label')}</strong>
            {t('componentLibrary.iconButtonExample.usage.defaultGhost.text')}
          </li>
          <li>
            <strong>{t('componentLibrary.iconButtonExample.usage.primary.label')}</strong>
            {t('componentLibrary.iconButtonExample.usage.primary.text')}
          </li>
          <li>
            <strong>{t('componentLibrary.iconButtonExample.usage.disabled.label')}</strong>
            {t('componentLibrary.iconButtonExample.usage.disabled.text')}
          </li>
          <li>
            <strong>{t('componentLibrary.iconButtonExample.usage.theme.label')}</strong>
            {t('componentLibrary.iconButtonExample.usage.theme.text')}
          </li>
        </ul>
      </div>
    </div>
  );
};

export default IconButtonExample;

