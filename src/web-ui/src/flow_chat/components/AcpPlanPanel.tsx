import React from 'react';
import { useTranslation } from 'react-i18next';
import { Check, CircleDashed, LoaderCircle } from 'lucide-react';

import type { AcpPlanEntry } from '@/infrastructure/api/service-api/ACPClientAPI';
import './AcpPlanPanel.scss';

export interface AcpPlanPanelProps {
  entries: AcpPlanEntry[];
}

function statusIcon(status: string): React.ReactNode {
  switch (status) {
    case 'completed':
      return <Check size={13} className="bitfun-acp-plan__icon bitfun-acp-plan__icon--done" />;
    case 'in_progress':
      return (
        <LoaderCircle
          size={13}
          className="bitfun-acp-plan__icon bitfun-acp-plan__icon--active"
        />
      );
    default:
      return (
        <CircleDashed size={13} className="bitfun-acp-plan__icon bitfun-acp-plan__icon--pending" />
      );
  }
}

export const AcpPlanPanel: React.FC<AcpPlanPanelProps> = ({ entries }) => {
  const { t } = useTranslation('flow-chat');
  if (entries.length === 0) return null;

  const done = entries.filter((entry) => entry.status === 'completed').length;

  return (
    <div data-bf-component="acp-plan-panel" data-bf-part="root" className="bitfun-acp-plan" data-testid="acp-plan-panel">
      <div data-bf-component="acp-plan-panel" data-bf-part="header" className="bitfun-acp-plan__header">
        <span data-bf-component="acp-plan-panel" data-bf-part="title" className="bitfun-acp-plan__title">{t('chatInput.acpPlan.title')}</span>
        <span data-bf-component="acp-plan-panel" data-bf-part="progress" className="bitfun-acp-plan__progress">
          {done}/{entries.length}
        </span>
      </div>
      <ul data-bf-component="acp-plan-panel" data-bf-part="list" className="bitfun-acp-plan__list">
        {entries.map((entry, index) => (
          <li
            key={`${index}-${entry.content}`}
            data-bf-component="acp-plan-panel"
            data-bf-part="item"
            data-bf-status={entry.status}
            className={`bitfun-acp-plan__item bitfun-acp-plan__item--${entry.status}`}
          >
            {statusIcon(entry.status)}
            <span data-bf-component="acp-plan-panel" data-bf-part="content" className="bitfun-acp-plan__content">{entry.content}</span>
          </li>
        ))}
      </ul>
    </div>
  );
};

AcpPlanPanel.displayName = 'AcpPlanPanel';
