import React from 'react';
import { useI18n } from '../i18n';

interface LanguageToggleButtonProps {
  className?: string;
}

const LanguageToggleButton: React.FC<LanguageToggleButtonProps> = ({ className }) => {
  const { language, toggleLanguage, t } = useI18n();
  const languageLabel = language === 'zh-CN' ? '中' : language === 'zh-TW' ? '繁' : 'EN';

  return (
    <button
      type="button"
      className={className || 'mobile-lang-btn'}
      onClick={toggleLanguage}
      aria-label={t('common.switchLanguage')}
      title={t('common.switchLanguage')}
    >
      {languageLabel}
    </button>
  );
};

export default LanguageToggleButton;

