import React from 'react';
import { useI18n } from '../i18n';
import { getMobileLanguageShortName } from '../i18n/localeRegistry';

interface LanguageToggleButtonProps {
  className?: string;
}

const LanguageToggleButton: React.FC<LanguageToggleButtonProps> = ({ className }) => {
  const { language, toggleLanguage, t } = useI18n();

  return (
    <button
      type="button"
      className={className || 'mobile-lang-btn'}
      onClick={toggleLanguage}
      aria-label={t('common.switchLanguage')}
      title={t('common.switchLanguage')}
    >
      {getMobileLanguageShortName(language)}
    </button>
  );
};

export default LanguageToggleButton;

