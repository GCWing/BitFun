/**
 * Theme selection step
 * ThemeStep - choose UI theme with live preview
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Palette } from 'lucide-react';
import { themeService } from '@/infrastructure/theme';
import type { ThemeConfig } from '@/infrastructure/theme';

interface ThemeStepProps {
  selectedTheme: string;
  onThemeChange: (theme: string) => void;
}

const THEME_OPTIONS = [
  { 
    id: 'bitfun-dark', 
    nameKey: 'theme.themes.bitfun-dark.name',
    descKey: 'theme.themes.bitfun-dark.description'
  },
  { 
    id: 'bitfun-light', 
    nameKey: 'theme.themes.bitfun-light.name',
    descKey: 'theme.themes.bitfun-light.description'
  },
  { 
    id: 'bitfun-midnight', 
    nameKey: 'theme.themes.bitfun-midnight.name',
    descKey: 'theme.themes.bitfun-midnight.description'
  },
  { 
    id: 'bitfun-china-style', 
    nameKey: 'theme.themes.bitfun-china-style.name',
    descKey: 'theme.themes.bitfun-china-style.description'
  },
  { 
    id: 'bitfun-china-night', 
    nameKey: 'theme.themes.bitfun-china-night.name',
    descKey: 'theme.themes.bitfun-china-night.description'
  },
  { 
    id: 'bitfun-cyber', 
    nameKey: 'theme.themes.bitfun-cyber.name',
    descKey: 'theme.themes.bitfun-cyber.description'
  },
  { 
    id: 'bitfun-slate', 
    nameKey: 'theme.themes.bitfun-slate.name',
    descKey: 'theme.themes.bitfun-slate.description'
  }
];

/**
 * Theme preview thumbnail
 * Simulates the app UI with theme colors
 */
interface ThemePreviewProps {
  theme: ThemeConfig;
}

function ThemePreview({ theme }: ThemePreviewProps) {
  const { colors } = theme;
  
  return (
    <div 
      className="bitfun-onboarding-theme__preview-thumbnail"
      style={{
        background: colors.background.primary,
        borderColor: colors.border.base,
      }}
    >
      {/* Title bar */}
      <div 
        className="bitfun-onboarding-theme__preview-titlebar"
        style={{ 
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        <span 
          className="bitfun-onboarding-theme__preview-dot"
          style={{ background: colors.accent['500'] }}
        />
        <span 
          className="bitfun-onboarding-theme__preview-title"
          style={{ color: colors.text.muted }}
        >
          BitFun
        </span>
      </div>
      
      {/* Main content */}
      <div className="bitfun-onboarding-theme__preview-main">
        {/* Sidebar tree */}
        <div 
          className="bitfun-onboarding-theme__preview-sidebar"
          style={{ 
            background: colors.background.secondary,
            borderColor: colors.border.subtle,
          }}
        >
          <div className="bitfun-onboarding-theme__preview-tree-item">
            <span 
              className="bitfun-onboarding-theme__preview-folder"
              style={{ background: colors.accent['500'] }}
            />
            <span 
              className="bitfun-onboarding-theme__preview-text"
              style={{ background: colors.text.secondary }}
            />
          </div>
          {[1, 2, 3].map((i) => (
            <div key={i} className="bitfun-onboarding-theme__preview-tree-item">
              <span 
                className="bitfun-onboarding-theme__preview-file"
                style={{ background: colors.semantic.info }}
              />
              <span 
                className="bitfun-onboarding-theme__preview-text bitfun-onboarding-theme__preview-text--short"
                style={{ background: colors.text.muted }}
              />
            </div>
          ))}
        </div>
        
        {/* Chat area */}
        <div 
          className="bitfun-onboarding-theme__preview-chat"
          style={{ background: colors.background.flowchat }}
        >
          {/* User message */}
          <div 
            className="bitfun-onboarding-theme__preview-message bitfun-onboarding-theme__preview-message--user"
            style={{ 
              background: colors.accent['200'],
              borderColor: colors.accent['400'],
            }}
          >
            <div 
              className="bitfun-onboarding-theme__preview-line"
              style={{ background: colors.text.primary }}
            />
          </div>
          {/* AI response */}
          <div 
            className="bitfun-onboarding-theme__preview-message bitfun-onboarding-theme__preview-message--ai"
            style={{ 
              background: colors.element.subtle,
              borderColor: colors.border.subtle,
            }}
          >
            <div 
              className="bitfun-onboarding-theme__preview-line"
              style={{ background: colors.text.secondary }}
            />
            <div 
              className="bitfun-onboarding-theme__preview-line bitfun-onboarding-theme__preview-line--short"
              style={{ background: colors.text.muted }}
            />
          </div>
        </div>
        
        {/* Editor */}
        <div 
          className="bitfun-onboarding-theme__preview-editor"
          style={{ 
            background: colors.background.workbench,
            borderColor: colors.border.subtle,
          }}
        >
          {/* Tabs */}
          <div 
            className="bitfun-onboarding-theme__preview-tabs"
            style={{ 
              background: colors.background.secondary,
              borderColor: colors.border.subtle,
            }}
          >
            <span 
              className="bitfun-onboarding-theme__preview-tab bitfun-onboarding-theme__preview-tab--active"
              style={{ 
                background: colors.background.primary,
                borderColor: colors.accent['500'],
              }}
            />
            <span 
              className="bitfun-onboarding-theme__preview-tab"
              style={{ background: colors.element.subtle }}
            />
          </div>
          {/* Code lines */}
          <div className="bitfun-onboarding-theme__preview-code">
            {[1, 2, 3, 4].map((i) => (
              <div key={i} className="bitfun-onboarding-theme__preview-code-line">
                <span 
                  className="bitfun-onboarding-theme__preview-line-num"
                  style={{ background: colors.text.disabled }}
                />
                <span 
                  className="bitfun-onboarding-theme__preview-line-code"
                  style={{ 
                    background: i % 2 === 0 ? colors.accent['500'] : colors.text.secondary,
                    width: `${30 + (i * 12) % 40}%`,
                  }}
                />
              </div>
            ))}
          </div>
        </div>
      </div>
      
      {/* Status bar */}
      <div 
        className="bitfun-onboarding-theme__preview-statusbar"
        style={{ 
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        <span 
          className="bitfun-onboarding-theme__preview-status-dot"
          style={{ background: colors.accent['500'] }}
        />
        <span 
          className="bitfun-onboarding-theme__preview-status-text"
          style={{ background: colors.text.muted }}
        />
      </div>
    </div>
  );
}

export const ThemeStep: React.FC<ThemeStepProps> = ({
  selectedTheme,
  onThemeChange
}) => {
  const { t } = useTranslation('onboarding');

  return (
    <div className="bitfun-onboarding-step bitfun-onboarding-theme">
      {/* Icon */}
      <div className="bitfun-onboarding-step__icon">
        <Palette />
      </div>

      {/* Title */}
      <div className="bitfun-onboarding-step__header">
        <h1 className="bitfun-onboarding-step__title">
          {t('theme.title')}
        </h1>
        <p className="bitfun-onboarding-step__description">
          {t('theme.description')}
        </p>
      </div>

      {/* Theme grid */}
      <div className="bitfun-onboarding-theme__grid">
        {THEME_OPTIONS.map((themeOption) => {
          const fullTheme = themeService.getTheme(themeOption.id as any);
          
          return (
            <div
              key={themeOption.id}
              className={`bitfun-onboarding-theme__card ${
                selectedTheme === themeOption.id ? 'bitfun-onboarding-theme__card--selected' : ''
              }`}
              onClick={() => onThemeChange(themeOption.id)}
            >
              {/* Theme preview */}
              {fullTheme ? (
                <ThemePreview theme={fullTheme} />
              ) : (
                <div className="bitfun-onboarding-theme__preview bitfun-onboarding-theme__preview--fallback" />
              )}
              
              <div className="bitfun-onboarding-theme__info">
                <div className="bitfun-onboarding-theme__name">
                  {t(themeOption.nameKey)}
                </div>
                <div className="bitfun-onboarding-theme__desc">
                  {t(themeOption.descKey)}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default ThemeStep;
