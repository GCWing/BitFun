 

import { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Upload } from 'lucide-react';
import { useTheme, useThemeManagement, ThemeMetadata, ThemeConfig as ThemeConfigType } from '@/infrastructure/theme';
import { themeService } from '@/infrastructure/theme/core/ThemeService';
import { useLanguageSelector } from '@/infrastructure/i18n';
import {
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageContent,
  ConfigPageSection,
  ConfigPageRow,
} from './common';
import { IconButton, Tooltip, Select } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './ThemeConfig.scss';

const log = createLogger('ThemeConfig');

export function ThemeConfig() {
  const { t } = useTranslation('settings/theme');
  const { themeId, themes, setTheme, loading } = useTheme();
  const { importTheme } = useThemeManagement();
  const { currentLanguage, supportedLocales, selectLanguage, isChanging } = useLanguageSelector();
  const [importing, setImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  
  
  const handleThemeChange = async (newThemeId: string) => {
    await setTheme(newThemeId);
  };
  
  const handleImportTheme = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    
    setImporting(true);
    try {
      const text = await file.text();
      const themeData = JSON.parse(text);
      await importTheme(themeData);
      alert(t('theme.importSuccess'));
    } catch (error) {
      log.error('Failed to import theme', error);
      alert(t('theme.importFailed'));
    } finally {
      setImporting(false);
      
      event.target.value = '';
    }
  };
  
  
  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const getThemeDisplayName = (theme: ThemeMetadata) => {
    const i18nKey = `theme.presets.${theme.id}`;
    return theme.builtin
      ? t(`${i18nKey}.name`, { defaultValue: theme.name })
      : theme.name;
  };

  const getThemeDisplayDescription = (theme: ThemeMetadata) => {
    const i18nKey = `theme.presets.${theme.id}`;
    return theme.builtin
      ? t(`${i18nKey}.description`, { defaultValue: theme.description || '' })
      : theme.description || '';
  };
  
  return (
    <ConfigPageLayout className="theme-config">
      <ConfigPageHeader
        title={t('theme.title')}
        subtitle={t('theme.subtitle')}
      />
      
      <ConfigPageContent className="theme-config__content">
        <ConfigPageSection
          title={t('theme.language')}
          description={t('theme.languageSectionHint', { defaultValue: 'Configure display language for the interface.' })}
        >
          <ConfigPageRow
            label={t('theme.language')}
            description={t('theme.languageRowHint', { defaultValue: 'Choose one language pack as the active UI language.' })}
            align="center"
          >
            <div className="theme-config__language-select">
              <Select
                value={currentLanguage}
                onChange={(value) => selectLanguage(value as string)}
                options={supportedLocales.map((locale) => ({
                  value: locale.id,
                  label: locale.nativeName,
                }))}
                disabled={isChanging}
                placeholder={t('theme.language')}
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('theme.themes')}
          description={t('theme.themeSectionHint', { defaultValue: 'Select, import, export, or manage visual themes.' })}
          extra={
            <>
              <IconButton
                variant="ghost"
                size="small"
                onClick={handleImportClick}
                disabled={importing || loading}
                isLoading={importing}
                title={importing ? t('theme.importing') : t('theme.importTheme')}
              >
                <Upload size={16} />
              </IconButton>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json"
                onChange={handleImportTheme}
                style={{ display: 'none' }}
              />
            </>
          }
        >
          <ConfigPageRow
            label={t('theme.themes')}
            description={t('theme.themeRowHint', { defaultValue: 'Pick an installed theme or manage custom themes.' })}
            align="center"
          >
            <div className="theme-config__theme-picker">
              <div className="theme-config__theme-select">
                <Select
                  value={themeId}
                  onChange={(value) => handleThemeChange(value as string)}
                  disabled={loading}
                  options={themes.map((theme) => ({
                    value: theme.id,
                    label: getThemeDisplayName(theme),
                    description: getThemeDisplayDescription(theme),
                  }))}
                  renderOption={(option) => {
                    const theme = themes.find((item) => item.id === String(option.value));
                    const fullTheme = theme ? themeService.getTheme(theme.id) : null;
                    const optionContent = (
                      <div className="theme-config__theme-option">
                        <span className="theme-config__theme-option-name">{option.label}</span>
                        {option.description && (
                          <span className="theme-config__theme-option-desc">{option.description}</span>
                        )}
                      </div>
                    );

                    if (!fullTheme) return optionContent;

                    return (
                      <Tooltip
                        content={<ThemePreviewThumbnail theme={fullTheme} />}
                        placement="right"
                        delay={300}
                        className="theme-preview-tooltip"
                      >
                        {optionContent}
                      </Tooltip>
                    );
                  }}
                />
              </div>
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
}

 
interface ThemePreviewThumbnailProps {
  theme: ThemeConfigType;
}

function ThemePreviewThumbnail({ theme }: ThemePreviewThumbnailProps) {
  const { colors } = theme;
  
  return (
    <div 
      className="theme-preview-thumbnail"
      style={{
        background: colors.background.primary,
        borderColor: colors.border.base,
      }}
    >
      
      <div 
        className="theme-preview-thumbnail__titlebar"
        style={{ 
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        
        <div className="theme-preview-thumbnail__menu">
          <span 
            className="theme-preview-thumbnail__menu-dot"
            style={{ background: colors.accent['500'] }}
          />
        </div>
        
        
        <div 
          className="theme-preview-thumbnail__title"
          style={{ color: colors.text.muted }}
        >
          BitFun
        </div>
        
        
        <div className="theme-preview-thumbnail__window-controls">
          
          <span 
            className="theme-preview-thumbnail__window-btn"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 14 14" fill="none">
              <line x1="3" y1="7" x2="11" y2="7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </span>
          
          <span 
            className="theme-preview-thumbnail__window-btn"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 12 12" fill="none">
              <rect x="2" y="2" width="8" height="8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </span>
          
          <span 
            className="theme-preview-thumbnail__window-btn theme-preview-thumbnail__window-btn--close"
            style={{ color: colors.text.secondary }}
          >
            <svg width="8" height="8" viewBox="0 0 14 14" fill="none">
              <line x1="3" y1="3" x2="11" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
              <line x1="11" y1="3" x2="3" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </span>
        </div>
      </div>
      
      
      <div className="theme-preview-thumbnail__main">
        
        <div 
          className="theme-preview-thumbnail__sidebar"
          style={{ 
            background: colors.background.secondary,
            borderColor: colors.border.subtle,
          }}
        >
          
          <div className="theme-preview-thumbnail__tree-item">
            <span 
              className="theme-preview-thumbnail__folder-icon"
              style={{ background: colors.accent['500'] }}
            />
            <span 
              className="theme-preview-thumbnail__tree-text"
              style={{ background: colors.text.secondary }}
            />
          </div>
          
          {[1, 2, 3].map((i) => (
            <div key={i} className="theme-preview-thumbnail__tree-item theme-preview-thumbnail__tree-item--file">
              <span 
                className="theme-preview-thumbnail__file-icon"
                style={{ background: colors.semantic.info }}
              />
              <span 
                className="theme-preview-thumbnail__tree-text theme-preview-thumbnail__tree-text--short"
                style={{ background: colors.text.muted }}
              />
            </div>
          ))}
        </div>
        
        
        <div 
          className="theme-preview-thumbnail__chat"
          style={{ background: colors.background.flowchat }}
        >
          
          <div 
            className="theme-preview-thumbnail__message theme-preview-thumbnail__message--user"
            style={{ 
              background: colors.accent['200'],
              borderColor: colors.accent['400'],
            }}
          >
            <div 
              className="theme-preview-thumbnail__message-line"
              style={{ background: colors.text.primary }}
            />
          </div>
          
          <div 
            className="theme-preview-thumbnail__message theme-preview-thumbnail__message--ai"
            style={{ 
              background: colors.element.subtle,
              borderColor: colors.border.subtle,
            }}
          >
            <div 
              className="theme-preview-thumbnail__message-line"
              style={{ background: colors.text.secondary }}
            />
            <div 
              className="theme-preview-thumbnail__message-line theme-preview-thumbnail__message-line--short"
              style={{ background: colors.text.muted }}
            />
          </div>
          
          <div 
            className="theme-preview-thumbnail__code-block"
            style={{ 
              background: colors.background.tertiary,
              borderColor: colors.border.base,
            }}
          >
            <div 
              className="theme-preview-thumbnail__code-line"
              style={{ background: colors.purple?.['500'] || colors.accent['500'] }}
            />
            <div 
              className="theme-preview-thumbnail__code-line theme-preview-thumbnail__code-line--long"
              style={{ background: colors.semantic.success }}
            />
          </div>
        </div>
        
        
        <div 
          className="theme-preview-thumbnail__editor"
          style={{ 
            background: colors.background.workbench,
            borderColor: colors.border.subtle,
          }}
        >
          
          <div 
            className="theme-preview-thumbnail__tabs"
            style={{ 
              background: colors.background.secondary,
              borderColor: colors.border.subtle,
            }}
          >
            <span 
              className="theme-preview-thumbnail__tab theme-preview-thumbnail__tab--active"
              style={{ 
                background: colors.background.primary,
                borderColor: colors.accent['500'],
              }}
            />
            <span 
              className="theme-preview-thumbnail__tab"
              style={{ background: colors.element.subtle }}
            />
          </div>
          
          <div className="theme-preview-thumbnail__code-content">
            {[1, 2, 3, 4, 5].map((i) => (
              <div key={i} className="theme-preview-thumbnail__editor-line">
                <span 
                  className="theme-preview-thumbnail__line-number"
                  style={{ background: colors.text.disabled }}
                />
                <span 
                  className="theme-preview-thumbnail__line-code"
                  style={{ 
                    background: i % 2 === 0 ? colors.accent['500'] : colors.text.secondary,
                    width: `${30 + (i * 8) % 40}%`,
                  }}
                />
              </div>
            ))}
          </div>
        </div>
      </div>
      
      
      <div 
        className="theme-preview-thumbnail__statusbar"
        style={{ 
          background: colors.background.secondary,
          borderColor: colors.border.subtle,
        }}
      >
        
        <div className="theme-preview-thumbnail__status-section">
          <span 
            className="theme-preview-thumbnail__status-icon"
            style={{ background: colors.accent['500'] }}
          />
          <span 
            className="theme-preview-thumbnail__status-text"
            style={{ background: colors.text.muted }}
          />
        </div>
        
        
        <div className="theme-preview-thumbnail__status-section">
          <span 
            className="theme-preview-thumbnail__git-icon"
            style={{ color: colors.git.branch }}
          >
            <svg width="7" height="7" viewBox="0 0 16 16" fill="none">
              <circle cx="4" cy="4" r="2" stroke="currentColor" strokeWidth="1.5"/>
              <circle cx="12" cy="12" r="2" stroke="currentColor" strokeWidth="1.5"/>
              <circle cx="12" cy="4" r="2" stroke="currentColor" strokeWidth="1.5"/>
              <path d="M4 6v4c0 1.1.9 2 2 2h4" stroke="currentColor" strokeWidth="1.5"/>
            </svg>
          </span>
          <span 
            className="theme-preview-thumbnail__status-text theme-preview-thumbnail__status-text--branch"
            style={{ background: colors.git.branch }}
          />
        </div>
        
        
        <span 
          className="theme-preview-thumbnail__status-icon theme-preview-thumbnail__status-icon--notification"
          style={{ background: colors.semantic.info }}
        />
      </div>
    </div>
  );
}

 
