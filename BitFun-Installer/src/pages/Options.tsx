import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Checkbox } from '../components/Checkbox';
import { InstallErrorPanel } from '../components/InstallErrorPanel';
import type {
  InstallOptions,
  DiskSpaceInfo,
  InstallPathValidation,
  ExistingInstallation,
} from '../types/installer';

interface OptionsProps {
  options: InstallOptions;
  setOptions: React.Dispatch<React.SetStateAction<InstallOptions>>;
  diskSpace: DiskSpaceInfo | null;
  error: string | null;
  refreshDiskSpace: (path: string) => Promise<void>;
  existingInstall: ExistingInstallation | null;
  onLaunchRegisteredUninstaller: () => void | Promise<void>;
  onBack: () => void;
  onInstall: () => Promise<void>;
  isInstalling: boolean;
  clearInstallError: () => void;
}

export function Options({
  options,
  setOptions,
  diskSpace,
  error,
  refreshDiskSpace,
  existingInstall,
  onLaunchRegisteredUninstaller,
  onBack,
  onInstall,
  isInstalling,
  clearInstallError,
}: OptionsProps) {
  const { t } = useTranslation();

  useEffect(() => {
    if (options.installPath) refreshDiskSpace(options.installPath);
  }, [options.installPath, refreshDiskSpace]);

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      defaultPath: options.installPath,
      title: t('options.pathLabel'),
    });
    if (selected && typeof selected === 'string') {
      try {
        const validated = await invoke<InstallPathValidation>('validate_install_path', {
          path: selected,
        });
        setOptions((prev) => ({ ...prev, installPath: validated.installPath }));
      } catch {
        setOptions((prev) => ({ ...prev, installPath: selected }));
      }
      clearInstallError();
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  const update = (key: keyof InstallOptions, value: boolean) => {
    setOptions((prev) => ({ ...prev, [key]: value }));
  };

  return (
    <div className="page-shell">
      <div className="page-scroll">
        <div className="page-container page-container--center" style={{ maxWidth: 560 }}>
          <div style={{ marginBottom: 8, fontSize: 12, color: 'var(--color-text-muted)' }}>
            {t('options.subtitle')}
          </div>
          {existingInstall?.detected ? (
            <div
              style={{
                marginBottom: 16,
                padding: '12px 14px',
                borderRadius: 10,
                border: '1px solid color-mix(in srgb, var(--color-accent-500) 45%, transparent)',
                background: 'color-mix(in srgb, var(--color-accent-500) 8%, transparent)',
                fontSize: 12,
                lineHeight: 1.55,
                color: 'var(--color-text-primary)',
              }}
            >
              <div style={{ fontWeight: 600, marginBottom: 8 }}>{t('options.existingInstallTitle')}</div>
              {existingInstall.displayVersion ? (
                <div style={{ marginBottom: 4, wordBreak: 'break-all' }}>
                  {t('options.existingInstallVersion', { version: existingInstall.displayVersion })}
                </div>
              ) : null}
              {existingInstall.installLocation ? (
                <div style={{ marginBottom: 8, wordBreak: 'break-all', opacity: 0.95 }}>
                  {t('options.existingInstallLocation', { path: existingInstall.installLocation })}
                </div>
              ) : null}
              {!existingInstall.mainBinaryPresent ? (
                <div style={{ marginBottom: 8, color: 'var(--color-warning, #c9a227)' }}>
                  {t('options.existingInstallBinaryMissing')}
                </div>
              ) : null}
              <p style={{ margin: '0 0 10px', opacity: 0.88 }}>{t('options.existingInstallHint')}</p>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                {existingInstall.uninstallString ? (
                  <button
                    type="button"
                    className="btn"
                    style={{ padding: '8px 12px', fontSize: 12 }}
                    onClick={() => {
                      void onLaunchRegisteredUninstaller();
                    }}
                  >
                    {t('options.existingInstallRunUninstaller')}
                  </button>
                ) : null}
              </div>
            </div>
          ) : null}
          <div style={{ marginBottom: 20 }}>
            <div className="section-label">
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
              </svg>
              {t('options.pathLabel')}
            </div>
            <div className="input-group">
              <input
                className="input"
                type="text"
                value={options.installPath}
                disabled={isInstalling}
                onChange={(e) => {
                  setOptions((prev) => ({ ...prev, installPath: e.target.value }));
                  clearInstallError();
                }}
                placeholder={t('options.pathPlaceholder')}
              />
              <button
                className="btn"
                type="button"
                disabled={isInstalling}
                onClick={handleBrowse}
                style={{ padding: '10px 14px', flexShrink: 0 }}
              >
                {t('options.browse')}
              </button>
            </div>
            {diskSpace && (
              <div
                style={{
                  display: 'flex',
                  gap: 16,
                  marginTop: 8,
                  fontSize: 11,
                  color: 'var(--color-text-muted)',
                  opacity: 0.7,
                  flexWrap: 'wrap',
                }}
              >
                <span>{t('options.required')}: {formatBytes(diskSpace.required)}</span>
                <span>
                  {t('options.available')}:{' '}
                  {diskSpace.available < Number.MAX_SAFE_INTEGER ? formatBytes(diskSpace.available) : '-'}
                </span>
                {!diskSpace.sufficient && (
                  <span style={{ color: 'var(--color-error)' }}>{t('options.insufficientSpace')}</span>
                )}
              </div>
            )}
            {error && <InstallErrorPanel message={error} variant="options" />}
          </div>

          <div>
            <div className="section-label">{t('options.optionsLabel')}</div>
            <div className="checkbox-group stagger-children">
              <Checkbox
                checked={options.desktopShortcut}
                onChange={(value) => update('desktopShortcut', value)}
                label={t('options.desktopShortcut')}
              />
              <Checkbox
                checked={options.startMenu}
                onChange={(value) => update('startMenu', value)}
                label={t('options.startMenu')}
              />
            </div>
          </div>
        </div>
      </div>

      <div className="page-footer page-footer--split">
        <button className="btn btn-ghost" type="button" disabled={isInstalling} onClick={onBack}>
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polyline points="15 18 9 12 15 6" />
          </svg>
          {t('options.changeLanguage')}
        </button>
        <button
          className="btn btn-primary"
          type="button"
          onClick={() => { void onInstall(); }}
          disabled={
            !options.installPath
            || (diskSpace !== null && !diskSpace.sufficient)
            || isInstalling
          }
        >
          {isInstalling ? t('options.installing') : t('options.install')}
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polyline points="9 18 15 12 9 6" />
          </svg>
        </button>
      </div>
    </div>
  );
}
