import React from 'react';
import { Tooltip } from '@/component-library';
import type { AgentCapabilityTooltipField } from './agentCapabilityTooltipUtils';
import './AgentCapabilityTooltip.scss';

export type { AgentCapabilityTooltipField } from './agentCapabilityTooltipUtils';

type TooltipPlacement = React.ComponentProps<typeof Tooltip>['placement'];

interface AgentCapabilityTooltipProps {
  title: string;
  description?: string;
  fields: AgentCapabilityTooltipField[];
  children: React.ReactElement;
  placement?: TooltipPlacement;
  titleMonospace?: boolean;
}

export const AgentCapabilityTooltip: React.FC<AgentCapabilityTooltipProps> = ({
  title,
  description,
  fields,
  children,
  placement = 'top',
  titleMonospace = false,
}) => {
  const visibleFields = fields.filter((field) => field.value !== null && field.value !== undefined && field.value !== '');

  return (
    <Tooltip
      content={(
        <div className="agent-capability-tooltip__body">
          <div className={`agent-capability-tooltip__title${titleMonospace ? ' is-monospace' : ''}`}>
            {title}
          </div>
          {description ? <div className="agent-capability-tooltip__description">{description}</div> : null}
          {visibleFields.length > 0 ? (
            <dl className="agent-capability-tooltip__fields">
              {visibleFields.map((field) => (
                <div key={field.label} className="agent-capability-tooltip__field">
                  <dt>{field.label}</dt>
                  <dd className={field.monospace ? 'is-monospace' : undefined}>{field.value}</dd>
                </div>
              ))}
            </dl>
          ) : null}
        </div>
      )}
      placement={placement}
      className="agent-capability-tooltip"
      interactive
    >
      {children}
    </Tooltip>
  );
};
