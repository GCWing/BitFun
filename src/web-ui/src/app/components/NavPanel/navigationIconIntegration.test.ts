import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

describe('navigation icon integration', () => {
  it('uses the standard extension icon in the expandable entry', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MainNav.tsx', import.meta.url)),
      'utf8',
    );
    const entryStart = source.indexOf('data-testid="agent-skill-entry"');
    const entryEnd = source.indexOf('</button>', entryStart);
    const entryMarkup = source.slice(entryStart, entryEnd);

    expect(entryStart).toBeGreaterThanOrEqual(0);
    expect(entryEnd).toBeGreaterThan(entryStart);
    expect(entryMarkup).toContain(
      'className="bitfun-nav-panel__top-action-icon-slot bitfun-nav-panel__top-action-expand-icons"',
    );
    expect(entryMarkup).toContain('name="extension"');
    expect(entryMarkup).toContain('size="sm"');
  });

  it('uses the reference clock icon for Task Board', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MainNav.tsx', import.meta.url)),
      'utf8',
    );
    const entryStart = source.indexOf('data-testid="nav-todos-btn"');
    const entryEnd = source.indexOf('</button>', entryStart);
    const entryMarkup = source.slice(entryStart, entryEnd);

    expect(entryStart).toBeGreaterThanOrEqual(0);
    expect(entryEnd).toBeGreaterThan(entryStart);
    expect(entryMarkup).toContain('<Icon name="clock" size="sm" />');
    expect(source).not.toContain('CalendarClock');
  });

  it('uses standard grouped and all-session icons in one view toggle', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./components/WorkspaceSessionGroupingToggle.tsx', import.meta.url)),
      'utf8',
    );

    expect(source).toContain("import { List, ListTree } from 'lucide-react'");
    expect(source).toContain('const ViewIcon = isAll ? List : ListTree');
    expect(source).toContain('size={16}');
    expect(source).toContain('data-session-view-icon={grouping}');
    expect(source).toContain('data-testid="nav-workspace-session-view-toggle"');
    expect(source).not.toContain('NavigationSessionView');
  });

  it('uses the standard folder-add icon in the add-group action', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MainNav.tsx', import.meta.url)),
      'utf8',
    );
    const actionStart = source.indexOf('data-testid="nav-workspace-add-btn"');
    const actionEnd = source.indexOf('</button>', actionStart);
    const actionMarkup = source.slice(actionStart, actionEnd);

    expect(actionStart).toBeGreaterThanOrEqual(0);
    expect(actionEnd).toBeGreaterThan(actionStart);
    expect(actionMarkup).toContain('<FolderPlus size={14}');
  });

  it('uses stable standard icons for every session-group type', () => {
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

    expect(assistantMarkup).toContain('<Icon name="user" size="sm" />');
    expect(assistantMarkup).not.toContain('SessionGroupAssistant');

    expect(workspaceMarkup).toContain('workspaceIsRemote');
    expect(workspaceMarkup).toContain('<Network size={16} />');
    expect(workspaceMarkup).toContain('<Icon name="folder" size="sm" />');
    expect(workspaceMarkup).not.toContain('SessionGroup');
  });
});
