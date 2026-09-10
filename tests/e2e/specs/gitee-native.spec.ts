import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { $, $$, browser, expect } from '@wdio/globals';
import { openWorkspace } from '../helpers/workspace-helper';
import { saveScreenshot } from '../helpers/screenshot-utils';

type State = 'all' | 'open' | 'draft' | 'merged' | 'closed';
interface Pull { number: number; state: string; draft?: boolean }
interface Page { items: Pull[]; total: number }
interface Statistics { files: number; additions: number; deletions: number }
type TraceWindow = Window & {
  __OPENBITFUN_STARTUP_TRACE__: { snapshot: () => {
    api: { byCommand: Array<{ command: string; count: number; failureCount: number }> };
  } };
};
const root = process.env.OPENBITFUN_GITEE_E2E_ROOT!;
const workspace = join(root, 'sa-token');
const evidence: object[] = [];
const expected = new Map<string, Page>();
let expectedStatistics: Statistics;

// Independent read-only API expectations, using the same optional token as the app.
async function publicResponse(path: string): Promise<Response> {
  for (let attempt = 0; ; attempt++) {
    await new Promise(resolve => setTimeout(resolve, 1500));
    const url = new URL(`https://gitee.com/api/v5/repos/dromara/sa-token${path}`);
    if (process.env.GITEE_TOKEN) url.searchParams.set('access_token', process.env.GITEE_TOKEN);
    const response = await fetch(url).catch(() => {
      // Do not expose a URL containing credentials through a transport error.
      throw new Error(`Gitee ${path}: request failed`);
    });
    if (!response.ok) {
      const message = await response.text();
      if (attempt < 3 && (response.status === 429 || (response.status === 403 && message.includes('Rate Limit Exceeded')))) {
        console.log(`Gitee rate limit on ${path}; waiting before retry`);
        await new Promise(resolve => setTimeout(resolve, 20000));
        continue;
      }
      throw new Error(`Gitee ${path}: HTTP ${response.status}`);
    }
    return response;
  }
}

async function publicPage(state: string, page: number, perPage = 10): Promise<Page> {
  const response = await publicResponse(`/pulls?state=${state}&sort=updated&direction=desc&page=${page}&per_page=${perPage}`);
  const total = response.headers.get('total_count');
  if (total === null) throw new Error('Gitee did not supply a total_count header');
  return { items: await response.json() as Pull[], total: Number(total) };
}

async function publicStatistics(number: number): Promise<Statistics> {
  const response = await publicResponse(`/pulls/${number}/files`);
  const files = await response.json() as Array<{ additions: string | number; deletions: string | number }>;
  if (!Array.isArray(files) || files.length >= 200) throw new Error('Live statistics require a complete file collection');
  const sum = (key: 'additions' | 'deletions') => files.reduce((total, file) => {
    if (file[key] === undefined || file[key] === null) throw new Error(`Missing live ${key} count`);
    const count = Number(file[key]);
    if (!Number.isSafeInteger(count) || count < 0) throw new Error(`Invalid live ${key} count`);
    return total + count;
  }, 0);
  return { files: files.length, additions: sum('additions'), deletions: sum('deletions') };
}

class NativeGiteePage {
  async open() {
    expect(await openWorkspace(workspace)).toBe(true);
    const workspaceItem = $('[data-testid="nav-workspace-item"][data-workspace-active="true"]');
    await workspaceItem.$('[data-testid="nav-workspace-name-btn"]').click();
    const menuButton = workspaceItem.$('[data-testid="nav-workspace-menu-btn"]');
    await menuButton.waitForDisplayed({ timeout: 10000 });
    await menuButton.click();
    const menu = $('[data-testid="nav-workspace-item-menu"]');
    await menu.waitForDisplayed({ timeout: 10000 });
    await expect(menu).toHaveAttribute('data-workspace-id', await workspaceItem.getAttribute('data-workspace-id'));
    await menu.$('[data-testid="nav-workspace-menu-create-session"]').click();
    await $('[data-testid="chat-input-container"]').waitForDisplayed({ timeout: 15000 });
    await $('[data-testid="flowchat-header-session-overview"]').click();
    await $('[data-testid="flowchat-header-pull-requests"] button').click();
    await this.ready();
  }
  filter(state: State) { return $(`[data-testid="review-platform-filter-${state}"]`); }
  get rows() { return $$('[data-testid="review-platform-pr-row"]'); }
  get next() { return $('[data-testid="review-platform-next-page"]'); }
  get previous() { return $('[data-testid="review-platform-previous-page"]'); }
  get pagination() { return $('[data-testid="review-platform-pagination"]'); }
  async snapshotRequests() {
    return browser.execute(() => (window as TraceWindow).__OPENBITFUN_STARTUP_TRACE__.snapshot().api.byCommand
      .find(item => item.command === 'review_platform_get_workspace_snapshot'));
  }
  async ready() {
    await this.filter('all').waitForDisplayed({ timeout: 45000 });
    await $('[data-testid="review-platform-list-loading"]').waitForExist({ reverse: true, timeout: 45000 });
    const error = $('[data-openbitfun-component="review-platform"][data-openbitfun-part="errorState"]');
    expect(await error.isExisting()).toBe(false);
  }
  async select(state: State) {
    await this.filter(state).click();
    await expect(this.filter(state)).toHaveAttribute('aria-pressed', 'true');
    await this.ready();
  }
  async assertPage(state: State, number = 1) {
    await this.ready();
    const reference = expected.get(`${state}:${number}`)!;
    const actual = await this.rows.map(async row => ({
      number: Number(await row.getAttribute('data-pr-number')),
      state: await row.getAttribute('data-pr-state'),
    }));
    expect(actual.map(item => item.number)).toEqual(reference.items.map(item => item.number));
    expect(actual.map(item => item.state)).toEqual(reference.items.map(item =>
      item.state === 'open' && item.draft ? 'draft' : item.state));
    if (reference.total > 10) {
      const start = (number - 1) * 10 + 1;
      await expect(this.pagination).toHaveText(`${start}-${start + actual.length - 1} of ${reference.total}`);
      if (number === 1) await expect(this.previous).toBeDisabled();
    }
    evidence.push({ checkedAt: new Date().toISOString(), state, page: number, total: reference.total, actual });
  }
  async capture(name: string) {
    await saveScreenshot(name, { directory: join(root, 'screenshots'), includeTimestamp: false });
  }
  async assertInitialStatistics() {
    const second = (await this.rows)[1];
    expect(Number(await second.getAttribute('data-pr-number'))).toBe(expected.get('merged:1')!.items[1].number);
    await expect(second).not.toHaveAttribute('data-openbitfun-state', 'selected');
    const values = [`${expectedStatistics.files} files`, `+${expectedStatistics.additions}`, `-${expectedStatistics.deletions}`];
    const actual: string[] = [];
    for (const [index, field] of ['files', 'additions', 'deletions'].entries()) {
      const value = second.$(`[data-testid="review-platform-pr-${field}"]`);
      await expect(value).toHaveText(values[index]);
      actual.push(await value.getText());
    }
    evidence.push({ checkedAt: new Date().toISOString(), stage: 'initial-list-before-selection',
      number: expected.get('merged:1')!.items[1].number, expectedStatistics, actual });
  }
}
const page = new NativeGiteePage();

describe('Gitee repository filters in the native desktop', () => {
  before(async () => {
    await mkdir(workspace, { recursive: true });
    const git = (...args: string[]) => execFileSync('git', args, { cwd: workspace, windowsHide: true, stdio: 'pipe' });
    git('init', '--initial-branch=main');
    git('remote', 'add', 'origin', 'https://gitee.com/dromara/sa-token.git');
    for (const [state, number] of [['all', 1], ['all', 2], ['merged', 1], ['merged', 2], ['closed', 1]] as const) {
      expected.set(`${state}:${number}`, await publicPage(state, number));
    }
    const open: Pull[] = [];
    for (let number = 1; ; number++) {
      if (number > 10) throw new Error('Live fixture exceeded bounded open collection');
      const incoming = await publicPage('open', number, 100);
      open.push(...incoming.items);
      if (open.length >= incoming.total) break;
    }
    for (const state of ['open', 'draft'] as const) {
      const items = open.filter(item => Boolean(item.draft) === (state === 'draft'));
      expected.set(`${state}:1`, { items: items.slice(0, 10), total: items.length });
    }
    expect(expected.get('merged:1')!.total).toBeGreaterThan(0);
    expectedStatistics = await publicStatistics(expected.get('merged:1')!.items[1].number);
    await page.open();
  });

  after(async () => {
    const transportSummary = await browser.execute(() => (window as TraceWindow).__OPENBITFUN_STARTUP_TRACE__.snapshot().api.byCommand
      .filter(item => item.command.startsWith('review_platform_')));
    await writeFile(join(root, 'result.json'), JSON.stringify({ repository: 'dromara/sa-token', evidence, transportSummary }, null, 2));
    console.log(`Gitee E2E evidence: ${root}`);
  });

  // Keep independent scenarios within Gitee's anonymous public API quota.
  afterEach(async () => {
    if (!process.env.GITEE_TOKEN) await new Promise(resolve => setTimeout(resolve, 30000));
  });

  it('loads repository-wide merged PRs after visiting All page two, then paginates and refreshes', async () => {
    await page.select('all');
    await page.assertPage('all');
    await page.next.click();
    await page.assertPage('all', 2);
    await page.capture('all-page-2');
    await page.select('merged');
    await page.assertPage('merged');
    await page.assertInitialStatistics();
    await page.capture('merged-page-1');
    await page.next.click();
    await page.assertPage('merged', 2);
    await $('[data-testid="review-platform-refresh"]').click();
    await page.assertPage('merged', 2);
    await page.capture('merged-page-2-refreshed');
  });

  it('loads Closed, Open and Draft with the corresponding repository totals', async () => {
    for (const state of ['closed', 'open', 'draft'] as const) {
      await page.select(state);
      await page.assertPage(state);
      await page.capture(state);
    }
  });

  it('keeps the selected PR detail and verified statistics consistent after rapid switching', async () => {
    for (const state of ['closed', 'merged', 'all'] as const) await page.select(state);
    const beforeRefresh = (await page.snapshotRequests())!;
    await $('[data-testid="review-platform-refresh"]').click();
    // Issue the clicks without waiting for network responses between them.
    for (const state of ['merged', 'closed', 'all', 'merged'] as const) await page.filter(state).click();
    await page.assertPage('merged');
    const second = (await page.rows)[1];
    await page.assertInitialStatistics();
    await page.capture('merged-statistics-before-selection');
    await second.click();
    await expect(second).toHaveAttribute('data-openbitfun-state', 'selected');
    await $('[data-openbitfun-component="review-platform"][data-openbitfun-part="loadingState"]')
      .waitForExist({ reverse: true, timeout: 45000 });
    expect(await $('.review-platform__detail-error').isExisting()).toBe(false);
    await expect($('[data-testid="review-platform-detail-state"]')).toHaveText('Merged');
    await expect($('[data-openbitfun-part="detailMeta"]')).toHaveText(
      expect.stringContaining(`#${expected.get('merged:1')!.items[1].number}`));
    // Let the earlier real All refresh finish, then check that it did not replace Merged.
    await browser.waitUntil(async () => (await page.snapshotRequests())!.count > beforeRefresh.count, { timeout: 45000 });
    expect((await page.snapshotRequests())!.failureCount).toBe(beforeRefresh.failureCount);
    await page.assertPage('merged');
    const actualStatistics: Record<string, string[]> = {};
    for (const prefix of ['pr', 'detail']) {
      const owner = prefix === 'pr' ? second : $('[data-openbitfun-component="review-platform"]');
      const values = [`${expectedStatistics.files} files`, `+${expectedStatistics.additions}`, `-${expectedStatistics.deletions}`];
      actualStatistics[prefix] = [];
      for (const [index, field] of ['files', 'additions', 'deletions'].entries()) {
        const value = owner.$(`[data-testid="review-platform-${prefix}-${field}"]`);
        await expect(value).toHaveText(values[index]);
        actualStatistics[prefix].push(await value.getText());
      }
    }
    evidence.push({ checkedAt: new Date().toISOString(), number: expected.get('merged:1')!.items[1].number,
      expectedStatistics, actualStatistics });
    await page.capture('merged-selected-detail');
  });
});
