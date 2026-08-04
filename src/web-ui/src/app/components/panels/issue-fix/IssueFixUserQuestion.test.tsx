// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  Button: ({ children, isLoading: _isLoading, ...props }: any) => (
    <button type="button" {...props}>{children}</button>
  ),
  Textarea: ({ label, className, autoResize: _autoResize, ...props }: any) => (
    <label className={className}>{label}<textarea {...props} /></label>
  ),
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

import { IssueFixUserQuestion } from './IssueFixUserQuestion';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('IssueFixUserQuestion', () => {
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

  it('submits a typed LoopX decision instead of a chat answer', () => {
    const onSubmit = vi.fn();
    act(() => {
      root.render(
        <IssueFixUserQuestion
          question={{ todoId: 'gate-1849', prompt: 'Open the validated PR?' }}
          submitting={false}
          onSubmit={onSubmit}
        />,
      );
    });

    const approve = container.querySelector<HTMLInputElement>('input[value="approve"]');
    const submit = container.querySelector<HTMLButtonElement>('.issue-fix__user-question-submit');
    expect(approve).not.toBeNull();
    expect(submit?.disabled).toBe(true);

    act(() => approve?.click());
    expect(submit?.disabled).toBe(false);
    act(() => submit?.click());

    expect(onSubmit).toHaveBeenCalledWith('approve', '');
  });
});
