 

import React, { useCallback } from 'react';

import { ActionItem, Button, Select, Tooltip, Icon } from '@openbitfun/ui';
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
        data-openbitfun-component="language-selector"
        data-openbitfun-part="root"
        data-openbitfun-mode="icon-only"
        data-openbitfun-state={isChanging ? 'changing' : undefined}
      >
        <span data-openbitfun-component="language-selector" data-openbitfun-part="trigger">
          <Tooltip content={currentLocale?.nativeName || currentLanguage}>
            <Button
              aria-label={currentLocale?.nativeName || currentLanguage}
              className="language-selector__button"
              variant="outline"
              size="sm"
              disabled={isChanging}
              leadingIcon={(
                <span data-openbitfun-component="language-selector" data-openbitfun-part="icon">
                  <Icon name="browser" size="md" />
                </span>
              )}
            >
              <span
                className="language-selector__code"
                data-openbitfun-component="language-selector"
                data-openbitfun-part="code"
              >
                {currentLanguage.split('-')[0].toUpperCase()}
              </span>
            </Button>
          </Tooltip>
        </span>
        <div
          className="language-selector__dropdown"
          data-openbitfun-component="language-selector"
          data-openbitfun-part="menu"
        >
          {supportedLocales.map(locale => (
            <ActionItem
              key={locale.id}
              className={`language-selector__option ${locale.id === currentLanguage ? 'language-selector__option--active' : ''}`}
              onClick={() => handleChange(locale.id)}
              disabled={isChanging}
              data-openbitfun-component="language-selector"
              data-openbitfun-part="option"
              data-openbitfun-state={locale.id === currentLanguage ? 'active' : undefined}
              metadata={locale.id === currentLanguage ? (
                <span
                  className="language-selector__check"
                  data-openbitfun-component="language-selector"
                  data-openbitfun-part="check"
                >✓</span>
              ) : undefined}
            >
              <span
                className="language-selector__option-name"
                data-openbitfun-component="language-selector"
                data-openbitfun-part="optionLabel"
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
        data-openbitfun-component="language-selector"
        data-openbitfun-part="root"
        data-openbitfun-mode="inline"
        data-openbitfun-state={isChanging ? 'changing' : undefined}
      >
        {supportedLocales.map(locale => (
          <Button
            key={locale.id}
            className={`language-selector__inline-button ${locale.id === currentLanguage ? 'language-selector__inline-button--active' : ''}`}
            variant="text"
            size="sm"
            onClick={() => handleChange(locale.id)}
            disabled={isChanging}
            data-openbitfun-component="language-selector"
            data-openbitfun-part="option"
            data-openbitfun-state={locale.id === currentLanguage ? 'active' : undefined}
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
      data-openbitfun-component="language-selector"
      data-openbitfun-part="root"
      data-openbitfun-mode="dropdown"
      data-openbitfun-state={isChanging ? 'changing' : undefined}
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
          data-openbitfun-component="language-selector"
          data-openbitfun-part="loading"
          data-openbitfun-state="changing"
        >...</span>
      )}
    </div>
  );
};

export default LanguageSelector;
