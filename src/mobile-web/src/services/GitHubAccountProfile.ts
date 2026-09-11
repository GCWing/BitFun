/** Public display metadata; never used as account authorization. */
export interface GitHubAccountProfile {
  userId: string;
  login: string;
  avatarUrl: string;
  fetchedAt: number;
}
type ProfileStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function normalizeGitHubProfile(userId: string, value: unknown): GitHubAccountProfile {
  const user = value as Record<string, unknown> | null;
  if (!/^[1-9][0-9]*$/.test(userId) || !user || !Number.isSafeInteger(user.id)
    || String(user.id) !== userId || typeof user.login !== 'string'
    || !/^[a-zA-Z0-9][a-zA-Z0-9-]{0,38}$/.test(user.login)) {
    throw new Error('Invalid GitHub profile identity');
  }
  return {
    userId, login: user.login,
    avatarUrl: typeof user.avatar_url === 'string' && user.avatar_url.startsWith('https://avatars.githubusercontent.com/')
      ? user.avatar_url : '',
    fetchedAt: Date.now(),
  };
}

function browserStorage(): ProfileStorage | null {
  try { return window.localStorage; } catch { return null; }
}

export async function loadGitHubAccountProfile(
  userId: string,
  apply: (profile: GitHubAccountProfile) => void,
  signal: AbortSignal,
  storage: ProfileStorage | null = browserStorage(),
  request: typeof fetch = fetch,
): Promise<void> {
  if (!/^[1-9][0-9]*$/.test(userId) || signal.aborted) return;
  const key = `openbitfun.mobile.github_profile.v1.${userId}`;
  try {
    const cached = JSON.parse(storage?.getItem(key) || 'null');
    if (cached?.userId === userId && Number.isFinite(cached.fetchedAt)) {
      const profile = normalizeGitHubProfile(userId, {
        id: Number(userId), login: cached.login, avatar_url: cached.avatarUrl,
      });
      profile.fetchedAt = cached.fetchedAt;
      apply(profile);
      const age = Date.now() - profile.fetchedAt;
      if (age >= 0 && age < 24 * 60 * 60 * 1000) return;
    }
  } catch { /* Corrupt or inaccessible cache stays in place. */ }
  const timeout = new AbortController();
  const abort = () => timeout.abort();
  signal.addEventListener('abort', abort, { once: true });
  const timer = setTimeout(abort, 5000);
  try {
    const response = await request(`https://api.github.com/user/${userId}`, {
      headers: { Accept: 'application/vnd.github+json' },
      credentials: 'omit', signal: timeout.signal,
    });
    if (!response.ok) return;
    const profile = normalizeGitHubProfile(userId, await response.json());
    if (signal.aborted || timeout.signal.aborted) return;
    apply(profile);
    try { storage?.setItem(key, JSON.stringify(profile)); } catch { /* Optional metadata. */ }
  } catch { /* Offline/rate-limited metadata must not affect the account session. */ }
  finally { clearTimeout(timer); signal.removeEventListener('abort', abort); }
}
