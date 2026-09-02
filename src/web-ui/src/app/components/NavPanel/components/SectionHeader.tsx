/** Static section title row with optional actions. */

import React from 'react';

interface SectionHeaderProps {
  label: string;
  actions?: React.ReactNode;
}

const SectionHeader: React.FC<SectionHeaderProps> = ({ label, actions }) => (
  <div className="bitfun-nav-panel__section-header">
    <span className="bitfun-nav-panel__section-label">{label}</span>
    {actions ? (
      <div className="bitfun-nav-panel__section-actions">
        {actions}
      </div>
    ) : null}
  </div>
);

export default React.memo(SectionHeader);
