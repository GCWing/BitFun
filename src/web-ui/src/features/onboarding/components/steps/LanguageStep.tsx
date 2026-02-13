/**
 * Language selection step
 * LanguageStep - choose UI language
 */

import React from 'react';
import { useTranslation } from 'react-i18next';

interface LanguageStepProps {
  selectedLanguage: string;
  onLanguageChange: (language: string) => void;
}

const LANGUAGE_OPTIONS = [
  { id: 'zh-CN', shortLabelKey: 'language.optionsShort.zh-CN', labelKey: 'language.options.zh-CN' },
  { id: 'en-US', shortLabelKey: 'language.optionsShort.en-US', labelKey: 'language.options.en-US' }
];

export const LanguageStep: React.FC<LanguageStepProps> = ({
  selectedLanguage,
  onLanguageChange
}) => {
  const { t } = useTranslation('onboarding');

  return (
    <div className="bitfun-onboarding-step bitfun-onboarding-language">
      {/* Logo */}
      <div className="bitfun-onboarding-language__logo">
        <img src="/Logo-ICON.png" alt="BitFun Logo" />
      </div>

      {/* Welcome text */}
      <h1 className="bitfun-onboarding-language__welcome">
        BitFun
      </h1>

      {/* Subtitle - both languages on one line with / separator */}
      <div className="bitfun-onboarding-language__subtitle">
        <span>选择界面语言 / Choose Your Language</span>
      </div>

      {/* Language options */}
      <div className="bitfun-onboarding-language__options">
        {LANGUAGE_OPTIONS.map((option) => (
          <div
            key={option.id}
            className={`bitfun-onboarding-language__option ${
              selectedLanguage === option.id ? 'bitfun-onboarding-language__option--selected' : ''
            }`}
            onClick={() => onLanguageChange(option.id)}
          >
            <div className="bitfun-onboarding-language__option-icon">
              {t(option.shortLabelKey)}
            </div>
            <div className="bitfun-onboarding-language__option-label">
              {t(option.labelKey)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default LanguageStep;
