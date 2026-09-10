// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import { STORAGE_KEYS } from '@/shared/constants/app';
import {
  deleteManualTerminalProfile, listManualTerminalProfiles, upsertManualTerminalProfile,
} from './manualTerminalProfileService';

const workspace = '/project';
const storageKey = `${STORAGE_KEYS.MANUAL_TERMINAL_PROFILES}:${workspace}`;
const legacyProfile = { id: 'saved-1', sessionId: 'terminal-1', name: 'Build' };

describe('saved terminal configuration compatibility', () => {
  beforeEach(() => { localStorage.clear(); });

  it('reads legacy records and preserves unknown fields when rebinding a host-allocated session', () => {
    localStorage.setItem(storageKey, JSON.stringify({
      version: 1, futureSetting: { enabled: true },
      profiles: [{ ...legacyProfile, futureField: 'preserve' }],
    }));
    expect(listManualTerminalProfiles(workspace)[0]).toMatchObject(legacyProfile);
    upsertManualTerminalProfile(workspace, { ...legacyProfile, sessionId: 'ssh-host-allocated-id' });
    expect(JSON.parse(localStorage.getItem(storageKey)!)).toMatchObject({
      version: 1, futureSetting: { enabled: true },
      profiles: [{ ...legacyProfile, sessionId: 'ssh-host-allocated-id', futureField: 'preserve' }],
    });
  });

  it.each([
    ['malformed JSON', '{unfinished'],
    ['invalid entry', JSON.stringify({ version: 1, profiles: [legacyProfile, null] })],
    ['newer format', JSON.stringify({ version: 2, profiles: [legacyProfile] })],
  ])('leaves %s intact and surfaces read and write failures', (_name, raw) => {
    localStorage.setItem(storageKey, raw);
    expect(() => listManualTerminalProfiles(workspace)).toThrow();
    expect(() => upsertManualTerminalProfile(workspace, legacyProfile)).toThrow();
    expect(() => deleteManualTerminalProfile(workspace, legacyProfile.id)).toThrow();
    expect(localStorage.getItem(storageKey)).toBe(raw);
  });

  it('isolates workspace records and removes only the explicitly selected configuration', () => {
    upsertManualTerminalProfile(workspace, legacyProfile);
    upsertManualTerminalProfile(workspace, { id: 'saved-2', sessionId: 'terminal-2', name: 'Tests' });
    upsertManualTerminalProfile('peer:/project', legacyProfile);
    deleteManualTerminalProfile(workspace, legacyProfile.id);
    expect(listManualTerminalProfiles(workspace).map(profile => profile.id)).toEqual(['saved-2']);
    expect(listManualTerminalProfiles('peer:/project')).toHaveLength(1);
  });
});
