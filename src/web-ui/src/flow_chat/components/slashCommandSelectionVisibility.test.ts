// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';
import { scrollSelectedSlashCommandIntoView } from './slashCommandSelectionVisibility';

function createCommandItem(state?: string): HTMLDivElement {
  const item = document.createElement('div');
  item.setAttribute('data-openbitfun-part', 'commandItem');
  if (state) {
    item.setAttribute('data-openbitfun-state', state);
  }
  return item;
}

describe('scrollSelectedSlashCommandIntoView', () => {
  it('keeps the selected item visible within the supplied portaled picker', () => {
    const picker = document.createElement('div');
    const unselectedItem = createCommandItem();
    const selectedItem = createCommandItem('selected');
    const scrollIntoView = vi.fn();
    selectedItem.scrollIntoView = scrollIntoView;
    picker.append(unselectedItem, selectedItem);

    scrollSelectedSlashCommandIntoView(picker);

    expect(scrollIntoView).toHaveBeenCalledOnce();
    expect(scrollIntoView).toHaveBeenCalledWith({
      behavior: 'auto',
      block: 'nearest',
      inline: 'nearest',
    });
  });

  it('does not scroll a selected item outside the supplied picker', () => {
    const picker = document.createElement('div');
    picker.append(createCommandItem());
    const outsideSelectedItem = createCommandItem('selected');
    const scrollIntoView = vi.fn();
    outsideSelectedItem.scrollIntoView = scrollIntoView;
    document.body.append(outsideSelectedItem);

    scrollSelectedSlashCommandIntoView(picker);

    expect(scrollIntoView).not.toHaveBeenCalled();
    outsideSelectedItem.remove();
  });
});
