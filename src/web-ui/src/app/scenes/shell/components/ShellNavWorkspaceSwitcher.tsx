import React from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';
import { Tooltip } from '@/component-library/components/Tooltip';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';

interface ShellNavWorkspaceSwitcherProps {
  workspaceName?: string;
  hasMultipleWorkspaces: boolean;
  workspaceMenuOpen: boolean;
  workspaceMenuPosition: { top: number; left: number } | null;
  openedWorkspacesList: WorkspaceInfo[];
  activeWorkspaceId?: string;
  workspaceMenuRef: React.RefObject<HTMLDivElement>;
  workspaceTriggerRef: React.RefObject<HTMLButtonElement>;
  switchWorkspaceLabel: string;
  onToggle: () => void;
  onSelectWorkspace: (workspaceId: string) => Promise<void>;
}

function getWorkspaceDisplayName(workspace: WorkspaceInfo): string {
  return workspace.workspaceKind === WorkspaceKind.Assistant
    ? workspace.identity?.name?.trim() || workspace.name
    : workspace.name;
}

const ShellNavWorkspaceSwitcher: React.FC<ShellNavWorkspaceSwitcherProps> = ({
  workspaceName,
  hasMultipleWorkspaces,
  workspaceMenuOpen,
  workspaceMenuPosition,
  openedWorkspacesList,
  activeWorkspaceId,
  workspaceMenuRef,
  workspaceTriggerRef,
  switchWorkspaceLabel,
  onToggle,
  onSelectWorkspace,
}) => {
  if (!workspaceName) {
    return null;
  }

  return (
    <div className="bitfun-shell-nav__workspace-switcher">
      <Tooltip
        content={hasMultipleWorkspaces ? switchWorkspaceLabel : workspaceName}
        placement="bottom"
      >
        <button
          ref={workspaceTriggerRef}
          type="button"
          className={`bitfun-shell-nav__workspace-trigger${workspaceMenuOpen ? ' is-active' : ''}${hasMultipleWorkspaces ? ' is-switchable' : ''}`}
          onClick={onToggle}
          aria-haspopup={hasMultipleWorkspaces ? 'menu' : undefined}
          aria-expanded={hasMultipleWorkspaces ? workspaceMenuOpen : undefined}
        >
          <span className="bitfun-shell-nav__workspace-separator">/</span>
          <span className="bitfun-shell-nav__workspace-name">{workspaceName}</span>
          {hasMultipleWorkspaces ? (
            <ChevronDown size={12} className="bitfun-shell-nav__workspace-trigger-icon" />
          ) : null}
        </button>
      </Tooltip>

      {workspaceMenuOpen && hasMultipleWorkspaces && workspaceMenuPosition
        ? createPortal(
            <div
              ref={workspaceMenuRef}
              className="bitfun-shell-nav__workspace-menu"
              role="menu"
              aria-label={switchWorkspaceLabel}
              style={{
                top: `${workspaceMenuPosition.top}px`,
                left: `${workspaceMenuPosition.left}px`,
              }}
            >
              {openedWorkspacesList.map((workspace) => {
                const isActive = workspace.id === activeWorkspaceId;
                const label = getWorkspaceDisplayName(workspace);

                return (
                  <Tooltip
                    key={workspace.id}
                    content={workspace.rootPath}
                    placement="right"
                    disabled={!workspace.rootPath}
                  >
                    <button
                      type="button"
                      role="menuitemradio"
                      aria-checked={isActive}
                      className={`bitfun-shell-nav__workspace-menu-item${isActive ? ' is-active' : ''}`}
                      onClick={() => { void onSelectWorkspace(workspace.id); }}
                    >
                      <span className="bitfun-shell-nav__workspace-menu-check" aria-hidden="true">
                        {isActive ? <Check size={12} /> : null}
                      </span>
                      <span className="bitfun-shell-nav__workspace-menu-text">{label}</span>
                    </button>
                  </Tooltip>
                );
              })}
            </div>,
            document.body,
          )
        : null}
    </div>
  );
};

export default ShellNavWorkspaceSwitcher;
