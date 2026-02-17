/**
 * LSP plugin list UI.
 */

import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Package, Trash2, CheckCircle, AlertCircle, ChevronDown, ChevronRight, RefreshCw, Upload } from 'lucide-react';
import { Button, IconButton, Card, CardBody } from '@/component-library';
import { useLspPlugins } from '../../hooks/useLsp';
import type { LspPlugin } from '../../types';
import { useNotification } from '@/shared/notification-system';
import './LspPluginList.scss';

export interface LspPluginListProps {
  className?: string;
  onInitialize?: () => void;
  onInstallPlugin?: () => void;
  isInitializing?: boolean;
  isInstalling?: boolean;
}

export const LspPluginList: React.FC<LspPluginListProps> = ({ 
  className,
  onInitialize,
  onInstallPlugin,
  isInitializing = false,
  isInstalling = false
}) => {
  const { t } = useTranslation('settings/lsp');
  const { plugins, loading, error, reload, uninstallPlugin } = useLspPlugins();
  const [expandedPlugins, setExpandedPlugins] = useState<Set<string>>(new Set());
  const notification = useNotification();

  const togglePlugin = (pluginId: string) => {
    setExpandedPlugins(prev => {
      const next = new Set(prev);
      if (next.has(pluginId)) {
        next.delete(pluginId);
      } else {
        next.add(pluginId);
      }
      return next;
    });
  };

  const handleUninstall = async (pluginId: string) => {
    if (!confirm(t('pluginList.confirmUninstall', { pluginId }))) {
      return;
    }

    const success = await uninstallPlugin(pluginId);
    if (success) {
      notification.success(t('pluginList.uninstallSuccess'));
    } else {
      notification.error(t('pluginList.uninstallFailed'));
    }
  };

  const renderHeader = () => (
    <div className="lsp-plugin-list__header">
      <h3>{t('pluginList.header', { count: plugins.length })}</h3>
      <div className="lsp-plugin-list__header-actions">
        {onInitialize && (
          <IconButton
            variant="ghost"
            size="medium"
            onClick={onInitialize}
            disabled={isInitializing}
            tooltip={t('pluginList.initTooltip')}
            tooltipPlacement="top"
          >
            <RefreshCw size={18} className={isInitializing ? 'lsp-plugin-list__spinning' : ''} />
          </IconButton>
        )}
        {onInstallPlugin && (
          <IconButton
            variant="ghost"
            size="medium"
            onClick={onInstallPlugin}
            disabled={isInstalling}
            tooltip={t('pluginList.installTooltip')}
            tooltipPlacement="top"
          >
            <Upload size={18} />
          </IconButton>
        )}
        <IconButton
          variant="ghost"
          size="medium"
          onClick={reload}
          tooltip={t('pluginList.refreshTooltip')}
          tooltipPlacement="top"
        >
          <RefreshCw size={18} />
        </IconButton>
      </div>
    </div>
  );

  if (loading) {
    return (
      <div className={`lsp-plugin-list ${className || ''}`}>
        {renderHeader()}
        <div className="lsp-plugin-list__loading">
          <div className="spinner"></div>
          <p>{t('pluginList.loading')}</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`lsp-plugin-list ${className || ''}`}>
        {renderHeader()}
        <div className="lsp-plugin-list__error">
          <AlertCircle size={32} />
          <p>{error}</p>
          <Button variant="secondary" size="small" onClick={reload}>
            {t('pluginList.retry')}
          </Button>
        </div>
      </div>
    );
  }

  if (plugins.length === 0) {
    return (
      <div className={`lsp-plugin-list ${className || ''}`}>
        {renderHeader()}
        <div className="lsp-plugin-list__empty">
          <Package size={64} />
          {onInstallPlugin && (
            <Button
              variant="dashed"
              size="medium"
              onClick={onInstallPlugin}
              disabled={isInstalling}
            >
              <Upload size={16} />
              {isInstalling ? t('pluginList.installing') : t('pluginList.installButton')}
            </Button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className={`lsp-plugin-list ${className || ''}`}>
      {renderHeader()}

      <div className="lsp-plugin-list__items">
        {plugins.map(plugin => (
          <PluginItem
            key={plugin.id}
            plugin={plugin}
            isExpanded={expandedPlugins.has(plugin.id)}
            onToggle={() => togglePlugin(plugin.id)}
            onUninstall={() => handleUninstall(plugin.id)}
            t={t}
          />
        ))}
      </div>
    </div>
  );
};

interface PluginItemProps {
  plugin: LspPlugin;
  isExpanded: boolean;
  onToggle: () => void;
  onUninstall: () => void;
  t: (key: string) => string;
}

const PluginItem: React.FC<PluginItemProps> = ({ plugin, isExpanded, onToggle, onUninstall, t }) => {
  return (
    <Card variant="default" padding="none" className={`lsp-plugin-item ${isExpanded ? 'is-expanded' : ''}`}>
      <div className="lsp-plugin-item__header" onClick={onToggle}>
        <div className="lsp-plugin-item__toggle">
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </div>
        
        <div className="lsp-plugin-item__icon">
          <Package size={16} />
        </div>
        
        <div className="lsp-plugin-item__main">
          <span className="lsp-plugin-item__name">{plugin.name}</span>
          <span className="lsp-plugin-item__version">v{plugin.version}</span>
        </div>

        <div className="lsp-plugin-item__badges">
          {plugin.languages.slice(0, 2).map(lang => (
            <span key={lang} className="lsp-plugin-item__badge">{lang}</span>
          ))}
          {plugin.languages.length > 2 && (
            <span className="lsp-plugin-item__badge-more">+{plugin.languages.length - 2}</span>
          )}
        </div>
      </div>

      {isExpanded && (
        <CardBody className="lsp-plugin-item__details">
          <div className="lsp-plugin-item__section">
            <p className="lsp-plugin-item__description">{plugin.description}</p>
          </div>

          <div className="lsp-plugin-item__section">
            <div className="lsp-plugin-item__label">{t('pluginList.details.author')}</div>
            <div className="lsp-plugin-item__value">{plugin.author}</div>
          </div>

          <div className="lsp-plugin-item__section">
            <div className="lsp-plugin-item__label">{t('pluginList.details.languages')}</div>
            <div className="lsp-plugin-item__tags">
              {plugin.languages.map(lang => (
                <span key={lang} className="lsp-plugin-item__tag">{lang}</span>
              ))}
            </div>
          </div>

          <div className="lsp-plugin-item__section">
            <div className="lsp-plugin-item__label">{t('pluginList.details.capabilities')}</div>
            <div className="lsp-plugin-item__capabilities">
              {plugin.capabilities.completion && (
                <span className="lsp-plugin-item__capability">
                  <CheckCircle size={12} />
                  {t('pluginList.details.completion')}
                </span>
              )}
              {plugin.capabilities.hover && (
                <span className="lsp-plugin-item__capability">
                  <CheckCircle size={12} />
                  {t('pluginList.details.hover')}
                </span>
              )}
              {plugin.capabilities.definition && (
                <span className="lsp-plugin-item__capability">
                  <CheckCircle size={12} />
                  {t('pluginList.details.definition')}
                </span>
              )}
              {plugin.capabilities.references && (
                <span className="lsp-plugin-item__capability">
                  <CheckCircle size={12} />
                  {t('pluginList.details.references')}
                </span>
              )}
              {plugin.capabilities.formatting && (
                <span className="lsp-plugin-item__capability">
                  <CheckCircle size={12} />
                  {t('pluginList.details.formatting')}
                </span>
              )}
            </div>
          </div>

          <div className="lsp-plugin-item__actions">
            <Button
              variant="danger"
              size="small"
              onClick={(e) => {
                e.stopPropagation();
                onUninstall();
              }}
            >
              <Trash2 size={14} />
              {t('pluginList.uninstall')}
            </Button>
          </div>
        </CardBody>
      )}
    </Card>
  );
};


