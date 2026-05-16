import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RemediationSelectionPanel } from './RemediationSelectionPanel';
import type { ReviewRemediationItem } from '../../utils/codeReviewRemediation';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => {
      const template = options?.defaultValue ?? _key;
      return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
  }) => <button type="button" disabled={disabled}>{children}</button>,
  Checkbox: ({
    checked,
    className,
    disabled,
    indeterminate,
    label,
    onChange,
  }: {
    checked?: boolean;
    className?: string;
    disabled?: boolean;
    indeterminate?: boolean;
    label?: React.ReactNode;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
  }) => (
    <label className={className}>
      <input
        type="checkbox"
        aria-checked={indeterminate ? 'mixed' : checked ? 'true' : 'false'}
        checked={checked}
        disabled={disabled}
        onChange={onChange}
        readOnly={!onChange}
      />
      {label}
    </label>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
  },
}));

const baseProps = {
  showRemediationList: true,
  onToggleRemediation: vi.fn(),
  onToggleAll: vi.fn(),
  onToggleGroup: vi.fn(),
  onToggleList: vi.fn(),
  onToggleDecisionExpansion: vi.fn(),
  onSetDecisionSelection: vi.fn(),
};

let JSDOMCtor: (new (
  html?: string,
  options?: { pretendToBeVisual?: boolean; url?: string }
) => { window: Window & typeof globalThis }) | null = null;

try {
  const jsdom = await import('jsdom');
  JSDOMCtor = jsdom.JSDOM as typeof JSDOMCtor;
} catch {
  JSDOMCtor = null;
}

const describeWithJsdom = JSDOMCtor ? describe : describe.skip;

describe('RemediationSelectionPanel', () => {
  it('renders grouped remediation counts and the empty-selection hint', () => {
    const remediationItems: ReviewRemediationItem[] = [
      {
        id: 'must-fix-1',
        index: 0,
        groupIndex: 0,
        plan: 'Fix critical issue',
        issueIndex: 0,
        groupId: 'must_fix',
        defaultSelected: true,
      },
    ];

    const html = renderToStaticMarkup(
      <RemediationSelectionPanel
        {...baseProps}
        remediationItems={remediationItems}
        selectedRemediationIds={new Set()}
        completedRemediationIds={new Set()}
        decisionSelections={{}}
        expandedDecisionIds={new Set()}
      />,
    );

    expect(html).toContain('0/1 selected');
    expect(html).toContain('must_fix');
    expect(html).toContain('0/1');
    expect(html).toContain('Select at least one remediation item to start fixing.');
  });

  it('renders completed and expanded decision remediation items', () => {
    const remediationItems: ReviewRemediationItem[] = [
      {
        id: 'decision-1',
        index: 0,
        groupIndex: 0,
        plan: 'Choose a migration strategy',
        issueIndex: 0,
        groupId: 'needs_decision',
        requiresDecision: true,
        decisionContext: {
          question: 'Which migration strategy should we use?',
          tradeoffs: 'Fast path is risky; staged path is safer.',
          options: ['Fast path', 'Staged path'],
          recommendation: 1,
        },
        defaultSelected: true,
      },
    ];

    const html = renderToStaticMarkup(
      <RemediationSelectionPanel
        {...baseProps}
        remediationItems={remediationItems}
        selectedRemediationIds={new Set(['decision-1'])}
        completedRemediationIds={new Set(['decision-1'])}
        decisionSelections={{ 'decision-1': 1 }}
        expandedDecisionIds={new Set(['decision-1'])}
      />,
    );

    expect(html).toContain('Decision');
    expect(html).toContain('Which migration strategy should we use?');
    expect(html).toContain('Fast path is risky; staged path is safer.');
    expect(html).toContain('Staged path (recommended)');
    expect(html).toContain('deep-review-action-bar__remediation-item--completed');
  });
});

describeWithJsdom('RemediationSelectionPanel interactions', () => {
  let dom: { window: Window & typeof globalThis };
  let container: HTMLDivElement;
  let root: Root;

  function mount(element: React.ReactElement): void {
    act(() => {
      root.render(element);
    });
  }

  function remediationItems(): ReviewRemediationItem[] {
    return [
      {
        id: 'remediation-should-improve-1',
        index: 0,
        groupIndex: 0,
        plan: 'Improve error copy',
        issueIndex: -1,
        groupId: 'should_improve',
        defaultSelected: false,
      },
      {
        id: 'remediation-should-improve-2',
        index: 1,
        groupIndex: 1,
        plan: 'Improve retry state',
        issueIndex: -1,
        groupId: 'should_improve',
        defaultSelected: false,
      },
    ];
  }

  beforeEach(() => {
    dom = new JSDOMCtor!('<!doctype html><html><body></body></html>', {
      pretendToBeVisual: true,
      url: 'http://localhost',
    });

    const { window } = dom;
    vi.stubGlobal('window', window);
    vi.stubGlobal('document', window.document);
    vi.stubGlobal('navigator', window.navigator);
    vi.stubGlobal('HTMLElement', window.HTMLElement);
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    dom.window.close();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('toggles a remediation group once when clicking the root checkbox', () => {
    const onToggleGroup = vi.fn();

    mount(
      <RemediationSelectionPanel
        {...baseProps}
        remediationItems={remediationItems()}
        selectedRemediationIds={new Set()}
        completedRemediationIds={new Set()}
        decisionSelections={{}}
        expandedDecisionIds={new Set()}
        onToggleGroup={onToggleGroup}
      />,
    );

    const groupCheckbox = container.querySelector<HTMLInputElement>(
      '.deep-review-action-bar__remediation-group-header input[type="checkbox"]',
    );
    expect(groupCheckbox).toBeTruthy();

    act(() => {
      groupCheckbox!.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onToggleGroup).toHaveBeenCalledTimes(1);
    expect(onToggleGroup).toHaveBeenCalledWith('should_improve');
  });

  it('keeps the tree visible but disables selection while fixing', () => {
    const onToggleRemediation = vi.fn();
    const onToggleGroup = vi.fn();

    mount(
      <RemediationSelectionPanel
        {...baseProps}
        remediationItems={remediationItems()}
        selectedRemediationIds={new Set(['remediation-should-improve-1'])}
        completedRemediationIds={new Set()}
        fixingRemediationIds={new Set(['remediation-should-improve-1'])}
        decisionSelections={{}}
        expandedDecisionIds={new Set()}
        selectionDisabled
        onToggleRemediation={onToggleRemediation}
        onToggleGroup={onToggleGroup}
      />,
    );

    expect(container.textContent).toContain('Improve error copy');
    expect(container.textContent).toContain('Fixing');
    const checkboxes = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'));
    expect(checkboxes.length).toBeGreaterThan(0);
    expect(checkboxes.every((checkbox) => checkbox.disabled)).toBe(true);

    act(() => {
      checkboxes[checkboxes.length - 1].dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(onToggleRemediation).not.toHaveBeenCalled();
    expect(onToggleGroup).not.toHaveBeenCalled();
  });
});
