import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { Checkbox } from '../components/Checkbox';
import type { InstallOptions, DiskSpaceInfo } from '../types/installer';

interface OptionsProps {
  options: InstallOptions;
  setOptions: React.Dispatch<React.SetStateAction<InstallOptions>>;
  diskSpace: DiskSpaceInfo | null;
  refreshDiskSpace: (path: string) => Promise<void>;
  onBack: () => void;
  onInstall: () => void;
}

export function Options({
  options, setOptions, diskSpace, refreshDiskSpace, onBack, onInstall,
}: OptionsProps) {
  const { t } = useTranslation();

  useEffect(() => {
    if (options.installPath) refreshDiskSpace(options.installPath);
  }, [options.installPath, refreshDiskSpace]);

  const handleBrowse = async () => {
    const selected = await open({ directory: true, defaultPath: options.installPath, title: t('options.pathLabel') });
    if (selected && typeof selected === 'string') setOptions((p) => ({ ...p, installPath: selected }));
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  const update = (key: keyof InstallOptions, value: boolean) => {
    setOptions((p) => ({ ...p, [key]: value }));
  };

  return (
    <div style={{
      flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'center',
      padding: '22px 42px 24px', maxWidth: 560, margin: '0 auto', width: '100%',
      animation: 'fadeIn 0.4s ease-out',
    }}>
      <div style={{ marginBottom: 8, fontSize: 12, color: 'var(--color-text-muted)' }}>
        {t('options.subtitle')}
      </div>
      <div style={{ marginBottom: 20 }}>
        <div className="section-label">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
          {t('options.pathLabel')}
        </div>
        <div className="input-group">
          <input
            className="input" type="text" value={options.installPath}
            onChange={(e) => setOptions((p) => ({ ...p, installPath: e.target.value }))}
            placeholder={t('options.pathPlaceholder')}
          />
          <button className="btn" onClick={handleBrowse} style={{ padding: '10px 14px', flexShrink: 0 }}>
            {t('options.browse')}
          </button>
        </div>
        {diskSpace && (
          <div style={{ display: 'flex', gap: 16, marginTop: 8, fontSize: 11, color: 'var(--color-text-muted)', opacity: 0.7 }}>
            <span>{t('options.required')}: {formatBytes(diskSpace.required)}</span>
            <span>{t('options.available')}: {diskSpace.available < Number.MAX_SAFE_INTEGER ? formatBytes(diskSpace.available) : '—'}</span>
            {!diskSpace.sufficient && <span style={{ color: 'var(--color-error)' }}>{t('options.insufficientSpace')}</span>}
          </div>
        )}
      </div>

      <div>
        <div className="section-label">{t('options.optionsLabel')}</div>
        <div className="checkbox-group stagger-children">
          <Checkbox checked={options.desktopShortcut} onChange={(v) => update('desktopShortcut', v)} label={t('options.desktopShortcut')} />
          <Checkbox checked={options.startMenu} onChange={(v) => update('startMenu', v)} label={t('options.startMenu')} />
          <Checkbox checked={options.contextMenu} onChange={(v) => update('contextMenu', v)} label={t('options.contextMenu')} />
          <Checkbox checked={options.addToPath} onChange={(v) => update('addToPath', v)} label={t('options.addToPath')} />
        </div>
      </div>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', paddingTop: 20, marginTop: 'auto' }}>
        <button className="btn btn-ghost" onClick={onBack}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="15 18 9 12 15 6" /></svg>
          {t('options.changeLanguage')}
        </button>
        <button
          className="btn btn-primary" onClick={onInstall}
          disabled={!options.installPath || (diskSpace !== null && !diskSpace.sufficient)}
        >
          {t('options.install')}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="9 18 15 12 9 6" /></svg>
        </button>
      </div>
    </div>
  );
}
