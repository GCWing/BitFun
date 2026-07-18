import { describe, expect, it } from 'vitest';
import {
  joinDirectoryPath,
  looksLikeWindowsPath,
  parentDirectoryPath,
} from './peerDirectoryPath';

describe('peerDirectoryPath', () => {
  it('detects windows-looking paths', () => {
    expect(looksLikeWindowsPath('C:\\Users')).toBe(true);
    expect(looksLikeWindowsPath('D:/tmp')).toBe(true);
    expect(looksLikeWindowsPath('/home/user')).toBe(false);
  });

  it('joins posix paths', () => {
    expect(joinDirectoryPath('/', 'home')).toBe('/home');
    expect(joinDirectoryPath('/home', 'user')).toBe('/home/user');
    expect(joinDirectoryPath('/home/', 'user')).toBe('/home/user');
  });

  it('joins windows paths', () => {
    expect(joinDirectoryPath('C:\\', 'Users')).toBe('C:\\Users');
    expect(joinDirectoryPath('C:\\Users', 'me')).toBe('C:\\Users\\me');
    expect(joinDirectoryPath('C:', 'Users')).toBe('C:\\Users');
  });

  it('resolves posix parents', () => {
    expect(parentDirectoryPath('/')).toBeNull();
    expect(parentDirectoryPath('/home')).toBe('/');
    expect(parentDirectoryPath('/home/user')).toBe('/home');
  });

  it('resolves windows parents', () => {
    expect(parentDirectoryPath('C:\\')).toBeNull();
    expect(parentDirectoryPath('C:\\Users')).toBe('C:\\');
    expect(parentDirectoryPath('C:\\Users\\me')).toBe('C:\\Users');
  });
});
