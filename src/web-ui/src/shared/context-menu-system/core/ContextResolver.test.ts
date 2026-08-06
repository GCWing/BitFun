// @vitest-environment jsdom

import { afterEach, describe, expect, it } from 'vitest';
import { ContextResolver } from './ContextResolver';
import { ContextType } from '../types/context.types';

function contextMenuEvent(target: HTMLElement): MouseEvent {
  return new MouseEvent('contextmenu', {
    bubbles: true,
    clientX: 12,
    clientY: 24,
  });
}

describe('ContextResolver file explorer context', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    window.getSelection()?.removeAllRanges();
  });

  it('keeps empty file explorer space ahead of a stale text selection', () => {
    const selectedText = document.createElement('span');
    selectedText.textContent = 'stale selection';
    const explorer = document.createElement('div');
    explorer.dataset.area = 'file-explorer';
    const emptySpace = document.createElement('div');
    explorer.appendChild(emptySpace);
    document.body.append(selectedText, explorer);

    const range = document.createRange();
    range.selectNodeContents(selectedText);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    const event = contextMenuEvent(emptySpace);
    emptySpace.dispatchEvent(event);

    const context = new ContextResolver().resolve(event);

    expect(context.type).toBe(ContextType.EMPTY_SPACE);
    expect('area' in context ? context.area : undefined).toBe('file-explorer');
  });

  it('still resolves file nodes inside the explorer', () => {
    const explorer = document.createElement('div');
    explorer.dataset.area = 'file-explorer';
    const fileNode = document.createElement('div');
    fileNode.dataset.filePath = 'C:/workspace/readme.md';
    fileNode.dataset.isDirectory = 'false';
    explorer.appendChild(fileNode);
    document.body.appendChild(explorer);

    const event = contextMenuEvent(fileNode);
    fileNode.dispatchEvent(event);

    const context = new ContextResolver().resolve(event);

    expect(context.type).toBe(ContextType.FILE_NODE);
  });
});