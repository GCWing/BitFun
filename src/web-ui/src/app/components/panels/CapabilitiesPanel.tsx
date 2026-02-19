/**
 * Capabilities panel — independent left panel for Skills, Subagents, and MCP servers.
 * Extracted from SessionsPanel to give it a dedicated panel entry.
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Puzzle, Bot, Plug, AlertTriangle, RefreshCw, Check, Copy } from 'lucide-react';
import { Switch, Card, CardBody } from '@/component-library';
import { PanelHeader } from './base';
import { createLogger } from '@/shared/utils/logger';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { SubagentAPI, type SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import type { SkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import './SessionsPanel.scss';

const log = createLogger('CapabilitiesPanel');

const MCP_HEALTHY_STATUSES = new Set(['connected', 'healthy']);
type CapabilityTab = 'skills' | 'subagents' | 'mcp';

const CapabilitiesPanel: React.FC = () => {
  const { t } = useTranslation('panels/sessions');
  const { error: notifyError, success: notifySuccess } = useNotification();

  const [activeTab, setActiveTab] = useState<CapabilityTab>('skills');
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
  const [copiedPath, setCopiedPath] = useState<string | null>(null);

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) { next.delete(id); } else { next.add(id); }
      return next;
    });
  }, []);

  const load = useCallback(async (silent = false) => {
    try {
      const [skillList, subagentList, serverList] = await Promise.all([
        configAPI.getSkillConfigs(),
        SubagentAPI.listSubagents(),
        MCPAPI.getServers(),
      ]);
      setSkills(skillList);
      setSubagents(subagentList);
      setMcpServers(serverList);
    } catch (error) {
      log.error('Failed to load capabilities', error);
      if (!silent) notifyError(t('capabilities.loadFailed'));
    }
  }, [notifyError, t]);

  useEffect(() => { load(); }, [load]);

  const handleRefresh = useCallback(async () => {
    setIsRefreshing(true);
    try { await load(true); } finally { setIsRefreshing(false); }
  }, [load]);

  const handleCopyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      setCopiedPath(path);
      notifySuccess(t('capabilities.pathCopied'));
      setTimeout(() => setCopiedPath(null), 2000);
    } catch {
      notifyError(t('capabilities.pathCopyFailed'));
    }
  }, [notifySuccess, notifyError, t]);

  const handleToggleSkill = useCallback(async (skill: SkillInfo) => {
    try {
      await configAPI.setSkillEnabled(skill.name, !skill.enabled);
      await load(true);
    } catch { notifyError(t('capabilities.toggleFailed')); }
  }, [load, notifyError, t]);

  const handleToggleSubagent = useCallback(async (agent: SubagentInfo) => {
    try {
      const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
      if (isCustom) {
        await SubagentAPI.updateSubagentConfig({ subagentId: agent.id, enabled: !agent.enabled });
      } else {
        await configAPI.setSubagentConfig(agent.id, !agent.enabled);
      }
      await load(true);
    } catch { notifyError(t('capabilities.toggleFailed')); }
  }, [load, notifyError, t]);

  const handleReconnectMcp = useCallback(async (server: MCPServerInfo) => {
    try {
      if ((server.status || '').toLowerCase() === 'stopped') {
        await MCPAPI.startServer(server.id);
      } else {
        await MCPAPI.restartServer(server.id);
      }
      await load(true);
      notifySuccess(t('capabilities.mcpReconnectSuccess', { name: server.name }));
    } catch { notifyError(t('capabilities.mcpReconnectFailed', { name: server.name })); }
  }, [load, notifyError, notifySuccess, t]);

  const enabledSkills = useMemo(() => skills.filter(s => s.enabled), [skills]);
  const enabledSubagents = useMemo(() => subagents.filter(a => a.enabled), [subagents]);
  const enabledMcp = useMemo(() => mcpServers.filter(s => s.enabled), [mcpServers]);
  const unhealthyMcp = useMemo(
    () => enabledMcp.filter(s => !MCP_HEALTHY_STATUSES.has((s.status || '').toLowerCase())),
    [enabledMcp]
  );
  const hasMcpIssue = unhealthyMcp.length > 0;
  const capIndex = activeTab === 'skills' ? 0 : activeTab === 'subagents' ? 1 : 2;

  return (
    <div className="bitfun-sessions-panel">
      <PanelHeader title={t('capabilities.panelTitle')} />

      <div className="bitfun-sessions-panel__capabilities-view">
        {/* Segmented control */}
        <div className="bitfun-sessions-panel__cap-segments">
          <div
            className="bitfun-sessions-panel__cap-segments-track"
            style={{ '--cap-index': capIndex } as React.CSSProperties}
          >
            <div className="bitfun-sessions-panel__cap-segments-slider" />
            <button
              className={`bitfun-sessions-panel__cap-seg ${activeTab === 'skills' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('skills')}
            >
              <Puzzle size={12} />
              <span className="bitfun-sessions-panel__cap-seg-label">{t('capabilities.skills')}</span>
              <span className="bitfun-sessions-panel__cap-seg-count">{enabledSkills.length}/{skills.length}</span>
            </button>
            <button
              className={`bitfun-sessions-panel__cap-seg ${activeTab === 'subagents' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('subagents')}
            >
              <Bot size={12} />
              <span className="bitfun-sessions-panel__cap-seg-label">{t('capabilities.subagents')}</span>
              <span className="bitfun-sessions-panel__cap-seg-count">{enabledSubagents.length}/{subagents.length}</span>
            </button>
            <button
              className={`bitfun-sessions-panel__cap-seg ${activeTab === 'mcp' ? 'is-active' : ''} ${hasMcpIssue ? 'is-warning' : ''}`}
              onClick={() => setActiveTab('mcp')}
            >
              <Plug size={12} />
              <span className="bitfun-sessions-panel__cap-seg-label">{t('capabilities.mcp')}</span>
              <span className="bitfun-sessions-panel__cap-seg-count">{enabledMcp.length}/{mcpServers.length}</span>
              {hasMcpIssue && <span className="bitfun-sessions-panel__cap-seg-warn" />}
            </button>
          </div>
        </div>

        {hasMcpIssue && activeTab === 'mcp' && (
          <div className="bitfun-sessions-panel__cap-alert">
            <AlertTriangle size={12} />
            <span>{t('capabilities.mcpWarning', { count: unhealthyMcp.length })}</span>
          </div>
        )}

        <div className="bitfun-sessions-panel__cap-content">
          {/* Skills */}
          {activeTab === 'skills' && (
            skills.length === 0 ? (
              <div className="bitfun-sessions-panel__cap-empty">
                <span>{t('capabilities.emptySkills')}</span>
              </div>
            ) : (
              <div className="bitfun-sessions-panel__cap-cards-grid">
                {skills.map(skill => {
                  const isExpanded = expandedIds.has(`skill:${skill.name}`);
                  return (
                    <Card key={skill.name} variant="default" padding="none"
                      className={`bitfun-sessions-panel__cap-card ${!skill.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                    >
                      <div className="bitfun-sessions-panel__cap-card-header"
                        onClick={() => toggleExpanded(`skill:${skill.name}`)}>
                        <div className="bitfun-sessions-panel__cap-card-icon bitfun-sessions-panel__cap-card-icon--skill">
                          <Puzzle size={13} />
                        </div>
                        <div className="bitfun-sessions-panel__cap-card-info">
                          <span className="bitfun-sessions-panel__cap-card-name">{skill.name}</span>
                          <span className="bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--purple">{skill.level}</span>
                        </div>
                        <div className="bitfun-sessions-panel__cap-card-actions" onClick={e => e.stopPropagation()}>
                          <Switch checked={skill.enabled} onChange={() => handleToggleSkill(skill)} size="small" />
                        </div>
                      </div>
                      {isExpanded && (
                        <CardBody className="bitfun-sessions-panel__cap-card-details">
                          {skill.description && (
                            <div className="bitfun-sessions-panel__cap-card-desc">{skill.description}</div>
                          )}
                          <button className="bitfun-sessions-panel__cap-card-path"
                            onClick={() => handleCopyPath(skill.path)} title={t('capabilities.clickToCopy')}>
                            <span className="bitfun-sessions-panel__cap-card-path-label">{t('capabilities.path')}</span>
                            <span className="bitfun-sessions-panel__cap-card-path-value">{skill.path}</span>
                            <span className="bitfun-sessions-panel__cap-card-path-copy">
                              {copiedPath === skill.path ? <Check size={11} /> : <Copy size={11} />}
                            </span>
                          </button>
                        </CardBody>
                      )}
                    </Card>
                  );
                })}
              </div>
            )
          )}

          {/* Subagents */}
          {activeTab === 'subagents' && (
            subagents.length === 0 ? (
              <div className="bitfun-sessions-panel__cap-empty">
                <span>{t('capabilities.emptySubagents')}</span>
              </div>
            ) : (
              <div className="bitfun-sessions-panel__cap-cards-grid">
                {subagents.map(agent => {
                  const isExpanded = expandedIds.has(`agent:${agent.id}`);
                  return (
                    <Card key={agent.id} variant="default" padding="none"
                      className={`bitfun-sessions-panel__cap-card ${!agent.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                    >
                      <div className="bitfun-sessions-panel__cap-card-header"
                        onClick={() => toggleExpanded(`agent:${agent.id}`)}>
                        <div className="bitfun-sessions-panel__cap-card-icon bitfun-sessions-panel__cap-card-icon--agent">
                          <Bot size={13} />
                        </div>
                        <div className="bitfun-sessions-panel__cap-card-info">
                          <span className="bitfun-sessions-panel__cap-card-name">{agent.name}</span>
                          {agent.model && <span className="bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--blue">{agent.model}</span>}
                          {agent.subagentSource && <span className="bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--gray">{agent.subagentSource}</span>}
                        </div>
                        <div className="bitfun-sessions-panel__cap-card-actions" onClick={e => e.stopPropagation()}>
                          <Switch checked={agent.enabled} onChange={() => handleToggleSubagent(agent)} size="small" />
                        </div>
                      </div>
                      {isExpanded && (
                        <CardBody className="bitfun-sessions-panel__cap-card-details">
                          {agent.description && (
                            <div className="bitfun-sessions-panel__cap-card-desc">{agent.description}</div>
                          )}
                          <div className="bitfun-sessions-panel__cap-card-meta-row">
                            <span className="bitfun-sessions-panel__cap-card-path-label">{t('capabilities.toolCount')}</span>
                            <span className="bitfun-sessions-panel__cap-card-path-value">{agent.toolCount}</span>
                          </div>
                        </CardBody>
                      )}
                    </Card>
                  );
                })}
              </div>
            )
          )}

          {/* MCP Servers */}
          {activeTab === 'mcp' && (
            mcpServers.length === 0 ? (
              <div className="bitfun-sessions-panel__cap-empty">
                <span>{t('capabilities.emptyMcp')}</span>
              </div>
            ) : (
              <div className="bitfun-sessions-panel__cap-cards-grid">
                {mcpServers.map(server => {
                  const healthy = MCP_HEALTHY_STATUSES.has((server.status || '').toLowerCase());
                  const isExpanded = expandedIds.has(`mcp:${server.id}`);
                  return (
                    <Card key={server.id} variant="default" padding="none"
                      className={`bitfun-sessions-panel__cap-card ${isExpanded ? 'is-expanded' : ''} ${!healthy ? 'is-unhealthy' : ''}`}
                    >
                      <div className="bitfun-sessions-panel__cap-card-header"
                        onClick={() => toggleExpanded(`mcp:${server.id}`)}>
                        <div className={`bitfun-sessions-panel__cap-card-icon bitfun-sessions-panel__cap-card-icon--mcp ${!healthy ? 'is-error' : ''}`}>
                          <Plug size={13} />
                        </div>
                        <div className="bitfun-sessions-panel__cap-card-info">
                          <span className="bitfun-sessions-panel__cap-card-name">{server.name}</span>
                          <span className={`bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--${healthy ? 'green' : 'yellow'}`}>{server.status}</span>
                        </div>
                        {!healthy && (
                          <div className="bitfun-sessions-panel__cap-card-actions" onClick={e => e.stopPropagation()}>
                            <button className="bitfun-sessions-panel__cap-row-reconnect"
                              onClick={() => handleReconnectMcp(server)} title={t('capabilities.reconnect')}>
                              <RefreshCw size={11} />
                            </button>
                          </div>
                        )}
                      </div>
                      {isExpanded && (
                        <CardBody className="bitfun-sessions-panel__cap-card-details">
                          <div className="bitfun-sessions-panel__cap-card-meta-row">
                            <span className="bitfun-sessions-panel__cap-card-path-label">{t('capabilities.serverType')}</span>
                            <span className="bitfun-sessions-panel__cap-card-path-value">{server.serverType}</span>
                          </div>
                        </CardBody>
                      )}
                    </Card>
                  );
                })}
              </div>
            )
          )}
        </div>

        <div className="bitfun-sessions-panel__cap-footer">
          <button
            className={`bitfun-sessions-panel__cap-refresh ${isRefreshing ? 'is-spinning' : ''}`}
            onClick={handleRefresh}
            title={t('capabilities.refresh')}
          >
            <RefreshCw size={11} />
            <span>{t('capabilities.refresh')}</span>
          </button>
        </div>
      </div>
    </div>
  );
};

export default CapabilitiesPanel;
