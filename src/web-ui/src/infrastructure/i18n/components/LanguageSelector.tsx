 

import React, { useCallback } from 'react';
import { Globe } from 'lucide-react';
import { ActionItem, Button, Select, Tooltip } from '@bitfun/ui';
import { useLanguageSelector } from '../hooks/useI18n';
import type { LocaleId } from '../types';
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
      <div
        className={`language-selector language-selector--icon-only ${className}`}
        data-bf-component="language-selector"
        data-bf-part="root"
        data-bf-mode="icon-only"
        data-bf-state={isChanging ? 'changing' : undefined}
      >
        <span data-bf-component="language-selector" data-bf-part="trigger">
          <Tooltip content={currentLocale?.nativeName || currentLanguage}>
            <Button
              aria-label={currentLocale?.nativeName || currentLanguage}
              className="language-selector__button"
              variant="outline"
              size="sm"
              disabled={isChanging}
              leadingIcon={(
                <span data-bf-component="language-selector" data-bf-part="icon">
                  <Globe size={16} />
                </span>
              )}
            >
              <span
                className="language-selector__code"
                data-bf-component="language-selector"
                data-bf-part="code"
              >
                {currentLanguage.split('-')[0].toUpperCase()}
              </span>
            </Button>
          </Tooltip>
        </span>
        <div
          className="language-selector__dropdown"
          data-bf-component="language-selector"
          data-bf-part="menu"
        >
          {supportedLocales.map(locale => (
            <ActionItem
              key={locale.id}
              className={`language-selector__option ${locale.id === currentLanguage ? 'language-selector__option--active' : ''}`}
              onClick={() => handleChange(locale.id)}
              disabled={isChanging}
              data-bf-component="language-selector"
              data-bf-part="option"
              data-bf-state={locale.id === currentLanguage ? 'active' : undefined}
              metadata={locale.id === currentLanguage ? (
                <span
                  className="language-selector__check"
                  data-bf-component="language-selector"
                  data-bf-part="check"
                >✓</span>
              ) : undefined}
            >
              <span
                className="language-selector__option-name"
                data-bf-component="language-selector"
                data-bf-part="optionLabel"
              >
                {showNativeName ? locale.nativeName : locale.englishName}
              </span>
            </ActionItem>
          ))}
        </div>
      </div>
    );
  }

  if (mode === 'inline') {
    return (
      <div
        className={`language-selector language-selector--inline ${className}`}
        data-bf-component="language-selector"
        data-bf-part="root"
        data-bf-mode="inline"
        data-bf-state={isChanging ? 'changing' : undefined}
      >
        {supportedLocales.map(locale => (
          <Button
            key={locale.id}
            className={`language-selector__inline-button ${locale.id === currentLanguage ? 'language-selector__inline-button--active' : ''}`}
            variant="text"
            size="sm"
            onClick={() => handleChange(locale.id)}
            disabled={isChanging}
            data-bf-component="language-selector"
            data-bf-part="option"
            data-bf-state={locale.id === currentLanguage ? 'active' : undefined}
          >
            {showNativeName ? locale.nativeName : locale.englishName}
          </Button>
        ))}
      </div>
    );
  }

  
  return (
    <div
      className={`language-selector language-selector--dropdown ${className}`}
      data-bf-component="language-selector"
      data-bf-part="root"
      data-bf-mode="dropdown"
      data-bf-state={isChanging ? 'changing' : undefined}
    >
      <Select
        className="language-selector__select"
        value={currentLanguage}
        onValueChange={(value) => handleChange(value as LocaleId)}
        disabled={isChanging}
        options={supportedLocales.map(locale => ({
          value: locale.id,
          label: showNativeName ? locale.nativeName : locale.englishName
        }))}
      />
      {isChanging && (
        <span
          className="language-selector__loading"
          data-bf-component="language-selector"
          data-bf-part="loading"
          data-bf-state="changing"
        >...</span>
      )}
    </div>
  );
};

export default LanguageSelector;
