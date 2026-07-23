// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GlobalPermissionRulesDialog } from './GlobalPermissionRulesDialog';

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

vi.mock('@/component-library', () => ({
  Modal: ({ isOpen, children }: { isOpen: boolean; children: React.ReactNode }) => (
    isOpen ? <div role="dialog">{children}</div> : null
  ),
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
  Select: ({ value, options, disabled, onChange, 'aria-label': ariaLabel }: {
    value: string;
    options: Array<{ value: string; label: string }>;
    disabled?: boolean;
    onChange?: (value: string) => void;
    'aria-label'?: string;
  }) => (
    <select
      value={value}
      aria-label={ariaLabel}
      disabled={disabled}
      onChange={(event) => onChange?.(event.target.value)}
    >
      {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
    </select>
  ),
  Input: ({ value, disabled, onChange, 'aria-label': ariaLabel }: {
    value: string;
    disabled?: boolean;
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
    'aria-label'?: string;
  }) => (
    <input value={value} aria-label={ariaLabel} disabled={disabled} onInput={onChange} />
  ),
}));

describe('GlobalPermissionRulesDialog', () => {
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
      resource.value = 'C:/trusted';
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
    await act(async () => {
      root.render(
        <GlobalPermissionRulesDialog
          isOpen
          rules={[]}
          isSaving={false}
          onSave={vi.fn(() => Promise.resolve(true))}
          onClose={vi.fn()}
        />,
      );
    });

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
  });
});
