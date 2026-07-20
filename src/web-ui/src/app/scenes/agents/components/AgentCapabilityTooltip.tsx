import React from 'react';
import { Tooltip } from '@/component-library';
import './AgentCapabilityTooltip.scss';

type TooltipPlacement = React.ComponentProps<typeof Tooltip>['placement'];

export interface AgentCapabilityTooltipField {
  label: string;
  value: React.ReactNode;
  monospace?: boolean;
}

interface AgentCapabilityTooltipProps {
  title: string;
  description?: string;
  fields: AgentCapabilityTooltipField[];
  children: React.ReactElement;
  placement?: TooltipPlacement;
  titleMonospace?: boolean;
}

export function capabilityTooltipAriaLabel(
  title: string,
  description: string | undefined,
  fields: AgentCapabilityTooltipField[],
): string {
  const fieldText = fields.flatMap((field) => (
    typeof field.value === 'string' && field.value.trim()
      ? [`${field.label}: ${field.value}`]
      : []
  ));
  return [title, description, ...fieldText].filter(Boolean).join('. ');
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
