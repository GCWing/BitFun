import React from 'react';
import { useI18n } from '../i18n';
import { getMobileLanguageShortName } from '../i18n/localeRegistry';

interface LanguageToggleButtonProps {
  className?: string;
}

const LanguageToggleButton: React.FC<LanguageToggleButtonProps> = ({ className }) => {
  const { language, toggleLanguage, t } = useI18n();
  const buttonClassName = ['mobile-lang-btn', className].filter(Boolean).join(' ');

  return (
    <button
      type="button"
      className={buttonClassName}
      onClick={toggleLanguage}
      aria-label={t('common.switchLanguage')}
      title={t('common.switchLanguage')}
    >
      {getMobileLanguageShortName(language)}
    </button>
  );
};

export default LanguageToggleButton;
