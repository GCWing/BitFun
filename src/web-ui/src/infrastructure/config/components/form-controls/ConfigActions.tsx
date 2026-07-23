import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/component-library';

export interface ConfigActionsProps {
   
  children: React.ReactNode;
   
  align?: 'start' | 'center' | 'end' | 'between';
   
  style?: React.CSSProperties;
   
  className?: string;
}

export const ConfigActions: React.FC<ConfigActionsProps> = ({
  children,
  align = 'end',
  style,
  className = ''
}) => {
  const actionsClass = `config-form-actions ${align} ${className}`.trim();
  
  return (
    <div className={actionsClass} style={style}>
      {children}
    </div>
  );
};

ConfigActions.displayName = 'ConfigActions';


export interface ConfigActionButtonsProps {
   
  showCancel?: boolean;
   
  showReset?: boolean;
   
  showSave?: boolean;
   
  hasChanges?: boolean;
   
  isSaving?: boolean;
   
  cancelText?: string;
   
  resetText?: string;
   
  saveText?: string;
   
  onCancel?: () => void;
   
  onReset?: () => void;
   
  onSave?: () => void;
   
  align?: 'start' | 'center' | 'end' | 'between';
   
  style?: React.CSSProperties;
   
  className?: string;
}

export const ConfigActionButtons: React.FC<ConfigActionButtonsProps> = ({
  showCancel = true,
  showReset = true,
  showSave = true,
  hasChanges = false,
  isSaving = false,
  cancelText,
  resetText,
  saveText,
  onCancel,
  onReset,
  onSave,
  align = 'end',
  style,
  className = ''
}) => {
  const { t } = useTranslation('common');
  const resolvedCancelText = cancelText ?? t('actions.cancel');
  const resolvedResetText = resetText ?? t('actions.reset');
  const resolvedSaveText = saveText ?? t('actions.save');
  const savingText = t('status.saving');
  return (
    <ConfigActions align={align} style={style} className={className}>
      {showCancel && onCancel && (
        <Button
          onClick={onCancel}
          variant="secondary"
          disabled={isSaving}
        >
          {resolvedCancelText}
        </Button>
      )}
      
      {showReset && onReset && (
        <Button
          onClick={onReset}
          variant="secondary"
          disabled={!hasChanges || isSaving}
        >
          {resolvedResetText}
        </Button>
      )}
      
      {showSave && onSave && (
        <Button
          onClick={onSave}
          variant="primary"
          disabled={!hasChanges || isSaving}
          isLoading={isSaving}
        >
          {isSaving ? savingText : resolvedSaveText}
        </Button>
      )}
    </ConfigActions>
  );
};

ConfigActionButtons.displayName = 'ConfigActionButtons';
