/**
 * Reproduction steps block component
 *
 * Used to display reproduction steps in the visual debugging flow
 */

import React, { useState, useCallback, useMemo } from 'react';
import { RefreshCw, Play, CheckCircle, ChevronDown, ChevronRight } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './ReproductionStepsBlock.scss';

const log = createLogger('ReproductionStepsBlock');

// Module-level state to avoid losing progress on remount
const proceededStepsSet = new Set<string>();

function getStepsKey(steps: string): string {
  return steps.trim().slice(0, 100);
}

export interface ReproductionStepsBlockProps {
  steps: string;
  onProceed?: () => void;
}

export const ReproductionStepsBlock: React.FC<ReproductionStepsBlockProps> = ({
  steps,
  onProceed
}) => {
  const { t } = useI18n('components');
  const stepsKey = useMemo(() => getStepsKey(steps), [steps]);
  const [isProceeding, setIsProceeding] = useState(false);
  const [hasProceeded, setHasProceeded] = useState(() => proceededStepsSet.has(stepsKey));
  const [isExpanded, setIsExpanded] = useState(() => !proceededStepsSet.has(stepsKey));
  
  const stepList = React.useMemo(() => {
    const lines = steps.split('\n').filter(line => line.trim());
    return lines.map(line => {
      const cleaned = line.replace(/^[\d.*)\s-]+/, '').trim();
      return cleaned || line.trim();
    });
  }, [steps]);
  
  const handleProceed = useCallback(async () => {
    setIsProceeding(true);
    
    try {
      const { FlowChatManager } = await import('../../../flow_chat/services/FlowChatManager');
      const flowChatManager = FlowChatManager.getInstance();
      
      // Log collection note: read .bitfun/debug.log
      await flowChatManager.sendMessage(
        t('reproductionSteps.userCompleted'),
        undefined,
        t('reproductionSteps.userCompletedShort')
      );
      
      log.info('Proceed instruction sent');
      
      onProceed?.();
      
      proceededStepsSet.add(stepsKey);
      setHasProceeded(true);
      setIsExpanded(false);
      
    } catch (error) {
      log.error('Failed to proceed', error);
      notificationService.error(t('reproductionSteps.operationFailed'));
    } finally {
      setIsProceeding(false);
    }
  }, [onProceed, stepsKey, t]);
  
  const toggleExpand = useCallback(() => {
    if (hasProceeded) {
      setIsExpanded(prev => !prev);
    }
  }, [hasProceeded]);

  const showContent = hasProceeded ? isExpanded : true;

  return (
    <div className={`reproduction-steps-block ${hasProceeded ? 'proceeded' : ''} ${showContent ? 'expanded' : 'collapsed'}`}>
      <div 
        className={`reproduction-steps-header ${hasProceeded ? 'clickable' : ''}`}
        onClick={hasProceeded ? toggleExpand : undefined}
        role={hasProceeded ? 'button' : undefined}
        tabIndex={hasProceeded ? 0 : undefined}
        onKeyDown={hasProceeded ? (e) => e.key === 'Enter' && toggleExpand() : undefined}
      >
        {hasProceeded && (
          <div className="reproduction-steps-toggle">
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          </div>
        )}
        <div className="reproduction-steps-icon">
          {hasProceeded ? (
            <CheckCircle size={16} />
          ) : (
            <RefreshCw size={16} />
          )}
        </div>
        <div className="reproduction-steps-title">
          {hasProceeded ? t('reproductionSteps.completedTitle') : t('reproductionSteps.instructionTitle')}
        </div>
      </div>
      
      {showContent && (
        <>
          <div className="reproduction-steps-content">
            <ol className="reproduction-steps-list">
              {stepList.map((step, index) => (
                <li key={index} className="reproduction-step-item">
                  {step}
                </li>
              ))}
            </ol>
          </div>
          
          {!hasProceeded && (
            <div className="reproduction-steps-actions">
              <div className="reproduction-steps-hint">
                {t('reproductionSteps.instruction')}
              </div>
              <button
                className="reproduction-proceed-btn"
                onClick={handleProceed}
                disabled={isProceeding}
              >
                {isProceeding ? (
                  <>
                    <RefreshCw size={16} className="spin" />
                    <span>{t('reproductionSteps.processing')}</span>
                  </>
                ) : (
                  <>
                    <Play size={16} />
                    <span>{t('reproductionSteps.continueButton')}</span>
                  </>
                )}
              </button>
            </div>
          )}
          
          {hasProceeded && (
            <div className="reproduction-steps-completed">
              <CheckCircle size={16} />
              <span>{t('reproductionSteps.notified')}</span>
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default ReproductionStepsBlock;
