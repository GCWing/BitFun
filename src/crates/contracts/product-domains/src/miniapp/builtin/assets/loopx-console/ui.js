const CACHE_KEY = 'loopx-console-cache-v2';
const CACHE_VERSION = 2;
const COMMAND_TIMEOUT_MS = 60_000;
const DEFAULT_CADENCE_MS = 60 * 60 * 1000;
const LOG_KEY = 'loopx-console-error-log';

const copy = {
  'zh-CN': {
    title: 'Issue 自动修复',
    otherGoals: '其他目标',
    goals: 'LoopX 目标',
    updated: '更新于 {time}',
    neverUpdated: '尚未更新',
    ready: '可用',
    refreshing: '正在刷新',
    cached: '正在显示缓存',
    unavailable: '不可用',
    retry: '重试',
    refresh: '刷新',
    executionWorkspace: '执行工作区',
    noWorkspace: '未打开工作区',
    localWorkspace: '本地工作区',
    remoteReadOnly: '远程工作区暂不支持自动修复',
    globalReadOnly: '打开本地工作区后可配置自动修复',
    matchedGoal: '已绑定当前工作区',
    noMatchedGoal: '尚未配置 GitHub Issue 自动修复',
    workspaceMismatch: '监控源与当前工作区不一致，已暂停自动推进',
    configureAutofix: '导入当前 Issues',
    startTracking: '导入当前 Issues',
    selectWorkspaceFirst: '请先在左侧选择一个本地工作区，再开始跟踪 Issue。',
    monitor: '自动推进',
    queue: '初始 Issue 队列',
    recentDelivery: '最近交付',
    confirmations: '需要确认',
    pipeline: 'Issue 交付流水线',
    pipelineIdle: '等待导入当前 Issues',
    pipelineReady: '按初始队列逐个修复并交付 PR',
    pipelineBlocked: '自动推进已暂停，请先处理异常',
    pipelineMonitor: '监控',
    pipelineTriage: '筛选',
    pipelineFix: '修复',
    pipelineValidate: '验证',
    pipelinePr: 'PR',
    noGoals: '没有其他 LoopX 目标',
    current: '当前',
    continue: '立即推进',
    running: 'Agent 执行中',
    mode: '模式',
    modeAutofix: '自动修复 + PR',
    modeTracking: '仅跟踪',
    modeUnconfigured: '未配置',
    status: '状态',
    waitingOn: '等待对象',
    heartbeatStatus: '下次推进',
    monitorReady: '运行中',
    monitorMissing: '未配置',
    monitorBroken: '需修复',
    monitorUnavailable: '不可读取',
    monitorRepair: '修复调度',
    monitorRepairing: '正在修复',
    monitorRepairSuccess: '自动推进已绑定到正确的 Goal 和工作区。',
    monitorRepairFailed: '自动推进修复失败，请展开执行详情检查 LoopX。',
    currentExecution: '当前执行',
    noNextAction: 'LoopX 暂未提供下一步动作。',
    queueEmpty: '初始导入范围内没有可执行 Issue。',
    userGate: '需要确认',
    executionDetails: '执行详情',
    reload: '重新加载',
    detailLoading: '正在读取 Goal 状态和交接包...',
    detailUnavailable: '执行详情暂不可用，可重试加载。',
    selectGoal: '导入当前 Open Issues',
    selectGoalCopy: '指定 GitHub Issues 地址和推进频率。LoopX 只导入配置时已有的 Open Issues。',
    setupTitle: '导入当前 Issues',
    issueSource: 'GitHub Issues（首次导入）',
    issueScopeNote: '之后新增的 Issue 不会加入；修复和临时文件都不得改动当前目录，代码只在独立 worktree 中执行。',
    issueSourceRequired: '请输入有效的 GitHub Issues 地址。',
    cadence: 'Agent 自动推进间隔',
    cadence30m: '每 30 分钟（高频）',
    cadence1h: '每小时（推荐）',
    cadence6h: '每 6 小时（低频）',
    cadence1d: '每天',
    autoPr: '修复后推送 fork 并创建 PR',
    constraints: '处理规则（可选）',
    constraintsPlaceholder: '例如：只处理 bug 标签；每轮最多修复一个 Issue',
    targetWorkspace: '执行工作区',
    cancel: '取消',
    startInAgent: '交给 Agent 导入',
    close: '关闭',
    loopxMissing: '未找到 LoopX。请先安装或修复 PATH，然后重试。',
    incompatible: 'LoopX JSON 契约版本不兼容。已保留上一次成功数据。',
    invalidData: 'LoopX 返回了无效数据。已保留上一次成功数据。',
    refreshFailed: '刷新失败。已保留上一次成功数据。',
    noCacheSuffix: '当前没有可用缓存。',
    agentStarted: 'Agent 已启动，正在导入初始 Issue；完成后会创建自动推进。',
    agentFailed: 'Agent 启动失败，请重试。',
    currentOnly: '只能在该 Goal 对应的本地工作区推进。',
    sourceMismatch: 'GitHub 仓库与当前目录不匹配，请检查地址。',
    cacheRestored: '已恢复上次成功数据，正在后台刷新。',
    composerPlaceholder: '让 Agent 按 LoopX 推进当前 Issue',
    heartbeatAutoCreated: '自动推进已创建，BitFun 将按设定频率处理初始 Issue 队列。',
    deleteGoal: '删除目标',
    deleteGoalTitle: '删除 LoopX 目标',
    deleteGoalCopy: '这会断开该目标与对应项目的连接。',
    deleteGoalArchive: 'LoopX 会先备份注册表，并归档该项目中的目标状态。',
    deleteGoalConfirm: '确认删除',
    deleteGoalDeleting: '正在删除',
    deleteGoalSuccess: '目标已删除，原状态已归档。',
    deleteGoalFailed: '目标删除失败，请检查 LoopX 状态后重试。',
    deleteUnavailable: '该目标缺少可用的本地项目路径，暂时无法删除。',
  },
  'en-US': {
    title: 'Issue Autofix',
    otherGoals: 'Other goals',
    goals: 'LoopX goals',
    updated: 'Updated {time}',
    neverUpdated: 'Not updated yet',
    ready: 'Ready',
    refreshing: 'Refreshing',
    cached: 'Showing cache',
    unavailable: 'Unavailable',
    retry: 'Retry',
    refresh: 'Refresh',
    executionWorkspace: 'Execution workspace',
    noWorkspace: 'No workspace open',
    localWorkspace: 'Local workspace',
    remoteReadOnly: 'Remote workspaces do not support autofix yet',
    globalReadOnly: 'Open a local workspace to configure autofix',
    matchedGoal: 'Bound to the current workspace',
    noMatchedGoal: 'GitHub Issue autofix is not configured',
    workspaceMismatch: 'The issue source does not match this workspace; automation is paused',
    configureAutofix: 'Import current issues',
    startTracking: 'Import current issues',
    selectWorkspaceFirst: 'Select a local workspace in the sidebar before tracking issues.',
    monitor: 'Auto advance',
    queue: 'Initial issue queue',
    recentDelivery: 'Recent delivery',
    confirmations: 'Confirmations',
    pipeline: 'Issue delivery pipeline',
    pipelineIdle: 'Waiting to import current issues',
    pipelineReady: 'Fix the initial queue and deliver pull requests',
    pipelineBlocked: 'Automation is paused until the issue is fixed',
    pipelineMonitor: 'Monitor',
    pipelineTriage: 'Triage',
    pipelineFix: 'Fix',
    pipelineValidate: 'Validate',
    pipelinePr: 'PR',
    noGoals: 'No other LoopX goals',
    current: 'Current',
    continue: 'Advance now',
    running: 'Agent running',
    mode: 'Mode',
    modeAutofix: 'Autofix + PR',
    modeTracking: 'Tracking only',
    modeUnconfigured: 'Not configured',
    status: 'Status',
    waitingOn: 'Waiting on',
    heartbeatStatus: 'Next run',
    monitorReady: 'Running',
    monitorMissing: 'Not configured',
    monitorBroken: 'Needs repair',
    monitorUnavailable: 'Unavailable',
    monitorRepair: 'Repair monitor',
    monitorRepairing: 'Repairing',
    monitorRepairSuccess: 'The monitor is now bound to the correct goal and workspace.',
    monitorRepairFailed: 'Could not repair the monitor. Inspect execution details for LoopX state.',
    currentExecution: 'Current execution',
    noNextAction: 'LoopX has not provided a next action.',
    queueEmpty: 'No actionable issues remain in the initial import.',
    userGate: 'Confirmations',
    executionDetails: 'Execution details',
    reload: 'Reload',
    detailLoading: 'Loading goal status and handoff...',
    detailUnavailable: 'Execution details are unavailable. Retry when ready.',
    selectGoal: 'Import current open issues',
    selectGoalCopy: 'Choose a GitHub Issues URL and cadence. LoopX imports only the issues that are open during setup.',
    setupTitle: 'Import current issues',
    issueSource: 'GitHub Issues (initial import)',
    issueScopeNote: 'Issues opened later are ignored. Fixes and temporary files must not change this checkout; code work runs in isolated worktrees.',
    issueSourceRequired: 'Enter a valid GitHub Issues URL.',
    cadence: 'Agent advance interval',
    cadence30m: 'Every 30 minutes (high frequency)',
    cadence1h: 'Hourly (recommended)',
    cadence6h: 'Every 6 hours (low frequency)',
    cadence1d: 'Daily',
    autoPr: 'Push to a fork and open a PR after each fix',
    constraints: 'Processing rules (optional)',
    constraintsPlaceholder: 'For example: bug label only; at most one issue per run',
    targetWorkspace: 'Execution workspace',
    cancel: 'Cancel',
    startInAgent: 'Import with Agent',
    close: 'Close',
    loopxMissing: 'LoopX was not found. Install it or repair PATH, then retry.',
    incompatible: 'The LoopX JSON contract is incompatible. Cached data is preserved.',
    invalidData: 'LoopX returned invalid data. Cached data is preserved.',
    refreshFailed: 'Refresh failed. Cached data is preserved.',
    noCacheSuffix: 'No cached data is available.',
    agentStarted: 'The Agent started and is importing the initial issues; automatic advancement will be scheduled after it completes.',
    agentFailed: 'Could not start the Agent. Retry when ready.',
    currentOnly: 'This goal can only advance in its local workspace.',
    sourceMismatch: 'The GitHub repository does not match the current folder.',
    cacheRestored: 'Restored cached data and refreshing in the background.',
    composerPlaceholder: 'Ask the Agent to advance the current issue through LoopX',
    heartbeatAutoCreated: 'Automatic advancement is scheduled for the initial issue queue.',
    deleteGoal: 'Delete goal',
    deleteGoalTitle: 'Delete LoopX goal',
    deleteGoalCopy: 'This disconnects the goal from its project.',
    deleteGoalArchive: 'LoopX backs up the registry and archives the project state first.',
    deleteGoalConfirm: 'Delete goal',
    deleteGoalDeleting: 'Deleting',
    deleteGoalSuccess: 'The goal was deleted and its previous state was archived.',
    deleteGoalFailed: 'Could not delete the goal. Check LoopX state and retry.',
    deleteUnavailable: 'This goal does not have an available local project path.',
  },
};

const state = {
  locale: 'zh-CN',
  version: '',
  registry: null,
  summary: null,
  fetchedAt: '',
  workspace: { available: false, name: '', path: '', kind: null, isRemote: false },
  selectedGoalId: null,
  packets: new Map(),
  cronJobs: [],
  cronAvailable: false,
  refreshing: false,
  repairingHeartbeat: false,
  running: null,
  notice: null,
  pendingNewGoal: null,
  deletingGoalId: null,
};

const dom = {};

const errorLog = [];
const MAX_LOG_ENTRIES = 50;

function logError(context, error) {
  const entry = {
    time: new Date().toISOString(),
    context: String(context),
    message: error?.message || String(error),
    stack: error?.stack || '',
    state: {
      running: state.running ? { sessionId: state.running.sessionId, turnId: state.running.turnId, goalId: state.running.goalId } : null,
      pendingNewGoal: state.pendingNewGoal ? { workspacePath: state.pendingNewGoal.workspacePath, issueUrl: state.pendingNewGoal.issueUrl } : null,
      selectedGoalId: state.selectedGoalId,
      workspaceAvailable: state.workspace.available,
      workspacePath: state.workspace.path,
      workspaceIsRemote: state.workspace.isRemote,
      refreshing: state.refreshing,
      repairingHeartbeat: state.repairingHeartbeat,
      deletingGoalId: state.deletingGoalId,
      registryGoals: asArray(state.registry?.goals).map((g) => g.id),
    },
  };
  errorLog.push(entry);
  if (errorLog.length > MAX_LOG_ENTRIES) errorLog.shift();
  console.error('[loopx-console]', context, error);
  void flushErrorLog().catch(() => {});
}

async function flushErrorLog() {
  try { await window.app.storage.set(LOG_KEY, errorLog); } catch { /* storage may be unavailable during init */ }
}

async function readErrorLog() {
  try {
    let cached = await window.app.storage.get(LOG_KEY);
    if (typeof cached === 'string') cached = JSON.parse(cached);
    return Array.isArray(cached) ? cached : [];
  } catch { return []; }
}

function $(id) { return document.getElementById(id); }

function t(key, values = {}) {
  const dictionary = copy[state.locale] || copy['en-US'];
  let value = dictionary[key] || copy['en-US'][key] || key;
  for (const [name, replacement] of Object.entries(values)) {
    value = value.replace(`{${name}}`, String(replacement));
  }
  return value;
}

function resolveLocale(locale) {
  return String(locale || '').toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US';
}

function createElement(tagName, className, text) {
  const element = document.createElement(tagName);
  if (className) element.className = className;
  if (text !== undefined && text !== null) element.textContent = String(text);
  return element;
}

function asArray(value) { return Array.isArray(value) ? value : []; }
function numberOrZero(value) { const number = Number(value); return Number.isFinite(number) ? number : 0; }

function formatTimestamp(value) {
  if (!value) return '--';
  return String(value).replace('T', ' ').replace(/:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/, '');
}

function formatEpoch(value) {
  const milliseconds = Number(value);
  if (!Number.isFinite(milliseconds) || milliseconds <= 0) return '--';
  return new Intl.DateTimeFormat(state.locale, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
    .format(new Date(milliseconds));
}

function normalizePath(value) {
  let path = String(value || '').trim().replace(/\//g, '\\');
  while (path.length > 3 && path.endsWith('\\')) path = path.slice(0, -1);
  return /^[A-Za-z]:\\/.test(path) ? path.toLowerCase() : path;
}

function selectedGoal() {
  return asArray(state.registry?.goals).find((goal) => goal.id === state.selectedGoalId) || null;
}

function currentGoal() {
  if (!state.workspace.available || state.workspace.isRemote) return null;
  const workspacePath = normalizePath(state.workspace.path);
  return asArray(state.registry?.goals).find((goal) => normalizePath(goal.repo) === workspacePath) || null;
}

function canDeleteGoal(goal) {
  return Boolean(goal && state.workspace.available && !state.workspace.isRemote && String(goal.repo || '').trim());
}

function packetState(goalId) { return state.packets.get(goalId) || null; }

function statusAttentionItem(goalId) {
  return asArray(packetState(goalId)?.statusData?.attention_queue?.items)
    .find((item) => item.goal_id === goalId) || null;
}

function goalFacts(goalId) {
  const groups = state.summary?.groups || {};
  const lanes = asArray(state.summary?.lanes);
  const attention = statusAttentionItem(goalId);
  const asset = attention?.project_asset || {};
  const detailedTodos = asArray(asset.agent_todos?.items || attention?.agent_todos?.first_open_items);
  const statusProgress = asArray(packetState(goalId)?.statusData?.run_history?.recent_runs);
  return {
    lane: attention || lanes.find((item) => item.goal_id === goalId) || null,
    gates: asArray(state.summary?.gates || groups.user_gates).filter((item) => item.goal_id === goalId),
    todos: detailedTodos.length
      ? detailedTodos.filter((item) => item.done !== true && item.status !== 'done')
      : asArray(state.summary?.todos || groups.runnable_agent_work).filter((item) => item.goal_id === goalId),
    progress: statusProgress.length
      ? statusProgress
      : asArray(state.summary?.recent_progress || groups.recent_progress).filter((item) => item.goal_id === goalId),
    asset,
  };
}

function searchableGoalText(goalId) {
  const goal = asArray(state.registry?.goals).find((item) => item.id === goalId);
  const details = packetState(goalId);
  return JSON.stringify({ goal, summary: goalFacts(goalId), status: details?.statusData, packet: details?.packet });
}

function detectIssueSource(goalId) {
  const match = searchableGoalText(goalId).match(/https:\/\/github\.com\/([^/\s\"'<>]+)\/([^/\s\"'<>]+?)(?:\.git)?(?:\/issues(?:[/?#][^\s\"'<>]*)?)?(?=[\s\"'<>]|$)/i);
  if (!match) return '';
  return `https://github.com/${match[1]}/${match[2]}/issues`;
}

function issueRepository(source) {
  const match = String(source || '').match(/^https:\/\/github\.com\/([^/]+)\/([^/?#]+)\/issues(?:[/?#].*)?$/i);
  return match ? { owner: match[1], repo: match[2].replace(/\.git$/i, '') } : null;
}

function repositoryMismatch(goal) {
  const repository = issueRepository(detectIssueSource(goal?.id));
  if (!repository || !goal?.repo) return false;
  const folder = normalizePath(goal.repo).split('\\').filter(Boolean).at(-1) || '';
  return folder.toLowerCase() !== repository.repo.toLowerCase();
}

function isIssueGoal(goal) {
  if (!goal) return false;
  const text = searchableGoalText(goal.id).toLowerCase();
  return Boolean(detectIssueSource(goal.id)) || text.includes('github_issue') || text.includes('issue monitor');
}

function isAutofixGoal(goalId) {
  const text = searchableGoalText(goalId).toLowerCase();
  const fixes = /auto.?fix|修复|implement|pull request|\bpr\b/.test(text);
  return isIssueGoal(asArray(state.registry?.goals).find((goal) => goal.id === goalId)) && fixes;
}

function ownedHeartbeatJobs(goalId) {
  return state.cronJobs.filter((job) => {
    const name = String(job?.name || '');
    const text = String(job?.payload?.text || '');
    return name.endsWith(`: ${goalId}`) || text.includes(`goal_id=${goalId}`) || text.includes(`\`${goalId}\``);
  });
}

function heartbeatFacts(goal) {
  if (!state.cronAvailable) return { state: 'unavailable', job: null, jobs: [] };
  const jobs = ownedHeartbeatJobs(goal?.id);
  const job = jobs.find((item) => normalizePath(item?.target?.workspace?.workspacePath) === normalizePath(goal?.repo)) || jobs[0] || null;
  if (!job) return { state: 'missing', job: null, jobs };
  const healthy = job.enabled !== false &&
    normalizePath(job?.target?.workspace?.workspacePath) === normalizePath(goal?.repo) &&
    numberOrZero(job?.state?.consecutiveFailures) === 0;
  return { state: healthy ? 'ready' : 'broken', job, jobs };
}

function goalCategory(goal) {
  const facts = goalFacts(goal.id);
  const status = String(facts.lane?.status || goal.status || '').toLowerCase();
  if (facts.gates.length || status.includes('gate')) return 'waiting';
  if (facts.todos.length || ['eligible', 'active'].includes(status)) return 'action';
  return 'neutral';
}

function isLoopxMissing(error) {
  const message = String(error?.message || error || '').toLowerCase();
  return ['not found', 'not recognized', 'cannot find', 'enoent'].some((part) => message.includes(part));
}

function schemaError() { const error = new Error('LOOPX_SCHEMA_INCOMPATIBLE'); error.code = 'schema'; return error; }
function invalidDataError() { const error = new Error('LOOPX_INVALID_DATA'); error.code = 'invalid'; return error; }

async function runLoopx(args, timeout = COMMAND_TIMEOUT_MS, cwd = window.app.appDataDir) {
  const result = await window.app.shell.exec(['loopx', ...args], { cwd, timeout });
  return String(result?.stdout || '').trim();
}

async function runLoopxJson(args, cwd) {
  const stdout = await runLoopx(args, COMMAND_TIMEOUT_MS, cwd);
  try { return JSON.parse(stdout.replace(/^\uFEFF/, '')); } catch { throw invalidDataError(); }
}

function validateRegistry(value) {
  if (value?.schema_version !== '0.1') throw schemaError();
  if (value?.ok !== true || !Array.isArray(value.goals)) throw invalidDataError();
  return value;
}

function validateSummary(value) {
  if (value?.schema_version !== 'global_manager_command_response_v0') throw schemaError();
  if (value?.ok !== true || !value.summary || !Array.isArray(value.lanes)) throw invalidDataError();
  return value;
}

function validatePacket(value, goalId) {
  if (value?.ok !== true || value?.goal_id !== goalId || value?.handoff_only !== true ||
    (typeof value.handoff_text !== 'string' && typeof value.project_agent_handoff !== 'string')) throw schemaError();
  return value;
}

function validateGoalStatus(value, goalId) {
  if (value?.ok !== true || value?.goal_filter !== goalId || numberOrZero(value?.status_contract?.schema_version) < 2) throw schemaError();
  return value;
}

function validateGoalDeletion(value, goalId) {
  if (value?.ok !== true || value?.schema_version !== 'loopx_project_uninstall_v0' || value?.execute !== true ||
    !asArray(value.goal_ids).includes(goalId)) throw schemaError();
  return value;
}

function validateHeartbeatPrompt(value, goalId) {
  if (value?.ok !== true || value?.goal_id !== goalId || typeof value?.task_body !== 'string' || !value.task_body.trim()) throw schemaError();
  return value;
}

async function readCache() {
  try {
    let cached = await window.app.storage.get(CACHE_KEY);
    if (typeof cached === 'string') cached = JSON.parse(cached);
    if (cached?.cacheVersion === CACHE_VERSION && cached.registry?.schema_version === '0.1' &&
      cached.summary?.schema_version === 'global_manager_command_response_v0') {
      state.version = String(cached.version || '');
      state.registry = cached.registry;
      state.summary = cached.summary;
      state.fetchedAt = String(cached.fetchedAt || '');
      state.selectedGoalId = cached.selectedGoalId || null;
      state.notice = { kind: 'cache', message: t('cacheRestored') };
      return true;
    }
  } catch { /* Live data remains authoritative. */ }
  return false;
}

async function writeCache() {
  await window.app.storage.set(CACHE_KEY, {
    cacheVersion: CACHE_VERSION, version: state.version, registry: state.registry,
    summary: state.summary, fetchedAt: state.fetchedAt, selectedGoalId: state.selectedGoalId,
  });
}

function setNotice(kind, message) { state.notice = message ? { kind, message } : null; renderNotice(); }

function renderNotice() {
  if (!state.notice) { dom.notice.hidden = true; return; }
  dom.notice.hidden = false;
  dom.notice.dataset.kind = state.notice.kind;
  dom.noticeText.textContent = state.notice.message;
  dom.noticeRetry.textContent = t('retry');
  dom.noticeRetry.hidden = !['error'].includes(state.notice.kind);
}

function renderRuntime() {
  dom.runtimeVersion.textContent = state.version || 'LoopX';
  dom.updatedAt.textContent = state.fetchedAt ? t('updated', { time: formatTimestamp(state.fetchedAt) }) : t('neverUpdated');
  dom.refreshButton.disabled = state.refreshing;
  dom.refreshButton.classList.toggle('loading', state.refreshing);
  dom.refreshButton.title = t('refresh');
  dom.refreshButton.setAttribute('aria-label', t('refresh'));
  dom.runtimeDot.className = 'status-dot';
  if (state.refreshing) dom.runtimeStatus.textContent = t('refreshing');
  else if (state.registry && state.summary) {
    dom.runtimeStatus.textContent = state.notice?.kind === 'error' ? t('cached') : t('ready');
    dom.runtimeDot.classList.add('online');
  } else {
    dom.runtimeStatus.textContent = t('unavailable');
    dom.runtimeDot.classList.add('error');
  }
}

function renderWorkspace() {
  const goal = selectedGoal();
  const workspace = state.workspace;
  const mismatch = repositoryMismatch(goal) || (goal && normalizePath(goal.repo) !== normalizePath(workspace.path));
  dom.workspaceKicker.textContent = t('executionWorkspace');
  dom.workspaceName.textContent = workspace.available ? workspace.name || t('localWorkspace') : t('noWorkspace');
  dom.workspacePath.textContent = workspace.available ? workspace.path || '--' : '--';
  dom.newGoalButton.hidden = false;
  dom.newGoalButton.disabled = !workspace.available || workspace.isRemote || Boolean(state.running) || Boolean(state.deletingGoalId);
  dom.newGoalButton.title = !workspace.available ? t('selectWorkspaceFirst') : '';
  dom.workspaceState.className = 'workspace-state';
  if (!workspace.available) dom.workspaceState.textContent = t('globalReadOnly');
  else if (workspace.isRemote) dom.workspaceState.textContent = t('remoteReadOnly');
  else if (mismatch) { dom.workspaceState.textContent = t('workspaceMismatch'); dom.workspaceState.classList.add('warning'); }
  else if (goal && isIssueGoal(goal)) { dom.workspaceState.textContent = t('matchedGoal'); dom.workspaceState.classList.add('ready'); }
  else dom.workspaceState.textContent = t('noMatchedGoal');
}

function renderGoals() {
  const goals = asArray(state.registry?.goals);
  const matched = currentGoal();
  dom.goalCount.textContent = String(goals.length);
  dom.goalsList.replaceChildren();
  if (!goals.length) { dom.goalsList.append(createElement('div', 'empty-state', t('noGoals'))); return; }
  for (const goal of goals) {
    const facts = goalFacts(goal.id);
    const category = goalCategory(goal);
    const item = createElement('button', 'goal-item');
    item.type = 'button';
    item.dataset.goalId = goal.id;
    item.classList.toggle('selected', goal.id === state.selectedGoalId);
    item.classList.toggle('current', goal.id === matched?.id);
    const primary = createElement('span', 'goal-primary');
    primary.append(createElement('span', 'goal-title', goal.id));
    primary.append(createElement('span', `goal-status ${category}`, facts.lane?.status || goal.status || '--'));
    item.append(primary, createElement('span', 'goal-path', goal.repo || '--'));
    const meta = createElement('span', 'goal-meta');
    if (isIssueGoal(goal)) meta.append(createElement('span', '', isAutofixGoal(goal.id) ? t('modeAutofix') : t('modeTracking')));
    if (facts.todos.length) meta.append(createElement('span', '', `${facts.todos.length} todo`));
    if (facts.gates.length) meta.append(createElement('span', '', `${facts.gates.length} gate`));
    item.append(meta);
    dom.goalsList.append(item);
  }
}

function renderMetrics() {
  const goal = selectedGoal();
  if (!goal) {
    dom.metricMonitor.textContent = '--'; dom.metricQueue.textContent = '--';
    dom.metricProgress.textContent = '--'; dom.metricGates.textContent = '--'; return;
  }
  const facts = goalFacts(goal.id);
  const heartbeat = heartbeatFacts(goal);
  const monitorKey = heartbeat.state === 'ready' ? 'monitorReady' : heartbeat.state === 'missing' ? 'monitorMissing' :
    heartbeat.state === 'unavailable' ? 'monitorUnavailable' : 'monitorBroken';
  dom.metricMonitor.textContent = t(monitorKey);
  dom.metricMonitor.className = `metric-value metric-value--text ${heartbeat.state === 'ready' ? 'ready' : heartbeat.state === 'missing' ? 'warning' : 'error'}`;
  dom.metricQueue.textContent = String(facts.todos.length);
  dom.metricProgress.textContent = String(facts.progress.length);
  dom.metricGates.textContent = String(facts.gates.length);
}

function pipelineState(goal, facts, heartbeat) {
  const text = searchableGoalText(goal.id).toLowerCase();
  const progress = JSON.stringify(facts.progress).toLowerCase();
  const mismatch = repositoryMismatch(goal) || normalizePath(goal.repo) !== normalizePath(state.workspace.path);
  const monitor = mismatch || heartbeat.state === 'broken' ? 'blocked' : heartbeat.state === 'ready' ? 'complete' : detectIssueSource(goal.id) ? 'active' : '';
  const triage = /triage|classif|筛选|分类/.test(progress) ? 'complete' : facts.todos.length ? 'active' : '';
  const fix = /fix|implement|修复/.test(progress) ? 'complete' : /fix|implement|修复/.test(text) && facts.todos.length ? 'active' : '';
  const validate = /validat|\btest|验证|测试/.test(progress) ? 'complete' : /validat|\btest|验证|测试/.test(text) ? 'active' : '';
  const pr = /github\.com\/[\w.-]+\/[\w.-]+\/pull\/\d+|pull request.*(?:created|opened)|pr.*(?:创建|提交)/.test(progress) ? 'complete' : /pull request|\bpr\b/.test(text) ? 'active' : '';
  return { monitor, triage, fix, validate, pr, blocked: mismatch || heartbeat.state === 'broken' };
}

function renderPipeline() {
  const goal = selectedGoal();
  const stages = ['monitor', 'triage', 'fix', 'validate', 'pr'];
  if (!goal) {
    dom.pipelineSummary.textContent = t('pipelineIdle');
    stages.forEach((name) => { dom[`pipeline${name[0].toUpperCase()}${name.slice(1)}`].className = 'pipeline-stage'; });
    return;
  }
  const status = pipelineState(goal, goalFacts(goal.id), heartbeatFacts(goal));
  dom.pipelineSummary.textContent = status.blocked ? t('pipelineBlocked') : t('pipelineReady');
  for (const name of stages) {
    const element = dom[`pipeline${name[0].toUpperCase()}${name.slice(1)}`];
    element.className = `pipeline-stage${status[name] ? ` ${status[name]}` : ''}`;
  }
}

function renderIssueQueue(facts) {
  dom.todoList.replaceChildren();
  dom.todoCount.textContent = String(facts.todos.length);
  if (!facts.todos.length) {
    dom.todoList.append(createElement('div', 'queue-empty', t('queueEmpty')));
  } else {
    for (const todo of facts.todos) {
      const item = createElement('article', 'issue-item');
      const priority = String(todo.priority || 'P2').toUpperCase();
      item.append(createElement('span', `issue-priority ${priority.toLowerCase()}`, priority));
      const body = createElement('div', 'issue-body');
      body.append(createElement('div', 'issue-title', todo.title || todo.text || todo.next_safe_action || todo.todo_id || '--'));
      const meta = createElement('div', 'issue-meta');
      const issueNumber = String(todo.title || todo.text || '').match(/#\d+/)?.[0];
      [issueNumber, todo.action_kind, todo.task_class].filter(Boolean).forEach((value) => meta.append(createElement('span', '', value)));
      body.append(meta);
      item.append(body);
      dom.todoList.append(item);
    }
  }
  dom.gateSection.hidden = !facts.gates.length;
  dom.gateCount.textContent = String(facts.gates.length);
  dom.gateList.replaceChildren();
  for (const gate of facts.gates) {
    dom.gateList.append(createElement('div', 'detail-list-item gate', gate.question || gate.next_safe_action || gate.gate_id || '--'));
  }
}

function renderDetail() {
  const goal = selectedGoal();
  dom.detailEmpty.hidden = Boolean(goal);
  dom.detailContent.hidden = !goal;
  if (!goal) {
    dom.emptyConfigureButton.disabled = !state.workspace.available || state.workspace.isRemote || Boolean(state.running);
    dom.emptyConfigureButton.title = !state.workspace.available ? t('selectWorkspaceFirst') : '';
    dom.emptyCopy.textContent = state.workspace.available ? t('selectGoalCopy') : t('selectWorkspaceFirst');
    dom.queueSource.textContent = 'GitHub Issues';
    renderIssueQueue({ todos: [], gates: [] });
    return;
  }

  const facts = goalFacts(goal.id);
  const details = packetState(goal.id);
  const heartbeat = heartbeatFacts(goal);
  const source = detectIssueSource(goal.id);
  const isCurrent = normalizePath(goal.repo) === normalizePath(state.workspace.path);
  const mismatch = !isCurrent || repositoryMismatch(goal);
  dom.trackedSource.textContent = source ? source.replace('https://github.com/', '') : 'GitHub Issues';
  dom.queueSource.textContent = source || goal.repo || '--';
  dom.detailHeading.textContent = goal.id;
  dom.detailRepo.textContent = goal.repo || '--';
  dom.detailCurrent.hidden = !isCurrent;
  dom.detailCurrent.textContent = t('current');
  dom.detailCurrent.className = `detail-status ${goalCategory(goal)}`;
  dom.detailMode.textContent = isIssueGoal(goal) ? (isAutofixGoal(goal.id) ? t('modeAutofix') : t('modeTracking')) : t('modeUnconfigured');
  dom.detailStatus.textContent = facts.lane?.status || goal.status || '--';
  dom.detailWaiting.textContent = facts.lane?.waiting_on || goal.waiting_on || '--';
  dom.detailHeartbeat.textContent = heartbeat.state === 'ready'
    ? formatEpoch(heartbeat.job?.state?.nextRunAtMs)
    : t(heartbeat.state === 'missing' ? 'monitorMissing' : heartbeat.state === 'unavailable' ? 'monitorUnavailable' : 'monitorBroken');
  dom.adapterLabel.textContent = facts.asset?.support_mode || goal.adapter_kind || '--';
  dom.nextActionText.textContent = facts.lane?.recommended_action || facts.lane?.next_safe_action ||
    facts.asset?.next_action || goal.recommended_action || goal.next_probe || t('noNextAction');
  renderIssueQueue(facts);

  if (details?.status === 'ready') dom.handoffContent.textContent = details.packet.handoff_text || details.packet.project_agent_handoff || '--';
  else if (details?.status === 'error') dom.handoffContent.textContent = t('detailUnavailable');
  else dom.handoffContent.textContent = t('detailLoading');

  dom.progressSection.hidden = !facts.progress.length;
  dom.progressList.replaceChildren();
  for (const progress of facts.progress) {
    const item = createElement('div', 'timeline-item');
    item.append(createElement('time', '', formatTimestamp(progress.generated_at)));
    item.append(createElement('div', '', progress.recommended_action || progress.classification || '--'));
    dom.progressList.append(item);
  }

  dom.continueButton.hidden = false;
  dom.continueButton.disabled = mismatch || state.workspace.isRemote || Boolean(state.running);
  dom.continueButton.title = mismatch ? t(repositoryMismatch(goal) ? 'sourceMismatch' : 'currentOnly') : '';
  dom.continueLabel.textContent = state.running?.goalId === goal.id ? t('running') : t('continue');
  dom.heartbeatButton.hidden = heartbeat.state === 'ready' || heartbeat.state === 'unavailable';
  dom.heartbeatButton.disabled = mismatch || state.repairingHeartbeat || Boolean(state.running);
  dom.heartbeatLabel.textContent = state.repairingHeartbeat ? t('monitorRepairing') : t('monitorRepair');
  dom.deleteGoalButton.hidden = false;
  dom.deleteGoalButton.disabled = !canDeleteGoal(goal) || Boolean(state.running) || Boolean(state.deletingGoalId);
  dom.deleteGoalButton.title = canDeleteGoal(goal) ? t('deleteGoal') : t('deleteUnavailable');
  dom.deleteGoalButton.setAttribute('aria-label', dom.deleteGoalButton.title);
}

function applyCopy() {
  document.documentElement.lang = state.locale;
  dom.appTitle.textContent = t('title');
  dom.otherGoalsLabel.textContent = t('otherGoals'); dom.goalsHeading.textContent = t('goals');
  dom.metricMonitorLabel.textContent = t('monitor'); dom.metricQueueLabel.textContent = t('queue');
  dom.metricProgressLabel.textContent = t('recentDelivery'); dom.metricGatesLabel.textContent = t('confirmations');
  dom.newGoalLabel.textContent = t('configureAutofix'); dom.pipelineHeading.textContent = t('pipeline');
  dom.emptyConfigureLabel.textContent = t('startTracking');
  dom.pipelineMonitorLabel.textContent = t('pipelineMonitor'); dom.pipelineTriageLabel.textContent = t('pipelineTriage');
  dom.pipelineFixLabel.textContent = t('pipelineFix'); dom.pipelineValidateLabel.textContent = t('pipelineValidate');
  dom.pipelinePrLabel.textContent = t('pipelinePr'); dom.todoHeading.textContent = t('queue');
  dom.gateHeading.textContent = t('userGate'); dom.emptyTitle.textContent = t('selectGoal'); dom.emptyCopy.textContent = t('selectGoalCopy');
  dom.modeLabel.textContent = t('mode'); dom.statusLabel.textContent = t('status'); dom.waitingLabel.textContent = t('waitingOn');
  dom.heartbeatStatusLabel.textContent = t('heartbeatStatus'); dom.nextActionHeading.textContent = t('currentExecution');
  dom.progressHeading.textContent = t('recentDelivery'); dom.handoffHeading.textContent = t('executionDetails');
  dom.reloadDetailButton.textContent = t('reload'); dom.dialogTitle.textContent = t('setupTitle');
  dom.issueSourceLabel.textContent = t('issueSource'); dom.issueScopeNote.textContent = t('issueScopeNote');
  dom.issueCadenceLabel.textContent = t('cadence');
  dom.cadence30m.textContent = t('cadence30m'); dom.cadence1h.textContent = t('cadence1h');
  dom.cadence6h.textContent = t('cadence6h'); dom.cadence1d.textContent = t('cadence1d');
  dom.issuePrDeliveryLabel.textContent = t('autoPr'); dom.goalTextLabel.textContent = t('constraints');
  dom.goalText.placeholder = t('constraintsPlaceholder'); dom.dialogWorkspaceLabel.textContent = t('targetWorkspace');
  dom.dialogCancel.textContent = t('cancel'); dom.dialogSubmitLabel.textContent = t('startInAgent');
  dom.dialogClose.title = t('close'); dom.dialogClose.setAttribute('aria-label', t('close'));
  dom.deleteDialogTitle.textContent = t('deleteGoalTitle'); dom.deleteDialogCopy.textContent = t('deleteGoalCopy');
  dom.deleteDialogArchive.textContent = t('deleteGoalArchive'); dom.deleteDialogCancel.textContent = t('cancel');
  dom.deleteDialogSubmitLabel.textContent = state.deletingGoalId ? t('deleteGoalDeleting') : t('deleteGoalConfirm');
  dom.deleteDialogClose.title = t('close'); dom.deleteDialogClose.setAttribute('aria-label', t('close'));
}

function render() {
  try {
    applyCopy(); renderNotice(); renderRuntime(); renderWorkspace(); renderGoals();
    renderMetrics(); renderPipeline(); renderDetail();
  } catch (error) { logError('render()', error); }
}

async function loadPacket(goalId, force = false) {
  if (!goalId || (!force && state.packets.has(goalId))) return;
  const goal = asArray(state.registry?.goals).find((item) => item.id === goalId);
  if (!goal?.repo) return;
  state.packets.set(goalId, { status: 'loading' }); render();
  try {
    const [packetValue, statusValue] = await Promise.all([
      runLoopxJson(['--format', 'json', 'review-packet', '--goal-id', goalId, '--handoff-only', '--limit', '5'], goal.repo),
      runLoopxJson(['--format', 'json', 'status', '--goal-id', goalId, '--limit', '8'], goal.repo),
    ]);
    state.packets.set(goalId, { status: 'ready', packet: validatePacket(packetValue, goalId), statusData: validateGoalStatus(statusValue, goalId) });
  } catch (error) { state.packets.set(goalId, { status: 'error', error }); }
  render();
}

async function loadCronJobs() {
  try { state.cronJobs = asArray(await window.app.cron.listJobs({})); state.cronAvailable = true; }
  catch { state.cronJobs = []; state.cronAvailable = false; }
}

function refreshFailureMessage(error) {
  const suffix = state.registry && state.summary ? '' : ` ${t('noCacheSuffix')}`;
  if (error?.code === 'schema') return `${t('incompatible')}${suffix}`;
  if (error?.code === 'invalid') return `${t('invalidData')}${suffix}`;
  if (isLoopxMissing(error)) return `${t('loopxMissing')}${suffix}`;
  return `${t('refreshFailed')}${suffix}`;
}

function chooseSelectedGoal(goals) {
  if (goals.some((goal) => goal.id === state.selectedGoalId)) return;
  const workspacePath = normalizePath(state.workspace.path);
  const localIssueGoal = goals.find((goal) => normalizePath(goal.repo) === workspacePath && isIssueGoal(goal));
  const localGoal = goals.find((goal) => normalizePath(goal.repo) === workspacePath);
  state.selectedGoalId = localIssueGoal?.id || localGoal?.id || goals.find(isIssueGoal)?.id || goals[0]?.id || null;
}

async function refresh() {
  if (state.refreshing) return;
  state.refreshing = true; renderRuntime();
  try {
    const [version, registryValue, workspace] = await Promise.all([
      runLoopx(['--version'], 20_000), runLoopxJson(['--format', 'json', 'registry']),
      window.app.workspace.info().catch(() => state.workspace),
    ]);
    state.workspace = workspace || state.workspace;
    const registry = validateRegistry(registryValue);
    const [summaryValue] = await Promise.all([
      runLoopxJson(['--format', 'json', 'global-summary', '--time-range', '24h', '--limit', '8']),
      loadCronJobs(),
    ]);
    state.version = version; state.registry = registry; state.summary = validateSummary(summaryValue);
    state.fetchedAt = state.summary.generated_at || new Date().toISOString(); state.notice = null;
    chooseSelectedGoal(asArray(registry.goals));
    await writeCache().catch(() => {});
    if (state.selectedGoalId) void loadPacket(state.selectedGoalId, true);
  } catch (error) { state.notice = { kind: 'error', message: refreshFailureMessage(error) }; }
  finally { state.refreshing = false; render(); }
}

async function startAgent(prompt, displayText, sessionName, goalId) {
  const workspacePath = state.workspace.available && !state.workspace.isRemote ? state.workspace.path : '';
  if (!workspacePath) { logError('startAgent: no workspace', new Error('WORKSPACE_UNAVAILABLE')); throw new Error('WORKSPACE_UNAVAILABLE'); }
  logError('startAgent: calling agent.run', new Error('STEP'));
  let result;
  try {
    result = await window.app.agent.run(prompt, { sessionName, displayText, enableTools: true, workspacePath });
  } catch (error) { logError('startAgent: agent.run rejected', error); throw error; }
  if (!result?.sessionId || !result?.turnId) { logError('startAgent: missing sessionId/turnId', new Error('AGENT_SESSION_UNAVAILABLE: ' + JSON.stringify(result))); throw new Error('AGENT_SESSION_UNAVAILABLE'); }
  state.running = { sessionId: result.sessionId, turnId: result.turnId, goalId };
  try {
    setNotice('success', t('agentStarted')); render();
  } catch (error) { logError('startAgent: render after success', error); }
  void window.app.chat.focusSession(result.sessionId).catch(() => {});
}

function buildIssueGoalText(issueUrl, allowPr, constraints) {
  return `Process the complete set of open GitHub Issues present at ${issueUrl} when this goal is initialized. Freeze that exact set as the goal's immutable intake scope and ignore issues opened later. Use LoopX's native Issue-Fix domain state for feasibility, selected successors, delivery evidence, outcome projection, and grouped PR lifecycle monitoring; do not maintain a parallel queue. Advance at most one bounded executable issue per run. Leave the registered checkout's tracked and untracked contents unchanged, and make code changes only in a clean isolated git worktree. ${allowPr ? 'After validation, push to the authenticated fork and open a pull request against the upstream repository only when LoopX authority permits it.' : 'Do not push or open a pull request; stop after a validated local fix and request approval.'} Never merge, release, close issues, or post GitHub comments without explicit user approval. Heartbeats may advance or monitor only the recorded initial scope. ${constraints ? `Additional constraints: ${constraints}` : ''}`;
}

function newGoalPrompt(goalText, workspacePath, issueUrl) {
  return `You are operating inside the exact local workspace: ${workspacePath}
The host surface is BitFun's interactive chat-box, not Codex App.
LoopX is the control-plane source of truth. Do not edit .loopx files, goal state files, todo files, or the global registry directly.

Start the durable goal below with LoopX guided flow in this exact project:
loopx start-goal --guided --project . --goal-text <the exact goal text below> --host-surface chat-box

<goal-text>
${goalText}
</goal-text>

Pass the goal text exactly and execute the transaction returned by the guided flow.

Perform one-time intake before any code change:
1. Fetch the complete paginated set of issue URLs that are OPEN at ${issueUrl} now, using public metadata only.
2. Record the snapshot time and materialize every concrete URL as LoopX-native intake work in this goal. This exact set is immutable: never refresh the repository issue list and never add issues opened later.
3. Create or update the LoopX-native successor todo/recommended action that tells future heartbeat turns to select at most one recorded URL per run, use "loopx issue-fix workflow-plan --url <concrete-issue-url> --fetch-metadata --repo-path . --format json", then use "loopx issue-fix feasibility ... --goal-id <created-goal-id> --project ." with real reproduction and scope observations. Follow the returned route, todo, gate, and writeback contracts. Do not pass the repository Issues page to workflow-plan and do not create an app-local or parallel workflow ledger.

Do not run workflow-plan or feasibility for every URL in this setup turn. Persist the entire initial URL set in LoopX, record the bounded successor todo, then stop so BitFun can create the heartbeat. Keep triage, the selected successor, validation, delivery evidence, and grouped PR lifecycle monitoring in LoopX. Preserve every approval and write gate; never approve on the user's behalf. Do not write snapshots, command packs, status output, or other temporary files into the registered checkout; use stdout or an OS temporary directory and remove temporary artifacts when finished. Before modifying code, create or reuse a clean issue-specific git worktree and leave both tracked and untracked contents of the registered checkout unchanged. Do not report that a heartbeat exists because BitFun will create it only after this guided turn succeeds.`;
}

function continueGoalPrompt(goalId) {
  return `You are operating inside the local repository registered for LoopX goal_id=${goalId}.
LoopX is the control-plane source of truth. Do not edit its state files directly.

Read the current packet with:
loopx --format json review-packet --goal-id ${goalId} --handoff-only --limit 5

Confirm the packet matches goal_id=${goalId}. Follow its gate, owner, stop condition, quota, and next action. Select at most one executable GitHub Issue already recorded in this goal's immutable initial scope. Do not query the repository issue list to discover or add issues opened after the initial snapshot. Use LoopX's native Issue-Fix workflow and feasibility state for that concrete issue, and keep PR monitoring grouped by LoopX's repository lifecycle bucket instead of creating one monitor per PR. Do not write snapshots, command output, or other temporary files into the registered checkout. Before modifying code, create or reuse a clean issue-specific git worktree and leave both tracked and untracked contents of the registered checkout unchanged. Implement a coherent fix, run targeted validation, and write evidence/state back through LoopX. Push a fork branch and create a PR only when current authority permits it. Never infer a user gate or merge/release/close/comment without explicit authority.`;
}

async function submitNewGoal() {
  const issueUrl = dom.issueSource.value.trim().replace(/\/$/, '');
  if (!issueRepository(issueUrl)) {
    dom.issueSource.setCustomValidity(t('issueSourceRequired')); dom.issueSource.reportValidity(); return;
  }
  dom.issueSource.setCustomValidity('');
  if (!state.workspace.available || state.workspace.isRemote || state.running) return;
  const workspaceRepo = normalizePath(state.workspace.path).split('\\').filter(Boolean).at(-1) || '';
  if (workspaceRepo.toLowerCase() !== issueRepository(issueUrl).repo.toLowerCase()) {
    dom.issueSource.setCustomValidity(t('sourceMismatch')); dom.issueSource.reportValidity(); return;
  }
  const everyMs = numberOrZero(dom.issueCadence.value) || DEFAULT_CADENCE_MS;
  const goalText = buildIssueGoalText(issueUrl, dom.issuePrDelivery.checked, dom.goalText.value.trim());
  const beforeGoalIds = asArray(state.registry?.goals).map((goal) => goal.id);
  dom.dialogSubmit.disabled = true;
  state.pendingNewGoal = { workspacePath: normalizePath(state.workspace.path), everyMs, issueUrl, beforeGoalIds };
  try {
    await startAgent(newGoalPrompt(goalText, state.workspace.path, issueUrl), goalText, `${t('title')}: ${state.workspace.name}`, null);
    dom.newGoalDialog.close(); dom.goalText.value = '';
  } catch (error) {
    state.pendingNewGoal = null;
    logError('submitNewGoal: startAgent failed', error);
    setNotice('error', `${t('agentFailed')} ${error?.message ? '(' + error.message + ')' : ''}`);
  }
  finally { dom.dialogSubmit.disabled = false; }
}

async function continueGoal() {
  const goal = selectedGoal();
  if (!goal || normalizePath(goal.repo) !== normalizePath(state.workspace.path) || repositoryMismatch(goal) || state.workspace.isRemote || state.running) return;
  try {
    await loadPacket(goal.id, true);
    if (packetState(goal.id)?.status !== 'ready') throw new Error('HANDOFF_UNAVAILABLE');
    await startAgent(continueGoalPrompt(goal.id), goalFacts(goal.id).lane?.recommended_action || `${t('continue')}: ${goal.id}`, `${t('title')}: ${goal.id}`, goal.id);
  } catch (error) {
    logError('continueGoal: failed', error);
    setNotice('error', `${t('agentFailed')} ${error?.message ? '(' + error.message + ')' : ''}`);
  }
}

function openDeleteGoalDialog() {
  const goal = selectedGoal();
  if (!canDeleteGoal(goal) || state.running || state.deletingGoalId) return;
  dom.deleteDialogGoal.textContent = goal.id; dom.deleteDialogRepo.textContent = goal.repo;
  dom.deleteGoalDialog.showModal(); dom.deleteDialogSubmit.focus();
}

async function deleteGoal() {
  const goal = selectedGoal();
  if (!goal || !canDeleteGoal(goal) || state.running || state.deletingGoalId) return;
  state.deletingGoalId = goal.id; dom.deleteDialogSubmit.disabled = true; render();
  try {
    validateGoalDeletion(await runLoopxJson(['--format', 'json', 'uninstall-project', '--goal-id', goal.id, '--archive-state', '--remove-empty-registry', '--execute'], goal.repo), goal.id);
    state.packets.delete(goal.id); state.selectedGoalId = null; dom.deleteGoalDialog.close();
    await refresh(); setNotice('success', t('deleteGoalSuccess'));
  } catch { setNotice('error', t('deleteGoalFailed')); }
  finally { state.deletingGoalId = null; dom.deleteDialogSubmit.disabled = false; render(); }
}

async function registeredAgentId(goal) {
  try {
    const history = await runLoopxJson(['--format', 'json', 'history', '--goal-id', goal.id, '--limit', '1'], goal.repo);
    const agents = asArray(history?.goals?.[0]?.coordination?.registered_agents);
    return agents[0]?.id || agents[0]?.agent_id || null;
  } catch { return null; }
}

async function heartbeatPrompt(goal) {
  const agentId = await registeredAgentId(goal);
  const args = ['--format', 'json', 'heartbeat-prompt', '--goal-id', goal.id];
  if (agentId) args.push('--agent-id', agentId);
  args.push('-H', 'local_scheduler', '-O', 'host_automation', '-M', 'hosted_automation', '--thin');
  return validateHeartbeatPrompt(await runLoopxJson(args, goal.repo), goal.id).task_body;
}

async function ensureHeartbeat(goal, everyMs = DEFAULT_CADENCE_MS, replaceOwned = false) {
  if (!goal || normalizePath(goal.repo) !== normalizePath(state.workspace.path) || repositoryMismatch(goal)) throw new Error('GOAL_WORKSPACE_MISMATCH');
  const taskBody = await heartbeatPrompt(goal);
  if (replaceOwned) {
    for (const job of ownedHeartbeatJobs(goal.id)) await window.app.cron.deleteJob(job.id);
  }
  await window.app.cron.createJob({
    name: `${t('title')}: ${goal.id}`,
    schedule: { kind: 'every', everyMs }, payload: { text: taskBody }, enabled: true,
    target: { kind: 'workspace', workspace: { workspacePath: goal.repo }, launch: { agentType: 'agentic' } },
  });
  await loadCronJobs();
}

async function repairHeartbeat() {
  const goal = selectedGoal();
  if (!goal || state.repairingHeartbeat) return;
  state.repairingHeartbeat = true; render();
  try {
    const everyMs = numberOrZero(heartbeatFacts(goal).job?.schedule?.everyMs) || DEFAULT_CADENCE_MS;
    await ensureHeartbeat(goal, everyMs, true); setNotice('success', t('monitorRepairSuccess'));
  } catch { setNotice('error', t('monitorRepairFailed')); }
  finally { state.repairingHeartbeat = false; render(); }
}

function resolveNewGoal(pending) {
  const goals = asArray(state.registry?.goals).filter((goal) => normalizePath(goal.repo) === pending.workspacePath);
  return goals.find((goal) => !pending.beforeGoalIds.includes(goal.id)) ||
    goals.find((goal) => isIssueGoal(goal)) ||
    (goals.length === 1 ? goals[0] : null);
}

function onAgentEvent(event) {
  try {
    if (!state.running || !event || typeof event !== 'object') return;
    const sourceEvent = String(event.sourceEvent || event.source_event || event.type || '');
    const sessionId = event.sessionId || event.session_id; const turnId = event.turnId || event.turn_id;
    if (sessionId && sessionId !== state.running.sessionId) return;
    if (turnId && turnId !== state.running.turnId) return;
    if (sourceEvent.endsWith('dialog-turn-completed')) {
      logError('onAgentEvent: dialog-turn-completed', new Error('STEP'));
      const pending = state.pendingNewGoal; state.pendingNewGoal = null; state.running = null; setNotice(null, null); render();
      void refresh().then(async () => {
        try {
          if (!pending || state.workspace.isRemote || normalizePath(state.workspace.path) !== pending.workspacePath) return;
          const goal = resolveNewGoal(pending);
          if (!goal) { logError('onAgentEvent: resolveNewGoal returned null', new Error('NO_NEW_GOAL')); setNotice('error', t('monitorRepairFailed')); return; }
          state.selectedGoalId = goal.id;
          try { await ensureHeartbeat(goal, pending.everyMs, false); setNotice('success', t('heartbeatAutoCreated')); }
          catch (error) { logError('onAgentEvent: ensureHeartbeat failed', error); setNotice('error', t('monitorRepairFailed')); }
          render(); void loadPacket(goal.id, true);
        } catch (error) { logError('onAgentEvent: post-refresh callback', error); }
      }).catch((error) => logError('onAgentEvent: refresh() rejected', error));
    } else if (sourceEvent.endsWith('dialog-turn-failed') || sourceEvent.endsWith('dialog-turn-cancelled')) {
      logError('onAgentEvent: ' + sourceEvent, new Error('TURN_FAILED_OR_CANCELLED'));
      state.pendingNewGoal = null; state.running = null; setNotice('error', t('agentFailed')); render();
    }
  } catch (error) { logError('onAgentEvent: top-level catch', error); }
}

function bindEvents() {
  dom.refreshButton.addEventListener('click', () => void refresh()); dom.noticeRetry.addEventListener('click', () => void refresh());
  dom.goalsList.addEventListener('click', (event) => {
    const button = event.target.closest('[data-goal-id]'); if (!button) return;
    state.selectedGoalId = button.dataset.goalId; dom.goalSwitcher.open = false; render(); void loadPacket(state.selectedGoalId);
  });
  dom.reloadDetailButton.addEventListener('click', (event) => { event.preventDefault(); void loadPacket(state.selectedGoalId, true); });
  const openSetupDialog = () => {
    if (!state.workspace.available || state.workspace.isRemote || state.running) {
      setNotice('error', t('selectWorkspaceFirst'));
      return;
    }
    dom.dialogWorkspace.textContent = state.workspace.path; dom.issueSource.value = '';
    dom.newGoalDialog.showModal(); dom.issueSource.focus();
  };
  dom.newGoalButton.addEventListener('click', openSetupDialog);
  dom.emptyConfigureButton.addEventListener('click', openSetupDialog);
  dom.dialogClose.addEventListener('click', () => dom.newGoalDialog.close()); dom.dialogCancel.addEventListener('click', () => dom.newGoalDialog.close());
  dom.newGoalForm.addEventListener('submit', (event) => { event.preventDefault(); void submitNewGoal(); });
  dom.issueSource.addEventListener('input', () => dom.issueSource.setCustomValidity(''));
  dom.continueButton.addEventListener('click', () => void continueGoal()); dom.heartbeatButton.addEventListener('click', () => void repairHeartbeat());
  dom.deleteGoalButton.addEventListener('click', openDeleteGoalDialog);
  dom.deleteDialogClose.addEventListener('click', () => dom.deleteGoalDialog.close()); dom.deleteDialogCancel.addEventListener('click', () => dom.deleteGoalDialog.close());
  dom.deleteGoalForm.addEventListener('submit', (event) => { event.preventDefault(); void deleteGoal(); });
  window.app.agent.onEvent(onAgentEvent);
  window.app.onLocaleChange((locale) => { state.locale = resolveLocale(locale); render(); });
}

function collectDom() {
  const ids = [
    'app-title', 'tracked-source', 'runtime-version', 'goal-switcher', 'other-goals-label', 'goal-count', 'goals-heading',
    'updated-at', 'goals-list', 'runtime-dot', 'runtime-status', 'refresh-button', 'notice', 'notice-text', 'notice-retry',
    'workspace-kicker', 'workspace-name', 'workspace-path', 'workspace-state', 'new-goal-button', 'new-goal-label',
    'metric-monitor', 'metric-monitor-label', 'metric-queue', 'metric-queue-label', 'metric-progress', 'metric-progress-label',
    'metric-gates', 'metric-gates-label', 'pipeline-heading', 'pipeline-summary', 'pipeline-monitor', 'pipeline-monitor-label',
    'pipeline-triage', 'pipeline-triage-label', 'pipeline-fix', 'pipeline-fix-label', 'pipeline-validate', 'pipeline-validate-label',
    'pipeline-pr', 'pipeline-pr-label', 'todo-heading', 'queue-source', 'todo-count', 'todo-list', 'gate-section', 'gate-heading',
    'gate-count', 'gate-list', 'detail-empty', 'empty-title', 'empty-copy', 'empty-configure-button', 'empty-configure-label',
    'detail-content', 'detail-heading', 'detail-current',
    'detail-repo', 'delete-goal-button', 'heartbeat-button', 'heartbeat-label', 'continue-button', 'continue-label', 'mode-label',
    'detail-mode', 'status-label', 'detail-status', 'waiting-label', 'detail-waiting', 'heartbeat-status-label', 'detail-heartbeat',
    'next-action-heading', 'adapter-label', 'next-action-text', 'progress-section', 'progress-heading', 'progress-list',
    'handoff-heading', 'reload-detail-button', 'handoff-content', 'new-goal-dialog', 'new-goal-form', 'dialog-title', 'dialog-close',
    'issue-source-label', 'issue-source', 'issue-scope-note', 'issue-cadence-label', 'issue-cadence', 'cadence-30m', 'cadence-1h', 'cadence-6h',
    'cadence-1d', 'issue-pr-delivery', 'issue-pr-delivery-label',
    'goal-text-label', 'goal-text', 'dialog-workspace-label', 'dialog-workspace', 'dialog-cancel', 'dialog-submit', 'dialog-submit-label',
    'delete-goal-dialog', 'delete-goal-form', 'delete-dialog-title', 'delete-dialog-close', 'delete-dialog-copy', 'delete-dialog-goal',
    'delete-dialog-repo', 'delete-dialog-archive', 'delete-dialog-cancel', 'delete-dialog-submit', 'delete-dialog-submit-label',
  ];
  for (const id of ids) dom[id.replace(/-([a-z])/g, (_, char) => char.toUpperCase())] = $(id);
}

async function initialize() {
  collectDom(); state.locale = resolveLocale(window.app.locale); bindEvents();
  window.addEventListener('error', (event) => { logError('global error handler', event.error || event.message); });
  window.addEventListener('unhandledrejection', (event) => { logError('unhandledrejection', event.reason); });
  const [hadCache, workspace] = await Promise.all([readCache(), window.app.workspace.info().catch((error) => { logError('initialize: workspace.info', error); return state.workspace; })]);
  state.workspace = workspace || state.workspace; chooseSelectedGoal(asArray(state.registry?.goals)); render();
  if (!hadCache) setNotice(null, null); void refresh();
  if (state.selectedGoalId) void loadPacket(state.selectedGoalId);
}

void initialize();
