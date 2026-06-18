import { beforeEach, describe, expect, it, vi } from 'vitest';
import { isHtmlFilePath, openHtmlFileInExternalBrowser } from './htmlFilePreview';

const openHtmlFileInBrowserMock = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/api/service-api/SystemAPI', () => ({
  systemAPI: {
    openHtmlFileInBrowser: openHtmlFileInBrowserMock,
  },
}));

describe('htmlFilePreview', () => {
  beforeEach(() => {
    openHtmlFileInBrowserMock.mockReset();
  });

  it('detects html and htm files case-insensitively', () => {
    expect(isHtmlFilePath('index.html')).toBe(true);
    expect(isHtmlFilePath('REPORT.HTM')).toBe(true);
    expect(isHtmlFilePath('index.tsx')).toBe(false);
    expect(isHtmlFilePath('html-notes.md')).toBe(false);
  });

  it('opens local html files through the native path opener', async () => {
    const path = 'E:\\Projects\\Demo App\\index #1.html';

    await openHtmlFileInExternalBrowser(path);

    expect(openHtmlFileInBrowserMock).toHaveBeenCalledWith(path);
  });
});
