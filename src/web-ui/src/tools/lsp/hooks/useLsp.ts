/**
 * LSP hooks used by React components.
 */

import { useState, useEffect, useCallback } from 'react';
import { lspService } from '../services/LspService';
import type { LspPlugin } from '../types';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';

const log = createLogger('useLsp');

/**
 * List and manage installed LSP plugins.
 */
export function useLspPlugins() {
  const { t } = useI18n('tools');
  const [plugins, setPlugins] = useState<LspPlugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadPlugins = useCallback(async () => {
    setLoading(true);
    setError(null);
    
    try {
      await lspService.initialize();
      
      const pluginList = await lspService.listPlugins();
      setPlugins(pluginList);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : t('lsp.errors.loadPluginsFailed');
      setError(errorMessage);
      log.error('Failed to load plugins', { error: err });
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  const installPlugin = useCallback(async (packagePath: string) => {
    try {
      await lspService.installPlugin(packagePath);
      await loadPlugins();
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : t('lsp.errors.installPluginFailed'));
      return false;
    }
  }, [loadPlugins, t]);

  const uninstallPlugin = useCallback(async (pluginId: string) => {
    try {
      await lspService.uninstallPlugin(pluginId);
      await loadPlugins();
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : t('lsp.errors.uninstallPluginFailed'));
      return false;
    }
  }, [loadPlugins, t]);

  return {
    plugins,
    loading,
    error,
    reload: loadPlugins,
    installPlugin,
    uninstallPlugin
  };
}

/**
 * Initialize LSP, optionally setting a workspace root.
 */
export function useLspInit(workspacePath?: string) {
  const { t } = useI18n('tools');
  const [initialized, setInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const init = async () => {
      try {
        await lspService.initialize();
        
        if (workspacePath) {
          await lspService.openWorkspace(workspacePath);
        }
        
        setInitialized(true);
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : t('lsp.errors.initializeFailed');
        setError(errorMessage);
        log.error('Failed to initialize LSP', { workspacePath, error: err });
      }
    };

    init();
  }, [workspacePath, t]);

  return { initialized, error };
}

