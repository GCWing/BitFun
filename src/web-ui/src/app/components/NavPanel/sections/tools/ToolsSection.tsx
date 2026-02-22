/**
 * ToolsSection — inline sub-list under the "Tools" nav item.
 * Shows two categories: built-in tools (with count) and MCP services (real server list).
 * Clicking opens the Capabilities scene → mcp view.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Wrench, Plug } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useCapabilitiesSceneStore } from '../../../../scenes/capabilities/capabilitiesSceneStore';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';

const MCP_HEALTHY_STATUSES = new Set(['connected', 'healthy']);

const ToolsSection: React.FC = () => {
  const { t } = useI18n('common');
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const openScene = useSceneStore((s) => s.openScene);
  const activeView = useCapabilitiesSceneStore((s) => s.activeView);
  const setActiveView = useCapabilitiesSceneStore((s) => s.setActiveView);

  const [builtinToolCount, setBuiltinToolCount] = useState(0);
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);

  const load = useCallback(async () => {
    try {
      const [tools, servers] = await Promise.all([
        SubagentAPI.listAgentToolNames(),
        MCPAPI.getServers(),
      ]);
      setBuiltinToolCount(tools.length);
      setMcpServers(servers);
    } catch {
      // silent
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleClick = useCallback(() => {
    openScene('capabilities');
    setActiveView('mcp');
  }, [openScene, setActiveView]);

  const isActive = activeTabId === 'capabilities' && activeView === 'mcp';

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--tools">
      {/* Built-in tools summary */}
      <Tooltip content={`${t('nav.tools.builtinTools')} (${builtinToolCount})`} placement="right" followCursor>
        <button
          type="button"
          className={['bitfun-nav-panel__inline-item', isActive && 'is-active'].filter(Boolean).join(' ')}
          onClick={handleClick}
        >
          <Wrench size={12} className="bitfun-nav-panel__inline-item-icon" aria-hidden />
          <span className="bitfun-nav-panel__inline-item-label">{t('nav.tools.builtinTools')}</span>
          <span className="bitfun-nav-panel__inline-item-badge">{builtinToolCount}</span>
        </button>
      </Tooltip>

      {/* MCP servers — one row per server */}
      {mcpServers.map((server) => {
        const healthy = MCP_HEALTHY_STATUSES.has((server.status || '').toLowerCase());
        const statusText = server.status || 'Unknown';
        const tooltipText = `${server.name} — ${statusText}`;
        return (
          <Tooltip key={server.id} content={tooltipText} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__inline-item',
                isActive && 'is-active',
                !healthy && 'is-unhealthy',
              ].filter(Boolean).join(' ')}
              onClick={handleClick}
            >
              <Plug size={12} className="bitfun-nav-panel__inline-item-icon" aria-hidden />
              <span className="bitfun-nav-panel__inline-item-label">{server.name}</span>
              <span className={`bitfun-nav-panel__inline-item-dot ${healthy ? 'is-healthy' : 'is-error'}`} />
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
};

export default ToolsSection;
