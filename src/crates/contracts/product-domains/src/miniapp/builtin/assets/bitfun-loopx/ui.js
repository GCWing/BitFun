'use strict';

// LoopX is owned by the BitFun host. This file only projects durable snapshots
// and cursor-addressed events into the MiniApp UI.
const app = window.app;
const byId = (id) => document.getElementById(id);
const MAX_EVENTS = 2000;
const MAX_RENDERED_EVENTS = 600;
const HIGH_RISK_SCOPES = new Set([
  'publish',
  'public_comment',
  'pull_request',
  'merge',
  'production_action',
]);
const KEY_EVENT_KINDS = new Set([
  'task_created',
  'state_changed',
  'phase_changed',
  'approval_required',
  'settlement_recorded',
  'environment_changed',
  'operation_cancelled',
  'snapshot_invalidated',
]);
const SNAPSHOT_EVENT_KINDS = new Set([
  'task_created',
  'state_changed',
  'phase_changed',
  'approval_required',
  'settlement_recorded',
  'environment_changed',
  'operation_cancelled',
  'snapshot_invalidated',
]);

const COPY = {
  'zh-CN': {
    skipToLogs: '跳到日志',
    connecting: '正在连接宿主',
    connected: '已连接',
    reconnecting: '正在重新同步',
    connectionFailed: '连接失败',
    intakeLabel: 'GitHub Issue、Pull Request 或仓库链接',
    intakePlaceholder: '粘贴 GitHub Issue、PR、仓库或 Issues 列表链接',
    model: '模型',
    modelAuto: '自动模型',
    modelPrimaryTag: '主模型',
    resolve: '分析链接',
    resolving: '正在实时复核链接',
    refresh: '刷新状态',
    unsupportedTitle: '当前执行位置不支持 LoopX',
    unsupportedDefault: 'LoopX 目前只支持本地 Desktop 工作区。此入口不会回退到控制端执行。',
    environment: '环境',
    coreEnvironment: '核心环境',
    optionalEnvironment: '增强能力',
    required: '必需',
    optional: '可选',
    retryEnvironment: '重新检查环境',
    tasks: '任务',
    collapseTasks: '收起任务栏',
    expandTasks: '展开任务栏',
    allActivity: '全部活动',
    noTasks: '暂无任务',
    activity: '运行日志',
    allTaskEvents: '所有任务的关键事件',
    selectedTaskEvents: '当前任务的事件与运行状态',
    keyEvents: '关键事件',
    fullLog: '完整日志',
    searchLogs: '搜索日志',
    errorsOnly: '仅错误',
    exportLogs: '导出日志',
    noLogs: '暂无运行事件',
    newEvents: '查看新事件',
    notConnected: '未连接',
    streamCurrent: '事件流正常',
    streamRecovering: '正在补齐事件',
    cursor: '游标',
    visible: '可见',
    confirmTask: '确认任务',
    repository: '仓库',
    workspace: '工作区',
    workspace_existing_worktree: '使用现有 Worktree',
    workspace_new_worktree: '将创建独立 Worktree',
    workspace_clone_required: '将克隆并创建 Worktree',
    workspace_unavailable: '工作区不可用',
    imageCapability: '图片能力',
    supported: '支持',
    unsupported: '不支持',
    items: 'Issue / PR',
    permissions: '本次权限',
    explicitGrant: '逐项授权',
    cancel: '取消',
    close: '关闭',
    createTasks: '创建所选任务',
    newAttempt: '新尝试',
    terminalExists: '已有终态任务',
    confirmNewAttempt: '确认新尝试',
    approvalGate: '审批门禁',
    approvalNote: '审批备注',
    approvalNotePlaceholder: '补充批准或拒绝的原因（可选）',
    reject: '拒绝',
    approve: '批准',
    pause: '暂停',
    resume: '恢复',
    archive: '归档',
    restore: '还原',
    retry: '重试',
    details: '详情',
    currentTool: '工具',
    turn: 'Turn',
    deadline: '截止',
    noRecentOutput: '已 {duration} 没有新输出',
    lastOutput: '最后输出 {duration} 前',
    updated: '更新于 {duration} 前',
    justNow: '刚刚',
    seconds: '{value} 秒',
    minutes: '{value} 分钟',
    hours: '{value} 小时',
    days: '{value} 天',
    deadlinePassed: '已超过截止时间 {duration}',
    deadlineRemaining: '剩余 {duration}',
    intakeUnavailable: '当前执行位置不支持创建任务。',
    bridgeUnavailable: '宿主没有提供受信任的 LoopX 控制器接口。请更新 BitFun 后重试。',
    selectAtLeastOne: '至少选择一个仍开放的 Issue 或 PR。',
    selectPermissions: '请确认本次任务需要的权限范围。',
    previewExpired: '确认信息已失效，请重新分析链接。',
    taskCreated: '任务已创建。',
    openedExisting: '已打开现有任务，没有重复创建。',
    closedNoop: '目标已关闭或合并，无需创建任务。',
    liveVerification: '目标需要再次在线复核，暂未创建任务。',
    retryRequired: '该目标已有终态任务。只有确认后才会创建新的 attempt。',
    actionApplied: '操作已应用。',
    actionDuplicate: '该操作已经应用，无需重复执行。',
    revisionConflict: '任务状态已经变化，已刷新到最新版本。',
    actionRejected: '宿主拒绝了该操作。',
    environmentRetryQueued: '环境检查已重新启动。',
    logsExported: '日志已导出。',
    noGate: '没有找到可回答的审批门禁，请刷新任务状态。',
    approvalNeeded: '任务正在等待远程可回答的审批。',
    coreBlocked: '核心环境未就绪，任务不会静默启动。',
    optionalDegraded: '增强能力不可用，核心修复流程仍可继续。',
    truncatedCandidates: '候选项已截断，请缩小仓库范围后重新分析。',
    imageWarning: '所选内容包含图片，但当前模型不支持图片输入。',
    modelUnavailable: '当前模型不可用，请返回并选择其他模型。',
    workspaceUnavailable: '宿主无法为该仓库准备受信任的 Worktree。',
    resolvedItem: '已处理',
    openItem: '开放',
    fromRepository: '仓库候选',
    attempt: '尝试 {value}',
    taskNumber: '任务 {value}',
    eventCount: '{value} 条事件',
    allSources: '全部来源',
    sidecar: 'LoopX sidecar',
    gitWorktree: 'Git / Worktree',
    agentModel: 'Agent 模型',
    pythonFallback: 'Python 备用',
    openViking: 'OpenViking',
    githubAuth: 'GitHub 登录',
    status_unknown: '未知',
    status_checking: '检查中',
    status_available: '可用',
    status_degraded: '已降级',
    status_unavailable: '不可用',
    status_ready: '就绪',
    status_blocked: '阻塞',
    state_preparing: '准备中',
    state_queued: '排队中',
    state_running: '运行中',
    state_waiting_for_user: '等待审批',
    state_retry_wait: '等待重试',
    state_cancelling: '正在停止',
    state_stopped: '已暂停',
    state_recovery_required: '需要恢复',
    state_completed: '已完成',
    state_failed: '失败',
    state_archived: '已归档',
    phase_unknown: '等待宿主状态',
    phase_validating_environment: '验证环境',
    phase_resolving_intake: '复核输入',
    phase_preparing_workspace: '准备 Worktree',
    phase_creating_goal: '创建 Goal',
    phase_queued: '等待调度',
    phase_inspecting_goal: '检查 Goal',
    phase_building_turn: '构建 Turn',
    phase_starting_agent: '启动 Agent',
    phase_agent_running: 'Agent 执行中',
    phase_validating_progress: '验证持久进度',
    phase_settling_turn: '结算 Turn',
    phase_waiting_for_approval: '等待审批',
    phase_retry_backoff: '重试退避',
    phase_cancelling: '正在取消',
    phase_recovering: '恢复并对账',
    phase_finished: '流程结束',
    scope_workspace_read: '读取工作区',
    scope_workspace_write: '修改工作区',
    scope_git_local: '本地 Git 操作',
    scope_github_read: '读取 GitHub',
    scope_agent_execution: '运行 Agent',
    scope_publish: '发布变更',
    scope_public_comment: '公开评论',
    scope_pull_request: '创建 Pull Request',
    scope_merge: '合并 Pull Request',
    scope_production_action: '生产环境操作',
    scopeHighRisk: '需要单独确认的外部副作用',
    scopeStandard: '本次修复所需能力',
  },
  'en-US': {
    skipToLogs: 'Skip to logs',
    connecting: 'Connecting to host',
    connected: 'Connected',
    reconnecting: 'Resynchronizing',
    connectionFailed: 'Connection failed',
    intakeLabel: 'GitHub issue, pull request, or repository URL',
    intakePlaceholder: 'Paste a GitHub issue, PR, repository, or issues-list URL',
    model: 'Model',
    modelAuto: 'Automatic model',
    modelPrimaryTag: 'Primary',
    resolve: 'Analyze URL',
    resolving: 'Verifying URL against the live source',
    refresh: 'Refresh status',
    unsupportedTitle: 'LoopX is unavailable in this execution location',
    unsupportedDefault: 'LoopX currently supports local Desktop workspaces only. It will not fall back to the controller machine.',
    environment: 'Environment',
    coreEnvironment: 'Core environment',
    optionalEnvironment: 'Optional capabilities',
    required: 'Required',
    optional: 'Optional',
    retryEnvironment: 'Check environment again',
    tasks: 'Tasks',
    collapseTasks: 'Collapse task rail',
    expandTasks: 'Expand task rail',
    allActivity: 'All activity',
    noTasks: 'No tasks yet',
    activity: 'Run log',
    allTaskEvents: 'Important events from every task',
    selectedTaskEvents: 'Events and liveness for the selected task',
    keyEvents: 'Key events',
    fullLog: 'Full log',
    searchLogs: 'Search logs',
    errorsOnly: 'Errors only',
    exportLogs: 'Export logs',
    noLogs: 'No run events yet',
    newEvents: 'View new events',
    notConnected: 'Not connected',
    streamCurrent: 'Event stream current',
    streamRecovering: 'Recovering events',
    cursor: 'cursor',
    visible: 'visible',
    confirmTask: 'Confirm task',
    repository: 'Repository',
    workspace: 'Workspace',
    workspace_existing_worktree: 'Use existing worktree',
    workspace_new_worktree: 'Create an isolated worktree',
    workspace_clone_required: 'Clone and create a worktree',
    workspace_unavailable: 'Workspace unavailable',
    imageCapability: 'Image support',
    supported: 'Supported',
    unsupported: 'Unsupported',
    items: 'Issue / PR',
    permissions: 'Permissions for this run',
    explicitGrant: 'Explicit grant',
    cancel: 'Cancel',
    close: 'Close',
    createTasks: 'Create selected tasks',
    newAttempt: 'New attempt',
    terminalExists: 'A terminal task already exists',
    confirmNewAttempt: 'Confirm new attempt',
    approvalGate: 'Approval gate',
    approvalNote: 'Approval note',
    approvalNotePlaceholder: 'Optional reason for approving or rejecting',
    reject: 'Reject',
    approve: 'Approve',
    pause: 'Pause',
    resume: 'Resume',
    archive: 'Archive',
    restore: 'Restore',
    retry: 'Retry',
    details: 'Details',
    currentTool: 'tool',
    turn: 'Turn',
    deadline: 'deadline',
    noRecentOutput: 'No new output for {duration}',
    lastOutput: 'Last output {duration} ago',
    updated: 'Updated {duration} ago',
    justNow: 'just now',
    seconds: '{value}s',
    minutes: '{value}m',
    hours: '{value}h',
    days: '{value}d',
    deadlinePassed: 'Deadline passed by {duration}',
    deadlineRemaining: '{duration} remaining',
    intakeUnavailable: 'Tasks cannot be created from this execution location.',
    bridgeUnavailable: 'The host did not expose the trusted LoopX controller. Update BitFun and try again.',
    selectAtLeastOne: 'Select at least one open issue or pull request.',
    selectPermissions: 'Confirm the permission scopes required by this run.',
    previewExpired: 'This preview is stale. Analyze the URL again.',
    taskCreated: 'Task created.',
    openedExisting: 'Opened the existing task without creating a duplicate.',
    closedNoop: 'The target is closed or merged; no task was created.',
    liveVerification: 'The target needs another live verification before a task can be created.',
    retryRequired: 'A terminal task exists. Confirm before creating a new attempt.',
    actionApplied: 'Action applied.',
    actionDuplicate: 'This action was already applied.',
    revisionConflict: 'Task state changed. The latest snapshot has been loaded.',
    actionRejected: 'The host rejected this action.',
    environmentRetryQueued: 'Environment verification restarted.',
    logsExported: 'Logs exported.',
    noGate: 'No answerable approval gate was found. Refresh the task state.',
    approvalNeeded: 'The task is waiting at an approval gate that can be answered remotely.',
    coreBlocked: 'The core environment is not ready, so execution will not start silently.',
    optionalDegraded: 'Optional capabilities are unavailable; the core fix flow can continue.',
    truncatedCandidates: 'The candidate list was truncated. Narrow the repository scope and analyze again.',
    imageWarning: 'Selected content contains images, but the current model does not support image input.',
    modelUnavailable: 'The selected model is unavailable. Go back and choose another model.',
    workspaceUnavailable: 'The host cannot prepare a trusted worktree for this repository.',
    resolvedItem: 'Resolved',
    openItem: 'Open',
    fromRepository: 'Repository candidate',
    attempt: 'Attempt {value}',
    taskNumber: 'Task {value}',
    eventCount: '{value} events',
    allSources: 'All sources',
    sidecar: 'LoopX sidecar',
    gitWorktree: 'Git / Worktree',
    agentModel: 'Agent model',
    pythonFallback: 'Python fallback',
    openViking: 'OpenViking',
    githubAuth: 'GitHub sign-in',
    status_unknown: 'Unknown',
    status_checking: 'Checking',
    status_available: 'Available',
    status_degraded: 'Degraded',
    status_unavailable: 'Unavailable',
    status_ready: 'Ready',
    status_blocked: 'Blocked',
    state_preparing: 'Preparing',
    state_queued: 'Queued',
    state_running: 'Running',
    state_waiting_for_user: 'Awaiting approval',
    state_retry_wait: 'Retry wait',
    state_cancelling: 'Stopping',
    state_stopped: 'Paused',
    state_recovery_required: 'Recovery required',
    state_completed: 'Completed',
    state_failed: 'Failed',
    state_archived: 'Archived',
    phase_unknown: 'Waiting for host state',
    phase_validating_environment: 'Validating environment',
    phase_resolving_intake: 'Resolving intake',
    phase_preparing_workspace: 'Preparing worktree',
    phase_creating_goal: 'Creating goal',
    phase_queued: 'Waiting for scheduler',
    phase_inspecting_goal: 'Inspecting goal',
    phase_building_turn: 'Building turn',
    phase_starting_agent: 'Starting agent',
    phase_agent_running: 'Agent running',
    phase_validating_progress: 'Validating durable progress',
    phase_settling_turn: 'Settling turn',
    phase_waiting_for_approval: 'Waiting for approval',
    phase_retry_backoff: 'Retry backoff',
    phase_cancelling: 'Cancelling',
    phase_recovering: 'Recovering and reconciling',
    phase_finished: 'Finished',
    scope_workspace_read: 'Read workspace',
    scope_workspace_write: 'Modify workspace',
    scope_git_local: 'Local Git operations',
    scope_github_read: 'Read GitHub',
    scope_agent_execution: 'Run agent',
    scope_publish: 'Publish changes',
    scope_public_comment: 'Post public comments',
    scope_pull_request: 'Create pull requests',
    scope_merge: 'Merge pull requests',
    scope_production_action: 'Production actions',
    scopeHighRisk: 'External side effect requiring separate confirmation',
    scopeStandard: 'Capability required for this run',
  },
};

const view = {
  root: byId('loopx-app'),
  connectionLabel: byId('connection-label'),
  intakeForm: byId('intake-form'),
  intakeInput: byId('intake-input'),
  modelSelect: byId('model-select'),
  resolveButton: byId('resolve-button'),
  syncButton: byId('sync-button'),
  notice: byId('notice'),
  unsupportedBanner: byId('unsupported-banner'),
  unsupportedReason: byId('unsupported-reason'),
  environmentPanel: byId('environment-panel'),
  environmentDot: byId('environment-dot'),
  environmentStatus: byId('environment-status'),
  environmentChecked: byId('environment-checked'),
  coreEnvironmentList: byId('core-environment-list'),
  optionalEnvironmentList: byId('optional-environment-list'),
  retryEnvironment: byId('retry-environment'),
  taskRail: byId('task-rail'),
  collapseTasks: byId('collapse-tasks'),
  taskCount: byId('task-count'),
  taskItems: byId('task-items'),
  taskEmpty: byId('task-empty'),
  allActivity: byId('all-activity'),
  allActivityMeta: byId('all-activity-meta'),
  logTitle: byId('log-title'),
  selectedState: byId('selected-state'),
  selectedSummary: byId('selected-summary'),
  modeKey: byId('mode-key'),
  modeFull: byId('mode-full'),
  logSearch: byId('log-search'),
  errorsOnly: byId('errors-only'),
  exportLogs: byId('export-logs'),
  livenessPanel: byId('liveness-panel'),
  livenessDot: byId('liveness-dot'),
  livenessPhase: byId('liveness-phase'),
  livenessSince: byId('liveness-since'),
  livenessTurn: byId('liveness-turn'),
  livenessTool: byId('liveness-tool'),
  livenessDeadline: byId('liveness-deadline'),
  livenessError: byId('liveness-error'),
  taskActions: byId('task-actions'),
  logScroll: byId('log-scroll'),
  logEmpty: byId('log-empty'),
  logList: byId('log-list'),
  newEvents: byId('new-events'),
  streamState: byId('stream-state'),
  streamCursor: byId('stream-cursor'),
  visibleLogCount: byId('visible-log-count'),
  intakeDialog: byId('intake-dialog'),
  intakeConfirmForm: byId('intake-confirm-form'),
  intakeDialogTitle: byId('intake-dialog-title'),
  previewRepository: byId('preview-repository'),
  previewWorkspace: byId('preview-workspace'),
  previewModel: byId('preview-model'),
  previewImages: byId('preview-images'),
  candidateCount: byId('candidate-count'),
  candidateList: byId('candidate-list'),
  permissionList: byId('permission-list'),
  intakeWarning: byId('intake-warning'),
  createButton: byId('create-button'),
  retryDialog: byId('retry-dialog'),
  retryMessage: byId('retry-message'),
  retryCancel: byId('retry-cancel'),
  retryConfirm: byId('retry-confirm'),
  gateDialog: byId('gate-dialog'),
  gateForm: byId('gate-form'),
  gateTitle: byId('gate-dialog-title'),
  gateMessage: byId('gate-message'),
  gateNote: byId('gate-note'),
  gateCancel: byId('gate-cancel'),
  gateReject: byId('gate-reject'),
  gateApprove: byId('gate-approve'),
};

const state = {
  snapshot: null,
  events: [],
  eventKeys: new Set(),
  selectedTaskId: null,
  logMode: 'key',
  query: '',
  errorsOnly: false,
  followLogs: true,
  preview: null,
  pendingCreate: null,
  pendingRetry: null,
  gate: null,
  syncing: false,
  syncRequested: false,
  gapRecovery: null,
  connected: false,
  railCollapsed: false,
  visibleEvents: [],
};

function localeId() {
  const raw = app && typeof app.locale === 'string' ? app.locale : 'en-US';
  return raw.startsWith('zh') ? 'zh-CN' : 'en-US';
}

function text(key, values) {
  const table = COPY[localeId()] || COPY['en-US'];
  let output = table[key] || COPY['en-US'][key] || key;
  if (values) {
    Object.entries(values).forEach(([name, value]) => {
      output = output.replace(new RegExp(`\\{${name}\\}`, 'g'), String(value));
    });
  }
  return output;
}

function applyLocale() {
  document.documentElement.lang = localeId();
  document.querySelectorAll('[data-i18n]').forEach((element) => {
    element.textContent = text(element.dataset.i18n);
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((element) => {
    element.setAttribute('placeholder', text(element.dataset.i18nPlaceholder));
  });
  document.querySelectorAll('[data-i18n-title]').forEach((element) => {
    const value = text(element.dataset.i18nTitle);
    element.setAttribute('title', value);
    if (element.getAttribute('aria-label')) element.setAttribute('aria-label', value);
  });
  view.intakeForm.setAttribute('aria-label', text('intakeLabel'));
  view.modelSelect.setAttribute('aria-label', text('model'));
  renderAll();
}

function normalizeTimestamp(value) {
  const number = Number(value || 0);
  if (!Number.isFinite(number) || number <= 0) return 0;
  return number < 100000000000 ? number * 1000 : number;
}

function durationLabel(milliseconds) {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  if (seconds < 8) return text('justNow');
  if (seconds < 60) return text('seconds', { value: seconds });
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return text('minutes', { value: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return text('hours', { value: hours });
  return text('days', { value: Math.floor(hours / 24) });
}

function relativeLabel(value) {
  const timestamp = normalizeTimestamp(value);
  if (!timestamp) return '--';
  return durationLabel(Date.now() - timestamp);
}

function clockLabel(value) {
  const timestamp = normalizeTimestamp(value);
  if (!timestamp) return '--:--:--';
  const date = new Date(timestamp);
  return [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((part) => String(part).padStart(2, '0'))
    .join(':');
}

function stateLabel(value) { return text(`state_${value || 'recovery_required'}`); }
function phaseLabel(value) { return text(`phase_${value || 'unknown'}`); }
function statusLabel(value) { return text(`status_${value || 'unknown'}`); }
function scopeLabel(value) { return text(`scope_${value}`); }

function showNotice(message, tone = 'neutral') {
  if (!message) {
    view.notice.hidden = true;
    view.notice.textContent = '';
    view.notice.dataset.tone = '';
    return;
  }
  view.notice.textContent = message;
  view.notice.dataset.tone = tone;
  view.notice.hidden = false;
}

function errorMessage(error) {
  if (error instanceof Error) return error.message;
  return String(error || 'Unknown error');
}

function setButtonBusy(button, busy) {
  button.disabled = busy;
  button.classList.toggle('is-spinning', busy);
}

function requestId() {
  if (window.crypto && typeof window.crypto.randomUUID === 'function') {
    return window.crypto.randomUUID();
  }
  return `loopx-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function repositoryLabel(repository) {
  if (!repository) return '--';
  return `${repository.owner || '?'}/${repository.repository || '?'}`;
}

function itemKey(item) {
  const repository = item && item.repository ? item.repository : {};
  return `${repository.host || ''}/${repository.owner || ''}/${repository.repository || ''}/${item.kind || ''}/${item.number || 0}`;
}

function itemLabel(item) {
  if (!item) return '--';
  const prefix = item.kind === 'pr' ? 'PR' : 'Issue';
  return `${repositoryLabel(item.repository)} ${prefix} #${item.number}`;
}

function taskForId(taskId) {
  if (!state.snapshot || !Array.isArray(state.snapshot.tasks)) return null;
  return state.snapshot.tasks.find((task) => task.taskId === taskId) || null;
}

function selectedTask() {
  return state.selectedTaskId ? taskForId(state.selectedTaskId) : null;
}

function snapshotSupported() {
  return state.snapshot && state.snapshot.executionSupport === 'supported';
}

function validEvent(event) {
  return event
    && typeof event === 'object'
    && typeof event.streamId === 'string'
    && Number.isSafeInteger(event.cursor)
    && event.cursor >= 0;
}

function addEvent(event) {
  if (!validEvent(event)) return false;
  const key = `${event.streamId}:${event.cursor}`;
  if (state.eventKeys.has(key)) return false;
  state.eventKeys.add(key);
  state.events.push(event);
  state.events.sort((left, right) => left.cursor - right.cursor);
  while (state.events.length > MAX_EVENTS) {
    const removed = state.events.shift();
    state.eventKeys.delete(`${removed.streamId}:${removed.cursor}`);
  }
  return true;
}

function replaceStreamEvents(streamId) {
  state.events = state.events.filter((event) => event.streamId === streamId);
  state.eventKeys = new Set(state.events.map((event) => `${event.streamId}:${event.cursor}`));
}

async function replayEvents(streamId, afterCursor, historical = false) {
  let cursor = afterCursor;
  let pageCount = 0;
  let changed = false;
  view.streamState.textContent = text('streamRecovering');
  while (pageCount < 30) {
    pageCount += 1;
    const page = await app.loopx.eventsSince({
      streamId,
      afterCursor: cursor,
      limit: 250,
    });
    if (!page || page.status === 'snapshot_required' || page.streamId !== streamId) {
      return { snapshotRequired: true, changed };
    }
    (page.events || []).forEach((event) => {
      changed = addEvent(event) || changed;
    });
    cursor = Math.max(cursor, Number(page.nextCursor || cursor));
    if (!page.hasMore) break;
  }
  if (!historical && state.snapshot) {
    state.snapshot.cursor = Math.max(Number(state.snapshot.cursor || 0), cursor);
  }
  return { snapshotRequired: false, changed };
}

function applySnapshot(snapshot) {
  if (!snapshot || typeof snapshot.streamId !== 'string') {
    throw new Error('The host returned an invalid LoopX snapshot.');
  }
  const previousStreamId = state.snapshot && state.snapshot.streamId;
  state.snapshot = snapshot;
  if (previousStreamId && previousStreamId !== snapshot.streamId) {
    replaceStreamEvents(snapshot.streamId);
  }
  if (state.selectedTaskId && !taskForId(state.selectedTaskId)) {
    state.selectedTaskId = null;
  }
  state.connected = true;
  view.connectionLabel.textContent = text('connected');
  view.root.setAttribute('aria-busy', 'false');
  renderAll();
}

async function attachSnapshot(loadHistory = false) {
  if (!app || !app.loopx) {
    showBridgeUnavailable();
    return;
  }
  if (state.syncing) {
    state.syncRequested = true;
    return;
  }
  state.syncing = true;
  setButtonBusy(view.syncButton, true);
  try {
    do {
      state.syncRequested = false;
      const knownStreamId = state.snapshot && state.snapshot.streamId;
      const afterCursor = state.snapshot && state.snapshot.cursor;
      view.connectionLabel.textContent = state.connected ? text('reconnecting') : text('connecting');
      const response = await app.loopx.attach({
        ...(knownStreamId ? { knownStreamId } : {}),
        ...(Number.isSafeInteger(afterCursor) ? { afterCursor } : {}),
      });
      applySnapshot(response && response.snapshot);
      const snapshot = state.snapshot;
      if (loadHistory && state.events.length === 0 && snapshot.cursor > 0) {
        const replay = await replayEvents(snapshot.streamId, 0, true);
        if (replay.changed) renderLogs();
      }
      view.streamState.textContent = text('streamCurrent');
    } while (state.syncRequested);
  } catch (error) {
    state.connected = false;
    view.connectionLabel.textContent = text('connectionFailed');
    view.streamState.textContent = text('connectionFailed');
    showNotice(errorMessage(error), 'error');
  } finally {
    state.syncing = false;
    setButtonBusy(view.syncButton, false);
  }
}

async function recoverEventGap(event) {
  if (state.gapRecovery) return state.gapRecovery;
  state.gapRecovery = (async () => {
    try {
      const snapshot = state.snapshot;
      if (!snapshot || event.streamId !== snapshot.streamId) {
        await attachSnapshot(false);
        return;
      }
      const replay = await replayEvents(snapshot.streamId, snapshot.cursor, false);
      if (replay.snapshotRequired) {
        await attachSnapshot(false);
        return;
      }
      if (event.cursor > state.snapshot.cursor) {
        addEvent(event);
        state.snapshot.cursor = event.cursor;
      }
      renderLogs();
      renderStreamStatus();
    } catch (error) {
      showNotice(errorMessage(error), 'error');
      await attachSnapshot(false);
    } finally {
      state.gapRecovery = null;
    }
  })();
  return state.gapRecovery;
}

function onLoopxEvent(payload) {
  const event = payload && payload.event ? payload.event : payload;
  if (!validEvent(event)) return;
  if (!state.snapshot || event.streamId !== state.snapshot.streamId) {
    void recoverEventGap(event);
    return;
  }
  const cursor = Number(state.snapshot.cursor || 0);
  if (event.cursor > cursor + 1) {
    void recoverEventGap(event);
    return;
  }
  const changed = addEvent(event);
  state.snapshot.cursor = Math.max(cursor, event.cursor);
  if (changed) {
    renderLogs();
    renderStreamStatus();
  }
  if (SNAPSHOT_EVENT_KINDS.has(event.kind)) {
    state.syncRequested = true;
    queueMicrotask(() => void attachSnapshot(false));
  }
}

function showBridgeUnavailable() {
  state.connected = false;
  view.root.setAttribute('aria-busy', 'false');
  view.connectionLabel.textContent = text('connectionFailed');
  view.unsupportedReason.textContent = text('bridgeUnavailable');
  view.unsupportedBanner.hidden = false;
  view.resolveButton.disabled = true;
  view.retryEnvironment.disabled = true;
}

function renderExecutionSupport() {
  const snapshot = state.snapshot;
  const supported = snapshotSupported();
  view.unsupportedBanner.hidden = !snapshot || supported;
  if (snapshot && !supported) {
    view.unsupportedReason.textContent = snapshot.unsupportedReason || text('unsupportedDefault');
  }
  view.resolveButton.disabled = !supported;
  view.retryEnvironment.disabled = !supported;
}

function environmentFact(name, label, fact) {
  const element = document.createElement('article');
  const status = fact && fact.status ? fact.status : 'unknown';
  element.className = 'environment-fact';
  element.dataset.status = status;

  const title = document.createElement('div');
  title.className = 'environment-fact__title';
  const strong = document.createElement('strong');
  strong.textContent = label;
  const value = document.createElement('span');
  value.textContent = fact && fact.version ? fact.version : statusLabel(status);
  title.append(strong, value);
  element.append(title);

  const detail = document.createElement('p');
  detail.textContent = (fact && (fact.detail || fact.remediation)) || statusLabel(status);
  detail.title = detail.textContent;
  element.append(detail);
  element.dataset.fact = name;
  return element;
}

function renderEnvironment() {
  const environment = state.snapshot && state.snapshot.environment;
  const status = environment && environment.status ? environment.status : 'unknown';
  view.environmentDot.dataset.status = status;
  view.environmentStatus.textContent = statusLabel(status);
  view.environmentChecked.textContent = environment && environment.checkedAt
    ? text('updated', { duration: relativeLabel(environment.checkedAt) })
    : '';

  const core = environment && environment.core ? environment.core : {};
  const optional = environment && environment.optional ? environment.optional : {};
  view.coreEnvironmentList.replaceChildren(
    environmentFact('sidecar', text('sidecar'), core.sidecar),
    environmentFact('gitWorktree', text('gitWorktree'), core.gitWorktree),
    environmentFact('agentModel', text('agentModel'), core.agentModel),
  );
  view.optionalEnvironmentList.replaceChildren(
    environmentFact('pythonFallback', text('pythonFallback'), optional.pythonFallback),
    environmentFact('openViking', text('openViking'), optional.openViking),
    environmentFact('githubAuth', text('githubAuth'), optional.githubAuth),
  );
}

function taskButton(task) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'task-item';
  if (task.taskId === state.selectedTaskId) button.classList.add('is-selected');
  button.dataset.taskId = task.taskId;
  button.setAttribute('aria-pressed', String(task.taskId === state.selectedTaskId));

  const icon = document.createElement('span');
  icon.className = 'task-item__icon';
  icon.setAttribute('aria-hidden', 'true');
  icon.textContent = task.identity && task.identity.item && task.identity.item.kind === 'pr' ? 'PR' : '#';

  const main = document.createElement('span');
  main.className = 'task-item__main';
  const label = document.createElement('strong');
  label.textContent = itemLabel(task.identity && task.identity.item);
  const meta = document.createElement('small');
  const activity = task.lastOutputAt || task.updatedAt;
  meta.textContent = `${phaseLabel(task.phase)} · ${relativeLabel(activity)}`;
  main.append(label, meta);

  const taskState = document.createElement('span');
  taskState.className = 'task-item__state status-dot';
  taskState.dataset.status = task.state || 'recovery_required';
  taskState.title = stateLabel(task.state);
  button.append(icon, main, taskState);
  button.addEventListener('click', () => selectTask(task.taskId));
  return button;
}

function renderTasks() {
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  const sorted = [...tasks].sort((left, right) => Number(right.updatedAt || 0) - Number(left.updatedAt || 0));
  const fragment = document.createDocumentFragment();
  sorted.forEach((task) => fragment.append(taskButton(task)));
  view.taskItems.replaceChildren(fragment);
  view.taskCount.textContent = String(tasks.length);
  view.taskEmpty.hidden = tasks.length !== 0;
  view.allActivity.classList.toggle('is-selected', !state.selectedTaskId);
  view.allActivity.setAttribute('aria-pressed', String(!state.selectedTaskId));
  view.allActivityMeta.textContent = text('eventCount', { value: state.events.length });
}

function latestGate(taskId) {
  for (let index = state.events.length - 1; index >= 0; index -= 1) {
    const event = state.events[index];
    if (event.taskId !== taskId || event.kind !== 'approval_required') continue;
    const details = event.details || {};
    const gateId = details.gateId || details.gate_id || details.id;
    if (gateId) return { event, gateId };
  }
  return null;
}

function makeActionButton(label, action, task, tone) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = tone === 'danger' ? 'danger-button' : 'text-button';
  button.textContent = label;
  button.addEventListener('click', () => {
    if (action === 'gate') openGateDialog(task);
    else void performAction(action, task);
  });
  return button;
}

function renderTaskActions(task) {
  view.taskActions.replaceChildren();
  if (!task || !snapshotSupported()) return;
  const fragment = document.createDocumentFragment();
  if (['preparing', 'queued', 'running', 'retry_wait'].includes(task.state)) {
    fragment.append(makeActionButton(text('pause'), 'pause', task));
  }
  if (['stopped', 'recovery_required', 'failed'].includes(task.state)) {
    fragment.append(makeActionButton(text('resume'), 'resume', task));
  }
  if (task.state === 'waiting_for_user') {
    fragment.append(makeActionButton(text('approvalGate'), 'gate', task));
  }
  if (['stopped', 'completed', 'failed'].includes(task.state)) {
    fragment.append(makeActionButton(text('archive'), 'archive', task));
  }
  if (task.state === 'archived') {
    fragment.append(makeActionButton(text('restore'), 'restore', task));
  }
  view.taskActions.append(fragment);
}

function renderLiveness() {
  const task = selectedTask();
  view.livenessPanel.hidden = !task;
  view.selectedState.hidden = !task;
  if (!task) {
    view.selectedSummary.textContent = text('allTaskEvents');
    view.logTitle.textContent = text('activity');
    return;
  }
  view.logTitle.textContent = itemLabel(task.identity && task.identity.item);
  view.selectedSummary.textContent = text('selectedTaskEvents');
  view.selectedState.hidden = false;
  view.selectedState.dataset.state = task.state;
  view.selectedState.textContent = stateLabel(task.state);
  view.livenessDot.dataset.status = task.state;
  view.livenessPhase.textContent = phaseLabel(task.phase);

  const lastActivity = task.lastOutputAt || task.updatedAt;
  const idleDuration = Date.now() - normalizeTimestamp(lastActivity);
  if (task.state === 'running' && idleDuration > 120000) {
    view.livenessSince.textContent = text('noRecentOutput', { duration: durationLabel(idleDuration) });
    view.livenessDot.dataset.status = 'degraded';
  } else if (task.lastOutputAt) {
    view.livenessSince.textContent = text('lastOutput', { duration: relativeLabel(task.lastOutputAt) });
  } else {
    view.livenessSince.textContent = text('updated', { duration: relativeLabel(task.updatedAt) });
  }

  view.livenessTool.hidden = !task.currentTool;
  view.livenessTool.textContent = task.currentTool ? `${text('currentTool')}: ${task.currentTool}` : '';
  view.livenessTurn.hidden = !task.currentTurnId;
  view.livenessTurn.textContent = task.currentTurnId ? `${text('turn')}: ${task.currentTurnId}` : '';
  const deadline = normalizeTimestamp(task.deadlineAt);
  view.livenessDeadline.hidden = !deadline;
  if (deadline) {
    const delta = deadline - Date.now();
    view.livenessDeadline.textContent = delta >= 0
      ? text('deadlineRemaining', { duration: durationLabel(delta) })
      : text('deadlinePassed', { duration: durationLabel(-delta) });
  }
  view.livenessError.hidden = !task.error;
  view.livenessError.textContent = task.error || '';
  renderTaskActions(task);
}

function eventMatches(event) {
  if (state.selectedTaskId && event.taskId !== state.selectedTaskId) return false;
  if (state.errorsOnly && event.level !== 'error') return false;
  if (
    state.logMode === 'key'
    && !event.important
    && event.level !== 'warning'
    && event.level !== 'error'
    && !KEY_EVENT_KINDS.has(event.kind)
  ) return false;
  if (!state.query) return true;
  const details = event.details ? Object.values(event.details).join(' ') : '';
  const haystack = [event.message, event.source, event.phase, event.kind, event.toolName, details]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
  return haystack.includes(state.query);
}

function eventRow(event) {
  const row = document.createElement('li');
  row.className = 'log-row';
  row.dataset.level = event.level || 'info';
  row.dataset.cursor = String(event.cursor);

  const time = document.createElement('time');
  time.className = 'log-time';
  time.dateTime = new Date(normalizeTimestamp(event.occurredAt)).toISOString();
  time.textContent = clockLabel(event.occurredAt);

  const level = document.createElement('span');
  level.className = 'event-level';
  level.dataset.level = event.level || 'info';
  level.textContent = String(event.level || 'info').toUpperCase();

  const source = document.createElement('span');
  source.className = 'log-source';
  source.textContent = event.source || 'controller';

  const content = document.createElement('div');
  content.className = 'log-content';
  const message = document.createElement('div');
  message.className = 'log-message';
  message.textContent = event.message || event.kind || 'event';
  content.append(message);

  const metaValues = [];
  if (!state.selectedTaskId && event.taskId) metaValues.push(event.taskId);
  if (event.phase) metaValues.push(phaseLabel(event.phase));
  if (event.toolName) metaValues.push(`${text('currentTool')}: ${event.toolName}`);
  if (event.deadlineAt) metaValues.push(`${text('deadline')}: ${clockLabel(event.deadlineAt)}`);
  if (metaValues.length) {
    const meta = document.createElement('div');
    meta.className = 'log-meta';
    metaValues.forEach((value) => {
      const span = document.createElement('span');
      span.textContent = value;
      meta.append(span);
    });
    content.append(meta);
  }

  const detailEntries = Object.entries(event.details || {});
  if (detailEntries.length) {
    const details = document.createElement('details');
    details.className = 'log-details';
    const summary = document.createElement('summary');
    summary.textContent = text('details');
    const list = document.createElement('dl');
    detailEntries.forEach(([key, value]) => {
      const term = document.createElement('dt');
      term.textContent = key;
      const description = document.createElement('dd');
      description.textContent = String(value);
      list.append(term, description);
    });
    details.append(summary, list);
    content.append(details);
  }

  if (event.kind === 'approval_required' && event.taskId) {
    const task = taskForId(event.taskId);
    if (task) {
      const actions = document.createElement('div');
      actions.className = 'task-actions log-meta';
      actions.append(makeActionButton(text('approvalGate'), 'gate', task));
      content.append(actions);
    }
  }

  row.append(time, level, source, content);
  return row;
}

function renderLogs() {
  const wasFollowing = state.followLogs;
  const matches = state.events.filter(eventMatches);
  state.visibleEvents = matches;
  const visible = matches.slice(-MAX_RENDERED_EVENTS);
  const fragment = document.createDocumentFragment();
  visible.forEach((event) => fragment.append(eventRow(event)));
  view.logList.replaceChildren(fragment);
  view.logEmpty.hidden = visible.length !== 0;
  view.visibleLogCount.textContent = `${text('visible')}: ${visible.length}/${matches.length}`;
  if (wasFollowing) {
    requestAnimationFrame(() => {
      view.logScroll.scrollTop = view.logScroll.scrollHeight;
      view.newEvents.hidden = true;
    });
  } else if (visible.length) {
    view.newEvents.hidden = false;
  }
}

function renderStreamStatus() {
  const snapshot = state.snapshot;
  view.streamState.textContent = state.connected ? text('streamCurrent') : text('notConnected');
  view.streamCursor.textContent = snapshot
    ? `${text('cursor')}: ${snapshot.cursor}`
    : `${text('cursor')}: --`;
}

function renderAll() {
  renderExecutionSupport();
  renderEnvironment();
  renderTasks();
  renderLiveness();
  renderLogs();
  renderStreamStatus();
}

function selectTask(taskId) {
  state.selectedTaskId = taskId || null;
  renderTasks();
  renderLiveness();
  renderLogs();
}

function factValue(value, fallback = '--') {
  return value == null || value === '' ? fallback : String(value);
}

function renderPreview(preview) {
  state.preview = preview;
  const repository = preview.repository || {};
  view.intakeDialogTitle.textContent = repositoryLabel(repository);
  view.previewRepository.textContent = repositoryLabel(repository);
  view.previewWorkspace.textContent = preview.workspace && preview.workspace.path
    ? preview.workspace.path
    : text(`workspace_${(preview.workspace && preview.workspace.disposition) || 'unavailable'}`);
  view.previewModel.textContent = factValue(preview.model && preview.model.modelId, view.modelSelect.value);
  view.previewImages.textContent = preview.model && preview.model.supportsImages
    ? text('supported')
    : text('unsupported');

  const candidates = Array.isArray(preview.candidates) ? preview.candidates : [];
  view.candidateCount.textContent = String(candidates.length);
  const candidateFragment = document.createDocumentFragment();
  candidates.forEach((candidate) => {
    const label = document.createElement('label');
    label.className = 'candidate-item';
    label.dataset.state = candidate.state || 'unknown';
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.name = 'candidate';
    input.value = itemKey(candidate.key);
    const resolved = candidate.state === 'closed' || candidate.state === 'merged';
    input.disabled = resolved;
    input.checked = !resolved && candidate.defaultSelected === true;
    input.addEventListener('change', updateCreateButton);

    const copy = document.createElement('span');
    copy.className = 'candidate-copy';
    const title = document.createElement('strong');
    title.textContent = candidate.title || itemLabel(candidate.key);
    const meta = document.createElement('small');
    meta.textContent = candidate.fromRepository
      ? `${itemLabel(candidate.key)} · ${text('fromRepository')}`
      : itemLabel(candidate.key);
    copy.append(title, meta);

    const itemState = document.createElement('span');
    itemState.className = 'candidate-state';
    itemState.textContent = resolved ? text('resolvedItem') : text('openItem');
    label.append(input, copy, itemState);
    candidateFragment.append(label);
  });
  view.candidateList.replaceChildren(candidateFragment);

  const scopes = Array.isArray(preview.permissionScopes) ? preview.permissionScopes : [];
  const permissionFragment = document.createDocumentFragment();
  scopes.forEach((scope) => {
    const highRisk = HIGH_RISK_SCOPES.has(scope);
    const label = document.createElement('label');
    label.className = 'permission-item';
    label.dataset.risk = highRisk ? 'high' : 'standard';
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.name = 'permission';
    input.value = scope;
    input.checked = !highRisk;
    input.addEventListener('change', updateCreateButton);
    const copy = document.createElement('span');
    copy.className = 'permission-copy';
    const title = document.createElement('strong');
    title.textContent = scopeLabel(scope);
    const detail = document.createElement('small');
    detail.textContent = highRisk ? text('scopeHighRisk') : text('scopeStandard');
    copy.append(title, detail);
    const risk = document.createElement('span');
    risk.className = 'candidate-state';
    risk.textContent = highRisk ? '!' : '';
    label.append(input, copy, risk);
    permissionFragment.append(label);
  });
  view.permissionList.replaceChildren(permissionFragment);

  const warnings = [];
  if (preview.truncated) warnings.push(text('truncatedCandidates'));
  if (
    candidates.some((candidate) => candidate.hasImages)
    && preview.model
    && !preview.model.supportsImages
  ) warnings.push(text('imageWarning'));
  if (preview.model && preview.model.available === false) warnings.push(text('modelUnavailable'));
  if (preview.workspace && preview.workspace.disposition === 'unavailable') {
    warnings.push(text('workspaceUnavailable'));
  }
  view.intakeWarning.hidden = warnings.length === 0;
  view.intakeWarning.textContent = warnings.join(' ');
  updateCreateButton();
}

function selectedPreviewItems() {
  if (!state.preview) return [];
  const selectedKeys = new Set(
    [...view.candidateList.querySelectorAll('input[name="candidate"]:checked')]
      .map((input) => input.value),
  );
  return (state.preview.candidates || [])
    .filter((candidate) => selectedKeys.has(itemKey(candidate.key)))
    .map((candidate) => candidate.key);
}

function selectedPermissionScopes() {
  return [...view.permissionList.querySelectorAll('input[name="permission"]:checked')]
    .map((input) => input.value);
}

function updateCreateButton() {
  const itemCount = selectedPreviewItems().length;
  const previewReady = Boolean(state.preview)
    && (!state.preview.model || state.preview.model.available !== false)
    && (!state.preview.workspace || state.preview.workspace.disposition !== 'unavailable');
  view.createButton.disabled = itemCount === 0 || !previewReady;
  view.createButton.textContent = itemCount > 1
    ? `${text('createTasks')} (${itemCount})`
    : text('createTasks');
}

async function loadModelCatalog() {
  const select = view.modelSelect;
  if (!select || select.tagName !== 'SELECT') return;
  if (!app || !app.ai || typeof app.ai.getModels !== 'function') return;
  try {
    const models = await app.ai.getModels();
    const current = select.value || 'auto';
    select.replaceChildren();
    const auto = document.createElement('option');
    auto.value = 'auto';
    auto.textContent = text('modelAuto');
    auto.selected = current === 'auto';
    select.appendChild(auto);
    for (const model of Array.isArray(models) ? models : []) {
      if (!model || !model.id) continue;
      const option = document.createElement('option');
      option.value = model.id;
      const tag = model.isDefault === true ? ` · ${text('modelPrimaryTag')}` : '';
      option.textContent = `${model.name || model.modelName || model.id}${tag}`;
      option.selected = current === model.id;
      select.appendChild(option);
    }
  } catch (error) {
    // The seeded "auto" option remains a safe fallback when the catalog is unavailable.
  }
}

async function resolveIntake() {
  const input = view.intakeInput.value.trim();
  if (!input) {
    view.intakeInput.focus();
    return;
  }
  if (!snapshotSupported()) {
    showNotice(text('intakeUnavailable'), 'error');
    return;
  }
  setButtonBusy(view.resolveButton, true);
  showNotice(text('resolving'));
  try {
    const response = await app.loopx.resolveIntake({
      input,
      modelId: view.modelSelect.value,
    });
    if (!response || !response.preview) throw new Error(text('previewExpired'));
    renderPreview(response.preview);
    showNotice('');
    view.intakeDialog.showModal();
  } catch (error) {
    showNotice(errorMessage(error), 'error');
  } finally {
    setButtonBusy(view.resolveButton, false);
  }
}

function outcomeMessage(outcome) {
  if (outcome && outcome.message) return outcome.message;
  const kind = outcome && outcome.kind;
  if (kind === 'created') return text('taskCreated');
  if (kind === 'opened_existing') return text('openedExisting');
  if (kind === 'closed_noop') return text('closedNoop');
  if (kind === 'needs_live_verification') return text('liveVerification');
  if (kind === 'retry_confirmation_required') return text('retryRequired');
  return kind || text('taskCreated');
}

async function createTasks(retryTerminal) {
  if (!state.preview) {
    showNotice(text('previewExpired'), 'error');
    return;
  }
  const selectedItems = selectedPreviewItems();
  if (!selectedItems.length) {
    view.intakeWarning.hidden = false;
    view.intakeWarning.textContent = text('selectAtLeastOne');
    return;
  }
  const grantedScopes = selectedPermissionScopes();
  if (!grantedScopes.length) {
    view.intakeWarning.hidden = false;
    view.intakeWarning.textContent = text('selectPermissions');
    return;
  }
  const createRequest = {
    clientRequestId: requestId(),
    previewFingerprint: state.preview.fingerprint,
    selectedItems,
    modelId: state.preview.model && state.preview.model.modelId
      ? state.preview.model.modelId
      : view.modelSelect.value,
    grantedScopes,
    retryTerminal: Boolean(retryTerminal),
  };
  state.pendingCreate = createRequest;
  view.createButton.disabled = true;
  view.retryConfirm.disabled = true;
  try {
    const response = await app.loopx.createTask(createRequest);
    const outcomes = response && Array.isArray(response.outcomes) ? response.outcomes : [];
    const retryOutcomes = outcomes.filter((outcome) => outcome.kind === 'retry_confirmation_required');
    if (!retryTerminal && retryOutcomes.length) {
      state.pendingRetry = {
        preview: state.preview,
        itemKeys: new Set(retryOutcomes.map((outcome) => itemKey(outcome.item))),
        scopes: grantedScopes,
      };
      view.retryMessage.textContent = retryOutcomes.map(outcomeMessage).join(' ');
      view.intakeDialog.close();
      view.retryDialog.showModal();
      return;
    }
    const messages = outcomes.map(outcomeMessage);
    const hasError = outcomes.some((outcome) => outcome.kind === 'needs_live_verification');
    showNotice(messages.join(' '), hasError ? 'error' : 'success');
    view.intakeDialog.close();
    view.retryDialog.close();
    state.preview = null;
    state.pendingRetry = null;
    await attachSnapshot(false);
    const openedTask = outcomes.find((outcome) => outcome.taskId);
    if (openedTask && openedTask.taskId) selectTask(openedTask.taskId);
  } catch (error) {
    showNotice(errorMessage(error), 'error');
  } finally {
    state.pendingCreate = null;
    view.retryConfirm.disabled = false;
    updateCreateButton();
  }
}

async function confirmRetry() {
  const pending = state.pendingRetry;
  if (!pending || !state.preview) {
    view.retryDialog.close();
    showNotice(text('previewExpired'), 'error');
    return;
  }
  view.candidateList.querySelectorAll('input[name="candidate"]').forEach((input) => {
    input.checked = pending.itemKeys.has(input.value);
  });
  await createTasks(true);
}

async function performAction(action, task, extra = {}) {
  if (!snapshotSupported()) {
    showNotice(text('intakeUnavailable'), 'error');
    return false;
  }
  const expectedRevision = task
    ? Number(task.revision || 0)
    : Number((state.snapshot && state.snapshot.revision) || 0);
  const request = {
    action,
    clientRequestId: requestId(),
    expectedRevision,
    ...(task ? { taskId: task.taskId } : {}),
    ...(extra.gateId ? { gateId: extra.gateId } : {}),
    ...(extra.note ? { note: extra.note } : {}),
  };
  try {
    const response = await app.loopx.action(request);
    if (response && response.task && state.snapshot) {
      const index = state.snapshot.tasks.findIndex((item) => item.taskId === response.task.taskId);
      if (index >= 0) state.snapshot.tasks.splice(index, 1, response.task);
      else state.snapshot.tasks.push(response.task);
      state.snapshot.revision = Math.max(state.snapshot.revision, response.currentRevision || 0);
    }
    const status = response && response.status;
    if (status === 'revision_conflict') {
      showNotice(response.message || text('revisionConflict'), 'error');
      await attachSnapshot(false);
      return false;
    } else if (status === 'rejected') {
      showNotice(response.message || text('actionRejected'), 'error');
      return false;
    } else if (status === 'duplicate') {
      showNotice(response.message || text('actionDuplicate'));
    } else {
      showNotice(response && response.message ? response.message : text('actionApplied'), 'success');
      await attachSnapshot(false);
    }
    return true;
  } catch (error) {
    showNotice(errorMessage(error), 'error');
    await attachSnapshot(false);
    return false;
  }
}

function openGateDialog(task) {
  const gate = latestGate(task.taskId);
  if (!gate) {
    showNotice(text('noGate'), 'error');
    return;
  }
  state.gate = { task, gateId: gate.gateId };
  view.gateTitle.textContent = itemLabel(task.identity && task.identity.item);
  view.gateMessage.textContent = gate.event.message || text('approvalNeeded');
  view.gateNote.value = '';
  view.gateDialog.showModal();
  view.gateNote.focus();
}

async function answerGate(action) {
  if (!state.gate) return;
  const gate = state.gate;
  const note = view.gateNote.value.trim();
  view.gateApprove.disabled = true;
  view.gateReject.disabled = true;
  try {
    const applied = await performAction(action, gate.task, { gateId: gate.gateId, note });
    if (applied) {
      state.gate = null;
      view.gateDialog.close();
    }
  } finally {
    view.gateApprove.disabled = false;
    view.gateReject.disabled = false;
  }
}

function filteredExportEvents() {
  return state.visibleEvents.map((event) => ({
    streamId: event.streamId,
    cursor: event.cursor,
    taskId: event.taskId || null,
    kind: event.kind,
    level: event.level,
    source: event.source,
    phase: event.phase || null,
    message: event.message,
    important: Boolean(event.important),
    toolName: event.toolName || null,
    deadlineAt: event.deadlineAt || null,
    details: event.details || {},
    occurredAt: event.occurredAt,
  }));
}

function exportLogs() {
  const payload = {
    schemaVersion: state.snapshot && state.snapshot.schemaVersion,
    streamId: state.snapshot && state.snapshot.streamId,
    exportedAt: Date.now(),
    selectedTaskId: state.selectedTaskId,
    mode: state.logMode,
    query: state.query,
    errorsOnly: state.errorsOnly,
    events: filteredExportEvents(),
  };
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = `loopx-events-${Date.now()}.json`;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
  showNotice(text('logsExported'), 'success');
}

function setLogMode(mode) {
  state.logMode = mode;
  const keyMode = mode === 'key';
  view.modeKey.classList.toggle('is-active', keyMode);
  view.modeKey.setAttribute('aria-checked', String(keyMode));
  view.modeFull.classList.toggle('is-active', !keyMode);
  view.modeFull.setAttribute('aria-checked', String(!keyMode));
  renderLogs();
}

function bindEvents() {
  view.intakeForm.addEventListener('submit', (event) => {
    event.preventDefault();
    void resolveIntake();
  });
  view.syncButton.addEventListener('click', () => void attachSnapshot(false));
  view.retryEnvironment.addEventListener('click', async () => {
    await performAction('retry_environment', null);
  });
  view.collapseTasks.addEventListener('click', () => {
    state.railCollapsed = !state.railCollapsed;
    view.taskRail.classList.toggle('is-collapsed', state.railCollapsed);
    view.taskRail.parentElement.classList.toggle('tasks-collapsed', state.railCollapsed);
    view.collapseTasks.setAttribute('aria-expanded', String(!state.railCollapsed));
    view.collapseTasks.setAttribute(
      'title',
      state.railCollapsed ? text('expandTasks') : text('collapseTasks'),
    );
  });
  view.allActivity.addEventListener('click', () => selectTask(null));
  view.modeKey.addEventListener('click', () => setLogMode('key'));
  view.modeFull.addEventListener('click', () => setLogMode('full'));
  view.logSearch.addEventListener('input', () => {
    state.query = view.logSearch.value.trim().toLowerCase();
    renderLogs();
  });
  view.errorsOnly.addEventListener('change', () => {
    state.errorsOnly = view.errorsOnly.checked;
    renderLogs();
  });
  view.exportLogs.addEventListener('click', exportLogs);
  view.logScroll.addEventListener('scroll', () => {
    const remaining = view.logScroll.scrollHeight - view.logScroll.scrollTop - view.logScroll.clientHeight;
    state.followLogs = remaining < 40;
    if (state.followLogs) view.newEvents.hidden = true;
  }, { passive: true });
  view.newEvents.addEventListener('click', () => {
    state.followLogs = true;
    renderLogs();
  });
  document.querySelectorAll('.dialog-close').forEach((button) => {
    button.addEventListener('click', () => button.closest('dialog').close());
  });
  view.intakeConfirmForm.addEventListener('submit', (event) => {
    event.preventDefault();
    void createTasks(false);
  });
  view.retryCancel.addEventListener('click', () => {
    state.pendingRetry = null;
    view.retryDialog.close();
  });
  view.retryConfirm.addEventListener('click', (event) => {
    event.preventDefault();
    void confirmRetry();
  });
  view.gateCancel.addEventListener('click', () => {
    state.gate = null;
    view.gateDialog.close();
  });
  view.gateForm.addEventListener('submit', (event) => {
    event.preventDefault();
    void answerGate('approve');
  });
  view.gateReject.addEventListener('click', () => void answerGate('reject'));
}

function updateLivenessClock() {
  renderLiveness();
  renderTasks();
}

async function start() {
  bindEvents();
  applyLocale();
  void loadModelCatalog();
  if (!app || !app.loopx) {
    showBridgeUnavailable();
    return;
  }
  app.loopx.onEvent(onLoopxEvent);
  if (typeof app.onLocaleChange === 'function') app.onLocaleChange(applyLocale);
  if (typeof app.onActivate === 'function') app.onActivate(() => void attachSnapshot(false));
  window.addEventListener('beforeunload', () => {
    if (app.loopx && typeof app.loopx.offEvent === 'function') {
      app.loopx.offEvent(onLoopxEvent);
    }
  });
  window.setInterval(updateLivenessClock, 15000);
  await attachSnapshot(true);
}

void start();
