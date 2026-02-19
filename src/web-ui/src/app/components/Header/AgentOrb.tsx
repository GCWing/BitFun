import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import './AgentOrb.scss';

interface AgentOrbProps {
  isAgenticMode: boolean;
  onToggle: () => void;
  tooltipText?: string;
}

/**
 * Agent orb component.
 * Used to toggle between Agentic and Editor modes.
 * Uses the product logo icon.
 */
export const AgentOrb: React.FC<AgentOrbProps> = ({ isAgenticMode, onToggle, tooltipText }) => {
  const { t } = useTranslation('common');
  const resolvedTooltipText = tooltipText ?? (isAgenticMode ? t('header.hideAgentic') : t('header.activateAgentic'));
  const [isHovered, setIsHovered] = useState(false);
  
  return (
    <Tooltip content={resolvedTooltipText} placement="bottom">
      <div 
        className={`agent-orb-logo ${isHovered ? 'agent-orb-logo--hover' : ''} ${isAgenticMode ? 'agent-orb-logo--active' : ''}`}
        onClick={onToggle}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        <img 
          src="/Logo-ICON.png" 
          alt="BitFun Logo" 
          className="agent-orb-logo__image"
        />
      </div>
    </Tooltip>
  );
};
