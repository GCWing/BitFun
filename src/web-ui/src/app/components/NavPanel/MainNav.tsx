/**
 * MainNav — default workspace navigation sidebar.
 *
 * Renders WorkspaceHeader and nav sections. When a scene-nav transition
 * is active (`isDeparting=true`), every item/section receives a positional
 * CSS class relative to the anchor item (`anchorNavSceneId`):
 *   - items above the anchor  → `.is-departing-up`   (slide up + fade)
 *   - the anchor item itself  → `.is-departing-anchor` (brief highlight)
 *   - items below the anchor  → `.is-departing-down`  (slide down + fade)
 * This creates the visual "split-open from the clicked item" effect while
 * the outer Grid accordion handles the actual height collapse.
 */

import React, { useCallback, useState, useMemo } from 'react';
import { Plus } from 'lucide-react';
import { useApp } from '../../hooks/useApp';
import { useSceneManager } from '../../hooks/useSceneManager';
import { useNavSceneStore } from '../../stores/navSceneStore';
import { useSessionModeStore } from '../../stores/sessionModeStore';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { NAV_SECTIONS } from './config';
import type { PanelType } from '../../types';
import type { NavItem as NavItemConfig } from './types';
import type { SceneTabId } from '../SceneBar/types';
import NavItem from './components/NavItem';
import SectionHeader from './components/SectionHeader';
import SessionsSection from './sections/sessions/SessionsSection';
import ShellsSection from './sections/shells/ShellsSection';
import ShellHubSection from './sections/shell-hub/ShellHubSection';
import GitSection from './sections/git/GitSection';
import TeamSection from './sections/team/TeamSection';
import WorkspaceHeader from './components/WorkspaceHeader';
import { useSceneStore } from '../../stores/sceneStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { createLogger } from '@/shared/utils/logger';
import './NavPanel.scss';

const log = createLogger('MainNav');

const INLINE_SECTIONS: Partial<Record<PanelType, React.ComponentType>> = {
  sessions: SessionsSection,
  terminal: ShellsSection,
  'shell-hub': ShellHubSection,
  git: GitSection,
  team: TeamSection,
};

type DepartDir = 'up' | 'anchor' | 'down' | null;

/**
 * Build a flat ordered list of (sectionId, itemTab) tuples so we can
 * determine each element's position relative to the anchor item.
 */
function buildFlatItemOrder(): { sectionId: string; tab: PanelType; navSceneId?: SceneTabId }[] {
  const list: { sectionId: string; tab: PanelType; navSceneId?: SceneTabId }[] = [];
  for (const section of NAV_SECTIONS) {
    for (const item of section.items) {
      list.push({ sectionId: section.id, tab: item.tab, navSceneId: item.navSceneId });
    }
  }
  return list;
}

const FLAT_ITEMS = buildFlatItemOrder();

function getAnchorIndex(anchorId: SceneTabId | null): number {
  if (!anchorId) return -1;
  return FLAT_ITEMS.findIndex(i => i.navSceneId === anchorId);
}

interface MainNavProps {
  isDeparting?: boolean;
  anchorNavSceneId?: SceneTabId | null;
}

const MainNav: React.FC<MainNavProps> = ({
  isDeparting = false,
  anchorNavSceneId = null,
}) => {
  const { state, switchLeftPanelTab } = useApp();
  const { openScene } = useSceneManager();
  const openNavScene = useNavSceneStore(s => s.openNavScene);
  const activeTabId = useSceneStore(s => s.activeTabId);
  const { t } = useI18n('common');

  const activeTab = state.layout.leftPanelActiveTab;

  const anchorIdx = useMemo(() => getAnchorIndex(anchorNavSceneId), [anchorNavSceneId]);

  const getDepartDir = useCallback(
    (flatIdx: number): DepartDir => {
      if (!isDeparting) return null;
      if (anchorIdx < 0) return 'up';
      if (flatIdx < anchorIdx) return 'up';
      if (flatIdx === anchorIdx) return 'anchor';
      return 'down';
    },
    [isDeparting, anchorIdx]
  );

  const getSectionDepartDir = useCallback(
    (sectionId: string): DepartDir => {
      if (!isDeparting) return null;
      if (anchorIdx < 0) return 'up';
      const first = FLAT_ITEMS.findIndex(i => i.sectionId === sectionId);
      const last  = FLAT_ITEMS.length - 1 - [...FLAT_ITEMS].reverse().findIndex(i => i.sectionId === sectionId);
      if (last < anchorIdx) return 'up';
      if (first > anchorIdx) return 'down';
      return null;
    },
    [isDeparting, anchorIdx]
  );

  const [expandedSections, setExpandedSections] = useState<Set<string>>(() => {
    const init = new Set<string>();
    NAV_SECTIONS.forEach(s => {
      if (s.defaultExpanded !== false) init.add(s.id);
    });
    return init;
  });

  const [inlineExpanded, setInlineExpanded] = useState<Set<PanelType>>(
    () => new Set<PanelType>(['sessions'])
  );

  React.useEffect(() => {
    if (activeTabId === 'git') {
      setInlineExpanded(prev => (prev.has('git') ? prev : new Set([...prev, 'git'])));
    }
    if (activeTabId === 'team') {
      setInlineExpanded(prev => (prev.has('team') ? prev : new Set([...prev, 'team'])));
    }
  }, [activeTabId]);

  const getSectionLabel = useCallback(
    (sectionId: string, fallbackLabel: string | null) => {
      if (!fallbackLabel) return null;
      const keyMap: Record<string, string> = {
        workspace: 'nav.sections.workspace',
        'my-agent': 'nav.sections.myAgent',
        'dev-suite': 'nav.sections.devSuite',
      };
      const key = keyMap[sectionId];
      return key ? t(key) || fallbackLabel : fallbackLabel;
    },
    [t]
  );

  const toggleSection = useCallback((id: string) => {
    setExpandedSections(prev => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  }, []);

  const handleItemClick = useCallback(
    (tab: PanelType, item: NavItemConfig) => {
      if (item.inlineExpandable) {
        setInlineExpanded(prev => {
          const next = new Set(prev);
          next.has(tab) ? next.delete(tab) : next.add(tab);
          return next;
        });
        return;
      }

      if (item.behavior === 'scene' && item.sceneId) {
        openScene(item.sceneId);
      } else {
        if (item.navSceneId) {
          openNavScene(item.navSceneId);
        }
        switchLeftPanelTab(tab);
      }
    },
    [switchLeftPanelTab, openScene, openNavScene]
  );

  const sessionMode = useSessionModeStore(s => s.mode);

  const handleCreateSession = useCallback(async () => {
    if (sessionMode === 'cowork') {
      log.info('Cowork session mode is not yet available');
      return;
    }
    openScene('session');
    switchLeftPanelTab('sessions');
    try {
      await flowChatManager.createChatSession({ modelName: 'claude-sonnet-4.5' });
    } catch (err) {
      log.error('Failed to create session', err);
    }
  }, [openScene, switchLeftPanelTab, sessionMode]);

  let flatCounter = 0;

  return (
    <>
      <div className={`bitfun-nav-panel__workspace-header-slot${isDeparting ? ' is-departing-up' : ''}`}>
        <WorkspaceHeader />
      </div>

      <div className="bitfun-nav-panel__sections">
        {NAV_SECTIONS.map(section => {
          const isSectionOpen = expandedSections.has(section.id);
          const isCollapsible = !!section.collapsible;
          const showItems     = !isCollapsible || isSectionOpen;
          const sectionDir    = getSectionDepartDir(section.id);
          const sectionDepartCls = sectionDir ? ` is-departing-${sectionDir}` : '';

          return (
            <div key={section.id} className={`bitfun-nav-panel__section${sectionDepartCls}`}>
              {section.label && (
                <SectionHeader
                  label={getSectionLabel(section.id, section.label) ?? section.label}
                  collapsible={isCollapsible}
                  isOpen={isSectionOpen}
                  onToggle={() => toggleSection(section.id)}
                />
              )}

              <div className={`bitfun-nav-panel__collapsible${showItems ? '' : ' is-collapsed'}`}>
                <div className="bitfun-nav-panel__collapsible-inner">
                  <div className="bitfun-nav-panel__items">
                    {section.items.map(item => {
                      const currentFlatIdx = flatCounter++;
                      const { tab } = item;
                      const dir = getDepartDir(currentFlatIdx);
                      const isActive = item.inlineExpandable || item.navSceneId
                        ? false
                        : item.sceneId
                          ? item.sceneId === activeTabId
                          : activeTabId === 'session' && tab === activeTab;
                      const isOpen        = !!item.inlineExpandable && inlineExpanded.has(tab);
                      const InlineContent = INLINE_SECTIONS[tab];
                      const displayLabel  = item.labelKey ? t(item.labelKey) : (item.label ?? '');
                      const tooltipContent = item.tooltipKey ? t(item.tooltipKey) : undefined;
                      const departCls = dir ? ` is-departing-${dir}` : '';

                      return (
                        <React.Fragment key={tab}>
                          <div className={`bitfun-nav-panel__item-slot${departCls}`}>
                            <NavItem
                              item={item}
                              displayLabel={displayLabel}
                              tooltipContent={tooltipContent}
                              isActive={isActive}
                              isOpen={isOpen}
                              onClick={() => handleItemClick(tab, item)}
                              actionIcon={tab === 'sessions' ? Plus : undefined}
                              actionTitle={tab === 'sessions' ? t('nav.sessions.newSession') : undefined}
                              onActionClick={tab === 'sessions' ? handleCreateSession : undefined}
                            />
                          </div>
                          {InlineContent && (
                            <div className={`bitfun-nav-panel__collapsible${departCls}${isOpen ? '' : ' is-collapsed'}`}>
                              <div className="bitfun-nav-panel__collapsible-inner">
                                <InlineContent />
                              </div>
                            </div>
                          )}
                        </React.Fragment>
                      );
                    })}
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
};

export default MainNav;
