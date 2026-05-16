import React, { useMemo } from 'react';
import { Check, ShieldCheck, ShieldX, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { IconButton } from '../../component-library';
import type { FlowToolItem, ToolCardProps } from '../types/flow-chat';
import type { AcpPermissionOption } from '@/infrastructure/api/service-api/ACPClientAPI';
import './AcpPermissionActions.scss';

const KIND_ORDER: Record<AcpPermissionOption['kind'], number> = {
  allow_once: 0,
  allow_always: 1,
  reject_once: 2,
  reject_always: 3,
};

interface AcpPermissionActionsProps {
  toolItem: FlowToolItem;
  input?: any;
  disabled?: boolean;
  presentation?: 'icon' | 'text';
  className?: string;
  buttonClassName?: string;
  onConfirm?: ToolCardProps['onConfirm'];
  onReject?: ToolCardProps['onReject'];
}

function isApprovalKind(kind: AcpPermissionOption['kind']): boolean {
  return kind === 'allow_once' || kind === 'allow_always';
}

function fallbackLabel(kind: AcpPermissionOption['kind'], t: TFunction<'flow-chat'>): string {
  switch (kind) {
    case 'allow_once':
      return t('toolCards.acpPermission.allowOnce');
    case 'allow_always':
      return t('toolCards.acpPermission.allowAlways');
    case 'reject_once':
      return t('toolCards.acpPermission.reject');
    case 'reject_always':
      return t('toolCards.acpPermission.rejectAlways');
    default:
      return t('toolCards.acpPermission.selectOption');
  }
}

function optionIcon(kind: AcpPermissionOption['kind']): React.ReactNode {
  switch (kind) {
    case 'allow_once':
      return <Check size={12} />;
    case 'allow_always':
      return <ShieldCheck size={12} />;
    case 'reject_always':
      return <ShieldX size={12} />;
    case 'reject_once':
    default:
      return <X size={12} />;
  }
}

function buttonVariant(kind: AcpPermissionOption['kind']): 'success' | 'danger' {
  return isApprovalKind(kind) ? 'success' : 'danger';
}

export const AcpPermissionActions: React.FC<AcpPermissionActionsProps> = ({
  toolItem,
  input,
  disabled = false,
  presentation = 'icon',
  className = '',
  buttonClassName = '',
  onConfirm,
  onReject,
}) => {
  const { t } = useTranslation('flow-chat');
  const options = useMemo(() => {
    return [...(toolItem.acpPermission?.options ?? [])].sort(
      (a, b) => KIND_ORDER[a.kind] - KIND_ORDER[b.kind],
    );
  }, [toolItem.acpPermission?.options]);

  if (options.length === 0) {
    return null;
  }

  return (
    <span className={`acp-permission-actions acp-permission-actions--${presentation} ${className}`}>
      {options.map((option) => {
        const approve = isApprovalKind(option.kind);
        const label = fallbackLabel(option.kind, t);
        const tooltip = option.name && option.name !== label ? `${label}: ${option.name}` : label;
        const handleClick = (event: React.MouseEvent<HTMLButtonElement>) => {
          event.preventDefault();
          event.stopPropagation();

          if (approve) {
            onConfirm?.(input, option.optionId, true);
          } else {
            onReject?.(option.optionId);
          }
        };

        if (presentation === 'text') {
          return (
            <button
              key={option.optionId}
              type="button"
              className={`acp-permission-actions__text-button acp-permission-actions__text-button--${approve ? 'allow' : 'reject'} ${buttonClassName}`}
              onClick={handleClick}
              disabled={disabled}
              title={tooltip}
              aria-label={tooltip}
            >
              {label}
            </button>
          );
        }

        return (
          <IconButton
            key={option.optionId}
            className={`tool-card-header-action acp-permission-actions__icon-button acp-permission-actions__icon-button--${option.kind} ${buttonClassName}`}
            variant={buttonVariant(option.kind)}
            size="xs"
            onClick={handleClick}
            disabled={disabled}
            tooltip={tooltip}
            aria-label={tooltip}
          >
            {optionIcon(option.kind)}
          </IconButton>
        );
      })}
    </span>
  );
};
