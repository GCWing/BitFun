import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

describe('BitFun navigation icon integration', () => {
  it('uses the native extensions and compatibility icon in the expandable entry', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MainNav.tsx', import.meta.url)),
      'utf8',
    );
    const entryStart = source.indexOf('data-testid="agent-skill-entry"');
    const entryEnd = source.indexOf('</button>', entryStart);
    const entryMarkup = source.slice(entryStart, entryEnd);

    expect(entryStart).toBeGreaterThanOrEqual(0);
    expect(entryEnd).toBeGreaterThan(entryStart);
    expect(entryMarkup).toContain('NavigationExtensionsCompatibilityIcon');
    expect(entryMarkup).toContain('size={BITFUN_ICON_SIZE.navigation}');
    expect(entryMarkup).not.toContain('<Blocks');
  });

  it('uses native grouped and all-session icons in one view toggle', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./components/WorkspaceSessionGroupingToggle.tsx', import.meta.url)),
      'utf8',
    );

    expect(source).toContain('NavigationSessionViewGroupedIcon');
    expect(source).toContain('NavigationSessionViewAllIcon');
    expect(source).toContain('size={BITFUN_ICON_SIZE.navigation}');
    expect(source).toContain('data-testid="nav-workspace-session-view-toggle"');
    expect(source).not.toContain('lucide-react');
  });

  it('uses the native session-context icon in the add-group action', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MainNav.tsx', import.meta.url)),
      'utf8',
    );
    const actionStart = source.indexOf('data-testid="nav-workspace-add-btn"');
    const actionEnd = source.indexOf('</button>', actionStart);
    const actionMarkup = source.slice(actionStart, actionEnd);

    expect(actionStart).toBeGreaterThanOrEqual(0);
    expect(actionEnd).toBeGreaterThan(actionStart);
    expect(actionMarkup).toContain('NavigationSessionContextAddIcon');
    expect(actionMarkup).toContain('size={BITFUN_ICON_SIZE.compact}');
    expect(actionMarkup).not.toContain('<FolderOpen');
    expect(actionMarkup).not.toContain('<Plus');
  });

  it('switches every session-group type to its native selected icon', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./sections/workspaces/WorkspaceItem.tsx', import.meta.url)),
      'utf8',
    );
    const assistantStart = source.indexOf('bitfun-nav-panel__assistant-item-group-icon');
    const assistantEnd = source.indexOf('</span>', assistantStart);
    const assistantMarkup = source.slice(assistantStart, assistantEnd);
    const workspaceStart = source.indexOf('bitfun-nav-panel__workspace-item-icon-default');
    const workspaceEnd = source.indexOf('</span>', workspaceStart);
    const workspaceMarkup = source.slice(workspaceStart, workspaceEnd);

    expect(assistantMarkup).toContain('isActive');
    expect(assistantMarkup).toContain('SessionGroupAssistantSelectedIcon');
    expect(assistantMarkup).toContain('SessionGroupAssistantIcon');

    expect(workspaceMarkup).toContain('workspaceIsRemote');
    expect(workspaceMarkup).toContain('isActive');
    expect(workspaceMarkup).toContain('SessionGroupRemoteWorkspaceSelectedIcon');
    expect(workspaceMarkup).toContain('SessionGroupRemoteWorkspaceIcon');
    expect(workspaceMarkup).toContain('SessionGroupWorkspaceSelectedIcon');
    expect(workspaceMarkup).toContain('SessionGroupWorkspaceIcon');
  });
});
