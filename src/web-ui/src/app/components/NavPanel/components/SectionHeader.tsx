/**
 * SectionHeader — collapsible (or static) section title row in NavPanel.
 */

import React from 'react';

interface SectionHeaderProps {
  label: string;
  collapsible: boolean;
  isOpen: boolean;
  onToggle?: () => void;
}

const SectionHeader: React.FC<SectionHeaderProps> = ({
  label,
  collapsible,
  isOpen,
  onToggle,
}) => (
  <div
    className={[
      'bitfun-nav-panel__section-header',
      collapsible && 'bitfun-nav-panel__section-header--collapsible',
    ]
      .filter(Boolean)
      .join(' ')}
    onClick={collapsible ? onToggle : undefined}
    role={collapsible ? 'button' : undefined}
    tabIndex={collapsible ? 0 : undefined}
    onKeyDown={
      collapsible
        ? e => {
            if (e.key === 'Enter' || e.key === ' ') onToggle?.();
          }
        : undefined
    }
  >
    <span className="bitfun-nav-panel__section-label">{label}</span>
  </div>
);

export default SectionHeader;
