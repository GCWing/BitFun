import { useTranslation } from 'react-i18next';
import {
  formatInstallPathError,
  installPathErrorShowsAdminHint,
  parseInstallPathErrorCode,
} from '../utils/installPathErrors';

interface InstallErrorPanelProps {
  message: string;
  /** Options page: red alert box. Progress: plain text under title. */
  variant?: 'options' | 'bare';
}

export function InstallErrorPanel({ message, variant = 'options' }: InstallErrorPanelProps) {
  const { t } = useTranslation();
  const text = formatInstallPathError(message, t);
  const code = parseInstallPathErrorCode(message);
  const showAdmin = installPathErrorShowsAdminHint(code);

  const adminBlock = showAdmin ? (
    <div
      style={{
        marginTop: 10,
        padding: '10px 12px',
        borderRadius: 10,
        border: '1px solid color-mix(in srgb, var(--openbitfun-color-border-default) 70%, transparent)',
        background: 'color-mix(in srgb, var(--openbitfun-color-surface-subtle) 80%, transparent)',
        color: 'var(--openbitfun-color-content-secondary)',
        fontSize: 'var(--openbitfun-type-support-font-size)',
        lineHeight: 'var(--openbitfun-type-support-line-height)',
        textAlign: variant === 'bare' ? 'center' : 'left',
      }}
    >
      {t('errors.installPath.adminHint')}
    </div>
  ) : null;

  if (variant === 'bare') {
    return (
      <>
        <div
          style={{
            color: 'var(--openbitfun-color-content-muted)',
            fontSize: 'var(--openbitfun-type-body-xs-font-size)',
            lineHeight: 'var(--openbitfun-type-body-lg-line-height)',
            textAlign: 'center',
            maxWidth: 320,
          }}
        >
          {text}
        </div>
        {adminBlock}
      </>
    );
  }

  return (
    <div
      style={{
        marginTop: 10,
        padding: '10px 12px',
        borderRadius: 10,
        border: '1px solid color-mix(in srgb, var(--openbitfun-color-status-danger-content) 55%, transparent)',
        background: 'color-mix(in srgb, var(--openbitfun-color-status-danger-content) 10%, transparent)',
        color: 'var(--openbitfun-color-content-primary)',
        fontSize: 'var(--openbitfun-type-body-xs-font-size)',
        lineHeight: 'var(--openbitfun-type-body-md-line-height)',
      }}
    >
      {text}
      {adminBlock}
    </div>
  );
}
