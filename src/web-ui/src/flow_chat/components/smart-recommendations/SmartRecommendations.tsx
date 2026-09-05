/**
 * Smart recommendations component
 */

import React, { useEffect, useState, useCallback } from 'react';
import { Button, IconButton, Tooltip, Icon, type IconName } from '@openbitfun/ui';
import { useTranslation } from 'react-i18next';
import { AlertCircle, ClipboardList, Code, FileText, Lightbulb, TestTube, Wrench, type LucideIcon } from 'lucide-react';
import { recommendationRegistry } from './RecommendationRegistry';
import { RecommendationAction, RecommendationContext } from './types';
import { createLogger } from '@/shared/utils/logger';
import './SmartRecommendations.scss';

const log = createLogger('SmartRecommendations');

const RECOMMENDATION_ICONS = {
  AlertCircle,
  ClipboardList,
  Code,
  FileText,
  Lightbulb,
  TestTube,
  Wrench,
} satisfies Record<string, LucideIcon>;

const RECOMMENDATION_CATALOG: Record<string, IconName> = {
  CheckCircle: 'check-circle',
  MessageSquarePlus: 'side-chat',
  Search: 'search',
};

export interface SmartRecommendationsProps {
  /** Recommendation context */
  context: RecommendationContext;
  /** Optional class name */
  className?: string;
}

export const SmartRecommendations: React.FC<SmartRecommendationsProps> = ({
  context,
  className = ''
}) => {
  const { t } = useTranslation('flow-chat');
  const [actions, setActions] = useState<RecommendationAction[]>([]);
  const [visible, setVisible] = useState(false);
  const [actionLoading, setActionLoading] = useState<Record<string, boolean>>({});

  const loadRecommendations = useCallback(async () => {
    try {
      const providers = recommendationRegistry.getAllProviders();

      const allActions: RecommendationAction[] = [];

      for (const provider of providers) {
        try {
          const shouldShow = await provider.shouldShow(context);

          if (shouldShow) {
            const providerActions = await provider.getActions(context);
            allActions.push(...providerActions);
          }
        } catch (error) {
          log.error('Provider error', { providerId: provider.id, error });
        }
      }

      setActions(allActions);
      setVisible(allActions.length > 0);
    } catch (error) {
      log.error('Failed to load recommendations', error);
      setVisible(false);
    }
  }, [context]);

  useEffect(() => {
    // Delay loading to avoid frequent refresh
    const timer = setTimeout(() => {
      loadRecommendations();
    }, 500);

    return () => clearTimeout(timer);
  }, [loadRecommendations]);

  const handleActionClick = useCallback(async (action: RecommendationAction) => {
    if (action.disabled || actionLoading[action.id]) {
      return;
    }

    setActionLoading(prev => ({ ...prev, [action.id]: true }));
    try {
      await action.onClick();
      // Reload after action completes in case state changed
      setTimeout(() => {
        loadRecommendations();
      }, 1000);
    } catch (error) {
      log.error('Action execution failed', { actionId: action.id, error });
    } finally {
      setActionLoading(prev => ({ ...prev, [action.id]: false }));
    }
  }, [actionLoading, loadRecommendations]);

  const handleClose = useCallback(() => {
    setVisible(false);
  }, []);

  if (!visible || actions.length === 0) {
    return null;
  }

  const hasLoadingAction = actions.some(action => actionLoading[action.id] || action.loading);

  return (
    <div
      data-openbitfun-component="smart-recommendations"
      data-openbitfun-part="root"
      data-openbitfun-state={hasLoadingAction ? 'loading' : undefined}
      className={`openbitfun-smart-recommendations ${className}`}
    >
      <div data-openbitfun-component="smart-recommendations" data-openbitfun-part="header" className="openbitfun-smart-recommendations__header">
        <span data-openbitfun-component="smart-recommendations" data-openbitfun-part="title" className="openbitfun-smart-recommendations__title">{t('smartRecommendations.title')}</span>
        <Tooltip content={t('smartRecommendations.close')}>
          <IconButton
            data-openbitfun-component="smart-recommendations"
            data-openbitfun-part="close"
            className="openbitfun-smart-recommendations__close"
            onClick={handleClose}
            icon={<Icon name="xmark" size="md" />}
            size="sm"
            variant="quiet"
            aria-label={t('smartRecommendations.close')}
          />
        </Tooltip>
      </div>

      <div data-openbitfun-component="smart-recommendations" data-openbitfun-part="actions" className="openbitfun-smart-recommendations__actions">
        {actions.map(action => {
          const CatalogIconName = action.icon ? RECOMMENDATION_CATALOG[action.icon] : undefined;
          const IconComponent = action.icon
            ? RECOMMENDATION_ICONS[action.icon as keyof typeof RECOMMENDATION_ICONS]
            : null;
          
          const isLoading = actionLoading[action.id] || action.loading;
          
          return (
            <Button
              key={action.id}
              variant={action.type === 'primary' ? 'fill' : 'outline'}
              size="sm"
              leadingIcon={CatalogIconName
                ? <Icon name={CatalogIconName} size="md" />
                : IconComponent ? <IconComponent size={16} /> : undefined}
              onClick={() => handleActionClick(action)}
              disabled={action.disabled || isLoading}
              loading={isLoading}
              title={action.description}
            >
              {action.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
};

