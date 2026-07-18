import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Checkbox } from '../components/Checkbox';
import type { InstallOptions, ThemeId, ThemePreferenceId } from '../types/installer';
import { SYSTEM_THEME_ID } from '../types/installer';
import { THEMES, THEME_DISPLAY_ORDER, findInstallerThemeById } from '../theme/installerThemesData';

interface ThemeSetupProps {
  options: InstallOptions;
  setOptions: React.Dispatch<React.SetStateAction<InstallOptions>>;
  onLaunch: () => Promise<void>;
  onClose: () => void;
}

export function ThemeSetup({ options, setOptions, onLaunch, onClose }: ThemeSetupProps) {
  const { t } = useTranslation();
  const [isFinishing, setIsFinishing] = useState(false);
  const [finishError, setFinishError] = useState<string | null>(null);
  const orderedThemes = [...THEMES].sort((a, b) => THEME_DISPLAY_ORDER.indexOf(a.id) - THEME_DISPLAY_ORDER.indexOf(b.id));
  const lightPreview = findInstallerThemeById('bitfun-light');
  const darkPreview = findInstallerThemeById('bitfun-dark');

  const selectTheme = (theme: ThemePreferenceId) => {
    setOptions((prev) => ({ ...prev, themePreference: theme }));
  };

  const cardStyle = (active: boolean) => ({
    width: '100%',
    borderRadius: 12,
    padding: 8,
    background: active
      ? 'color-mix(in srgb, var(--color-accent-500) 8%, transparent)'
      : 'var(--element-bg-subtle)',
    border: 'none',
    cursor: 'pointer',
    transition: 'background 0.2s ease',
    textAlign: 'left' as const,
  });

  const previewBaseStyle = {
    height: 72,
    borderRadius: 8,
    overflow: 'hidden' as const,
    marginBottom: 8,
  };

  const handleFinish = async () => {
    if (isFinishing) return;
    setIsFinishing(true);
    setFinishError(null);

    try {
      try {
        await invoke('set_theme_preference', { themePreference: options.themePreference });
      } catch (err) {
        console.warn('Failed to persist theme preference:', err);
      }

      if (options.launchAfterInstall) {
        await onLaunch();
      }
      onClose();
    } catch (err: unknown) {
      setFinishError(typeof err === 'string' ? err : (err as Error)?.message || 'Failed to launch BitFun');
    } finally {
      setIsFinishing(false);
    }
  };

  return (
    <div className="page-shell">
      <div className="page-scroll">
        <div className="page-container page-container--center" style={{ maxWidth: 760 }}>
          <p style={{
            fontSize: 14,
            color: 'var(--color-text-secondary)',
            marginBottom: 12,
          }}>
            {t('themeSetup.subtitle')}
          </p>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(132px, 1fr))',
              gap: 8,
              marginBottom: 16,
              width: '100%',
            }}
          >
            <button
              type="button"
              style={cardStyle(options.themePreference === SYSTEM_THEME_ID)}
              onClick={() => selectTheme(SYSTEM_THEME_ID)}
            >
              <div style={{ ...previewBaseStyle, display: 'flex' }}>
                <div style={{ flex: 1, background: lightPreview.colors.background.primary }} />
                <div style={{ flex: 1, background: darkPreview.colors.background.primary }} />
              </div>
              <div style={{ fontSize: 11, color: 'var(--color-text-primary)' }}>
                {t('themeSetup.followSystem')}
              </div>
            </button>

            {orderedThemes.map((theme) => (
              <button key={theme.id} type="button" style={cardStyle(options.themePreference === theme.id)} onClick={() => selectTheme(theme.id as ThemeId)}>
                <div style={{ ...previewBaseStyle, background: theme.colors.background.primary }}>
                  <div style={{ height: 16, background: theme.colors.background.secondary, opacity: 0.9 }} />
                  <div style={{ display: 'flex', gap: 6, padding: 8 }}>
                    <div style={{ width: 22, height: 22, borderRadius: 5, background: theme.colors.element.soft, opacity: 0.9 }} />
                    <div style={{ flex: 1 }}>
                      <div style={{ height: 5, width: '62%', background: theme.colors.text.muted, opacity: 0.5, borderRadius: 3, marginBottom: 5 }} />
                      <div style={{ height: 5, width: '78%', background: theme.colors.accent, borderRadius: 3 }} />
                    </div>
                  </div>
                </div>
                <div style={{ fontSize: 11, color: 'var(--color-text-primary)' }}>
                  {t(`themeSetup.themeNames.${theme.id}`, { defaultValue: theme.name })}
                </div>
              </button>
            ))}
          </div>

          <div style={{ width: '100%', marginBottom: 14 }}>
            <Checkbox
              checked={options.launchAfterInstall}
              onChange={(checked) => setOptions((prev) => ({ ...prev, launchAfterInstall: checked }))}
              label={t('options.launchAfterInstall')}
            />
          </div>

          {finishError && (
            <div style={{
              color: 'var(--color-error)',
              marginBottom: 12,
              fontSize: 12,
              width: '100%',
            }}>
              {finishError}
            </div>
          )}
        </div>
      </div>

      <div className="page-footer page-footer--center">
        <button
          className="btn btn-success"
          onClick={handleFinish}
          disabled={isFinishing}
          style={{ minWidth: 120, justifyContent: 'center' }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M20 6L9 17L4 12" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          {t('complete.finish')}
        </button>
      </div>
    </div>
  );
}
