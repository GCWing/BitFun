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
  error: string | null;
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
  const [snapshot, setSnapshot] = useState<{
    key: string | undefined; profiles: ManualTerminalProfile[]; error: string | null;
  }>({ key: workspacePath, profiles: [], error: null });
  const profiles = useMemo(() => snapshot.key === workspacePath ? snapshot.profiles : [], [snapshot, workspacePath]);

  const refreshProfiles = useCallback(() => {
    if (!workspacePath) {
      setSnapshot({ key: workspacePath, profiles: [], error: null });
      return;
    }

    try {
      setSnapshot({ key: workspacePath, profiles: listManualTerminalProfiles(workspacePath), error: null });
    } catch (error) {
      setSnapshot(previous => ({
        key: workspacePath, profiles: previous.key === workspacePath ? previous.profiles : [],
        error: error instanceof Error ? error.message : String(error),
      }));
    }
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
    error: snapshot.key === workspacePath ? snapshot.error : null,
    profilesBySessionId,
    refreshProfiles,
    saveProfile,
    removeProfile,
    getProfileById,
    getProfileBySessionId,
  };
}
