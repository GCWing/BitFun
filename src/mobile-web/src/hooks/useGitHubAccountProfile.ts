import { useEffect, useState } from 'react';
import { loadGitHubAccountProfile, type GitHubAccountProfile } from '../services/GitHubAccountProfile';

export function useGitHubAccountProfile(userId: string | null) {
  const [profile, setProfile] = useState<GitHubAccountProfile | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    setProfile(null);
    if (userId) void loadGitHubAccountProfile(userId, setProfile, controller.signal);
    return () => controller.abort();
  }, [userId]);
  return profile?.userId === userId ? profile : null;
}
