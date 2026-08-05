import { describe, expect, it } from 'vitest';
import { validateFileName } from './validateFileName';

describe('validateFileName', () => {
  it('uses distinct invalid-character errors for files and folders', () => {
    expect(validateFileName('invalid:name', { isRemote: false })).toBe(
      'validation.invalidFilename',
    );
    expect(
      validateFileName('invalid:name', { isRemote: false, isDirectory: true }),
    ).toBe('validation.invalidFolderName');
  });

  it('reports duplicate names without losing the shared file/folder error', () => {
    expect(
      validateFileName('README.md', {
        isRemote: false,
        isDirectory: false,
        siblings: ['readme.md'],
      }),
    ).toBe('validation.duplicateName');
  });
});
