 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, CheckCircle, XCircle, Layers, ExternalLink } from 'lucide-react';
import {
  Button,
  Alert,
  Select,
  Tooltip,
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRefreshButton,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { configManager } from '../services/ConfigManager';
import { getTerminalService } from '@/tools/terminal';
import { systemAPI } from '@/infrastructure/api/service-api/SystemAPI';
import { createTerminalTab } from '@/shared/utils/tabUtils';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import type { TerminalConfig as TerminalConfigType } from '../types';
import type { ShellInfo } from '@/tools/terminal/types/session';
import './TerminalConfig.scss';

const log = createLogger('TerminalConfig');



 
interface CLIAgentInfo {
  id: string;
  name: string;
  command: string; 
  startUpCommand: string; 
  installCommandWin: string;
  installCommandUnix: string;
  websiteUrl: string;
  requiresNpm: boolean;  
}

 
interface CLIAgentStatus {
  exists: boolean;
  path: string | null;
  checking: boolean;
  installing: boolean;
}

 
const CLI_AGENTS: CLIAgentInfo[] = [
  {
    id: 'claude',
    name: 'Claude Code',
    command: 'claude',
    startUpCommand: ' claude',
    // installCommandWin: ' irm https://claude.ai/install.ps1 | iex',
    // installCommandUnix: ' curl -fsSL https://claude.ai/install.sh | bash',
    installCommandWin: ' npm install -g @anthropic-ai/claude-code',
    installCommandUnix: ' npm install -g @anthropic-ai/claude-code',
    websiteUrl: 'https://claude.com/product/claude-code',
    requiresNpm: true,
  },
  {
    id: 'codex',
    name: 'CodeX',
    command: 'codex',
    startUpCommand: ' codex',
    installCommandWin: ' npm i -g @openai/codex',
    installCommandUnix: ' npm i -g @openai/codex',
    websiteUrl: 'https://openai.com/codex/',
    requiresNpm: true,
  },
  {
    id: 'gemini',
    name: 'Gemini CLI',
    command: 'gemini',
    startUpCommand: ' gemini',
    installCommandWin: ' npm i -g @google/gemini-cli',
    installCommandUnix: ' npm i -g @google/gemini-cli',
    websiteUrl: 'https://geminicli.com/',
    requiresNpm: true,
  },
  {
    id: 'opencode',
    name: 'OpenCode',
    command: 'opencode',
    startUpCommand: ' opencode',
    installCommandWin: ' npm i -g opencode-ai',
    installCommandUnix: ' npm i -g opencode-ai',
    websiteUrl: 'https://opencode.ai/',
    requiresNpm: true,
  },
];

const TerminalConfig: React.FC = () => {
  const { t } = useTranslation('settings/terminal');
  const [defaultShell, setDefaultShell] = useState<string>('');
  const [availableShells, setAvailableShells] = useState<ShellInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [platform, setPlatform] = useState<string>('');
  
  
  const [cliAgentStatus, setCLIAgentStatus] = useState<Record<string, CLIAgentStatus>>({});
  
  
  const [npmAvailable, setNpmAvailable] = useState<boolean | null>(null); 
  
  
  const { workspacePath } = useCurrentWorkspace();

  
  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);

      
      const [terminalConfig, shells, systemInfo] = await Promise.all([
        configManager.getConfig<TerminalConfigType>('terminal'),
        getTerminalService().getAvailableShells(),
        systemAPI.getSystemInfo().catch(() => ({ platform: '' })) 
      ]);

      
      setDefaultShell(terminalConfig?.default_shell || '');

      
      const availableOnly = shells.filter(s => s.available);
      setAvailableShells(availableOnly);

      
      setPlatform(systemInfo.platform || '');
      
      
      await checkCLIAgents();
    } catch (error) {
      log.error('Failed to load terminal config data', error);
      showMessage('error', t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  };
  
  
  const checkCLIAgents = async () => {
    
    const initialStatus: Record<string, CLIAgentStatus> = {};
    CLI_AGENTS.forEach(agent => {
      initialStatus[agent.id] = { exists: false, path: null, checking: true, installing: false };
    });
    setCLIAgentStatus(initialStatus);
    setNpmAvailable(null); 
    
    try {
      
      const commands = [...CLI_AGENTS.map(agent => agent.command), 'npm'];
      const results = await systemAPI.checkCommandsExist(commands);
      
      const newStatus: Record<string, CLIAgentStatus> = {};
      results.forEach(([command, result]) => {
        
        if (command === 'npm') {
          setNpmAvailable(result.exists);
          return;
        }
        
        const agent = CLI_AGENTS.find(a => a.command === command);
        if (agent) {
          newStatus[agent.id] = {
            exists: result.exists,
            path: result.path,
            checking: false,
            installing: false,
          };
        }
      });
      setCLIAgentStatus(newStatus);
    } catch (error) {
      log.error('Failed to check CLI agents', { error });
      const errorStatus: Record<string, CLIAgentStatus> = {};
      CLI_AGENTS.forEach(agent => {
        errorStatus[agent.id] = { exists: false, path: null, checking: false, installing: false };
      });
      setCLIAgentStatus(errorStatus);
      setNpmAvailable(false);
    }
  };

  const handleShellChange = useCallback(async (value: string) => {
    try {
      setSaving(true);
      setDefaultShell(value);
      
      
      await configManager.setConfig('terminal.default_shell', value);
      
      
      configManager.clearCache();
      
      showMessage('success', t('messages.updated'));
    } catch (error) {
      log.error('Failed to save terminal config', { shell: value, error });
      showMessage('error', t('messages.saveFailed'));
    } finally {
      setSaving(false);
    }
  }, []);

  const showMessage = (type: 'success' | 'error' | 'info', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  const handleRefresh = useCallback(async () => {
    await loadData();
    showMessage('info', t('messages.refreshed'));
  }, [t]);
  
  
  const handleRefreshCLIAgents = useCallback(async () => {
    await checkCLIAgents();
    showMessage('info', t('messages.cliRefreshed'));
  }, [t]);
  
  
  const handleInstallAgent = useCallback(async (agent: CLIAgentInfo) => {
    const terminalService = getTerminalService();
    
    
    setCLIAgentStatus(prev => ({
      ...prev,
      [agent.id]: { ...prev[agent.id], installing: true }
    }));
    
    try {
      
      const isWindows = platform === 'windows';
      const installCommand = isWindows ? agent.installCommandWin : agent.installCommandUnix;
      
      
      const session = await terminalService.createSession({
        workingDirectory: workspacePath || undefined,
        name: t('cliAgents.installSessionName', { name: agent.name }),
      });
      
      
      createTerminalTab(session.id, t('cliAgents.installTabTitle', { name: agent.name }));
      
      
      await terminalService.sendCommand(session.id, installCommand);
      
      showMessage('info', t('messages.installingAgent', { name: agent.name }));
    } catch (error) {
      log.error('Failed to install CLI agent', { agentName: agent.name, agentId: agent.id, error });
      showMessage('error', t('messages.installAgentFailed', { name: agent.name }));
    } finally {
      
      setCLIAgentStatus(prev => ({
        ...prev,
        [agent.id]: { ...prev[agent.id], installing: false }
      }));
    }
  }, [platform, workspacePath]);
  
  
  const handleOpenInHub = useCallback((agent: CLIAgentInfo) => {
    
    window.dispatchEvent(new CustomEvent('create-hub-terminal', {
      detail: {
        name: agent.name,
        startupCommand: agent.startUpCommand,
      }
    }));
    
    showMessage('success', t('messages.hubCreated', { name: agent.name }));
  }, [t]);

  
  const shouldShowPowerShellCoreRecommendation = useCallback(() => {
    
    const isWindows = platform === 'windows';
    if (!isWindows) return false;

    
    const hasPowerShellCore = availableShells.some(
      shell => shell.shellType === 'PowerShellCore'
    );

    return !hasPowerShellCore;
  }, [availableShells, platform]);

  
  const shouldShowNpmRecommendation = useCallback(() => {
    
    return npmAvailable === false;
  }, [npmAvailable]);

  
  const shellOptions = [
    { value: '', label: t('terminal.autoDetect') },
    ...availableShells.map(shell => ({
      value: shell.shellType,
      label: `${shell.name}${shell.version ? ` (${shell.version})` : ''}`
    }))
  ];

  if (loading) {
    return (
      <ConfigPageLayout className="bitfun-terminal-config">
        <ConfigPageHeader
          title={t('title')}
          subtitle={t('subtitle')}
        />
        <ConfigPageContent>
          <ConfigPageLoading text={t('messages.loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-terminal-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />
      
      <ConfigPageContent className="bitfun-terminal-config__content">
        
        <ConfigPageMessage message={message} />

        
        {shouldShowPowerShellCoreRecommendation() && (
          <div className="bitfun-terminal-config__message-container">
            <Alert
              type="info"
              message={
                <span style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                  <Download size={16} />
                  <span>{t('recommendations.pwsh.prefix')} </span>
                  <strong>{t('recommendations.pwsh.name')}</strong>
                  <span>{t('recommendations.pwsh.suffix')}</span>
                  <a
                    href="https://aka.ms/PSWindows"
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{
                      color: 'var(--color-primary)',
                      textDecoration: 'underline',
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px'
                    }}
                  >
                    {t('recommendations.pwsh.link')}
                  </a>
                </span>
              }
            />
          </div>
        )}

        
        <ConfigPageSection
          title={t('sections.defaultTerminal')}
          extra={(
            <ConfigPageRefreshButton
              tooltip={t('terminal.refreshTooltip')}
              onClick={handleRefresh}
              loading={loading}
              disabled={loading}
            />
          )}
        >
          <ConfigPageRow
            label={t('sections.defaultTerminal')}
            description={t('terminal.description')}
            align="center"
          >
            <div className="bitfun-terminal-config__select-wrapper">
              {availableShells.length > 0 ? (
                <Select
                  value={defaultShell}
                  onChange={(v) => handleShellChange(v as string)}
                  options={shellOptions}
                  placeholder={t('terminal.placeholder')}
                  disabled={saving}
                />
              ) : (
                <div className="bitfun-terminal-config__no-shells">
                  {t('terminal.noShells')}
                </div>
              )}
            </div>
          </ConfigPageRow>

          {platform === 'windows' && defaultShell === 'Cmd' && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert type="warning" message={t('terminal.warnings.cmd')} />
            </div>
          )}
          {platform === 'windows' && defaultShell === 'Bash' && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert type="warning" message={t('terminal.warnings.gitBash')} />
            </div>
          )}
        </ConfigPageSection>

        <ConfigPageSection
          title={t('sections.cliAgents')}
          description={t('cliAgents.description')}
          extra={(
            <ConfigPageRefreshButton
              tooltip={t('cliAgents.refreshTooltip')}
              onClick={handleRefreshCLIAgents}
              loading={loading}
              disabled={loading}
            />
          )}
        >
          {shouldShowNpmRecommendation() && (
            <div className="bitfun-terminal-config__inline-alert">
              <Alert
                type="warning"
                message={
                  <span style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                    <span>{t('recommendations.npm.prefix')} </span>
                    <strong>{t('recommendations.npm.name')}</strong>
                    <span> {t('recommendations.npm.suffix')}</span>
                    <a
                      href="https://nodejs.org/zh-cn/download"
                      target="_blank"
                      rel="noopener noreferrer"
                      style={{
                        color: 'var(--color-primary)',
                        textDecoration: 'underline',
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '4px'
                      }}
                    >
                      {t('recommendations.npm.link')}
                    </a>
                  </span>
                }
              />
            </div>
          )}

          <div className="bitfun-terminal-config__cli-agents-list">
            {CLI_AGENTS.map(agent => {
              const status = cliAgentStatus[agent.id] || { exists: false, path: null, checking: true, installing: false };
              return (
                <div key={agent.id} className="bitfun-terminal-config__cli-agent-item">
                  <div className="bitfun-terminal-config__cli-agent-info">
                    <div className="bitfun-terminal-config__cli-agent-header">
                      <span className="bitfun-terminal-config__cli-agent-name">{agent.name}</span>
                      <span className={`bitfun-terminal-config__cli-agent-status ${status.checking ? 'checking' : status.exists ? 'available' : 'unavailable'}`}>
                        {status.checking ? (
                          <div className="bitfun-terminal-config__status-spinner" />
                        ) : status.exists ? (
                          <CheckCircle size={12} />
                        ) : (
                          <XCircle size={12} />
                        )}
                        <span>{status.checking ? t('cliAgents.checking') : status.exists ? t('cliAgents.available') : t('cliAgents.notInstalled')}</span>
                      </span>
                    </div>
                  </div>

                  <div className="bitfun-terminal-config__cli-agent-actions">
                    {agent.websiteUrl && (
                      <Tooltip content={t('cliAgents.visitWebsite', { name: agent.name })}>
                        <Button
                          size="small"
                          variant="ghost"
                          onClick={() => systemAPI.openExternal(agent.websiteUrl)}
                        >
                          <ExternalLink size={14} />
                        </Button>
                      </Tooltip>
                    )}
                    {!status.exists && !status.checking && (
                      <Tooltip content={agent.requiresNpm && !npmAvailable ? t('cliAgents.needNpm') : t('cliAgents.install')}>
                        <Button
                          size="small"
                          variant="secondary"
                          onClick={() => handleInstallAgent(agent)}
                          disabled={status.installing || (agent.requiresNpm && !npmAvailable)}
                        >
                          <Download size={14} />
                          {status.installing ? t('cliAgents.installing') : t('cliAgents.install')}
                        </Button>
                      </Tooltip>
                    )}
                    <Tooltip content={!status.exists ? t('cliAgents.needInstall') : t('cliAgents.openInHub', { name: agent.name })}>
                      <Button
                        size="small"
                        variant="primary"
                        onClick={() => handleOpenInHub(agent)}
                        disabled={!status.exists || status.checking}
                      >
                        <Layers size={14} />
                        {t('cliAgents.launch')}
                      </Button>
                    </Tooltip>
                  </div>
                </div>
              );
            })}
          </div>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default TerminalConfig;

