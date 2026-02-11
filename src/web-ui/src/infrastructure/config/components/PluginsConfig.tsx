import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, FolderOpen, RefreshCw, Trash2 } from 'lucide-react';
import { Button, Card, CardBody, ConfirmDialog, IconButton, Input, Search, Switch, Tooltip } from '@/component-library';
import { open } from '@tauri-apps/plugin-dialog';
import { createLogger } from '@/shared/utils/logger';
import { useNotification } from '@/shared/notification-system';
import { pluginAPI, type PluginInfo } from '@/infrastructure/api/service-api/PluginAPI';
import { ConfigPageContent, ConfigPageHeader, ConfigPageLayout } from './common';
import './PluginsConfig.scss';

const log = createLogger('PluginsConfig');

const PluginsConfig: React.FC = () => {
  const { t } = useTranslation('settings/plugins');
  const notification = useNotification();

  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchKeyword, setSearchKeyword] = useState('');
  const [overwriteExisting, setOverwriteExisting] = useState(false);

  const [deleteConfirm, setDeleteConfirm] = useState<{ show: boolean; plugin: PluginInfo | null }>({
    show: false,
    plugin: null,
  });

  const loadPlugins = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const list = await pluginAPI.listPlugins();
      setPlugins(list);
    } catch (err) {
      log.error('Failed to load plugins', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  const filteredPlugins = useMemo(() => {
    if (!searchKeyword.trim()) return plugins;
    const keyword = searchKeyword.toLowerCase();
    return plugins.filter(p => (
      p.id.toLowerCase().includes(keyword) ||
      p.name.toLowerCase().includes(keyword) ||
      (p.description || '').toLowerCase().includes(keyword) ||
      p.path.toLowerCase().includes(keyword)
    ));
  }, [plugins, searchKeyword]);

  const handleInstallFromFile = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: t('toolbar.installFromFile'),
        filters: [{ name: 'Plugin', extensions: ['plugin', 'zip'] }],
      });
      if (!selected) return;
      await pluginAPI.installPlugin(selected as string);
      notification.success(t('messages.installSuccess'));
      loadPlugins();
    } catch (err) {
      notification.error(t('messages.installFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  }, [loadPlugins, notification, t]);

  const handleInstallFromFolder = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: true,
        title: t('toolbar.installFromFolder'),
      });
      if (!selected) return;
      await pluginAPI.installPlugin(selected as string);
      notification.success(t('messages.installSuccess'));
      loadPlugins();
    } catch (err) {
      notification.error(t('messages.installFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  }, [loadPlugins, notification, t]);

  const handleToggleEnabled = useCallback(async (plugin: PluginInfo) => {
    try {
      await pluginAPI.setPluginEnabled(plugin.id, !plugin.enabled);
      notification.success(t('messages.toggleSuccess', { name: plugin.name }));
      loadPlugins();
    } catch (err) {
      notification.error(t('messages.toggleFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  }, [loadPlugins, notification, t]);

  const handleImportMcpServers = useCallback(async (plugin: PluginInfo) => {
    try {
      const result = await pluginAPI.importPluginMcpServers(plugin.id, overwriteExisting);
      notification.success(t('messages.importSuccess', { added: result.added, overwritten: result.overwritten, skipped: result.skipped }));
    } catch (err) {
      notification.error(t('messages.importFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  }, [notification, overwriteExisting, t]);

  const showDeleteConfirm = (plugin: PluginInfo) => {
    setDeleteConfirm({ show: true, plugin });
  };

  const cancelDelete = () => {
    setDeleteConfirm({ show: false, plugin: null });
  };

  const confirmDelete = useCallback(async () => {
    const plugin = deleteConfirm.plugin;
    if (!plugin) return;
    try {
      await pluginAPI.uninstallPlugin(plugin.id);
      notification.success(t('messages.uninstallSuccess', { name: plugin.name }));
      loadPlugins();
    } catch (err) {
      notification.error(t('messages.uninstallFailed', { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setDeleteConfirm({ show: false, plugin: null });
    }
  }, [deleteConfirm.plugin, loadPlugins, notification, t]);

  const renderPluginsList = () => {
    if (loading) return <div className="bitfun-plugins-config__loading">{t('list.loading')}</div>;
    if (error) return <div className="bitfun-plugins-config__error">{t('list.errorPrefix')}{error}</div>;
    if (filteredPlugins.length === 0) return <div className="bitfun-plugins-config__empty">{t('list.empty')}</div>;

    return (
      <div className="bitfun-plugins-config__list">
        {filteredPlugins.map((plugin) => (
          <Card key={plugin.id} variant="default" padding="none" className={`bitfun-plugins-config__item ${!plugin.enabled ? 'is-disabled' : ''}`}>
            <CardBody className="bitfun-plugins-config__item-body">
              <div className="bitfun-plugins-config__item-main">
                <div className="bitfun-plugins-config__item-title">
                  <div className="bitfun-plugins-config__item-name">{plugin.name}</div>
                  {plugin.version ? <div className="bitfun-plugins-config__item-version">v{plugin.version}</div> : null}
                </div>
                {plugin.description ? <div className="bitfun-plugins-config__item-description">{plugin.description}</div> : null}
                <div className="bitfun-plugins-config__item-meta">
                  <div className="bitfun-plugins-config__item-path">{plugin.path}</div>
                  {plugin.hasMcpConfig ? (
                    <div className="bitfun-plugins-config__item-mcp">
                      {t('list.item.mcpServers', { count: plugin.mcpServerCount })}
                    </div>
                  ) : (
                    <div className="bitfun-plugins-config__item-mcp is-missing">
                      {t('list.item.noMcp')}
                    </div>
                  )}
                </div>
              </div>

              <div className="bitfun-plugins-config__item-actions" onClick={(e) => e.stopPropagation()}>
                <div className="bitfun-plugins-config__toggle">
                  <Switch checked={plugin.enabled} onChange={() => handleToggleEnabled(plugin)} />
                </div>

                <Button
                  size="small"
                  variant="secondary"
                  disabled={!plugin.hasMcpConfig}
                  onClick={() => handleImportMcpServers(plugin)}
                >
                  {t('list.item.importMcp')}
                </Button>

                <Tooltip content={t('list.item.uninstall')}>
                  <IconButton
                    size="small"
                    variant="ghost"
                    onClick={() => showDeleteConfirm(plugin)}
                  >
                    <Trash2 size={14} />
                  </IconButton>
                </Tooltip>
              </div>
            </CardBody>
          </Card>
        ))}
      </div>
    );
  };

  return (
    <ConfigPageLayout className="bitfun-plugins-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-plugins-config__content">
        <div className="bitfun-plugins-config__toolbar">
          <div className="bitfun-plugins-config__search-box">
            <Search size={16} className="bitfun-plugins-config__search-icon" />
            <Input
              value={searchKeyword}
              onChange={(e) => setSearchKeyword(e.target.value)}
              placeholder={t('toolbar.searchPlaceholder')}
            />
          </div>

          <div className="bitfun-plugins-config__toolbar-actions">
            <div className="bitfun-plugins-config__overwrite">
              <span className="bitfun-plugins-config__overwrite-label">{t('toolbar.overwriteExisting')}</span>
              <Switch checked={overwriteExisting} onChange={() => setOverwriteExisting(v => !v)} />
            </div>

            <Tooltip content={t('toolbar.refreshTooltip')}>
              <IconButton variant="ghost" size="small" onClick={loadPlugins}>
                <RefreshCw size={16} />
              </IconButton>
            </Tooltip>

            <Button size="small" variant="secondary" onClick={handleInstallFromFolder}>
              <FolderOpen size={14} />
              {t('toolbar.installFromFolder')}
            </Button>

            <Button size="small" variant="primary" onClick={handleInstallFromFile}>
              <Download size={14} />
              {t('toolbar.installFromFile')}
            </Button>
          </div>
        </div>

        {renderPluginsList()}

        <ConfirmDialog
          isOpen={deleteConfirm.show && !!deleteConfirm.plugin}
          onClose={cancelDelete}
          onConfirm={confirmDelete}
          title={t('deleteModal.title')}
          message={<p>{t('deleteModal.message', { name: deleteConfirm.plugin?.name })}</p>}
          type="warning"
          confirmDanger
          confirmText={t('deleteModal.delete')}
          cancelText={t('deleteModal.cancel')}
        />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default PluginsConfig;

