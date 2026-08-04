import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircle2,
  CircleDashed,
  AlertTriangle,
  Plug,
  Plus,
  Trash2,
  RefreshCw,
  TestTube2,
  Loader2,
} from 'lucide-react';
import { Button, ConfigPageLoading } from '@/component-library';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import {
  connectorsAPI,
  CONNECTOR_PROVIDERS,
  CONNECTOR_PROVIDER_CONFIGS,
  type ConnectorInstance,
  type ConnectorProviderType,
  type ConnectorConnectionStatus,
} from '@/infrastructure/api/service-api/ConnectorsAPI';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('ConnectorsConfig');

function statusIcon(status: ConnectorConnectionStatus): React.ReactNode {
  switch (status) {
    case 'connected':
      return <CheckCircle2 size={16} className="connectors-config__status-icon--connected" />;
    case 'disconnected':
      return <CircleDashed size={16} className="connectors-config__status-icon--disconnected" />;
    case 'error':
      return <AlertTriangle size={16} className="connectors-config__status-icon--error" />;
    case 'pending':
      return <Loader2 size={16} className="connectors-config__status-icon--pending" />;
    default:
      return null;
  }
}

function statusLabelKey(status: ConnectorConnectionStatus): string {
  switch (status) {
    case 'connected':
      return 'connectors.status.connected';
    case 'disconnected':
      return 'connectors.status.disconnected';
    case 'error':
      return 'connectors.status.error';
    case 'pending':
      return 'connectors.status.pending';
    default:
      return 'connectors.status.disconnected';
  }
}

const AddConnectorDialog: React.FC<{
  open: boolean;
  onClose: () => void;
  onAdd: (provider: ConnectorProviderType, name: string, config: Record<string, string>) => void;
  adding: boolean;
}> = ({ open, onClose, onAdd, adding }) => {
  const { t } = useTranslation('settings/connectors');
  const [selectedProvider, setSelectedProvider] = useState<ConnectorProviderType>('dingtalk');
  const [name, setName] = useState('');
  const [configValues, setConfigValues] = useState<Record<string, string>>({});

  const providerConfig = CONNECTOR_PROVIDER_CONFIGS[selectedProvider];

  const handleSubmit = useCallback(() => {
    onAdd(selectedProvider, name || providerConfig.fields[0]?.label || 'Connector', configValues);
    setName('');
    setConfigValues({});
    setSelectedProvider('dingtalk');
  }, [selectedProvider, name, configValues, onAdd, providerConfig]);

  const handleCancel = useCallback(() => {
    setName('');
    setConfigValues({});
    setSelectedProvider('dingtalk');
    onClose();
  }, [onClose]);

  if (!open) return null;

  return (
    <div className="connectors-config__dialog-overlay" onClick={handleCancel}>
      <div
        className="connectors-config__dialog"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <h3 className="connectors-config__dialog-title">{t('connectors.addDialog.title')}</h3>

        <div className="connectors-config__dialog-field">
          <label className="connectors-config__dialog-label">{t('connectors.addDialog.provider')}</label>
          <select
            className="connectors-config__dialog-select"
            value={selectedProvider}
            onChange={(e) => {
              setSelectedProvider(e.target.value as ConnectorProviderType);
              setConfigValues({});
            }}
          >
            {CONNECTOR_PROVIDERS.map((p) => (
              <option key={p.type} value={p.type}>
                {p.displayName}
              </option>
            ))}
          </select>
        </div>

        <div className="connectors-config__dialog-field">
          <label className="connectors-config__dialog-label">{t('connectors.addDialog.name')}</label>
          <input
            className="connectors-config__dialog-input"
            type="text"
            value={name}
            placeholder={t('connectors.addDialog.namePlaceholder')}
            onChange={(e) => setName(e.target.value)}
          />
        </div>

        {providerConfig.fields.map((field) => (
          <div key={field.key} className="connectors-config__dialog-field">
            <label className="connectors-config__dialog-label">
              {field.label}
              {field.required && <span className="connectors-config__required-mark"> *</span>}
            </label>
            <input
              className="connectors-config__dialog-input"
              type={field.type === 'secret' ? 'password' : field.type === 'url' ? 'url' : 'text'}
              value={configValues[field.key] ?? ''}
              placeholder={field.placeholder ?? ''}
              onChange={(e) =>
                setConfigValues((prev) => ({ ...prev, [field.key]: e.target.value }))
              }
            />
            {field.helpText && (
              <span className="connectors-config__dialog-help">{field.helpText}</span>
            )}
          </div>
        ))}

        <div className="connectors-config__dialog-actions">
          <Button variant="ghost" onClick={handleCancel} disabled={adding}>
            {t('connectors.addDialog.cancel')}
          </Button>
          <Button variant="primary" onClick={handleSubmit} disabled={adding}>
            {adding ? t('connectors.addDialog.adding') : t('connectors.addDialog.add')}
          </Button>
        </div>
      </div>
    </div>
  );
};

const ConnectorCard: React.FC<{
  connector: ConnectorInstance;
  onDelete: (id: string) => void;
  onTest: (id: string) => void;
  onSync: (id: string) => void;
  deleting: string | null;
  testing: string | null;
  syncing: string | null;
}> = ({ connector, onDelete, onTest, onSync, deleting, testing, syncing }) => {
  const { t } = useTranslation('settings/connectors');
  const providerDef = CONNECTOR_PROVIDERS.find((p) => p.type === connector.provider);
  const isBusy = deleting === connector.id || testing === connector.id || syncing === connector.id;

  return (
    <div className="connectors-config__card" data-connector-id={connector.id}>
      <div className="connectors-config__card-header">
        <div className="connectors-config__card-info">
          <Plug size={18} className="connectors-config__card-icon" />
          <div className="connectors-config__card-text">
            <span className="connectors-config__card-name">{connector.name}</span>
            <span className="connectors-config__card-provider">
              {providerDef?.displayName ?? connector.provider}
            </span>
          </div>
        </div>
        <div className="connectors-config__card-status">
          {statusIcon(connector.status)}
          <span className="connectors-config__card-status-label">
            {t(statusLabelKey(connector.status))}
          </span>
        </div>
      </div>

      {connector.errorMessage && (
        <div className="connectors-config__card-error">
          <AlertTriangle size={14} />
          <span>{connector.errorMessage}</span>
        </div>
      )}

      {connector.lastSyncedAt && (
        <div className="connectors-config__card-synced">
          {t('connectors.card.lastSynced')}: {connector.lastSyncedAt}
        </div>
      )}

      <div className="connectors-config__card-actions">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onTest(connector.id)}
          disabled={isBusy}
        >
          <TestTube2 size={14} />
          {testing === connector.id ? t('connectors.card.testing') : t('connectors.card.test')}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onSync(connector.id)}
          disabled={isBusy}
        >
          <RefreshCw size={14} />
          {syncing === connector.id ? t('connectors.card.syncing') : t('connectors.card.sync')}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onDelete(connector.id)}
          disabled={isBusy}
        >
          <Trash2 size={14} />
          {deleting === connector.id ? t('connectors.card.deleting') : t('connectors.card.delete')}
        </Button>
      </div>
    </div>
  );
};

const ConnectorsConfig: React.FC = () => {
  const { t, ready } = useTranslation('settings/connectors');
  const [connectors, setConnectors] = useState<ConnectorInstance[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [testing, setTesting] = useState<string | null>(null);
  const [syncing, setSyncing] = useState<string | null>(null);

  const loadConnectors = useCallback(async () => {
    setLoading(true);
    try {
      const response = await connectorsAPI.listConnectors();
      setConnectors(response.connectors);
    } catch (err) {
      logger.warn('Failed to load connectors', { error: String(err) });
      setConnectors([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadConnectors();
  }, [loadConnectors]);

  const handleAdd = useCallback(
    async (
      provider: ConnectorProviderType,
      name: string,
      config: Record<string, string>,
    ) => {
      setAdding(true);
      try {
        const instance = await connectorsAPI.createConnector(provider, name, config);
        setConnectors((prev) => [...prev, instance]);
        setShowAddDialog(false);
      } catch (err) {
        logger.error('Failed to create connector', { error: String(err) });
      } finally {
        setAdding(false);
      }
    },
    [],
  );

  const handleDelete = useCallback(async (id: string) => {
    setDeleting(id);
    try {
      await connectorsAPI.deleteConnector(id);
      setConnectors((prev) => prev.filter((c) => c.id !== id));
    } catch (err) {
      logger.error('Failed to delete connector', { error: String(err), id });
    } finally {
      setDeleting(null);
    }
  }, []);

  const handleTest = useCallback(async (id: string) => {
    setTesting(id);
    try {
      await connectorsAPI.testConnector(id);
    } catch (err) {
      logger.error('Connector test failed', { error: String(err), id });
    } finally {
      setTesting(null);
    }
  }, []);

  const handleSync = useCallback(async (id: string) => {
    setSyncing(id);
    try {
      const updated = await connectorsAPI.syncConnector(id);
      setConnectors((prev) => prev.map((c) => (c.id === id ? updated : c)));
    } catch (err) {
      logger.error('Connector sync failed', { error: String(err), id });
    } finally {
      setSyncing(null);
    }
  }, []);

  if (loading || !ready) {
    return <ConfigPageLoading />;
  }

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('connectors.title')}
        description={t('connectors.description')}
      />
      <ConfigPageContent>
        <ConfigPageSection>
          <ConfigPageRow>
            <div className="connectors-config__header-row">
              <h2 className="connectors-config__section-title">
                {t('connectors.section.configuredConnectors')}
              </h2>
              <Button
                variant="primary"
                size="sm"
                onClick={() => setShowAddDialog(true)}
              >
                <Plus size={14} />
                {t('connectors.actions.add')}
              </Button>
            </div>
          </ConfigPageRow>

          {connectors.length === 0 ? (
            <ConfigPageRow>
              <div className="connectors-config__empty-state">
                <Plug size={32} className="connectors-config__empty-icon" />
                <p className="connectors-config__empty-text">
                  {t('connectors.emptyState.description')}
                </p>
              </div>
            </ConfigPageRow>
          ) : (
            <ConfigPageRow>
              <div className="connectors-config__card-list">
                {connectors.map((connector) => (
                  <ConnectorCard
                    key={connector.id}
                    connector={connector}
                    onDelete={handleDelete}
                    onTest={handleTest}
                    onSync={handleSync}
                    deleting={deleting}
                    testing={testing}
                    syncing={syncing}
                  />
                ))}
              </div>
            </ConfigPageRow>
          )}
        </ConfigPageSection>

        <ConfigPageSection>
          <ConfigPageRow>
            <h3 className="connectors-config__section-title">
              {t('connectors.section.availableProviders')}
            </h3>
          </ConfigPageRow>
          <ConfigPageRow>
            <div className="connectors-config__provider-grid">
              {CONNECTOR_PROVIDERS.map((provider) => (
                <div
                  key={provider.type}
                  className="connectors-config__provider-card"
                >
                  <div className="connectors-config__provider-header">
                    <span className="connectors-config__provider-name">
                      {provider.displayName}
                    </span>
                  </div>
                  <p className="connectors-config__provider-description">
                    {provider.description}
                  </p>
                  {provider.supportedCapabilities.length > 0 && (
                    <div className="connectors-config__provider-capabilities">
                      {provider.supportedCapabilities.map((cap) => (
                        <span key={cap} className="connectors-config__capability-tag">
                          {cap}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <AddConnectorDialog
          open={showAddDialog}
          onClose={() => setShowAddDialog(false)}
          onAdd={handleAdd}
          adding={adding}
        />
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ConnectorsConfig;
