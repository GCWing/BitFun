import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ReviewPlatformAPI,
  type ReviewPlatformIssueRequest,
  type ReviewPlatformPullRequestIdentityRequest,
} from './ReviewPlatformAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

describe('ReviewPlatformAPI identity evidence wire', () => {
  let reviewPlatformAPI: ReviewPlatformAPI;

  beforeEach(() => {
    reviewPlatformAPI = new ReviewPlatformAPI();
    invokeMock.mockReset();
  });

  it('sends Issue identity and bounded pagination in a structured request', async () => {
    const evidence = { issueId: '42', comments: [] };
    invokeMock.mockResolvedValueOnce(evidence);
    const request: ReviewPlatformIssueRequest = {
      platform: 'github' as const,
      host: 'github.com',
      projectPath: 'example/repo',
      issueId: '42',
      repositoryPath: 'D:/workspace/example',
      page: 2,
      perPage: 100,
    };

    await expect(reviewPlatformAPI.getIssue(request)).resolves.toBe(evidence);

    expect(invokeMock).toHaveBeenCalledWith('review_platform_get_issue', { request });
  });

  it('sends pull request identity in a structured request', async () => {
    const target = { pullRequest: { id: '7' }, files: [] };
    invokeMock.mockResolvedValueOnce(target);
    const request: ReviewPlatformPullRequestIdentityRequest = {
      platform: 'gitlab' as const,
      host: 'gitlab.com',
      projectPath: 'example/group/repo',
      pullRequestId: '7',
      repositoryPath: 'D:/workspace/example',
    };

    await expect(
      reviewPlatformAPI.getPullRequestReviewTargetByIdentity(request),
    ).resolves.toBe(target);

    expect(invokeMock).toHaveBeenCalledWith(
      'review_platform_get_pull_request_review_target_by_identity',
      { request },
    );
  });

  it('loads workspace context without requesting a pull request list', async () => {
    const context = { remotes: [], pullRequests: [] };
    invokeMock.mockResolvedValueOnce(context);

    await expect(
      reviewPlatformAPI.getWorkspaceContext('D:/workspace/example', 'origin:github:example__repo'),
    ).resolves.toBe(context);

    expect(invokeMock).toHaveBeenCalledWith('review_platform_get_workspace_context', {
      request: {
        repositoryPath: 'D:/workspace/example',
        remoteId: 'origin:github:example__repo',
      },
    });
  });
});
