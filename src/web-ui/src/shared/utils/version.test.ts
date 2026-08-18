import { describe, expect, it } from 'vitest';
import type { VersionInfo } from '../types/version';
import { formatDisplayedVersion } from './version';

const productionInfo: VersionInfo = {
  name: 'BitFun',
  version: '1.2.2',
  buildDate: '2026-08-06T00:00:00Z',
  buildTimestamp: 0,
  buildEnv: 'production',
  releaseChannel: 'stable',
  isDev: false,
};

describe('formatDisplayedVersion', () => {
  it('uses the native package version without a dev suffix in production', () => {
    expect(formatDisplayedVersion(productionInfo, '1.2.3', true, false)).toBe('1.2.3');
  });

  it('marks a native development runtime as dev', () => {
    expect(formatDisplayedVersion(productionInfo, '1.2.3', true, true)).toBe('1.2.3-dev');
  });

  it('uses generated metadata for the web surface', () => {
    const developmentInfo = { ...productionInfo, isDev: true, buildEnv: 'development' as const };
    expect(formatDisplayedVersion(developmentInfo, null, false, false)).toBe('1.2.2-dev');
  });
});
