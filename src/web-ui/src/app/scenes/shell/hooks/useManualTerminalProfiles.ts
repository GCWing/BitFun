import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  deleteManualTerminalProfile,
  getManualTerminalProfileById,
  getManualTerminalProfileBySessionId,
  listManualTerminalProfiles,
  type ManualTerminalProfile,
  type ManualTerminalProfileInput,
  upsertManualTerminalProfile,
} from '@/tools/terminal/services/manualTerminalProfileService';

interface UseManualTerminalProfilesReturn {
  profiles: ManualTerminalProfile[];
  profilesBySessionId: Map<string, ManualTerminalProfile>;
  refreshProfiles: () => void;
  saveProfile: (input: ManualTerminalProfileInput) => ManualTerminalProfile | null;
  removeProfile: (profileId: string) => void;
  getProfileById: (profileId: string) => ManualTerminalProfile | undefined;
  getProfileBySessionId: (sessionId: string) => ManualTerminalProfile | undefined;
}

export function useManualTerminalProfiles(
  workspacePath?: string,
): UseManualTerminalProfilesReturn {
  const [profiles, setProfiles] = useState<ManualTerminalProfile[]>([]);

  const refreshProfiles = useCallback(() => {
    if (!workspacePath) {
      setProfiles([]);
      return;
    }

    setProfiles(listManualTerminalProfiles(workspacePath));
  }, [workspacePath]);

  useEffect(() => {
    refreshProfiles();
  }, [refreshProfiles]);

  const saveProfile = useCallback((input: ManualTerminalProfileInput) => {
    if (!workspacePath) {
      return null;
    }

    const profile = upsertManualTerminalProfile(workspacePath, input);
    refreshProfiles();
    return profile;
  }, [refreshProfiles, workspacePath]);

  const removeProfile = useCallback((profileId: string) => {
    if (!workspacePath) {
      return;
    }

    deleteManualTerminalProfile(workspacePath, profileId);
    refreshProfiles();
  }, [refreshProfiles, workspacePath]);

  const getProfileById = useCallback((profileId: string) => {
    if (!workspacePath) {
      return undefined;
    }

    return getManualTerminalProfileById(workspacePath, profileId);
  }, [workspacePath]);

  const getProfileBySessionId = useCallback((sessionId: string) => {
    if (!workspacePath) {
      return undefined;
    }

    return getManualTerminalProfileBySessionId(workspacePath, sessionId);
  }, [workspacePath]);

  const profilesBySessionId = useMemo(
    () => new Map(profiles.map((profile) => [profile.sessionId, profile])),
    [profiles],
  );

  return {
    profiles,
    profilesBySessionId,
    refreshProfiles,
    saveProfile,
    removeProfile,
    getProfileById,
    getProfileBySessionId,
  };
}
