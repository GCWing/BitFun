import { api } from './ApiClient';

export type ConnectorProviderType = 'dingtalk' | 'xiaoshouyi' | 'custom';

export type ConnectorConnectionStatus =
  | 'disconnected'
  | 'connected'
  | 'error'
  | 'pending';

export interface ConnectorProviderDefinition {
  type: ConnectorProviderType;
  displayName: string;
  description: string;
  iconKey: string;
  docsUrl?: string;
  supportedCapabilities: string[];
}

export interface ConnectorConfigField {
  key: string;
  label: string;
  type: 'string' | 'secret' | 'url';
  required: boolean;
  placeholder?: string;
  helpText?: string;
}

export interface ConnectorProviderConfig {
  type: ConnectorProviderType;
  fields: ConnectorConfigField[];
}

export interface ConnectorInstance {
  id: string;
  provider: ConnectorProviderType;
  name: string;
  status: ConnectorConnectionStatus;
  config: Record<string, string>;
  lastSyncedAt?: string;
  errorMessage?: string;
}

export interface ConnectorListResponse {
  connectors: ConnectorInstance[];
}

export interface ConnectorTestResult {
  success: boolean;
  message: string;
}

export const CONNECTOR_PROVIDERS: readonly ConnectorProviderDefinition[] = [
  {
    type: 'dingtalk',
    displayName: 'DingTalk',
    description: 'Connect to DingTalk (钉钉) for messages, contacts, and approval data.',
    iconKey: 'dingtalk',
    docsUrl: 'https://open.dingtalk.com',
    supportedCapabilities: ['messages', 'contacts', 'approvals', 'calendar'],
  },
  {
    type: 'xiaoshouyi',
    displayName: 'XiaoShouYi (Neocrm)',
    description: 'Connect to XiaoShouYi (销售易) for CRM data, leads, and customer insights.',
    iconKey: 'xiaoshouyi',
    docsUrl: 'https://www.xiaoshouyi.com',
    supportedCapabilities: ['leads', 'customers', 'opportunities', 'reports'],
  },
];

export const CONNECTOR_PROVIDER_CONFIGS: Record<ConnectorProviderType, ConnectorProviderConfig> = {
  dingtalk: {
    type: 'dingtalk',
    fields: [
      {
        key: 'appKey',
        label: 'App Key',
        type: 'string',
        required: true,
        placeholder: 'dingxxxxx',
        helpText: 'DingTalk application App Key from the open platform.',
      },
      {
        key: 'appSecret',
        label: 'App Secret',
        type: 'secret',
        required: true,
        placeholder: 'Enter App Secret',
        helpText: 'DingTalk application App Secret.',
      },
      {
        key: 'baseUrl',
        label: 'API Base URL',
        type: 'url',
        required: false,
        placeholder: 'https://oapi.dingtalk.com',
        helpText: 'Override the default DingTalk API endpoint.',
      },
    ],
  },
  xiaoshouyi: {
    type: 'xiaoshouyi',
    fields: [
      {
        key: 'clientId',
        label: 'Client ID',
        type: 'string',
        required: true,
        placeholder: 'Enter Client ID',
        helpText: 'XiaoShouYi API Client ID.',
      },
      {
        key: 'clientSecret',
        label: 'Client Secret',
        type: 'secret',
        required: true,
        placeholder: 'Enter Client Secret',
        helpText: 'XiaoShouYi API Client Secret.',
      },
      {
        key: 'baseUrl',
        label: 'API Base URL',
        type: 'url',
        required: false,
        placeholder: 'https://api.xiaoshouyi.com',
        helpText: 'Override the default XiaoShouYi API endpoint.',
      },
    ],
  },
  custom: {
    type: 'custom',
    fields: [
      {
        key: 'baseUrl',
        label: 'API Base URL',
        type: 'url',
        required: true,
        placeholder: 'https://api.example.com',
        helpText: 'Custom connector API base URL.',
      },
      {
        key: 'apiKey',
        label: 'API Key',
        type: 'secret',
        required: true,
        placeholder: 'Enter API Key',
        helpText: 'Authentication key for the custom connector.',
      },
    ],
  },
};

export const connectorsAPI = {
  async listConnectors(): Promise<ConnectorListResponse> {
    return api.invoke<ConnectorListResponse>('connectors_list', { request: {} });
  },

  async createConnector(
    provider: ConnectorProviderType,
    name: string,
    config: Record<string, string>,
  ): Promise<ConnectorInstance> {
    return api.invoke<ConnectorInstance>('connectors_create', {
      request: { provider, name, config },
    });
  },

  async updateConnector(
    id: string,
    config: Record<string, string>,
  ): Promise<ConnectorInstance> {
    return api.invoke<ConnectorInstance>('connectors_update', {
      request: { id, config },
    });
  },

  async deleteConnector(id: string): Promise<void> {
    await api.invoke<void>('connectors_delete', { request: { id } });
  },

  async testConnector(id: string): Promise<ConnectorTestResult> {
    return api.invoke<ConnectorTestResult>('connectors_test', {
      request: { id },
    });
  },

  async syncConnector(id: string): Promise<ConnectorInstance> {
    return api.invoke<ConnectorInstance>('connectors_sync', {
      request: { id },
    });
  },
};
