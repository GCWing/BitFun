import type { ReviewPlatformRemote } from '@/infrastructure/api';

export interface PullRequestLinkTarget {
  webUrl: string;
  host: string;
  projectPath: string;
  pullRequestId: string;
}

function cleanSegment(segment: string): string {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

function normalizeProjectPath(value: string): string {
  return value
    .replace(/\\/g, '/')
    .replace(/\.git$/i, '')
    .replace(/^\/+|\/+$/g, '')
    .toLowerCase();
}

export function parsePullRequestUrl(value: string): PullRequestLinkTarget | null {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return null;
  }

  const segments = url.pathname
    .split('/')
    .map(cleanSegment)
    .filter(Boolean);

  const mergeRequestIndex = segments.findIndex(segment => segment === 'merge_requests');
  if (mergeRequestIndex >= 0 && segments[mergeRequestIndex + 1]) {
    const projectEnd = segments[mergeRequestIndex - 1] === '-'
      ? mergeRequestIndex - 1
      : mergeRequestIndex;
    const projectPath = segments.slice(0, projectEnd).join('/');
    if (projectPath) {
      return {
        webUrl: url.toString(),
        host: url.host.toLowerCase(),
        projectPath,
        pullRequestId: segments[mergeRequestIndex + 1],
      };
    }
  }

  const pullIndex = segments.findIndex(segment => segment === 'pull' || segment === 'pulls');
  if (pullIndex >= 0 && segments[pullIndex + 1]) {
    const projectPath = segments.slice(0, pullIndex).join('/');
    if (projectPath) {
      return {
        webUrl: url.toString(),
        host: url.host.toLowerCase(),
        projectPath,
        pullRequestId: segments[pullIndex + 1],
      };
    }
  }

  return null;
}

export function remoteMatchesPullRequestLink(remote: ReviewPlatformRemote, target: PullRequestLinkTarget): boolean {
  if (remote.host.toLowerCase() !== target.host) {
    return false;
  }

  const targetProject = normalizeProjectPath(target.projectPath);
  const remoteProject = normalizeProjectPath(remote.projectPath);
  if (targetProject === remoteProject) {
    return true;
  }

  try {
    const remoteUrl = new URL(remote.webUrl || remote.url);
    const remoteUrlProject = normalizeProjectPath(remoteUrl.pathname);
    return targetProject === remoteUrlProject;
  } catch {
    return false;
  }
}
