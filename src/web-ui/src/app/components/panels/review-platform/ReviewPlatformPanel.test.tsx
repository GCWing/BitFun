// @vitest-environment jsdom
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReviewPlatformPullRequest, ReviewPlatformPullRequestDetailPage, ReviewPlatformWorkspaceSnapshot } from '@/infrastructure/api/service-api/ReviewPlatformAPI';
import { ReviewPlatformPanel } from './ReviewPlatformPanel';

const mocks = vi.hoisted(() => ({ snapshot: vi.fn(), detail: vi.fn(), t: (key: string) => key }));
vi.mock('@/infrastructure/api', () => ({ reviewPlatformAPI: { getWorkspaceSnapshot: mocks.snapshot, getPullRequestDetailPage: mocks.detail }, systemAPI: {} }));
vi.mock('@/infrastructure/markdown', () => ({ MarkdownRenderer: () => null }));
vi.mock('@/shared/notification-system', () => ({ notificationService: {} }));
vi.mock('@/infrastructure/i18n', () => ({ useI18n: () => ({ t: mocks.t }), i18nService: { t: mocks.t, formatDate: () => '' } }));
vi.mock('@/flow_chat/services/sessionActivation', () => ({ openMainSession: vi.fn() }));
vi.mock('@/flow_chat/services/btwSessionPane', () => ({ openBtwSessionInAuxPane: vi.fn() }));
vi.mock('@/flow_chat/services/ReviewService', () => ({ launchPreparedReviewSession: vi.fn(), prepareReviewLaunchFromPullRequest: vi.fn() }));
vi.mock('@/flow_chat/components/DeepReviewConsentDialog', () => ({ useDeepReviewConsent: () => ({ confirmDeepReviewLaunch: vi.fn(), deepReviewConsentDialog: null }) }));
vi.mock('@/flow_chat/store/FlowChatStore', () => ({ flowChatStore: { getState: () => ({ sessions: new Map(), activeSessionId: null }), subscribe: () => () => {} } }));
vi.mock('@/shared/stores/contextStore', () => ({ useContextStore: {} }));
vi.mock('@/shared/services/ide-control', () => ({ quickActions: {} }));
vi.mock('@openbitfun/ui', async () => {
  const { createElement } = await import('react');
  const control = (tag: string) => ({ children, onClick, disabled, ...props }: Record<string, any>) => createElement(tag, {
    onClick, disabled, ...Object.fromEntries(Object.entries(props).filter(([key]) => key.startsWith('data-') || key.startsWith('aria-') || key === 'className')),
  }, children);
  const box = control('div');
  return {
    Button: control('button'), IconButton: control('button'), Input: control('input'),
    Combobox: () => null, Icon: () => null, Field: box, ScrollArea: box, TabGroup: () => null,
    Tooltip: box, OverflowText: box, Dialog: () => null, DialogBody: box, DialogClose: box, DialogHeader: box,
    DialogHeading: box, DialogTitle: box,
  };
});

function pull(number: number, state: 'open' | 'merged'): ReviewPlatformPullRequest {
  return { id: String(number), number, state, title: `PR ${number}`, author: 'author', sourceBranch: 'feature', targetBranch: 'main',
    baseRevision: 'a'.repeat(40), headRevision: 'b'.repeat(40), updatedAt: '', webUrl: '', additions: 0, deletions: 0,
    changedFiles: 0, changedFileCountKnown: false, lineStatsKnown: false, comments: 0,
    reviewDecision: 'pending', checks: { total: 0, passed: 0, failed: 0, pending: 0 } };
}
function snapshot(path: string, page = 1, state = 'all'): ReviewPlatformWorkspaceSnapshot {
  const remote = { id: 'origin', name: 'origin', url: 'https://gitee.com/example/repo.git', platform: 'gitee' as const,
    host: 'gitee.com', owner: 'example', repositoryName: 'repo', projectPath: 'example/repo', webUrl: '', supported: true,
    authState: 'not_required' as const, authSource: 'none' as const };
  return { remotes: [remote], selectedRemoteId: 'origin', accounts: [],
    repository: { providerId: 'origin', platform: 'gitee', host: 'gitee.com', owner: 'example', name: 'repo', projectPath: 'example/repo',
      defaultBranch: 'main', workspacePath: path, webUrl: '' },
    pullRequests: Array.from({ length: 10 }, (_, index) => pull((state === 'merged' ? 100 : page * 10) + index, state === 'merged' ? 'merged' : 'open')),
    pagination: { page, perPage: 10, total: state === 'merged' ? 269 : 376, hasNext: true },
    capabilities: { canCreateReview: true, canCreatePullRequest: false, canReplyToThread: false, canResolveThread: false,
      canApprove: false, canRevokeApproval: false, canRequestChanges: false, canMerge: false, supportsDraftReview: false,
      supportedPullRequestStates: ['all', 'open', 'draft', 'merged', 'closed'] } };
}
function detail(number: number, section: ReviewPlatformPullRequestDetailPage['section']): ReviewPlatformPullRequestDetailPage {
  return { ...pull(number, number >= 100 ? 'merged' : 'open'), section, body: '', ci: [], files: [], commits: [], threads: [],
    ...(section === 'overview' ? { changedFiles: 2, changedFileCountKnown: true, additions: 4, deletions: 4, lineStatsKnown: true } : {}),
    pagination: { page: 1, perPage: 20, total: 0, hasNext: false } };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

let root: Root;
let host: HTMLDivElement;
let testNumber = 0;
async function click(id: string) {
  await act(async () => { host.querySelector<HTMLButtonElement>(`[data-testid="${id}"]`)!.click(); });
}
async function mount() {
  const path = `/gitee-panel-test-${++testNumber}`;
  await act(async () => { root.render(<ReviewPlatformPanel workspacePath={path} />); });
  return path;
}

describe('Gitee panel state and asynchronous request ordering', () => {
  beforeEach(() => {
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    mocks.snapshot.mockReset().mockImplementation((path, _remote, page, _size, state) => Promise.resolve(snapshot(path, page, state)));
    mocks.detail.mockReset().mockImplementation(() => new Promise(() => {}));
  });
  afterEach(async () => { await act(async () => root.unmount()); host.remove(); });

  it('renders statistics for every initial row while selected detail is still pending', async () => {
    mocks.snapshot.mockImplementation((path, _remote, page, _size, state) => {
      const result = snapshot(path, page, state);
      result.pullRequests = result.pullRequests.map((pr, index) => ({ ...pr,
        changedFiles: index, changedFileCountKnown: true,
        additions: index * 4, deletions: index * 2, lineStatsKnown: true,
      }));
      return Promise.resolve(result);
    });
    await mount();
    const rows = [...host.querySelectorAll('[data-testid="review-platform-pr-row"]')];
    expect(rows).toHaveLength(10);
    for (const [index, row] of rows.entries()) {
      expect(row.querySelector('[data-testid="review-platform-pr-files"]')?.textContent).toBe(`${index} files`);
      expect(row.querySelector('[data-testid="review-platform-pr-additions"]')?.textContent).toBe(`+${index * 4}`);
      expect(row.querySelector('[data-testid="review-platform-pr-deletions"]')?.textContent).toBe(`-${index * 2}`);
    }
    expect(mocks.detail.mock.calls.map(([request]) => request.pullRequestId)).toEqual(['10']);
  });

  it.each([true, false])('uses complete list statistics for unknown detail counts only at the same revisions (%s)', async sameRevisions => {
    mocks.snapshot.mockImplementation((path, _remote, page, _size, state) => {
      const result = snapshot(path, page, state);
      result.pullRequests = result.pullRequests.map(pr => ({ ...pr,
        changedFiles: 2, changedFileCountKnown: true, additions: 4, deletions: 3, lineStatsKnown: true,
      }));
      return Promise.resolve(result);
    });
    mocks.detail.mockImplementation(({ pullRequestId, section }) => Promise.resolve({
      ...detail(Number(pullRequestId), section),
      headRevision: (sameRevisions ? 'b' : 'c').repeat(40),
      changedFiles: 0, changedFileCountKnown: false, additions: 0, deletions: 0, lineStatsKnown: false,
    }));
    await mount();
    expect(host.querySelector('[data-testid="review-platform-detail-files"]')?.textContent).toBe(sameRevisions ? '2 files' : '— files');
    expect(host.querySelector('[data-testid="review-platform-detail-additions"]')?.textContent).toBe(sameRevisions ? '+4' : '—');
    expect(host.querySelector('[data-testid="review-platform-detail-deletions"]')?.textContent).toBe(sameRevisions ? '-3' : '—');
  });

  it('updates both row and detail statistics to zero for a legacy overview', async () => {
    mocks.snapshot.mockImplementation((path, _remote, page, _size, state) => {
      const result = snapshot(path, page, state);
      result.pullRequests = result.pullRequests.map(pr => ({ ...pr,
        changedFiles: 2, changedFileCountKnown: true, additions: 4, deletions: 3, lineStatsKnown: undefined,
      }));
      return Promise.resolve(result);
    });
    mocks.detail.mockImplementation(({ pullRequestId, section }) => Promise.resolve({
      ...detail(Number(pullRequestId), section),
      changedFiles: 0, changedFileCountKnown: section === 'overview',
      additions: 0, deletions: 0, lineStatsKnown: undefined,
    }));
    await mount();
    const row = host.querySelector('[data-testid="review-platform-pr-row"][data-pr-number="10"]')!;
    for (const [owner, prefix] of [[row, 'pr'], [host, 'detail']] as const) {
      expect(owner.querySelector(`[data-testid="review-platform-${prefix}-files"]`)?.textContent).toBe('0 files');
      expect(owner.querySelector(`[data-testid="review-platform-${prefix}-additions"]`)?.textContent).toBe('+0');
      expect(owner.querySelector(`[data-testid="review-platform-${prefix}-deletions"]`)?.textContent).toBe('-0');
    }
  });

  it('requests Merged from page one after All page two and ignores a late All refresh', async () => {
    const path = await mount();
    await click('review-platform-next-page');
    expect(host.querySelector('[data-testid="review-platform-pagination"]')?.textContent).toContain('11-20 of 376');
    const pending = deferred<ReviewPlatformWorkspaceSnapshot>();
    mocks.snapshot.mockReturnValueOnce(pending.promise);
    await click('review-platform-refresh');
    await click('review-platform-filter-merged');
    expect(mocks.snapshot).toHaveBeenLastCalledWith(path, null, 1, 10, 'merged');
    await act(async () => pending.resolve(snapshot(path, 2)));
    expect(host.querySelector('[data-testid="review-platform-pagination"]')?.textContent).toContain('1-10 of 269');
    expect([...host.querySelectorAll('[data-testid="review-platform-pr-row"]')].map(row => row.getAttribute('data-pr-state'))).toEqual(Array(10).fill('merged'));
  });

  it('does not enqueue old PR reviews when its CI response arrives after switching state', async () => {
    const pendingCi = deferred<ReviewPlatformPullRequestDetailPage>();
    mocks.detail.mockImplementation(({ pullRequestId, section }) => section === 'ci' && pullRequestId === '10'
      ? pendingCi.promise : Promise.resolve(detail(Number(pullRequestId), section)));
    await mount();
    expect(mocks.detail).toHaveBeenCalledWith(expect.objectContaining({ pullRequestId: '10', section: 'ci' }));
    await click('review-platform-filter-merged');
    await act(async () => pendingCi.resolve(detail(10, 'ci')));
    expect(mocks.detail).not.toHaveBeenCalledWith(expect.objectContaining({ pullRequestId: '10', section: 'reviews' }));
    expect(host.querySelector('[data-testid="review-platform-detail-state"]')?.textContent).toBe('Merged');
    expect(host.querySelector('[data-openbitfun-part="detailMeta"]')?.textContent).toContain('#100');
  });

  it.each([4, 0])('shares verified statistics (%i lines) with the list and discards them for a new revision', async lines => {
    const pendingOverview = deferred<ReviewPlatformPullRequestDetailPage>();
    mocks.detail.mockImplementation(({ pullRequestId, section }) => section === 'overview'
      ? pendingOverview.promise : Promise.resolve(detail(Number(pullRequestId), section)));
    const path = await mount();
    const row = () => host.querySelector('[data-testid="review-platform-pr-row"][data-pr-number="10"]')!;
    const text = (owner: Element, id: string) => owner.querySelector(`[data-testid="review-platform-${id}"]`)?.textContent;
    expect(text(row(), 'pr-files')).toBe('— files');
    expect(text(row(), 'pr-additions')).toBe('—');
    expect(text(row(), 'pr-deletions')).toBe('—');
    await act(async () => pendingOverview.resolve({ ...detail(10, 'overview'), additions: lines, deletions: lines }));
    for (const [owner, prefix] of [[row(), 'pr'], [host, 'detail']] as const) {
      expect(text(owner, `${prefix}-files`)).toBe('2 files');
      expect(text(owner, `${prefix}-additions`)).toBe(`+${lines}`);
      expect(text(owner, `${prefix}-deletions`)).toBe(`-${lines}`);
    }
    expect(mocks.detail.mock.calls.map(([request]) => request.section)).toEqual(['overview', 'ci', 'reviews']);

    // Revalidating the same revision must accept an authoritative zero.
    mocks.detail.mockImplementation(({ pullRequestId, section }) => Promise.resolve({
      ...detail(Number(pullRequestId), section),
      ...(section === 'overview' ? { additions: 0, deletions: 0 } : {}),
    }));
    await click('review-platform-refresh');
    expect(text(row(), 'pr-additions')).toBe('+0');
    expect(text(host, 'detail-additions')).toBe('+0');

    const next = snapshot(path);
    next.pullRequests[0].headRevision = 'c'.repeat(40);
    mocks.snapshot.mockResolvedValueOnce(next);
    mocks.detail.mockImplementation(() => new Promise(() => {}));
    await click('review-platform-refresh');
    expect(text(row(), 'pr-files')).toBe('— files');
    expect(text(row(), 'pr-additions')).toBe('—');
    expect(text(row(), 'pr-deletions')).toBe('—');
  });
});
