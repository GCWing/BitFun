import React from 'react';

import type { AgentCapabilityTooltipField } from './agentCapabilityTooltipUtils';
import './AgentCapabilityTooltip.scss';
import { Tooltip } from '@bitfun/ui';

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
        <div
          className="agent-capability-tooltip__body"
          data-bf-component="agent-capability-tooltip"
          data-bf-part="body"
        >
          <div data-bf-component="agent-capability-tooltip" data-bf-part="title" className={`agent-capability-tooltip__title${titleMonospace ? ' is-monospace' : ''}`}>
            {title}
          </div>
          {description ? <div className="agent-capability-tooltip__description" data-bf-component="agent-capability-tooltip" data-bf-part="description">{description}</div> : null}
          {visibleFields.length > 0 ? (
            <dl className="agent-capability-tooltip__fields" data-bf-component="agent-capability-tooltip" data-bf-part="fields">
              {visibleFields.map((field) => (
                <div key={field.label} className="agent-capability-tooltip__field" data-bf-component="agent-capability-tooltip" data-bf-part="field">
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
