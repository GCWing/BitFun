import React from 'react';
import { useTranslation } from 'react-i18next';
import { AcpPermissionActions } from '../tool-cards/AcpPermissionActions';
import { hasAcpPermissionOptions } from '../tool-cards/AcpPermissionActions.utils';
import type { FlowToolItem, ToolRejectOptions } from '../types/flow-chat';
import './ToolApprovalBar.scss';

interface ToolApprovalBarProps {
  toolItem: FlowToolItem;
  onConfirm?: (permissionOptionId?: string, approve?: boolean) => void;
  onReject?: (options?: ToolRejectOptions) => void;
}

export const ToolApprovalBar: React.FC<ToolApprovalBarProps> = ({
  toolItem,
  onConfirm,
  onReject,
}) => {
  const { t } = useTranslation('flow-chat');

  if (toolItem.status !== 'pending_confirmation' || !hasAcpPermissionOptions(toolItem)) {
    return null;
  }

  return (
    <div data-bf-component="tool-approval-bar" data-bf-part="root" className="tool-approval-bar" role="group" aria-label={t('toolCards.approval.ariaLabel')}>
      <div data-bf-component="tool-approval-bar" data-bf-part="main" className="tool-approval-bar__main">
        <span data-bf-component="tool-approval-bar" data-bf-part="message" className="tool-approval-bar__message">{t('toolCards.approval.waiting')}</span>
        <AcpPermissionActions
          toolItem={toolItem}
          presentation="text"
          className="tool-approval-bar__permission-actions"
          onConfirm={onConfirm}
          onReject={onReject}
        />
      </div>
    </div>
  );
};

export default ToolApprovalBar;
