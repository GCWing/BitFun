/**
 * @vitest-environment jsdom
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';

const GROUP_KEY: Record<string, 'primaryGroup'|'secondaryGroup'|'tertiaryGroup'|'slot4Group'|'slot5Group'|'slot6Group'|'slot7Group'|'slot8Group'|'slot9Group'|'slot10Group'|'slot11Group'|'slot12Group'|'slot13Group'|'slot14Group'|'slot15Group'|'slot16Group'> = {
  primary: 'primaryGroup', secondary: 'secondaryGroup', tertiary: 'tertiaryGroup',
  slot4: 'slot4Group', slot5: 'slot5Group', slot6: 'slot6Group',
  slot7: 'slot7Group', slot8: 'slot8Group', slot9: 'slot9Group',
  slot10: 'slot10Group', slot11: 'slot11Group', slot12: 'slot12Group',
  slot13: 'slot13Group', slot14: 'slot14Group', slot15: 'slot15Group',
  slot16: 'slot16Group',
};

function tabsIn(groupId: string): { title: string; id: string }[] {
  const state = useAgentCanvasStore.getState();
  return (state[GROUP_KEY[groupId]] as { tabs: { title: string; id: string }[] }).tabs;
}

function findTab(groupId: string, title: string) {
  return tabsIn(groupId).find(t => t.title === title);
}

function addTab(title: string, groupId: string) {
  useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title, data: {} }, 'active', groupId as any);
}

describe('grid9 templates', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('applyGrid9Template 2x2 sets cols/rows and splitMode', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
  });

  it('applyGrid9Template clamps to 1..GRID_MAX_DIM', () => {
    useAgentCanvasStore.getState().applyGrid9Template(9, 0);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(4);
    expect(s.layout.grid9RowsCount).toBe(1);
  });

  it('applyGrid9Template supports 4x4 and clamps to 1..4', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(4);
    expect(s.layout.grid9RowsCount).toBe(4);
    // Beyond 4 clamps to the max dimension.
    useAgentCanvasStore.getState().applyGrid9Template(7, 9);
    const s2 = useAgentCanvasStore.getState();
    expect(s2.layout.grid9ColsCount).toBe(4);
    expect(s2.layout.grid9RowsCount).toBe(4);
  });

  it('4x4 template keeps tabs in a slot inside the template', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    addTab('A', 'slot15');  // row3 col3 — inside a 4x4 template
    expect(tabsIn('slot15').some(t => t.title === 'A')).toBe(true);
    expect(useAgentCanvasStore.getState().layout.grid9ColsCount).toBe(4);
    expect(useAgentCanvasStore.getState().layout.grid9RowsCount).toBe(4);
  });

  it('applyGrid9Template moves tabs outside the template into primary (no silent drop)', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    addTab('C', 'tertiary');
    // 2x2 keeps primary/secondary + slot4/slot5; tertiary (row0 col2) is out.
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    const s = useAgentCanvasStore.getState();
    // C moved into primary (kept), tertiary reset.
    expect(tabsIn('primary').some(t => t.title === 'C')).toBe(true);
    expect(tabsIn('tertiary').length).toBe(0);
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
  });

  it('applyGrid9Template resets leftover ratios so cells tile evenly', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    // Simulate a user resizing: distort the first column ratio.
    useAgentCanvasStore.getState().setGrid9ColRatio(0, 0.6);
    expect(useAgentCanvasStore.getState().layout.grid9Cols[0]).toBe(0.6);
    // Applying a template must reset ratios to equal shares (explicit
    // re-tile control, d7-P2-7).
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9Cols[0]).toBeCloseTo(1 / 4);
    expect(s.layout.grid9Cols[1]).toBeCloseTo(1 / 4);
    expect(s.layout.grid9Rows[0]).toBeCloseTo(1 / 4);
    expect(s.layout.grid9RatiosUserAdjusted).toBe(false);
  });

  it('keeps user-adjusted ratios across edge-drop growth (d7-P2-7)', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 1);  // 2 cols x 1 row
    addTab('A', 'primary');
    useAgentCanvasStore.getState().setGrid9ColRatio(0, 0.7);
    // Grow a row via a bottom-edge drop: user shares must survive.
    const store = useAgentCanvasStore.getState();
    store.handleDrop(
      tabsIn('primary').find(t => t.title === 'A')!.id,
      'primary' as any,
      'primary' as any,
      'bottom',
    );
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(s.layout.grid9Cols[0]).toBe(0.7);
    expect(s.layout.grid9RatiosUserAdjusted).toBe(true);
  });

  it('applyGrid9Template resets activeGroupId to primary when it points outside', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    addTab('A', 'slot7');  // row1 col2 in 4x4 row-major — outside a 2x2 template
    useAgentCanvasStore.getState().setActiveGroup('slot7' as any);
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('primary');
  });

  it('applyGrid9Template keeps activeGroupId inside the template', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 3);
    addTab('A', 'secondary');
    useAgentCanvasStore.getState().setActiveGroup('secondary' as any);
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('secondary');
  });
});

describe('grid -> grid9 upgrade (existing boundary)', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  // Known existing boundary (pre-dates the 4x4 work): the grid→grid9 upgrade
  // path in handleDrop (drag onto tertiary bottom edge) places the dragged tab
  // into slot5 (row1 col0) and switches to grid9 2x2, but tabs that were
  // already living in tertiary (row0 col2 — outside the 2x2 template) stay in
  // tertiary. They are NOT dropped: the data survives in the tertiary group,
  // it is just outside the rendered template so it is not visible. This is
  // intentional (no silent data loss) and matches the 3x3-era behaviour.
  it('keeps pre-existing tertiary tabs in tertiary (outside 2x2 template, not visible, not dropped)', () => {
    // Arrange: build a grid layout (splitMode 'grid') with a tertiary tab, then
    // drag a primary tab onto the tertiary bottom edge to trigger the
    // grid→grid9 upgrade branch in handleDrop.
    const store = useAgentCanvasStore.getState();
    store.setSplitMode('grid');
    store.addTab({ type: 'markdown-viewer', title: 'T', data: {} }, 'active', 'tertiary' as any);
    store.addTab({ type: 'markdown-viewer', title: 'D', data: {} }, 'active', 'primary' as any);
    const dragged = tabsIn('primary').find(t => t.title === 'D')!;
    // Drag D onto the bottom edge of tertiary: handleDrop's grid branch
    // upgrades to grid9 2x2 and lands D in slot6 (row1 col1 — the cell below
    // tertiary, computed from GRID_MAX_DIM so it stays correct at 4x4).
    store.handleDrop(dragged.id, 'primary' as any, 'tertiary' as any, 'bottom');
    const s = useAgentCanvasStore.getState();
    // Upgrade switched to grid9 2x2.
    expect(s.layout.splitMode).toBe('grid9');
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
    // D landed in slot6 (row1 col1), inside the 2x2 template.
    expect(tabsIn('slot6').some(t => t.title === 'D')).toBe(true);
    // Existing boundary (3x3-era behaviour, unchanged): the pre-existing
    // tertiary tab T stays in tertiary. tertiary (row0 col2) is outside the
    // 2x2 template, so T is preserved but not visible in the rendered grid —
    // it is never silently dropped.
    expect(tabsIn('tertiary').some(t => t.title === 'T')).toBe(true);
  });
});

describe('mergeGrid9Cells', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('merges tabs from secondary into primary and empties secondary', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    useAgentCanvasStore.getState().mergeGrid9Cells('secondary' as any, 'primary');
    const s = useAgentCanvasStore.getState();
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('secondary').length).toBe(0);
    expect(s.activeGroupId).toBe('primary');
  });

  it('no-op when source is empty or same group', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    const before = tabsIn('primary').length;
    useAgentCanvasStore.getState().mergeGrid9Cells('secondary' as any, 'primary');
    expect(tabsIn('primary').length).toBe(before);
    useAgentCanvasStore.getState().mergeGrid9Cells('primary' as any, 'primary');
    expect(tabsIn('primary').length).toBe(before);
  });

  it('merges active tab id from source into target', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    // Make B active in secondary by switching to it.
    const tabB = findTab('secondary', 'B');
    useAgentCanvasStore.getState().switchToTab(tabB.id, 'secondary' as any);
    useAgentCanvasStore.getState().mergeGrid9Cells('secondary' as any, 'primary');
    const s = useAgentCanvasStore.getState();
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(true);
    expect(s.primaryGroup.activeTabId).toBe(tabB.id);
  });
});

describe('removeGrid9Cell', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('removing a blank middle column shifts columns left and keeps tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 2);  // 3 cols x 2 rows
    addTab('A', 'primary');
    addTab('B', 'tertiary');   // row0 col2
    // Delete blank secondary (row0 col1): column 1 removed; tertiary shifts
    // into secondary's slot; col2 (row0) becomes empty.
    useAgentCanvasStore.getState().removeGrid9Cell('secondary' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
    // Tertiary tabs (B) now live in secondary (shifted left).
    expect(tabsIn('secondary').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('tertiary').length).toBe(0);
    // Primary kept its tabs.
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });

  it('keeps user-adjusted ratios when a blank column is removed (d7-P2-7)', () => {
    useAgentCanvasStore.getState().applyGrid9Template(3, 2);  // 3 cols x 2 rows
    addTab('A', 'primary');
    addTab('B', 'tertiary');   // row0 col2
    // Distort a ratio so we can verify it is preserved after the shrink.
    useAgentCanvasStore.getState().setGrid9ColRatio(2, 0.6);
    useAgentCanvasStore.getState().removeGrid9Cell('secondary' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(2);
    expect(s.layout.grid9RowsCount).toBe(2);
    // User-adjusted share survives: the value set at col2 stays at its index
    // (the ratio array is not shifted with the cell removal), and the active
    // axis is never re-normalized while the flag is set.
    expect(s.layout.grid9Cols[2]).toBe(0.6);
    expect(s.layout.grid9Cols[0]).toBeCloseTo(1 / 4);
    expect(s.layout.grid9RatiosUserAdjusted).toBe(true);
  });

  it('removing the first column shifts everything left without losing tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'primary');
    addTab('B', 'secondary');
    // Delete blank primary column? primary has A — the delete button only
    // shows on blank cells, but the store must still behave: removing col0
    // merges A into col1 and shifts col1 into col0.
    useAgentCanvasStore.getState().removeGrid9Cell('primary' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(2);
    // A (was primary) now in primary (col0), B in slot4 (row1 col0).
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
    expect(tabsIn('primary').some(t => t.title === 'B')).toBe(true);
    expect(s.activeGroupId).toBe('primary');
  });

  it('removing a blank row shifts rows up', () => {
    useAgentCanvasStore.getState().applyGrid9Template(1, 3);  // 1 col x 3 rows
    addTab('A', 'primary');
    addTab('B', 'slot9');  // row2 col0 in 4x4 row-major
    // Delete blank slot5 (row1 col0): row 1 removed, slot9 shifts into slot5.
    useAgentCanvasStore.getState().removeGrid9Cell('slot5' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(2);
    expect(tabsIn('slot5').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('slot9').length).toBe(0);
  });

  it('removing a blank middle column on a 4x4 grid shifts columns and keeps tabs', () => {
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);  // 4 cols x 4 rows
    addTab('A', 'primary');   // row0 col0
    addTab('B', 'tertiary');  // row0 col2
    // Delete blank secondary (row0 col1): column 1 removed; tertiary shifts
    // into secondary's slot.
    useAgentCanvasStore.getState().removeGrid9Cell('secondary' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(3);
    expect(s.layout.grid9RowsCount).toBe(4);
    expect(tabsIn('secondary').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('tertiary').length).toBe(0);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });

  it('removing a blank row on a 4-row grid shifts rows up', () => {
    useAgentCanvasStore.getState().applyGrid9Template(1, 4);  // 1 col x 4 rows
    addTab('A', 'primary');
    addTab('B', 'slot13');  // row3 col0 in 4x4 row-major
    // Delete blank slot5 (row1 col0): row 1 removed; slot13 (row3) shifts up
    // two rows into slot9 (new row2 col0).
    useAgentCanvasStore.getState().removeGrid9Cell('slot5' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(3);
    expect(tabsIn('slot9').some(t => t.title === 'B')).toBe(true);
    expect(tabsIn('slot13').length).toBe(0);
  });

  it('does nothing on a 1x1 grid (mirror of canRemoveCell)', () => {
    useAgentCanvasStore.getState().applyGrid9Template(1, 1);
    addTab('A', 'primary');
    useAgentCanvasStore.getState().removeGrid9Cell('primary' as any);
    const s = useAgentCanvasStore.getState();
    expect(s.layout.grid9ColsCount).toBe(1);
    expect(s.layout.grid9RowsCount).toBe(1);
    expect(tabsIn('primary').some(t => t.title === 'A')).toBe(true);
  });

  it('fixes activeGroupId when the active cell is removed', () => {
    useAgentCanvasStore.getState().applyGrid9Template(2, 2);
    addTab('A', 'secondary');
    useAgentCanvasStore.getState().setActiveGroup('secondary' as any);
    useAgentCanvasStore.getState().removeGrid9Cell('secondary' as any);
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('primary');
  });
});

describe('closeAllTabs (no-arg) clears all 16 groups', () => {
  beforeEach(() => {
    useAgentCanvasStore.getState().reset();
  });

  it('empties every group slot (primary..slot16) while keeping pinned tabs', () => {
    // Grid9 keeps all 16 slots addressable; seed one tab per group.
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    const seed = [
      'primary', 'secondary', 'tertiary',
      'slot4', 'slot5', 'slot6', 'slot7', 'slot8', 'slot9',
      'slot10', 'slot11', 'slot12', 'slot13', 'slot14', 'slot15', 'slot16',
    ];
    seed.forEach((gid, i) => addTab(`tab-${i}`, gid));

    // Ensure every group has a tab pre-close.
    seed.forEach(gid => expect(tabsIn(gid).length).toBe(1));

    useAgentCanvasStore.getState().closeAllTabs();

    // All 16 groups must be emptied (keepPinnedTabsOnly keeps pinned tabs,
    // and none of the seeded tabs are pinned — so they all close).
    seed.forEach(gid => expect(tabsIn(gid).length).toBe(0));
  });

  it('keeps pinned tabs in every group, not only slots 4-9', () => {
    // Grid9 keeps all 16 slots addressable. Seed pinned tabs in slot10 and
    // slot16 (the previously-hardcoded loop missed these) plus an unpinned
    // tab that must be cleared. When p/s/t are all empty, closeAllTabs
    // collects surviving pinned tabs into primary before resetting the grid.
    useAgentCanvasStore.getState().applyGrid9Template(4, 4);
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'P10', data: {} }, 'pinned', 'slot10' as any);
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'U10', data: {} }, 'preview', 'slot10' as any);
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'P16', data: {} }, 'pinned', 'slot16' as any);
    useAgentCanvasStore.getState().addTab({ type: 'markdown-viewer', title: 'U16', data: {} }, 'preview', 'slot16' as any);

    useAgentCanvasStore.getState().closeAllTabs();

    // Unpinned tabs cleared everywhere.
    expect(tabsIn('slot10').some(t => t.title === 'U10')).toBe(false);
    expect(tabsIn('slot16').some(t => t.title === 'U16')).toBe(false);
    // Pinned tabs from every group (incl. slot10/slot16) survive in primary.
    expect(tabsIn('primary').some(t => t.title === 'P10')).toBe(true);
    expect(tabsIn('primary').some(t => t.title === 'P16')).toBe(true);
    // Grid collapsed to single column with pinned tabs.
    expect(useAgentCanvasStore.getState().layout.splitMode).toBe('none');
    expect(useAgentCanvasStore.getState().activeGroupId).toBe('primary');
  });
});
