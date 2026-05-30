import React from 'react';
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

/**
 * Renders an ACP agent's execution plan as a live task checklist. Presentational
 * only — fed by {@link useAcpPlan}. Renders nothing when there are no entries.
 */
export const AcpPlanPanel: React.FC<AcpPlanPanelProps> = ({ entries }) => {
  if (entries.length === 0) return null;

  const done = entries.filter((entry) => entry.status === 'completed').length;

  return (
    <div className="bitfun-acp-plan" data-testid="acp-plan-panel">
      <div className="bitfun-acp-plan__header">
        <span className="bitfun-acp-plan__title">Plan</span>
        <span className="bitfun-acp-plan__progress">
          {done}/{entries.length}
        </span>
      </div>
      <ul className="bitfun-acp-plan__list">
        {entries.map((entry, index) => (
          <li
            key={`${index}-${entry.content}`}
            className={`bitfun-acp-plan__item bitfun-acp-plan__item--${entry.status}`}
          >
            {statusIcon(entry.status)}
            <span className="bitfun-acp-plan__content">{entry.content}</span>
          </li>
        ))}
      </ul>
    </div>
  );
};

AcpPlanPanel.displayName = 'AcpPlanPanel';
export default AcpPlanPanel;
