// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useToolCardHeightContract } from './useToolCardHeightContract';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function Harness({ expanded }: { expanded: boolean }) {
  const [isExpanded, setIsExpanded] = React.useState(true);
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId: 'tool-1',
    toolName: 'Write',
  });
  React.useEffect(() => {
    applyExpandedState(isExpanded, expanded, setIsExpanded);
  }, [applyExpandedState, expanded, isExpanded]);
  return <div ref={cardRootRef} data-expanded={String(isExpanded)} />;
}

describe('useToolCardHeightContract', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('notifies Virtuoso after an expanded-state change', () => {
    const handleToggle = vi.fn();
    window.addEventListener('tool-card-toggle', handleToggle);
    act(() => root.render(<Harness expanded />));
    act(() => root.render(<Harness expanded={false} />));
    expect(handleToggle).toHaveBeenCalledOnce();
    expect(container.firstElementChild?.getAttribute('data-expanded')).toBe('false');
    window.removeEventListener('tool-card-toggle', handleToggle);
  });

  it('does not emit a pre-collapse viewport compensation event', () => {
    const handleCollapseIntent = vi.fn();
    window.addEventListener('flowchat:tool-card-collapse-intent', handleCollapseIntent);
    act(() => root.render(<Harness expanded />));
    act(() => root.render(<Harness expanded={false} />));
    expect(handleCollapseIntent).not.toHaveBeenCalled();
    window.removeEventListener('flowchat:tool-card-collapse-intent', handleCollapseIntent);
  });
});
