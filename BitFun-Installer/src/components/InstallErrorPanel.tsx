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
        border: '1px solid color-mix(in srgb, var(--border-base) 70%, transparent)',
        background: 'color-mix(in srgb, var(--element-bg-subtle) 80%, transparent)',
        color: 'var(--color-text-secondary)',
        fontSize: 11,
        lineHeight: 1.55,
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
            color: 'var(--color-text-muted)',
            fontSize: 12,
            lineHeight: 1.6,
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
        border: '1px solid color-mix(in srgb, var(--color-error) 55%, transparent)',
        background: 'color-mix(in srgb, var(--color-error) 10%, transparent)',
        color: 'var(--color-text-primary)',
        fontSize: 12,
        lineHeight: 1.5,
      }}
    >
      {text}
      {adminBlock}
    </div>
  );
}
