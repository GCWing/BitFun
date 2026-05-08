 

import React, { useCallback } from 'react';
import { Globe } from 'lucide-react';
import { useLanguageSelector } from '../hooks/useI18n';
import type { LocaleId } from '../types';
import { IconButton } from '@components/IconButton/IconButton';
import { Select } from '@components/Select/Select';
import './LanguageSelector.scss';

export interface LanguageSelectorProps {
   
  mode?: 'dropdown' | 'inline' | 'icon-only';
   
  className?: string;
   
  showNativeName?: boolean;
   
  onChange?: (locale: LocaleId) => void;
}

 
export const LanguageSelector: React.FC<LanguageSelectorProps> = ({
  mode = 'dropdown',
  className = '',
  showNativeName = true,
  onChange,
}) => {
  const { currentLanguage, supportedLocales, selectLanguage, isChanging } = useLanguageSelector();

  const handleChange = useCallback(async (locale: LocaleId) => {
    await selectLanguage(locale);
    onChange?.(locale);
  }, [selectLanguage, onChange]);

  const currentLocale = supportedLocales.find(l => l.id === currentLanguage);

  if (mode === 'icon-only') {
    return (
      <div className={`language-selector language-selector--icon-only ${className}`}>
        <IconButton
          className="language-selector__button"
          variant="ghost"
          size="small"
          disabled={isChanging}
          tooltip={currentLocale?.nativeName || currentLanguage}
        >
          <span className="language-selector__icon">
            <Globe size={16} />
          </span>
          <span className="language-selector__code">{currentLanguage.split('-')[0].toUpperCase()}</span>
        </IconButton>
        <div className="language-selector__dropdown">
          {supportedLocales.map(locale => (
            <IconButton
              key={locale.id}
              className={`language-selector__option ${locale.id === currentLanguage ? 'language-selector__option--active' : ''}`}
              variant="ghost"
              size="small"
              onClick={() => handleChange(locale.id)}
              disabled={isChanging}
            >
              <span className="language-selector__option-name">
                {showNativeName ? locale.nativeName : locale.englishName}
              </span>
              {locale.id === currentLanguage && (
                <span className="language-selector__check">✓</span>
              )}
            </IconButton>
          ))}
        </div>
      </div>
    );
  }

  if (mode === 'inline') {
    return (
      <div className={`language-selector language-selector--inline ${className}`}>
        {supportedLocales.map(locale => (
          <IconButton
            key={locale.id}
            className={`language-selector__inline-button ${locale.id === currentLanguage ? 'language-selector__inline-button--active' : ''}`}
            variant="ghost"
            size="small"
            onClick={() => handleChange(locale.id)}
            disabled={isChanging}
          >
            {showNativeName ? locale.nativeName : locale.englishName}
          </IconButton>
        ))}
      </div>
    );
  }

  
  return (
    <div className={`language-selector language-selector--dropdown ${className}`}>
      <Select
        className="language-selector__select"
        value={currentLanguage}
        onChange={(value) => handleChange(value as LocaleId)}
        disabled={isChanging}
        options={supportedLocales.map(locale => ({
          value: locale.id,
          label: showNativeName ? locale.nativeName : locale.englishName
        }))}
      />
      {isChanging && <span className="language-selector__loading">...</span>}
    </div>
  );
};

export default LanguageSelector;
