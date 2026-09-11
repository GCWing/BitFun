import { http } from '@kit.NetworkKit';

/** Public presentation data only. The relay's immutable user ID still owns authorization. */
export interface GitHubAccountProfile {
  userId: string;
  login: string;
  avatarUrl: string;
  fetchedAt: number;
}

export interface GitHubPublicUser {
  id: number;
  login: string;
  avatar_url?: string;
}

export interface GitHubProfileLoader {
  load(userId: string): Promise<GitHubAccountProfile>;
}

export function normalizeGitHubProfile(userId: string, user: GitHubPublicUser): GitHubAccountProfile {
  if (!/^[1-9][0-9]*$/.test(userId) || !Number.isSafeInteger(user.id) || String(user.id) !== userId ||
    typeof user.login !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9-]{0,38}$/.test(user.login)) {
    throw new Error('Invalid GitHub profile identity.');
  }
  const avatar = typeof user.avatar_url === 'string' ? user.avatar_url : '';
  return {
    userId, login: user.login,
    avatarUrl: avatar.startsWith('https://avatars.githubusercontent.com/') ? avatar : '',
    fetchedAt: Date.now()
  };
}

export function readCachedGitHubProfile(userId: string, text: string): GitHubAccountProfile | undefined {
  if (!text) return undefined;
  try {
    const profile = JSON.parse(text) as GitHubAccountProfile;
    if (profile.userId !== userId || typeof profile.fetchedAt !== 'number' || !Number.isFinite(profile.fetchedAt)) return undefined;
    const normalized = normalizeGitHubProfile(userId, {
      id: Number(profile.userId), login: profile.login, avatar_url: profile.avatarUrl
    });
    normalized.fetchedAt = profile.fetchedAt;
    return normalized;
  } catch (_err) { return undefined; }
}

/** Works for already-signed-in controllers without sending any OpenBitFun token to GitHub. */
export class GitHubAccountProfileClient implements GitHubProfileLoader {
  async load(userId: string): Promise<GitHubAccountProfile> {
    if (!/^[1-9][0-9]*$/.test(userId)) throw new Error('Invalid GitHub account ID.');
    const request = http.createHttp();
    try {
      const response = await request.request(`https://api.github.com/user/${userId}`, {
        method: http.RequestMethod.GET,
        header: { 'Accept': 'application/vnd.github+json', 'User-Agent': 'OpenBitFun-Mobile' },
        expectDataType: http.HttpDataType.STRING,
        connectTimeout: 5000, readTimeout: 5000
      });
      if (response.responseCode !== 200 || typeof response.result !== 'string') {
        throw new Error(`GitHub profile unavailable (HTTP ${response.responseCode}).`);
      }
      return normalizeGitHubProfile(userId, JSON.parse(response.result) as GitHubPublicUser);
    } finally { request.destroy(); }
  }
}
