import React from 'react';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';

interface PrivacyStatementLinkProps {
  disabled?: boolean;
  onClick: () => void;
}

export const PrivacyStatementLink: React.FC<PrivacyStatementLinkProps> = ({
  disabled = false,
  onClick,
}) => {
  const { t } = useI18n('common');

  return (
    <button
      type="button"
      className="bitfun-feedback__privacy-link"
      aria-haspopup="dialog"
      disabled={disabled}
      onClick={onClick}
    >
      {t('feedback.privacyStatement')}
    </button>
  );
};
