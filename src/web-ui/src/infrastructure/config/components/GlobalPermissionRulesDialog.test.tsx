// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GlobalPermissionRulesDialog } from './GlobalPermissionRulesDialog';
import {
  discardAndContinueSettingsNavigation,
  getSettingsDraftSnapshot,
  requestSettingsDraftExit,
  resetSettingsDraftRegistryForTests,
} from '@/infrastructure/config/settingsDraftRegistry';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const TRANSLATIONS: Record<string, string> = {
  'permissionPolicy.globalRulesDialogTitle': 'Global tool permission rules',
  'permissionPolicy.globalRulesDialogDescription': 'Apply to every workspace',
  'permissionPolicy.globalRulesTitle': 'Custom global rules',
  'permissionPolicy.globalRulesEmpty': 'No custom global rules configured.',
  'permissionPolicy.addGlobalRule': 'Add rule',
  'permissionPolicy.globalRulesEffect': 'Effect',
  'permissionPolicy.globalRulesAction': 'Action',
  'permissionPolicy.globalRulesResource': 'Resource',
  'permissionPolicy.globalRulesResourcePlaceholder': 'For example, C:/workspace/*',
  'permissionPolicy.moveGlobalRuleUp': 'Move rule up',
  'permissionPolicy.moveGlobalRuleDown': 'Move rule down',
  'permissionPolicy.removeGlobalRule': 'Remove rule',
  'permissionPolicy.discardGlobalRules': 'Discard changes',
  'permissionPolicy.saveGlobalRules': 'Save rules',
  'permissionPolicy.globalRulesEffects.allow': 'Allow',
  'permissionPolicy.globalRulesEffects.ask': 'Ask',
  'permissionPolicy.globalRulesEffects.deny': 'Deny',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => TRANSLATIONS[key] ?? key,
  }),
}));

vi.mock('@openbitfun/ui', () => ({
  Icon: ({ name, ...props }: { name: string } & React.HTMLAttributes<HTMLSpanElement>) => <span data-icon={name} {...props} />,
  FormSection: ({
    children,
    title,
    actions,
    ...props
  }: React.HTMLAttributes<HTMLElement> & { title?: React.ReactNode; actions?: React.ReactNode }) => (
    <section {...props}>{title}{actions}{children}</section>
  ),
  FieldGroup: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  Tooltip: ({ children }: React.PropsWithChildren) => <>{children}</>,
  Dialog: ({ open, children }: { open: boolean; children: React.ReactNode }) => (
    open ? <div role="dialog">{children}</div> : null
  ),
  DialogBody: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogClose: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button type="button" {...props} />,
  DialogHeader: ({ children }: React.PropsWithChildren) => <header>{children}</header>,
  DialogHeading: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DialogTitle: ({ children }: React.PropsWithChildren) => <h2>{children}</h2>,
  Button: ({ children, disabled, onClick }: {
    children: React.ReactNode;
    disabled?: boolean;
    onClick?: () => void;
  }) => (
    <button type="button" disabled={disabled} onClick={onClick}>{children}</button>
  ),
  IconButton: ({ children, disabled, onClick, 'aria-label': ariaLabel }: {
    children: React.ReactNode;
    disabled?: boolean;
    onClick?: () => void;
    'aria-label'?: string;
  }) => (
    <button type="button" aria-label={ariaLabel} disabled={disabled} onClick={onClick}>{children}</button>
  ),
  Input: ({ value, disabled, onChange, 'aria-label': ariaLabel }: {
    value: string;
    disabled?: boolean;
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
    'aria-label'?: string;
  }) => (
    <input value={value} aria-label={ariaLabel} disabled={disabled} onInput={onChange} />
  ),
  Select: ({ value, options, disabled, onValueChange, 'aria-label': ariaLabel }: {
    value: string;
    options: Array<{ value: string; label: string }>;
    disabled?: boolean;
    onValueChange?: (value: string) => void;
    'aria-label'?: string;
  }) => (
    <select
      value={value}
      aria-label={ariaLabel}
      disabled={disabled}
      onChange={(event) => onValueChange?.(event.target.value)}
    >
      {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
    </select>
  ),
}));

describe('GlobalPermissionRulesDialog', () => {
  let container: HTMLDivElement;
  let root: Root;
  let animateMock: ReturnType<typeof vi.fn>;

  const mockReducedMotion = (matches: boolean) => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({
      matches,
      media: '(prefers-reduced-motion: reduce)',
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })));
  };

  const renderDialog = async (
    rules: Array<{ action: string; resource: string; effect: 'allow' | 'ask' | 'deny' }>,
    overrides: Partial<React.ComponentProps<typeof GlobalPermissionRulesDialog>> = {},
  ) => {
    await act(async () => {
      root.render(
        <GlobalPermissionRulesDialog
          isOpen
          rules={rules}
          isSaving={false}
          onSave={vi.fn(() => Promise.resolve(true))}
          onClose={vi.fn()}
          {...overrides}
        />,
      );
    });
  };

  beforeEach(() => {
    resetSettingsDraftRegistryForTests();
    mockReducedMotion(false);
    animateMock = vi.fn(() => ({
      cancel: vi.fn(),
      onfinish: null,
      oncancel: null,
    }) as unknown as Animation);
    Object.defineProperty(HTMLElement.prototype, 'animate', {
      configurable: true,
      writable: true,
      value: animateMock,
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    resetSettingsDraftRegistryForTests();
    vi.useRealTimers();
    vi.unstubAllGlobals();
    delete (HTMLElement.prototype as Partial<HTMLElement>).animate;
  });

  it('edits and saves ordered global permission rules', async () => {
    const onSave = vi.fn(() => Promise.resolve(true));
    await act(async () => {
      root.render(
        <GlobalPermissionRulesDialog
          isOpen
          rules={[{ action: 'read', resource: 'C:/external/*', effect: 'allow' }]}
          isSaving={false}
          onSave={onSave}
          onClose={vi.fn()}
        />,
      );
    });

    const [effect, action] = [...container.querySelectorAll('select')];
    const resource = container.querySelector('input[aria-label="Resource"]') as HTMLInputElement;
    await act(async () => {
      effect.value = 'deny';
      effect.dispatchEvent(new Event('change', { bubbles: true }));
      action.value = 'external_directory';
      action.dispatchEvent(new Event('change', { bubbles: true }));
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')
        ?.set?.call(resource, 'C:/trusted');
      resource.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    const saveButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('Save rules'),
    );
    expect(saveButton).toBeDefined();
    await act(async () => {
      saveButton?.click();
      await Promise.resolve();
    });

    expect(onSave).toHaveBeenCalledWith([
      { action: 'external_directory', resource: 'C:/trusted', effect: 'deny' },
    ]);
  });

  it('adds an editable rule and prevents saving it until complete', async () => {
    await renderDialog([]);

    const addButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('Add rule'),
    );
    await act(async () => {
      addButton?.click();
    });

    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(1);
    const saveButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('Save rules'),
    );
    expect(saveButton?.disabled).toBe(true);

    expect(animateMock).toHaveBeenCalledTimes(1);
    expect(animateMock.mock.calls[0]?.[0]).toEqual([
      { opacity: 0, transform: 'translateY(4px)' },
      { opacity: 1, transform: 'translateY(0)' },
    ]);
    expect(animateMock.mock.calls[0]?.[1]).toMatchObject({ duration: 150 });
  });

  it('reorders rows immediately and bridges their changed positions with FLIP', async () => {
    await renderDialog([
      { action: 'read', resource: '/first', effect: 'allow' },
      { action: 'git', resource: '/second', effect: 'ask' },
    ]);

    const rows = [...container.querySelectorAll<HTMLDivElement>(
      '.global-permission-rules-dialog__rule-row',
    )];
    rows.forEach((row) => {
      vi.spyOn(row, 'getBoundingClientRect').mockImplementation(() => {
        const currentRows = [...container.querySelectorAll(
          '.global-permission-rules-dialog__rule-row',
        )];
        const index = currentRows.indexOf(row);
        return {
          x: 0,
          y: index * 40,
          top: index * 40,
          right: 400,
          bottom: (index * 40) + 32,
          left: 0,
          width: 400,
          height: 32,
          toJSON: () => ({}),
        };
      });
    });

    const firstMoveDown = container.querySelectorAll<HTMLButtonElement>(
      'button[aria-label="Move rule down"]',
    )[0];
    await act(async () => {
      firstMoveDown.click();
    });

    expect([...container.querySelectorAll<HTMLSelectElement>('select[aria-label="Action"]')]
      .map((select) => select.value)).toEqual(['git', 'read']);
    expect(animateMock).toHaveBeenCalledTimes(2);
    expect(animateMock.mock.calls.every((call) => (
      (call[1] as KeyframeAnimationOptions).duration === 160
    ))).toBe(true);
    expect(animateMock.mock.calls.map((call) => call[0])).toEqual(expect.arrayContaining([
      [
        { transform: 'translate(0px, -40px)' },
        { transform: 'translate(0, 0)' },
      ],
      [
        { transform: 'translate(0px, 40px)' },
        { transform: 'translate(0, 0)' },
      ],
    ]));

    const firstAnimations = animateMock.mock.results.map(
      (result) => result.value as Animation & { cancel: ReturnType<typeof vi.fn> },
    );
    const originalRuleMoveUp = container.querySelectorAll<HTMLButtonElement>(
      'button[aria-label="Move rule up"]',
    )[1];
    await act(async () => {
      originalRuleMoveUp.click();
    });

    expect(firstAnimations.every((animation) => animation.cancel.mock.calls.length === 1)).toBe(true);
    expect([...container.querySelectorAll<HTMLSelectElement>('select[aria-label="Action"]')]
      .map((select) => select.value)).toEqual(['read', 'git']);
  });

  it('retains a removed row for its exit but saves the intended live rules', async () => {
    vi.useFakeTimers();
    const onSave = vi.fn(() => Promise.resolve(true));
    await renderDialog([
      { action: 'read', resource: '/remove', effect: 'allow' },
      { action: 'git', resource: '/keep', effect: 'ask' },
    ], { onSave });

    const firstRemove = container.querySelectorAll<HTMLButtonElement>(
      'button[aria-label="Remove rule"]',
    )[0];
    await act(async () => {
      firstRemove.focus();
      firstRemove.click();
    });

    const exitingRow = container.querySelector<HTMLDivElement>(
      '.global-permission-rules-dialog__rule-row[data-exiting="true"]',
    );
    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(2);
    expect(exitingRow?.getAttribute('aria-hidden')).toBe('true');
    expect(exitingRow?.hasAttribute('inert')).toBe(true);
    expect(exitingRow?.querySelector('input')?.disabled).toBe(true);
    expect(document.activeElement).not.toBe(firstRemove);

    const saveButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('Save rules'),
    );
    await act(async () => {
      saveButton?.click();
      await Promise.resolve();
    });
    expect(onSave).toHaveBeenCalledWith([
      { action: 'git', resource: '/keep', effect: 'ask' },
    ]);

    await act(async () => {
      vi.advanceTimersByTime(119);
    });
    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(2);
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(1);
    expect(container.querySelector<HTMLSelectElement>('select[aria-label="Action"]')?.value).toBe('git');
  });

  it('cancels an old removal when the dialog closes and opens with new rules', async () => {
    vi.useFakeTimers();
    const commonProps = {
      isSaving: false,
      onSave: vi.fn(() => Promise.resolve(true)),
      onClose: vi.fn(),
    };
    await renderDialog([
      { action: 'read', resource: '/old', effect: 'allow' },
    ], commonProps);

    await act(async () => {
      container.querySelector<HTMLButtonElement>('button[aria-label="Remove rule"]')?.click();
    });
    await act(async () => {
      root.render(
        <GlobalPermissionRulesDialog
          {...commonProps}
          isOpen={false}
          rules={[{ action: 'read', resource: '/old', effect: 'allow' }]}
        />,
      );
    });
    await act(async () => {
      root.render(
        <GlobalPermissionRulesDialog
          {...commonProps}
          isOpen
          rules={[{ action: 'git', resource: '/new', effect: 'deny' }]}
        />,
      );
    });
    await act(async () => {
      vi.advanceTimersByTime(200);
    });

    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(1);
    expect(container.querySelector<HTMLSelectElement>('select[aria-label="Action"]')?.value).toBe('git');
    expect(container.querySelector<HTMLInputElement>('input[aria-label="Resource"]')?.value).toBe('/new');
  });

  it('handles rapid removals without reviving or skipping a rule', async () => {
    vi.useFakeTimers();
    await renderDialog([
      { action: 'read', resource: '/first', effect: 'allow' },
      { action: 'git', resource: '/second', effect: 'ask' },
      { action: 'bash', resource: '/keep', effect: 'deny' },
    ]);

    const removeButtons = container.querySelectorAll<HTMLButtonElement>(
      'button[aria-label="Remove rule"]',
    );
    await act(async () => {
      removeButtons[0].click();
      removeButtons[1].click();
    });
    expect(container.querySelectorAll(
      '.global-permission-rules-dialog__rule-row[data-exiting="true"]',
    )).toHaveLength(2);

    await act(async () => {
      vi.advanceTimersByTime(120);
    });
    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(1);
    expect(container.querySelector<HTMLSelectElement>('select[aria-label="Action"]')?.value).toBe('bash');
  });

  it('uses opacity-only short feedback when reduced motion is requested', async () => {
    vi.useFakeTimers();
    mockReducedMotion(true);
    await renderDialog([]);

    const addButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('Add rule'),
    );
    await act(async () => {
      addButton?.click();
    });

    expect(animateMock).toHaveBeenCalledTimes(1);
    expect(animateMock.mock.calls[0]?.[0]).toEqual([{ opacity: 0 }, { opacity: 1 }]);
    expect(animateMock.mock.calls[0]?.[1]).toMatchObject({ duration: 70 });

    await act(async () => {
      container.querySelector<HTMLButtonElement>('button[aria-label="Remove rule"]')?.click();
      vi.advanceTimersByTime(59);
    });
    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(1);
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(container.querySelectorAll('.global-permission-rules-dialog__rule-row')).toHaveLength(0);
  });

  it('publishes dialog edits to the shared close guard and discards before closing', async () => {
    const onClose = vi.fn();
    await renderDialog([], { onClose });
    const addButton = [...container.querySelectorAll('button')].find(
      (button) => button.textContent?.includes('Add rule'),
    );
    await act(async () => {
      addButton?.click();
    });

    expect(requestSettingsDraftExit(['global-permission-rules'], onClose)).toBe(false);
    expect(getSettingsDraftSnapshot().pendingNavigation?.resourceLabels).toEqual([
      'Global tool permission rules',
    ]);

    await act(async () => {
      await discardAndContinueSettingsNavigation();
    });
    expect(onClose).toHaveBeenCalledOnce();
  });
});
