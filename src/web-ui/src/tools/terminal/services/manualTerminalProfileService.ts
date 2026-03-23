import { STORAGE_KEYS } from '@/shared/constants/app';
import { createLogger } from '@/shared/utils/logger';

const logger = createLogger('ManualTerminalProfileService');

export interface ManualTerminalProfile {
  id: string;
  sessionId: string;
  name: string;
  workingDirectory?: string;
  startupCommand?: string;
  shellType?: string;
}

export interface ManualTerminalProfilesState {
  version: 1;
  profiles: ManualTerminalProfile[];
}

export interface ManualTerminalProfileInput {
  id?: string;
  sessionId: string;
  name: string;
  workingDirectory?: string;
  startupCommand?: string;
  shellType?: string;
}

const EMPTY_STATE: ManualTerminalProfilesState = {
  version: 1,
  profiles: [],
};

function getStorageKey(workspacePath: string): string {
  return `${STORAGE_KEYS.MANUAL_TERMINAL_PROFILES}:${workspacePath}`;
}

export function generateManualTerminalProfileId(): string {
  return `manual_profile_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

function normalizeProfile(profile: Partial<ManualTerminalProfile>): ManualTerminalProfile | null {
  if (!profile.id || !profile.sessionId || !profile.name?.trim()) {
    return null;
  }

  return {
    id: profile.id,
    sessionId: profile.sessionId,
    name: profile.name.trim(),
    workingDirectory: profile.workingDirectory?.trim() || undefined,
    startupCommand: profile.startupCommand?.trim() || undefined,
    shellType: profile.shellType?.trim() || undefined,
  };
}

function normalizeState(raw: unknown): ManualTerminalProfilesState {
  if (!raw || typeof raw !== 'object') {
    return EMPTY_STATE;
  }

  const profiles = Array.isArray((raw as { profiles?: unknown[] }).profiles)
    ? (raw as { profiles: unknown[] }).profiles
        .map((item) => normalizeProfile(item as Partial<ManualTerminalProfile>))
        .filter((item): item is ManualTerminalProfile => item !== null)
    : [];

  return {
    version: 1,
    profiles,
  };
}

export function loadManualTerminalProfiles(workspacePath: string): ManualTerminalProfilesState {
  try {
    const raw = localStorage.getItem(getStorageKey(workspacePath));
    if (raw) {
      return normalizeState(JSON.parse(raw));
    }
  } catch (error) {
    logger.error('Failed to load manual terminal profiles', { workspacePath, error });
  }

  return EMPTY_STATE;
}

export function saveManualTerminalProfiles(
  workspacePath: string,
  state: ManualTerminalProfilesState,
): void {
  try {
    localStorage.setItem(getStorageKey(workspacePath), JSON.stringify(normalizeState(state)));
  } catch (error) {
    logger.error('Failed to save manual terminal profiles', { workspacePath, error });
  }
}

export function listManualTerminalProfiles(workspacePath: string): ManualTerminalProfile[] {
  return loadManualTerminalProfiles(workspacePath).profiles;
}

export function getManualTerminalProfileById(
  workspacePath: string,
  profileId: string,
): ManualTerminalProfile | undefined {
  return listManualTerminalProfiles(workspacePath).find((profile) => profile.id === profileId);
}

export function getManualTerminalProfileBySessionId(
  workspacePath: string,
  sessionId: string,
): ManualTerminalProfile | undefined {
  return listManualTerminalProfiles(workspacePath).find((profile) => profile.sessionId === sessionId);
}

export function upsertManualTerminalProfile(
  workspacePath: string,
  input: ManualTerminalProfileInput,
): ManualTerminalProfile {
  const currentState = loadManualTerminalProfiles(workspacePath);
  const existingProfile = currentState.profiles.find(
    (profile) => profile.id === input.id || profile.sessionId === input.sessionId,
  );
  const normalizedProfile = normalizeProfile({
    id: existingProfile?.id ?? input.id ?? generateManualTerminalProfileId(),
    sessionId: input.sessionId,
    name: input.name,
    workingDirectory: input.workingDirectory,
    startupCommand: input.startupCommand,
    shellType: input.shellType,
  });

  if (!normalizedProfile) {
    throw new Error('Invalid manual terminal profile');
  }

  const existingIndex = currentState.profiles.findIndex((profile) => profile.id === normalizedProfile.id);
  const nextProfiles = [...currentState.profiles];

  if (existingIndex >= 0) {
    nextProfiles[existingIndex] = normalizedProfile;
  } else {
    nextProfiles.push(normalizedProfile);
  }

  saveManualTerminalProfiles(workspacePath, {
    version: 1,
    profiles: nextProfiles,
  });

  return normalizedProfile;
}

export function deleteManualTerminalProfile(workspacePath: string, profileId: string): void {
  const currentState = loadManualTerminalProfiles(workspacePath);
  saveManualTerminalProfiles(workspacePath, {
    version: 1,
    profiles: currentState.profiles.filter((profile) => profile.id !== profileId),
  });
}
