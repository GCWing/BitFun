/**
 * Markdown editor helpers for inline AI E2E flows.
 */

import { browser, $ } from '@wdio/globals';

export interface InlineAiPreviewState {
  isError: boolean;
  isReady: boolean;
  error: string | null;
  previewText: string;
}

const markdownEditorSelector = '.bitfun-markdown-editor .m-editor-tiptap .ProseMirror';

export async function openMarkdownFile(workspacePath: string, filePath: string): Promise<void> {
  await browser.execute(async (targetWorkspacePath: string, targetFilePath: string) => {
    const { fileTabManager } = await import('/src/shared/services/FileTabManager.ts');
    fileTabManager.openFile({
      filePath: targetFilePath,
      workspacePath: targetWorkspacePath,
      mode: 'agent',
    });
  }, workspacePath, filePath);
}

export async function waitForMarkdownEditor(timeout: number = 15000): Promise<void> {
  await browser.waitUntil(async () => {
    const editor = await $(markdownEditorSelector);
    return editor.isExisting();
  }, {
    timeout,
    interval: 500,
    timeoutMsg: 'Markdown editor did not appear',
  });
}

export async function getMarkdownEditorText(): Promise<string> {
  return browser.execute(() => {
    const editor = document.querySelector<HTMLElement>('.m-editor-tiptap .ProseMirror');
    return editor?.innerText || '';
  });
}

export async function focusLastMarkdownBlockEnd(): Promise<void> {
  const focused = await browser.execute(() => {
    const blocks = Array.from(
      document.querySelectorAll<HTMLElement>('.m-editor-tiptap .ProseMirror [data-block-id]')
    );
    const target = [...blocks].reverse().find(block => (block.textContent || '').trim().length > 0);
    if (!target) {
      return false;
    }

    target.scrollIntoView({ block: 'center' });
    target.click();
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(target);
    range.collapse(false);
    selection?.removeAllRanges();
    selection?.addRange(range);
    return true;
  });

  if (!focused) {
    throw new Error('Failed to focus the last non-empty markdown block');
  }
}

export async function ensureTrailingEmptyParagraph(): Promise<void> {
  const hasTrailingEmptyBlock = await browser.execute(() => {
    const blocks = Array.from(
      document.querySelectorAll<HTMLElement>('.m-editor-tiptap .ProseMirror [data-block-id]')
    );
    const lastBlock = blocks[blocks.length - 1];
    return lastBlock ? (lastBlock.textContent || '').trim().length === 0 : false;
  });

  if (!hasTrailingEmptyBlock) {
    await focusLastMarkdownBlockEnd();
    await browser.pause(300);
    await browser.keys('Enter');
  }

  await browser.waitUntil(async () => {
    return browser.execute(() => {
      const blocks = Array.from(
        document.querySelectorAll<HTMLElement>('.m-editor-tiptap .ProseMirror [data-block-id]')
      );
      const lastBlock = blocks[blocks.length - 1];
      return lastBlock ? (lastBlock.textContent || '').trim().length === 0 : false;
    });
  }, {
    timeout: 5000,
    interval: 250,
    timeoutMsg: 'Failed to create a trailing empty paragraph for inline AI',
  });
}

export async function openInlineAiComposerAtCaret(): Promise<void> {
  await browser.pause(300);
  await browser.keys(' ');

  await browser.waitUntil(async () => {
    const panel = await $('[data-testid="md-inline-ai-panel"]');
    return panel.isExisting();
  }, {
    timeout: 5000,
    interval: 250,
    timeoutMsg: 'Inline AI panel did not open after pressing space in empty paragraph',
  });
}

export async function clickInlineAiContinueQuickAction(): Promise<void> {
  const continueButton = await $('[data-testid="md-inline-ai-continue"]');
  await continueButton.click();

  await browser.waitUntil(async () => {
    const preview = await $('[data-testid="md-inline-ai-preview"]');
    return preview.isExisting();
  }, {
    timeout: 10000,
    interval: 250,
    timeoutMsg: 'Inline AI preview did not appear after requesting continue',
  });
}

export async function waitForInlineAiPreviewCompletion(timeout: number = 60000): Promise<InlineAiPreviewState> {
  await browser.waitUntil(async () => {
    const status = await browser.execute(() => {
      const preview = document.querySelector<HTMLElement>('[data-testid="md-inline-ai-preview"]');
      return preview?.dataset.status || 'missing';
    });

    return status === 'ready' || status === 'error';
  }, {
    timeout,
    interval: 1000,
    timeoutMsg: `Inline AI preview did not finish within ${timeout}ms`,
  });

  return browser.execute(() => {
    const preview = document.querySelector<HTMLElement>('[data-testid="md-inline-ai-preview"]');
    const error = document.querySelector<HTMLElement>('[data-testid="md-inline-ai-preview-error"]')?.innerText || null;
    const previewText = document.querySelector<HTMLElement>('[data-testid="md-inline-ai-preview-content"]')?.innerText || '';
    const status = preview?.dataset.status;

    return {
      isError: status === 'error',
      isReady: status === 'ready',
      error,
      previewText,
    };
  });
}

export async function acceptInlineAiPreview(): Promise<void> {
  const acceptButton = await $('[data-testid="md-inline-ai-accept"]');
  await acceptButton.click();

  await browser.waitUntil(async () => {
    const preview = await $('[data-testid="md-inline-ai-preview"]');
    return !(await preview.isExisting());
  }, {
    timeout: 10000,
    interval: 250,
    timeoutMsg: 'Inline AI preview did not close after accepting generated content',
  });
}

export default {
  openMarkdownFile,
  waitForMarkdownEditor,
  getMarkdownEditorText,
  focusLastMarkdownBlockEnd,
  ensureTrailingEmptyParagraph,
  openInlineAiComposerAtCaret,
  clickInlineAiContinueQuickAction,
  waitForInlineAiPreviewCompletion,
  acceptInlineAiPreview,
};
