/**
 * PluginsConfig — Plugin management settings page.
 *
 * Shows a global enable/disable toggle, and lists discovered plugins grouped
 * by scope (user vs workspace). Workspace-scoped plugins are only shown when
 * a workspace is open, using the same pattern as SkillsConfig.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  RefreshCw,
  Package,
  CheckCircle,
  AlertTriangle,
  Ban,
} from 'lucide-react';
import { Button, Switch } from '@/component-library';
import {
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageContent,
  ConfigPageSection,
  ConfigPageRow,
} from './common';
import { useNotification } from '@/shared/notification-system';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { pluginAPI, type PluginStatusView, type PluginStatusResponse } from '../../api/service-api/PluginAPI';

interface ScopeGroup {
  scope: string;
  label: string;
  plugins: PluginStatusView[];
}

export const PluginsConfig: React.FC = () => {
  const { t } = useTranslation('settings/plugins');
  const notification = useNotification();
  const { workspacePath, hasWorkspace } = useCurrentWorkspace();

  const [status, setStatus] = useState<PluginStatusResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [toggling, setToggling] = useState<Set<string>>(new Set());

  const loadStatus = useCallback(async () => {
    try {
      setLoading(true);
      const result = await pluginAPI.getPluginStatus(workspacePath || undefined);
      setStatus(result);
    } catch (err) {
      notification.error(t('errorLoading'));
    } finally {
      setLoading(false);
    }
  }, [t, notification, workspacePath]);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const handleGlobalToggle = async (enabled: boolean) => {
    try {
      await pluginAPI.setPluginsEnabled(enabled);
      const refreshed = await pluginAPI.getPluginStatus(workspacePath || undefined);
      setStatus(refreshed);
    } catch (err) {
      notification.error(t('errorToggleGlobal'));
    }
  };

  const handlePluginToggle = async (plugin: PluginStatusView, trusted: boolean) => {
    setToggling((prev) => new Set(prev).add(plugin.pluginId));
    try {
      await pluginAPI.setPluginTrust(plugin.pluginId, trusted);
      const result = await pluginAPI.getPluginStatus(workspacePath || undefined);
      setStatus(result);
    } catch (err) {
      notification.error(t('errorTogglePlugin'));
    } finally {
      setToggling((prev) => {
        const next = new Set(prev);
        next.delete(plugin.pluginId);
        return next;
      });
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      const result = await pluginAPI.refreshPlugins(workspacePath || undefined);
      setStatus(result);
    } catch (err) {
      notification.error(t('errorRefresh'));
    } finally {
      setRefreshing(false);
    }
  };

  const scopeGroups = useMemo((): ScopeGroup[] => {
    const plugins = status?.plugins ?? [];
    const userPlugins = plugins.filter((p) => p.scope === 'user');
    const wsPlugins = plugins.filter((p) => p.scope === 'workspace');

    const groups: ScopeGroup[] = [
      { scope: 'user', label: t('userPlugins'), plugins: userPlugins },
    ];
    if (hasWorkspace && wsPlugins.length > 0) {
      groups.push({ scope: 'workspace', label: t('workspacePlugins'), plugins: wsPlugins });
    }
    return groups;
  }, [status?.plugins, hasWorkspace, t]);

  if (loading) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <div className="plugins-config__loading">{t('loading')}</div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const pluginsEnabled = status?.pluginsEnabled ?? true;
  const plugins = status?.plugins ?? [];
  const enabledCount = plugins.filter((p) => p.enabled && p.trustLevel !== 'Denied').length;

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={
          <Button variant="ghost" size="small" onClick={handleRefresh} isLoading={refreshing}>
            <RefreshCw size={16} /> {t('refresh')}
          </Button>
        }
      />

      <ConfigPageContent>
        {/* Global enable/disable */}
        <ConfigPageSection title={t('globalSection')}>
          <ConfigPageRow
            label={t('pluginsEnabled')}
            description={
              pluginsEnabled
                ? t('pluginsEnabledDesc', { count: enabledCount })
                : t('pluginsDisabledDesc')
            }
            align="center"
          >
            <Switch
              checked={pluginsEnabled}
              onChange={(e) => handleGlobalToggle(e.target.checked)}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        {/* Plugin list grouped by scope */}
        {pluginsEnabled &&
          scopeGroups.map((group) => (
            <ConfigPageSection
              key={group.scope}
              title={group.label}
              description={
                group.scope === 'workspace' && workspacePath
                  ? workspacePath
                  : undefined
              }
            >
              {group.plugins.length === 0 ? (
                <ConfigPageRow label={t('noPluginsFound')} align="center">
                  <div />
                </ConfigPageRow>
              ) : (
                group.plugins.map((plugin) => (
                  <ConfigPageRow
                    key={`${group.scope}:${plugin.pluginId}`}
                    label={
                      <div className="plugins-config__plugin-label">
                        <span className="plugins-config__plugin-name">{plugin.name}</span>
                        {plugin.version && (
                          <span className="plugins-config__plugin-version">
                            v{plugin.version}
                          </span>
                        )}
                      </div>
                    }
                    description={
                      <div className="plugins-config__plugin-meta">
                        <span className="plugins-config__plugin-source">
                          <Package size={12} />
                          {plugin.source}
                        </span>
                        {plugin.skillCount > 0 && (
                          <span className="plugins-config__plugin-skills">
                            {t('skillsCount', { count: plugin.skillCount })}
                          </span>
                        )}
                        {plugin.diagnostics.length > 0 && (
                          <span className="plugins-config__plugin-diags">
                            <AlertTriangle size={12} />
                            {t('diagnosticsCount', { count: plugin.diagnostics.length })}
                          </span>
                        )}
                      </div>
                    }
                    align="center"
                  >
                    <div className="plugins-config__plugin-status">
                      {plugin.enabled ? (
                        <span className="plugins-config__badge plugins-config__badge--active">
                          <CheckCircle size={12} />
                          {t('enabled')}
                        </span>
                      ) : (
                        <span className="plugins-config__badge plugins-config__badge--disabled">
                          <Ban size={12} />
                          {t('disabled')}
                        </span>
                      )}
                      <Switch
                        checked={plugin.enabled}
                        onChange={(e) => handlePluginToggle(plugin, e.target.checked)}
                        disabled={toggling.has(plugin.pluginId)}
                        size="small"
                      />
                    </div>
                  </ConfigPageRow>
                ))
              )}
            </ConfigPageSection>
          ))}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};
export default PluginsConfig;
