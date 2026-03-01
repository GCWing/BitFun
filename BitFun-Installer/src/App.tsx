import { useTranslation } from 'react-i18next';
import { WindowControls } from './components/WindowControls';
import { LanguageSelect } from './pages/LanguageSelect';
import { Options } from './pages/Options';
import { ModelSetup } from './pages/ModelSetup';
import { ProgressPage } from './pages/Progress';
import { ThemeSetup } from './pages/ThemeSetup';
import { UninstallPage } from './pages/Uninstall';
import { useInstaller } from './hooks/useInstaller';
import './styles/global.css';

const STEP_NUMBERS: Record<string, number> = {
  options: 2,
  progress: 2,
  model: 3,
  theme: 4,
};

function App() {
  const installer = useInstaller();
  const { t, i18n } = useTranslation();

  const handleLanguageSelect = (lang: string) => {
    i18n.changeLanguage(lang);
    installer.setOptions((prev) => ({
      ...prev,
      appLanguage: lang === 'en' ? 'en-US' : 'zh-CN',
    }));
    installer.next();
  };

  const STEP_TITLES: Record<string, string> = {
    options: t('options.title'),
    model: t('model.title'),
    progress: t('progress.title'),
    theme: t('themeSetup.title'),
    uninstall: t('uninstall.title'),
  };

  const renderPage = () => {
    switch (installer.step) {
      case 'lang':
        return <LanguageSelect onSelect={handleLanguageSelect} />;
      case 'options':
        return (
          <Options
            options={installer.options}
            setOptions={installer.setOptions}
            diskSpace={installer.diskSpace}
            refreshDiskSpace={installer.refreshDiskSpace}
            onBack={installer.back}
            onInstall={installer.install}
          />
        );
      case 'model':
        return (
          <ModelSetup
            options={installer.options}
            setOptions={installer.setOptions}
            onSkip={installer.next}
            onTestConnection={installer.testModelConnection}
            onNext={async () => {
              await installer.saveModelConfig();
              installer.next();
            }}
          />
        );
      case 'progress':
        return (
          <ProgressPage
            progress={installer.progress}
            error={installer.error}
            canConfirmProgress={installer.canConfirmProgress}
            onConfirmProgress={installer.confirmProgress}
            onRetry={installer.retryInstall}
            onBackToOptions={installer.backToOptions}
          />
        );
      case 'theme':
        return (
          <ThemeSetup
            options={installer.options}
            setOptions={installer.setOptions}
            onLaunch={installer.launchApp}
            onClose={installer.closeInstaller}
          />
        );
      case 'uninstall':
        return (
          <UninstallPage
            installPath={installer.options.installPath}
            isUninstalling={installer.isUninstalling}
            uninstallCompleted={installer.uninstallCompleted}
            uninstallError={installer.uninstallError}
            uninstallProgress={installer.uninstallProgress}
            onUninstall={installer.startUninstall}
            onClose={installer.closeInstaller}
          />
        );
      default:
        return null;
    }
  };

  const isFullscreen = installer.step === 'lang' || installer.step === 'uninstall';
  const stepNum = STEP_NUMBERS[installer.step];
  const title = STEP_TITLES[installer.step] || t('titlebar.default');
  const useSuccessStepColor = installer.installationCompleted;

  return (
    <div className="installer-app">
      <div className="titlebar" data-tauri-drag-region>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="titlebar-title">
            {isFullscreen ? t('titlebar.default') : (
              <>
                <span style={{ opacity: 0.4 }}>{stepNum} / 4</span>
                <span style={{ margin: '0 6px', opacity: 0.2 }}>·</span>
                <span>{title}</span>
              </>
            )}
          </span>
        </div>
        <WindowControls />
      </div>

      {!isFullscreen && (
        <div style={{
          height: 1,
          background: 'repeating-linear-gradient(90deg, var(--element-bg-medium) 0 5px, transparent 5px 10px)',
          position: 'relative',
          flexShrink: 0,
          overflow: 'hidden',
        }}>
          <div style={{
            height: '100%',
            width: `${((stepNum ?? 0) / 4) * 100}%`,
            background: useSuccessStepColor
              ? 'repeating-linear-gradient(90deg, var(--color-success) 0 5px, transparent 5px 10px)'
              : 'repeating-linear-gradient(90deg, var(--color-accent-500) 0 5px, transparent 5px 10px)',
            transition: 'width 400ms cubic-bezier(0.4, 0, 0.2, 1), background 300ms ease',
          }} />
        </div>
      )}

      <div className="installer-content">
        {renderPage()}
      </div>
    </div>
  );
}

export default App;
