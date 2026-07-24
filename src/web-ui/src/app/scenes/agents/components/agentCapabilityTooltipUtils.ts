import type React from 'react';

export interface AgentCapabilityTooltipField {
  label: string;
  value: React.ReactNode;
  monospace?: boolean;
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
