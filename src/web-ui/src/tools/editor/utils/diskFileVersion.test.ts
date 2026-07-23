import { describe, expect, it } from 'vitest';
import {
  diskContentMatchesEditorForExternalSync,
  diskVersionsDiffer,
  diskVersionFromMetadata,
  editorSyncContentSha256Hex,
  normalizeTextForDiskSyncComparison,
} from './diskFileVersion';

describe('diskVersionFromMetadata', () => {
  it('parses modified and size', () => {
    expect(diskVersionFromMetadata({ modified: 1, size: 2 })).toEqual({ modified: 1, size: 2 });
  });

  it('defaults missing size to 0', () => {
    expect(diskVersionFromMetadata({ modified: 1 })).toEqual({ modified: 1, size: 0 });
  });
});

describe('diskVersionsDiffer', () => {
  it('detects mtime or size change', () => {
    expect(diskVersionsDiffer({ modified: 1, size: 1 }, { modified: 2, size: 1 })).toBe(true);
    expect(diskVersionsDiffer({ modified: 1, size: 1 }, { modified: 1, size: 2 })).toBe(true);
    expect(diskVersionsDiffer({ modified: 1, size: 1 }, { modified: 1, size: 1 })).toBe(false);
  });
});

describe('normalizeTextForDiskSyncComparison', () => {
  it('strips BOM and normalizes newlines', () => {
    expect(normalizeTextForDiskSyncComparison('\uFEFFa\r\nb')).toBe('a\nb');
    expect(normalizeTextForDiskSyncComparison('x\ry')).toBe('x\ny');
  });
});

describe('diskContentMatchesEditorForExternalSync', () => {
  it('treats CRLF disk and LF editor as equal', () => {
    expect(diskContentMatchesEditorForExternalSync('a\r\nb', 'a\nb')).toBe(true);
  });

  it('treats BOM on disk as ignorable when editor has none', () => {
    expect(diskContentMatchesEditorForExternalSync('\uFEFFhello', 'hello')).toBe(true);
  });

  it('still distinguishes real content changes', () => {
    expect(diskContentMatchesEditorForExternalSync('a', 'b')).toBe(false);
  });
});

describe('editorSyncContentSha256Hex', () => {
  it('matches known UTF-8 digest for hello', async () => {
    const h = await editorSyncContentSha256Hex('hello');
    expect(h).toBe(
      '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824'
    );
  });
});
