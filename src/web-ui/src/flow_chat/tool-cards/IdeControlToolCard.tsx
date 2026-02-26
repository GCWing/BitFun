/**
 * IDE control tool display component
 * Displays IDE control operation execution status with concise gray text prompts
 * Supports expanding to view detailed configuration information
 */

import React, { useState, useMemo } from 'react';
import { Loader2, CheckCircle, Settings, Clock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import './IdeControlToolCard.scss';

export const IdeControlToolCard: React.FC<ToolCardProps> = ({
  toolItem
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const toolInput = toolCall?.input;
  const [isExpanded, setIsExpanded] = useState(false);

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={12} />;
      case 'completed':
        return <CheckCircle size={12} />;
      case 'pending':
      default:
        return <Clock size={12} />;
    }
  };

  const getOperationDescription = () => {
    if (!toolInput) return t('toolCards.ideControl.parsingOperation');
    
    if (Object.keys(toolInput).length === 0) return t('toolCards.ideControl.parsingOperation');

    try {
      const input = typeof toolInput === 'string' ? JSON.parse(toolInput) : toolInput;
      const action = input.action || 'unknown';
      const panelType = input.target?.panel_type || '';

      const panelNames: Record<string, string> = {
        'git-settings': 'Git Settings',
        'git-diff': 'Git Diff',
        'planner': t('toolCards.ideControl.planner'),
        'terminal': t('toolCards.ideControl.terminal'),
        'file-viewer': t('toolCards.ideControl.fileViewer'),
        'code-editor': t('toolCards.ideControl.codeEditor'),
        'markdown-editor': 'Markdown Editor',
      };

      const panelName = panelNames[panelType] || panelType;

      switch (action) {
        case 'open_panel':
          return `Open ${panelName}`;
        case 'close_panel':
          return `Close ${panelName}`;
        case 'toggle_panel':
          return `Toggle ${panelName}`;
        case 'navigate_to':
          return t('toolCards.ideControl.navigateToPosition');
        case 'set_layout':
          return t('toolCards.ideControl.adjustIdeLayout');
        case 'manage_tab':
          return t('toolCards.ideControl.manageTab');
        case 'focus_view':
          return t('toolCards.ideControl.focusView');
        default:
          return `${action}`;
      }
    } catch {
      return t('toolCards.ideControl.executeIdeOperation');
    }
  };

  const operationDesc = useMemo(() => getOperationDescription(), [toolInput]);

  const getDetailInfo = useMemo(() => {
    if (!toolInput) return null;
    
    try {
      const input = typeof toolInput === 'string' ? JSON.parse(toolInput) : toolInput;
      const details: Array<{ label: string; value: string }> = [];
      
      if (input.action) {
        details.push({ label: t('toolCards.ideControl.operationType'), value: input.action });
      }
      
      if (input.target?.panel_type) {
        details.push({ label: t('toolCards.ideControl.panelType'), value: input.target.panel_type });
      }
      
      if (input.target?.panel_config) {
        const config = input.target.panel_config;
        if (config.section) {
          details.push({ label: t('toolCards.ideControl.configSection'), value: config.section });
        }
        if (config.session_id) {
          details.push({ label: t('toolCards.ideControl.sessionId'), value: config.session_id });
        }
        if (config.file_path) {
          details.push({ label: t('toolCards.ideControl.filePath'), value: config.file_path });
        }
        if (config.workspace_path) {
          details.push({ label: t('toolCards.ideControl.workspacePath'), value: config.workspace_path });
        }
      }
      
      if (input.position) {
        details.push({ label: t('toolCards.ideControl.panelPosition'), value: input.position });
      }
      
      return details.length > 0 ? details : null;
    } catch {
      return null;
    }
  }, [toolInput]);

  const detailInfo = getDetailInfo;
  const hasDetails = detailInfo && detailInfo.length > 0;

  const handleCardClick = () => {
    if (hasDetails) {
      setIsExpanded(!isExpanded);
    }
  };

  const renderContent = () => {
    if (status === 'completed') {
      return <>Executed: {operationDesc}</>;
    }
    if (status === 'running' || status === 'streaming') {
      return <>Executing {operationDesc}...</>;
    }
    if (status === 'pending') {
      return <>{operationDesc}</>;
    }
    return null;
  };

  const expandedContent = useMemo(() => {
    if (!detailInfo) return null;
    
    return (
      <div className="ide-control-details">
        {detailInfo.map((item, index) => (
          <div key={index} className="detail-item">
            <span className="detail-label">{item.label}:</span>
            <span className="detail-value">{item.value}</span>
          </div>
        ))}
      </div>
    );
  }, [detailInfo]);

  // Important: do not conditionally skip hooks (e.g. useMemo) across renders.
  // Returning early here is safe because all hooks above have already run.
  if ((status as string) === 'error') {
    return null;
  }

  return (
    <CompactToolCard
      status={status}
      isExpanded={isExpanded}
      onClick={handleCardClick}
      className="ide-control-card"
      clickable={hasDetails}
      header={
        <CompactToolCardHeader
          statusIcon={getStatusIcon()}
          content={renderContent()}
        />
      }
      expandedContent={expandedContent}
    />
  );
};

