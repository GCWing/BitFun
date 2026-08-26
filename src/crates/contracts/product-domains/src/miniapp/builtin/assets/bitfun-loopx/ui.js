'use strict';

// LoopX is owned by the BitFun host. This file only projects durable snapshots
// and cursor-addressed events into the MiniApp UI.
const app = window.app;
const byId = (id) => document.getElementById(id);
const MAX_EVENTS = 2000;
const MAX_RENDERED_EVENTS = 600;
const MAX_TURN_OUTPUT_EVENTS = 1200;
const MODEL_SELECTION_STORAGE_KEY = 'loopx.modelId';
const HIGH_RISK_SCOPES = new Set([
  'publish',
  'public_comment',
  'pull_request',
  'merge',
  'production_action',
]);
const KEY_EVENT_KINDS = new Set([
  'task_created',
  'progress',
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
    modelLoading: '正在加载模型…',
    modelReloadTitle: '刷新模型列表',
    modelSelectionChanged: '已切换模型，请重新分析链接。',
    modelPrimaryTag: '主模型',
    resolve: '分析链接',
    resolving: '正在实时复核链接',
    refresh: '刷新状态',
    resetLoopx: '重置 LoopX',
    resettingLoopx: '正在重置 LoopX…',
    destructiveAction: '危险操作',
    resetLoopxTitle: '清空并重新开始',
    resetLoopxMessage: '将停止并删除 {tasks} 个任务、{events} 条日志、全部 Goal 状态和受管工作区。此操作不可撤销。',
    resetLoopxRetained: '模型配置、GitHub 登录和 MiniApp 设置会保留。',
    resetLoopxConfirm: '清空并重新开始',
    resetLoopxApplied: 'LoopX 已清空，可以重新开始测试。',
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
    resizeTasks: '调整任务栏宽度',
    expandTasks: '展开任务栏',
    allActivity: '全部活动',
    noTasks: '暂无任务',
    activity: '运行日志',
    allTaskEvents: '所有任务的实时日志和阶段进度',
    selectedTaskEvents: '当前任务的事件与运行状态',
    mainProgress: '进行中 {active} · 排队 {queued}',
    mainProgressOnlyQueued: '排队 {queued} · 等待调度',
    mainProgressIdle: '暂无进行中的任务',
    mainProgressCurrent: '当前：{item}',
    mainProgressPhase: '阶段：{phase}',
    worktreeQuiet: '正在准备 Worktree：{item}。首次克隆可能需要几分钟；Git 静默时不会产生子进程输出。',
    mainProgressWaiting: '正在同步任务状态；阶段变化和宿主输出会继续出现在这里。',
    keyEvents: '关键事件',
    fullLog: '完整日志',
    liveOutput: '实时输出',
    searchLogs: '搜索日志',
    errorsOnly: '仅错误',
    exportLogs: '导出日志',
    noLogs: '暂无运行事件',
    noLiveOutput: '暂无实时模型输出',
    outputUnavailable: '实时输出暂不可用',
    outputNotRunning: '当前任务没有正在运行的 Agent turn',
    outputThinking: '思考',
    outputTool: '工具',
    outputModel: '模型',
    outputText: '输出',
    outputChunks: '{value} 段',
    newEvents: '查看新事件',
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
    approvalGate: '查看并决定',
    approvalNote: '审批备注',
    approvalNotePlaceholder: '补充批准或拒绝的原因（可选）',
    reject: '拒绝',
    approve: '批准',
    pause: '暂停',
    resume: '恢复',
    continueRun: '仅继续此任务',
    resumeRepository: '恢复此仓库的异常任务（{value}）',
    resumingRepository: '正在恢复异常任务…',
    repositorySerial: '同仓库串行执行',
    batchAction: '批量操作',
    resumeRepositoryTitle: '恢复此仓库的异常任务',
    confirmContinue: '确认继续',
    resumeRepositoryMessage: '将恢复 {repository} 中 {value} 个失败或需要恢复的任务。同一时间只运行 1 个，其余任务进入队列。',
    resumeRepositoryApplied: '已将 {value} 个异常任务加入仓库队列。',
    repositoryPausedByModel: '模型请求失败，仓库队列已暂停',
    fixModelBeforeContinue: '修复模型并重新检查环境后继续',
    groupDecision: '待你决定',
    groupError: '运行异常',
    groupActive: '正在运行',
    groupQueued: '排队中',
    groupPaused: '已暂停',
    groupCompleted: '已完成',
    groupArchived: '已归档',
    expandGroup: '展开',
    collapseGroup: '收起',
    expandGroupTitle: '展开 {label}（{count}）',
    collapseGroupTitle: '收起 {label}（{count}）',
    runningNow: '运行中',
    decisionCountTitle: '待你决定的任务',
    viewError: '查看错误',
    archive: '归档并清理工作区',
    restore: '还原',
    retry: '重试',
    details: '详情',
    currentTool: '工具',
    turn: 'Turn',
    goal: 'Goal',
    deadline: '截止',
    noRecentOutput: '已 {duration} 没有新输出',
    lastOutput: '最后输出 {duration} 前',
    updated: '更新于 {duration} 前',
    openInGithub: '在 GitHub 中打开',
    issueDescription: 'Issue 描述',
    loadingIssueDescription: '正在加载 Issue 描述…',
    issueDescriptionUnavailable: '暂时无法加载 Issue 描述。',
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
    selectAll: '全选',
    selectPermissions: '请确认本次任务需要的权限范围。',
    previewExpired: '确认信息已失效，请重新分析链接。',
    taskCreated: '任务已创建。',
    tasksCreated: '已创建 {value} 个任务。',
    outcomeCount: '{message}（{value} 项）',
    selectedCandidates: '已选 {selected} / {total}',
    batchSelection: '将为已选的 {value} 个不同 Issue / PR 分别创建独立任务，工作区会逐个准备。',
    workspacePreparationFailed: '工作区准备失败',
    queuedRepoBusy: '等待同仓库当前任务完成',
    queuedBoundedWait: '回合之间等待调度（有界回合）',
    openedExisting: '已打开现有任务，没有重复创建。',
    closedNoop: '目标已关闭或合并，无需创建任务。',
    liveVerification: '目标需要再次在线复核，暂未创建任务。',
    retryRequired: '该目标已有终态任务。只有确认后才会创建新的 attempt。',
    actionApplied: '操作已应用。',
    actionPending: '正在提交操作',
    pausePending: '正在暂停',
    resumePending: '正在继续',
    archivePending: '正在归档',
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
    phase_recovering: '恢复并同步',
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
    modelLoading: 'Loading models...',
    modelReloadTitle: 'Refresh model list',
    modelSelectionChanged: 'Model changed. Analyze the URL again.',
    modelPrimaryTag: 'Primary',
    resolve: 'Analyze URL',
    resolving: 'Verifying URL against the live source',
    refresh: 'Refresh status',
    resetLoopx: 'Reset LoopX',
    resettingLoopx: 'Resetting LoopX...',
    destructiveAction: 'Destructive action',
    resetLoopxTitle: 'Clear and start over',
    resetLoopxMessage: 'Stop and delete {tasks} tasks, {events} log events, every Goal state, and all managed workspaces. This cannot be undone.',
    resetLoopxRetained: 'Model configuration, GitHub login, and MiniApp settings are retained.',
    resetLoopxConfirm: 'Clear and start over',
    resetLoopxApplied: 'LoopX was cleared. You can start a fresh test.',
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
    resizeTasks: 'Resize task rail',
    expandTasks: 'Expand task rail',
    allActivity: 'All activity',
    noTasks: 'No tasks yet',
    activity: 'Run log',
    allTaskEvents: 'Live logs and phase progress for every task',
    selectedTaskEvents: 'Events and liveness for the selected task',
    mainProgress: '{active} active · {queued} queued',
    mainProgressOnlyQueued: '{queued} queued · waiting for the scheduler',
    mainProgressIdle: 'No tasks are currently active',
    mainProgressCurrent: 'Current: {item}',
    mainProgressPhase: 'Phase: {phase}',
    worktreeQuiet: 'Preparing worktree: {item}. The first clone can take a few minutes; Git may not emit output while it is working.',
    mainProgressWaiting: 'Syncing task state; phase changes and host output will continue to appear here.',
    keyEvents: 'Key events',
    fullLog: 'Full log',
    liveOutput: 'Live output',
    searchLogs: 'Search logs',
    errorsOnly: 'Errors only',
    exportLogs: 'Export logs',
    noLogs: 'No run events yet',
    noLiveOutput: 'No live model output yet',
    outputUnavailable: 'Live output is unavailable',
    outputNotRunning: 'This task has no running Agent turn',
    outputThinking: 'Thinking',
    outputTool: 'Tool',
    outputModel: 'Model',
    outputText: 'Output',
    outputChunks: '{value} chunks',
    newEvents: 'View new events',
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
    approvalGate: 'Review and decide',
    approvalNote: 'Approval note',
    approvalNotePlaceholder: 'Optional reason for approving or rejecting',
    reject: 'Reject',
    approve: 'Approve',
    pause: 'Pause',
    resume: 'Resume',
    continueRun: 'Continue only this task',
    resumeRepository: 'Recover repository failures ({value})',
    resumingRepository: 'Recovering failed tasks...',
    repositorySerial: 'Runs serially per repository',
    batchAction: 'Batch action',
    resumeRepositoryTitle: 'Recover repository failures',
    confirmContinue: 'Continue tasks',
    resumeRepositoryMessage: 'Recover {value} failed or recovery-required tasks in {repository}. One task runs at a time; the rest remain queued.',
    resumeRepositoryApplied: 'Queued {value} failed repository tasks.',
    repositoryPausedByModel: 'Model request failed; repository queue paused',
    fixModelBeforeContinue: 'Fix the model and recheck the environment',
    groupDecision: 'Waiting for you',
    groupError: 'Run failures',
    groupActive: 'Running',
    groupQueued: 'Queued',
    groupPaused: 'Paused',
    groupCompleted: 'Completed',
    groupArchived: 'Archived',
    expandGroup: 'Expand',
    collapseGroup: 'Collapse',
    expandGroupTitle: 'Expand {label} ({count})',
    collapseGroupTitle: 'Collapse {label} ({count})',
    runningNow: 'Running',
    decisionCountTitle: 'Tasks waiting for your decision',
    viewError: 'View error',
    archive: 'Archive & clean workspace',
    restore: 'Restore',
    retry: 'Retry',
    details: 'Details',
    currentTool: 'tool',
    turn: 'Turn',
    goal: 'Goal',
    deadline: 'deadline',
    noRecentOutput: 'No new output for {duration}',
    lastOutput: 'Last output {duration} ago',
    updated: 'Updated {duration} ago',
    openInGithub: 'Open in GitHub',
    issueDescription: 'Issue description',
    loadingIssueDescription: 'Loading issue description...',
    issueDescriptionUnavailable: 'Issue description is temporarily unavailable.',
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
    selectAll: 'Select all',
    selectPermissions: 'Confirm the permission scopes required by this run.',
    previewExpired: 'This preview is stale. Analyze the URL again.',
    taskCreated: 'Task created.',
    tasksCreated: '{value} tasks created.',
    outcomeCount: '{message} ({value} items)',
    selectedCandidates: '{selected} / {total} selected',
    batchSelection: 'Each of the {value} selected issues or pull requests will become a separate task. Workspaces are prepared one at a time.',
    workspacePreparationFailed: 'Workspace setup failed',
    queuedRepoBusy: 'Waiting for the active task in this repository',
    queuedBoundedWait: 'Between bounded turns',
    openedExisting: 'Opened the existing task without creating a duplicate.',
    closedNoop: 'The target is closed or merged; no task was created.',
    liveVerification: 'The target needs another live verification before a task can be created.',
    retryRequired: 'A terminal task exists. Confirm before creating a new attempt.',
    actionApplied: 'Action applied.',
    actionPending: 'Applying action',
    pausePending: 'Pausing',
    resumePending: 'Continuing',
    archivePending: 'Archiving',
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
    phase_recovering: 'Recovering and syncing',
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
  resetLoopx: byId('reset-loopx'),
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
  railSplitter: byId('rail-splitter'),
  collapseTasks: byId('collapse-tasks'),
  taskCount: byId('task-count'),
  decisionCount: byId('decision-count'),
  repositoryActions: byId('repository-actions'),
  resumeRepository: byId('resume-repository'),
  repositoryActionsMeta: byId('repository-actions-meta'),
  taskItems: byId('task-items'),
  taskEmpty: byId('task-empty'),
  showAllEvents: byId('show-all-events'),
  logTitle: byId('log-title'),
  selectedState: byId('selected-state'),
  selectedSummary: byId('selected-summary'),
  modeKey: byId('mode-key'),
  modeFull: byId('mode-full'),
  modeOutput: byId('mode-output'),
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
  livenessGoal: byId('liveness-goal'),
  livenessError: byId('liveness-error'),
  livenessItem: byId('liveness-item'),
  issueDescriptionPanel: byId('issue-description-panel'),
  livenessDescription: byId('liveness-description'),
  taskActions: byId('task-actions'),
  logScroll: byId('log-scroll'),
  logEmpty: byId('log-empty'),
  logList: byId('log-list'),
  newEvents: byId('new-events'),
  intakeDialog: byId('intake-dialog'),
  intakeConfirmForm: byId('intake-confirm-form'),
  intakeDialogTitle: byId('intake-dialog-title'),
  previewRepository: byId('preview-repository'),
  previewWorkspace: byId('preview-workspace'),
  previewModel: byId('preview-model'),
  previewImages: byId('preview-images'),
  candidateCount: byId('candidate-count'),
  candidateSelectAll: byId('candidate-select-all'),
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
  repositoryResumeDialog: byId('repository-resume-dialog'),
  repositoryResumeMessage: byId('repository-resume-message'),
  repositoryResumeCancel: byId('repository-resume-cancel'),
  repositoryResumeConfirm: byId('repository-resume-confirm'),
  resetLoopxDialog: byId('reset-loopx-dialog'),
  resetLoopxMessage: byId('reset-loopx-message'),
  resetLoopxCancel: byId('reset-loopx-cancel'),
  resetLoopxConfirm: byId('reset-loopx-confirm'),
};

const state = {
  snapshot: null,
  events: [],
  eventKeys: new Set(),
  selectedTaskId: null,
  logMode: 'full',
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
  taskGroupOpen: {},
  visibleEvents: [],
  turnOutput: {
    taskId: null,
    turnId: null,
    streamId: null,
    cursor: 0,
    events: [],
    message: '',
    status: 'not_running',
    inFlight: false,
    timer: null,
  },
  itemMetadata: new Map(),
  metadataRequests: new Set(),
  repositoryResumeTarget: null,
  repositoryResumePending: false,
  taskActionPending: new Map(),
  modelCatalogLoading: false,
  modelCatalogLoaded: false,
  resetPending: false,
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

function isWorkspacePreparationFailure(task) {
  return Boolean(task && task.error && !task.workspacePath && !task.goalId);
}

function taskStateLabel(task) {
  return isWorkspacePreparationFailure(task) ? stateLabel('failed') : stateLabel(task && task.state);
}

function pendingActionFor(task) {
  return task && task.taskId ? state.taskActionPending.get(task.taskId) : '';
}

function taskVisualState(task) {
  const pending = pendingActionFor(task);
  if (pending === 'pause' || pending === 'abort') return 'cancelling';
  return isWorkspacePreparationFailure(task)
    ? 'failed'
    : ((task && task.state) || 'recovery_required');
}

function taskStateDisplayLabel(task) {
  const pending = pendingActionFor(task);
  if (pending === 'pause' || pending === 'abort') return text('pausePending');
  if (pending === 'resume' || pending === 'restore') return text('resumePending');
  if (pending === 'archive') return text('archivePending');
  if (pending) return text('actionPending');
  return taskStateLabel(task);
}

function taskPhaseLabel(task) {
  return isWorkspacePreparationFailure(task)
    ? text('workspacePreparationFailed')
    : phaseLabel(task && task.phase);
}

function compactItemLabel(item) {
  if (!item) return '--';
  return `${item.kind === 'pr' ? 'PR' : 'Issue'} #${item.number}`;
}

function shortId(value) {
  const raw = value == null ? '' : String(value);
  return raw.length > 14 ? raw.slice(0, 8) : raw;
}

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

function readStoredModelSelection() {
  try {
    return window.localStorage.getItem(MODEL_SELECTION_STORAGE_KEY) || '';
  } catch (_error) {
    return '';
  }
}

function writeStoredModelSelection(value) {
  try {
    window.localStorage.setItem(MODEL_SELECTION_STORAGE_KEY, value || 'auto');
  } catch (_error) {
    // Ignore storage failures; the current select value still applies.
  }
}

function currentModelSelection() {
  const stored = readStoredModelSelection();
  const selected = view.modelSelect && view.modelSelect.value ? view.modelSelect.value : '';
  if (selected && (selected !== 'auto' || !stored || stored === 'auto')) return selected;
  return stored || selected || 'auto';
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

function itemUrl(item) {
  if (!item || !item.repository) return '';
  const { host, owner, repository } = item.repository;
  if (!host || !owner || !repository || !item.number) return '';
  const path = item.kind === 'pr' ? 'pull' : 'issues';
  return `https://${host}/${owner}/${repository}/${path}/${item.number}`;
}

function identityTitleOf(task) {
  const title = task && task.identity && task.identity.title;
  if (typeof title === 'string' && title.trim()) return title.trim();
  const item = task && task.identity && task.identity.item;
  return (state.itemMetadata.get(itemKey(item)) || {}).title || '';
}

function identityDescriptionOf(task) {
  const description = task && task.identity && task.identity.description;
  if (typeof description === 'string' && description.trim()) return description.trim();
  const item = task && task.identity && task.identity.item;
  return (state.itemMetadata.get(itemKey(item)) || {}).description || '';
}

function latestTaskWaitReason(task) {
  if (!task || task.state !== 'queued') return '';
  for (let index = state.events.length - 1; index >= 0; index -= 1) {
    const event = state.events[index];
    if (event.taskId !== task.taskId || !event.message) continue;
    const message = String(event.message);
    if (/another task for this repository/i.test(message)) return text('queuedRepoBusy');
    if (/bounded turn/i.test(message)) return text('queuedBoundedWait');
    return message;
  }
  return text('queuedRepoBusy');
}

function taskForId(taskId) {
  if (!state.snapshot || !Array.isArray(state.snapshot.tasks)) return null;
  return state.snapshot.tasks.find((task) => task.taskId === taskId) || null;
}

function selectedTask() {
  return state.selectedTaskId ? taskForId(state.selectedTaskId) : null;
}

function runningOutputTask() {
  const selected = selectedTask();
  if (selected && selected.state === 'running') return selected;
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks)
    ? state.snapshot.tasks
    : [];
  return tasks.find((task) => task.state === 'running') || null;
}

function resetTurnOutput(task) {
  state.turnOutput.taskId = task ? task.taskId : null;
  state.turnOutput.turnId = task ? (task.currentTurnId || null) : null;
  state.turnOutput.streamId = null;
  state.turnOutput.cursor = 0;
  state.turnOutput.events = [];
  state.turnOutput.message = '';
  state.turnOutput.status = task ? 'current' : 'not_running';
}

function ensureTurnOutputTarget(task) {
  const currentTurn = task && task.currentTurnId ? task.currentTurnId : null;
  if (
    state.turnOutput.taskId !== (task && task.taskId)
    || state.turnOutput.turnId !== currentTurn
  ) {
    resetTurnOutput(task);
  }
}

function clearTurnOutputTimer() {
  if (state.turnOutput.timer) {
    clearTimeout(state.turnOutput.timer);
    state.turnOutput.timer = null;
  }
}

function scheduleTurnOutputPoll(delay = 1200) {
  clearTurnOutputTimer();
  if (state.logMode !== 'output') return;
  const task = runningOutputTask();
  if (!task || !app || !app.loopx || typeof app.loopx.turnOutputSince !== 'function') return;
  state.turnOutput.timer = setTimeout(() => {
    state.turnOutput.timer = null;
    void refreshTurnOutput();
  }, delay);
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

async function refreshTurnOutput() {
  if (state.turnOutput.inFlight || state.logMode !== 'output') return;
  const task = runningOutputTask();
  if (!task) {
    resetTurnOutput(null);
    renderLogs();
    return;
  }
  ensureTurnOutputTarget(task);
  if (!app || !app.loopx || typeof app.loopx.turnOutputSince !== 'function') {
    state.turnOutput.status = 'output_unavailable';
    state.turnOutput.message = text('outputUnavailable');
    renderLogs();
    return;
  }

  state.turnOutput.inFlight = true;
  try {
    const page = await app.loopx.turnOutputSince({
      taskId: task.taskId,
      ...(state.turnOutput.turnId ? { turnId: state.turnOutput.turnId } : {}),
      ...(state.turnOutput.streamId ? { streamId: state.turnOutput.streamId } : {}),
      afterCursor: state.turnOutput.cursor,
      limit: 200,
    });
    if (!page || page.taskId !== task.taskId) {
      state.turnOutput.status = 'output_unavailable';
      state.turnOutput.message = text('outputUnavailable');
      return;
    }
    if (page.turnId && page.turnId !== state.turnOutput.turnId) {
      resetTurnOutput({ ...task, currentTurnId: page.turnId });
    }
    if (page.streamId && page.streamId !== state.turnOutput.streamId) {
      state.turnOutput.streamId = page.streamId;
      state.turnOutput.cursor = 0;
      state.turnOutput.events = [];
    }
    state.turnOutput.status = page.status || 'current';
    state.turnOutput.message = page.message || '';
    (page.events || []).forEach((event) => {
      if (!Number.isSafeInteger(event.cursor)) return;
      if (state.turnOutput.events.some((existing) => existing.cursor === event.cursor)) return;
      state.turnOutput.events.push(event);
    });
    state.turnOutput.events.sort((left, right) => left.cursor - right.cursor);
    while (state.turnOutput.events.length > MAX_TURN_OUTPUT_EVENTS) {
      state.turnOutput.events.shift();
    }
    state.turnOutput.cursor = Math.max(
      state.turnOutput.cursor,
      Number(page.nextCursor || state.turnOutput.cursor),
    );
    if (page.hasMore) scheduleTurnOutputPoll(0);
    else scheduleTurnOutputPoll(1200);
  } catch (error) {
    state.turnOutput.status = 'output_unavailable';
    state.turnOutput.message = error instanceof Error ? error.message : String(error);
    scheduleTurnOutputPoll(3000);
  } finally {
    state.turnOutput.inFlight = false;
    renderLogs();
  }
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
  if (!view.repositoryActions.hidden) view.resumeRepository.disabled = true;
  try {
    do {
      state.syncRequested = false;
      const knownStreamId = state.snapshot && state.snapshot.streamId;
      const afterCursor = state.snapshot && state.snapshot.cursor;
      if (!state.connected) view.connectionLabel.textContent = text('connecting');
      const response = await app.loopx.attach({
        ...(knownStreamId ? { knownStreamId } : {}),
        ...(Number.isSafeInteger(afterCursor) ? { afterCursor } : {}),
      });
      applySnapshot(response && response.snapshot);
      void loadModelCatalog();
      const snapshot = state.snapshot;
      if (loadHistory && state.events.length === 0 && snapshot.cursor > 0) {
        const replay = await replayEvents(snapshot.streamId, 0, true);
        if (replay.changed) renderLogs();
      }
    } while (state.syncRequested);
  } catch (error) {
    state.connected = false;
    view.connectionLabel.textContent = text('connectionFailed');
    showNotice(errorMessage(error), 'error');
  } finally {
    state.syncing = false;
    setButtonBusy(view.syncButton, false);
    const tasks = state.snapshot && Array.isArray(state.snapshot.tasks)
      ? state.snapshot.tasks
      : [];
    renderRepositoryActions(tasks);
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
  if (event.kind === 'snapshot_invalidated' && event.cursor === 0) {
    state.syncRequested = true;
    queueMicrotask(() => void attachSnapshot(false));
    return;
  }
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
  const environmentStatus = snapshot && snapshot.environment && snapshot.environment.status;
  const coreReady = environmentStatus === 'ready' || environmentStatus === 'degraded';
  view.unsupportedBanner.hidden = !snapshot || supported;
  if (snapshot && !supported) {
    view.unsupportedReason.textContent = snapshot.unsupportedReason || text('unsupportedDefault');
  }
  view.resolveButton.disabled = !supported || !coreReady;
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

const DECISION_TASK_STATES = new Set(['waiting_for_user']);
const ERROR_TASK_STATES = new Set(['recovery_required', 'failed']);
const ACTIVE_TASK_STATES = new Set([
  'running',
  'preparing',
  'cancelling',
]);
const QUEUED_TASK_STATES = new Set(['queued', 'retry_wait']);
const GLOBAL_PROGRESS_TASK_STATES = new Set([...ACTIVE_TASK_STATES, ...QUEUED_TASK_STATES]);

function repositoryKey(repository) {
  return repository
    ? `${repository.host || ''}/${repository.owner || ''}/${repository.repository || ''}`
    : '';
}

function taskGroupKey(task) {
  if (DECISION_TASK_STATES.has(task.state)) return 'decision';
  if (ERROR_TASK_STATES.has(task.state)) return 'error';
  if (ACTIVE_TASK_STATES.has(task.state)) return 'active';
  if (QUEUED_TASK_STATES.has(task.state)) return 'queued';
  if (task.state === 'stopped') return 'paused';
  if (task.state === 'archived') return 'archived';
  if (task.state === 'completed') return 'completed';
  return 'error';
}

function taskSortPriority(task) {
  const priorities = {
    running: 0,
    waiting_for_user: 1,
    preparing: 2,
    cancelling: 3,
    queued: 4,
    retry_wait: 5,
    failed: 10,
    recovery_required: 11,
    stopped: 12,
  };
  return priorities[task.state] ?? 20;
}

function sortedTaskList(tasks) {
  return [...tasks].sort((left, right) =>
    taskSortPriority(left) - taskSortPriority(right)
    || Number(right.updatedAt || 0) - Number(left.updatedAt || 0));
}

function primaryProgressTask(tasks) {
  return sortedTaskList(tasks).find((task) => GLOBAL_PROGRESS_TASK_STATES.has(task.state)) || null;
}

function progressCounts(tasks) {
  return {
    active: tasks.filter((task) => ACTIVE_TASK_STATES.has(task.state)).length,
    queued: tasks.filter((task) => QUEUED_TASK_STATES.has(task.state)).length,
  };
}

function progressSummary(counts) {
  if (counts.active > 0) {
    return text('mainProgress', counts);
  }
  if (counts.queued > 0) {
    return text('mainProgressOnlyQueued', counts);
  }
  return text('mainProgressIdle');
}

function progressItemLabel(task) {
  const item = task && task.identity && task.identity.item;
  return identityTitleOf(task) || compactItemLabel(item);
}

function activeWorkEmptyMessage() {
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  const task = primaryProgressTask(tasks);
  if (!task) return text('noLogs');
  if (task.phase === 'preparing_workspace') {
    return text('worktreeQuiet', { item: progressItemLabel(task) });
  }
  return text('mainProgressWaiting');
}

function taskStatusIcon(task) {
  const status = isWorkspacePreparationFailure(task) ? 'failed' : task.state;
  const symbols = {
    running: '▶',
    preparing: '…',
    queued: '◷',
    retry_wait: '◷',
    waiting_for_user: '?',
    cancelling: '■',
    recovery_required: '↻',
    failed: '!',
    stopped: 'Ⅱ',
    completed: '✓',
    archived: '□',
  };
  return { status, symbol: symbols[status] || '•' };
}

function recoverableTasksForRepository(tasks, repository) {
  const key = repositoryKey(repository);
  return tasks.filter((task) =>
    ERROR_TASK_STATES.has(task.state)
    && repositoryKey(task.identity && task.identity.item && task.identity.item.repository) === key);
}

function renderRepositoryActions(tasks) {
  const selected = selectedTask();
  const eligibleRepositories = new Map();
  tasks.forEach((task) => {
    if (!ERROR_TASK_STATES.has(task.state)) return;
    const repository = task.identity && task.identity.item && task.identity.item.repository;
    const key = repositoryKey(repository);
    if (key && !eligibleRepositories.has(key)) eligibleRepositories.set(key, repository);
  });
  const selectedRepository = selected
    && selected.identity
    && selected.identity.item
    && selected.identity.item.repository;
  const repository = selectedRepository && eligibleRepositories.has(repositoryKey(selectedRepository))
    ? selectedRepository
    : (eligibleRepositories.size === 1 ? [...eligibleRepositories.values()][0] : null);
  const eligible = repository ? recoverableTasksForRepository(tasks, repository) : [];
  state.repositoryResumeTarget = repository && eligible.length > 1
    ? { repository, tasks: eligible }
    : null;
  view.repositoryActions.hidden = !state.repositoryResumeTarget;
  if (!state.repositoryResumeTarget) return;
  const modelStatus = state.snapshot
    && state.snapshot.environment
    && state.snapshot.environment.core
    && state.snapshot.environment.core.agentModel
    && state.snapshot.environment.core.agentModel.status;
  const modelBlocked = modelStatus === 'degraded' || modelStatus === 'unavailable';
  view.resumeRepository.disabled = modelBlocked || state.repositoryResumePending || state.syncing;
  view.resumeRepository.textContent = state.repositoryResumePending
    ? text('resumingRepository')
    : text('resumeRepository', { value: eligible.length });
  view.repositoryActionsMeta.textContent = modelBlocked
    ? text('repositoryPausedByModel')
    : `${repositoryLabel(repository)} · ${text('repositorySerial')}`;
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
  const statusIcon = taskStatusIcon(task);
  icon.dataset.status = statusIcon.status;
  icon.textContent = statusIcon.symbol;

  const main = document.createElement('span');
  main.className = 'task-item__main';
  const label = document.createElement('strong');
  const item = task.identity && task.identity.item;
  const identityTitle = identityTitleOf(task);
  label.textContent = identityTitle || compactItemLabel(item);
  const meta = document.createElement('small');
  const activity = task.lastOutputAt || task.updatedAt;
  meta.textContent = `${repositoryLabel(item && item.repository)} · ${compactItemLabel(item)} · ${relativeLabel(activity)}`;
  main.append(label, meta);
  const gate = task.state === 'waiting_for_user' ? latestGate(task.taskId) : null;
  const reasonText = gate && gate.event.message
    ? gate.event.message
    : (ERROR_TASK_STATES.has(task.state) ? task.error : '');
  if (reasonText) {
    const reason = document.createElement('small');
    reason.className = 'task-item__reason';
    reason.textContent = reasonText;
    main.append(reason);
  }
  if (task.state === 'queued') {
    const reason = latestTaskWaitReason(task);
    if (reason) main.title = reason;
  }

  const taskState = document.createElement('span');
  taskState.className = 'task-item__state';
  const pendingAction = pendingActionFor(task);
  const visualState = taskVisualState(task);
  button.dataset.state = visualState;
  if (pendingAction) button.dataset.pending = pendingAction;
  taskState.dataset.status = visualState;
  if (pendingAction) {
    taskState.classList.add('task-item__hint', 'task-item__hint--pending');
    taskState.textContent = taskStateDisplayLabel(task);
  } else if (task.state === 'waiting_for_user' || ERROR_TASK_STATES.has(visualState)) {
    taskState.classList.add('task-item__hint');
    taskState.textContent = task.state === 'waiting_for_user'
      ? text('approvalGate')
      : text('viewError');
  } else if (visualState === 'running') {
    taskState.classList.add('task-item__hint', 'task-item__hint--running');
    taskState.textContent = text('runningNow');
  } else {
    taskState.classList.add('status-dot');
  }
  taskState.title = taskStateDisplayLabel(task);
  button.setAttribute('aria-label', `${label.textContent}, ${taskStateDisplayLabel(task)}`);
  button.append(icon, main, taskState);
  button.addEventListener('click', () => selectTask(task.taskId));
  return button;
}

function updateTaskGroupHeading(heading, definition, count, open) {
  if (!definition.collapsible) return;
  const toggle = heading.querySelector('.task-group__toggle');
  if (toggle) toggle.textContent = text(open ? 'collapseGroup' : 'expandGroup');
  const title = text(open ? 'collapseGroupTitle' : 'expandGroupTitle', {
    label: definition.label,
    count,
  });
  heading.title = title;
  heading.setAttribute('aria-label', title);
  heading.setAttribute('aria-expanded', String(open));
}

function renderTasks() {
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  const fragment = document.createDocumentFragment();
  const definitions = [
    { key: 'decision', label: text('groupDecision'), collapsible: false },
    { key: 'error', label: text('groupError'), collapsible: false },
    { key: 'active', label: text('groupActive'), collapsible: false },
    { key: 'queued', label: text('groupQueued'), collapsible: true },
    { key: 'paused', label: text('groupPaused'), collapsible: true },
    { key: 'completed', label: text('groupCompleted'), collapsible: true },
    { key: 'archived', label: text('groupArchived'), collapsible: true },
  ];
  definitions.forEach((definition) => {
    const grouped = sortedTaskList(tasks.filter((task) => taskGroupKey(task) === definition.key));
    if (!grouped.length) return;
    const group = document.createElement(definition.collapsible ? 'details' : 'section');
    group.className = 'task-group';
    group.dataset.group = definition.key;
    if (definition.collapsible) {
      const saved = Object.prototype.hasOwnProperty.call(state.taskGroupOpen, definition.key)
        ? state.taskGroupOpen[definition.key]
        : undefined;
      group.open = saved === undefined
        ? grouped.some((task) => task.taskId === state.selectedTaskId)
        : Boolean(saved);
    }
    const heading = document.createElement(definition.collapsible ? 'summary' : 'div');
    heading.className = 'task-group__heading';
    const label = document.createElement('span');
    label.className = 'task-group__label';
    if (definition.collapsible) {
      const chevron = document.createElement('span');
      chevron.className = 'task-group__chevron';
      chevron.setAttribute('aria-hidden', 'true');
      label.append(chevron);
    }
    const labelText = document.createElement('span');
    labelText.className = 'task-group__label-text';
    labelText.textContent = definition.label;
    label.append(labelText);
    const meta = document.createElement('span');
    meta.className = 'task-group__meta';
    const count = document.createElement('span');
    count.className = 'task-group__count';
    count.textContent = String(grouped.length);
    meta.append(count);
    if (definition.collapsible) {
      const toggle = document.createElement('span');
      toggle.className = 'task-group__toggle';
      meta.append(toggle);
      group.addEventListener('toggle', () => {
        state.taskGroupOpen[definition.key] = group.open;
        updateTaskGroupHeading(heading, definition, grouped.length, group.open);
      });
    }
    heading.append(label, meta);
    if (definition.collapsible) {
      updateTaskGroupHeading(heading, definition, grouped.length, group.open);
    }
    const items = document.createElement('div');
    grouped.forEach((task) => items.append(taskButton(task)));
    if (definition.key === 'error') group.append(heading, view.repositoryActions, items);
    else group.append(heading, items);
    fragment.append(group);
  });
  view.taskItems.replaceChildren(fragment);
  view.taskCount.textContent = String(tasks.length);
  const decisionCount = tasks.filter((task) => DECISION_TASK_STATES.has(task.state)).length;
  view.decisionCount.hidden = decisionCount === 0;
  view.decisionCount.textContent = String(decisionCount);
  view.decisionCount.title = text('decisionCountTitle');
  view.decisionCount.setAttribute('aria-label', `${text('decisionCountTitle')}: ${decisionCount}`);
  view.taskEmpty.hidden = tasks.length !== 0;
  renderRepositoryActions(tasks);
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
  button.className = tone === 'danger'
    ? 'danger-button'
    : (tone === 'primary' ? 'primary-button' : 'text-button');
  button.dataset.action = action;
  button.textContent = label;
  button.addEventListener('click', () => {
    if (action === 'gate') openGateDialog(task);
    else void performAction(action, task);
  });
  return button;
}

function makePendingActionButton(task) {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'text-button is-pending';
  button.disabled = true;
  button.textContent = taskStateDisplayLabel(task);
  return button;
}

function renderTaskActions(task) {
  view.taskActions.replaceChildren();
  if (!task || !snapshotSupported()) return;
  const fragment = document.createDocumentFragment();
  if (pendingActionFor(task)) {
    fragment.append(makePendingActionButton(task));
    view.taskActions.append(fragment);
    return;
  }
  if (['preparing', 'queued', 'running', 'retry_wait'].includes(task.state)) {
    fragment.append(makeActionButton(text('pause'), 'pause', task));
  }
  if (['stopped', 'recovery_required', 'failed'].includes(task.state)) {
    const modelStatus = state.snapshot
      && state.snapshot.environment
      && state.snapshot.environment.core
      && state.snapshot.environment.core.agentModel
      && state.snapshot.environment.core.agentModel.status;
    const modelBlocked = modelStatus === 'degraded' || modelStatus === 'unavailable';
    const resumeButton = makeActionButton(
      modelBlocked
        ? text('fixModelBeforeContinue')
        : (isWorkspacePreparationFailure(task) ? text('retry') : text('continueRun')),
      'resume',
      task,
      'primary',
    );
    resumeButton.disabled = modelBlocked;
    fragment.append(resumeButton);
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
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  view.selectedState.hidden = !task;
  view.showAllEvents.hidden = !task;
  if (!task) {
    const primary = primaryProgressTask(tasks);
    const counts = progressCounts(tasks);
    const showProgress = Boolean(primary || counts.queued > 0);
    view.livenessPanel.hidden = !showProgress;
    view.selectedSummary.hidden = false;
    view.selectedSummary.textContent = text('allTaskEvents');
    view.logTitle.textContent = text('activity');
    view.livenessItem.hidden = true;
    view.livenessItem.removeAttribute('href');
    view.issueDescriptionPanel.hidden = true;
    view.livenessDescription.textContent = '';
    renderTaskActions(null);
    if (!showProgress) {
      view.livenessPanel.removeAttribute('data-state');
      view.livenessGoal.hidden = true;
      view.livenessGoal.textContent = '';
      view.livenessTurn.hidden = true;
      view.livenessTurn.textContent = '';
      view.livenessTool.hidden = true;
      view.livenessTool.textContent = '';
      view.livenessDeadline.hidden = true;
      view.livenessDeadline.textContent = '';
      view.livenessError.hidden = true;
      view.livenessError.textContent = '';
      return;
    }
    const visualState = primary ? taskVisualState(primary) : 'queued';
    view.livenessPanel.dataset.state = visualState;
    view.livenessDot.dataset.status = visualState;
    view.livenessPhase.textContent = primary
      ? (pendingActionFor(primary) ? taskStateDisplayLabel(primary) : taskPhaseLabel(primary))
      : text('groupQueued');
    view.livenessSince.textContent = progressSummary(counts);
    view.livenessTurn.hidden = !primary;
    view.livenessTurn.textContent = primary
      ? text('mainProgressCurrent', { item: progressItemLabel(primary) })
      : '';
    view.livenessTool.hidden = !primary;
    view.livenessTool.textContent = primary
      ? text('mainProgressPhase', { phase: taskPhaseLabel(primary) })
      : '';
    const waitReason = primary && primary.state === 'queued' ? latestTaskWaitReason(primary) : '';
    view.livenessDeadline.hidden = !waitReason;
    view.livenessDeadline.textContent = waitReason;
    view.livenessGoal.hidden = !primary || !primary.goalId;
    view.livenessGoal.textContent = primary && primary.goalId ? `${text('goal')}: ${primary.goalId}` : '';
    view.livenessError.hidden = !primary || !primary.error;
    view.livenessError.textContent = primary && primary.error ? primary.error : '';
    return;
  }
  view.livenessPanel.hidden = false;
  const selectedVisualState = taskVisualState(task);
  view.livenessPanel.dataset.state = selectedVisualState;
  view.logTitle.textContent = identityTitleOf(task) || itemLabel(task.identity && task.identity.item);
  view.selectedSummary.hidden = true;
  view.selectedState.hidden = false;
  view.selectedState.dataset.state = selectedVisualState;
  view.selectedState.textContent = taskStateDisplayLabel(task);
  view.livenessDot.dataset.status = selectedVisualState;
  view.livenessPhase.textContent = pendingActionFor(task) ? taskStateDisplayLabel(task) : taskPhaseLabel(task);

  const item = task.identity && task.identity.item;
  if (item && item.repository) {
    const itemLabelText = itemLabel(item);
    view.livenessItem.hidden = false;
    view.livenessItem.textContent = itemLabelText;
    view.livenessItem.href = itemUrl(item);
    view.livenessItem.setAttribute('aria-label', `${text('openInGithub')}: ${itemLabelText}`);
    view.livenessItem.removeAttribute('title');
  } else {
    view.livenessItem.hidden = true;
    view.livenessItem.removeAttribute('href');
    view.livenessItem.removeAttribute('aria-label');
    view.livenessItem.textContent = '';
  }
  const description = identityDescriptionOf(task);
  const metadataKey = itemKey(item);
  const loadingDescription = state.metadataRequests.has(metadataKey);
  const descriptionUnavailable = (state.itemMetadata.get(metadataKey) || {}).unavailable === true;
  view.issueDescriptionPanel.hidden = !description && !loadingDescription && !descriptionUnavailable;
  view.livenessDescription.textContent = description
    || (loadingDescription ? text('loadingIssueDescription') : text('issueDescriptionUnavailable'));
  view.livenessDescription.removeAttribute('title');

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
  view.livenessTurn.textContent = task.currentTurnId ? `${text('turn')}: ${shortId(task.currentTurnId)}` : '';
  view.livenessGoal.hidden = !task.goalId;
  view.livenessGoal.textContent = task.goalId ? `${text('goal')}: ${task.goalId}` : '';
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
  if (!state.selectedTaskId && event.taskId) {
    const task = taskForId(event.taskId);
    const item = task && task.identity && task.identity.item;
    metaValues.push(item ? compactItemLabel(item) : event.taskId);
  }
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

function liveProgressRow(task, counts) {
  const row = document.createElement('li');
  row.className = 'log-row log-row--live';
  row.dataset.level = 'info';

  const time = document.createElement('span');
  time.className = 'log-time';
  time.textContent = 'LIVE';

  const level = document.createElement('span');
  level.className = 'event-level';
  level.dataset.level = 'info';
  level.textContent = 'NOW';

  const source = document.createElement('span');
  source.className = 'log-source';
  source.textContent = 'controller';

  const content = document.createElement('div');
  content.className = 'log-content';
  const message = document.createElement('div');
  message.className = 'log-message';
  message.textContent = task.phase === 'preparing_workspace'
    ? text('worktreeQuiet', { item: progressItemLabel(task) })
    : `${taskPhaseLabel(task)} · ${progressItemLabel(task)}`;
  content.append(message);

  const meta = document.createElement('div');
  meta.className = 'log-meta';
  [
    progressSummary(counts),
    taskStateLabel(task),
    relativeLabel(task.lastOutputAt || task.updatedAt),
  ].forEach((value) => {
    const span = document.createElement('span');
    span.textContent = value;
    meta.append(span);
  });
  content.append(meta);
  row.append(time, level, source, content);
  return row;
}

function outputKindLabel(kind) {
  if (kind === 'thinking') return text('outputThinking');
  if (kind === 'tool') return text('outputTool');
  if (kind === 'model_round_started' || kind === 'model_round_completed') return text('outputModel');
  return text('outputText');
}

function appendOutputText(existing, next) {
  const value = next == null ? '' : String(next);
  if (!value) return existing;
  return existing ? `${existing}${value}` : value;
}

function outputEventFallbackText(event) {
  if (event.text) return event.text;
  if (event.toolState) return event.toolState;
  if (event.kind === 'model_round_started') return 'Model round started';
  if (event.kind === 'model_round_completed') return 'Model round completed';
  return event.kind || text('outputText');
}

function canMergeOutputEvent(event) {
  return event.kind === 'thinking' || event.kind === 'text';
}

function compactTurnOutputBlocks(events) {
  const blocks = [];
  events.forEach((event) => {
    const kind = event.kind || 'text';
    const roundId = event.roundId || '';
    const toolName = event.toolName || '';
    const last = blocks[blocks.length - 1];
    if (
      canMergeOutputEvent(event)
      && last
      && last.kind === kind
      && last.roundId === roundId
      && !last.isEnd
    ) {
      last.endCursor = event.cursor;
      last.text = appendOutputText(last.text, event.text);
      last.isEnd = Boolean(last.isEnd || event.isEnd);
      last.eventCount += 1;
      return;
    }
    blocks.push({
      startCursor: event.cursor,
      endCursor: event.cursor,
      kind,
      roundId,
      toolName,
      toolState: event.toolState || '',
      text: outputEventFallbackText(event),
      isEnd: Boolean(event.isEnd),
      eventCount: 1,
    });
  });
  return blocks;
}

function turnOutputBlockMatches(block) {
  if (state.errorsOnly && block.toolState !== 'failed') return false;
  if (!state.query) return true;
  const haystack = [
    block.kind,
    block.text,
    block.toolName,
    block.toolState,
    block.roundId,
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
  return haystack.includes(state.query);
}

function cursorRangeLabel(block) {
  return block.startCursor === block.endCursor
    ? `#${block.startCursor}`
    : `#${block.startCursor}-${block.endCursor}`;
}

function turnOutputBlockRow(block) {
  const row = document.createElement('li');
  row.className = 'log-row turn-output-row';
  row.dataset.kind = block.kind;
  row.dataset.level = block.toolState === 'failed' ? 'error' : 'info';
  row.dataset.cursor = String(block.endCursor);

  const header = document.createElement('div');
  header.className = 'output-block__header';

  const level = document.createElement('span');
  level.className = 'event-level';
  level.dataset.level = block.toolState === 'failed' ? 'error' : 'info';
  level.textContent = outputKindLabel(block.kind);

  const source = document.createElement('span');
  source.className = 'output-block__source';
  source.textContent = block.toolName || 'agent';

  const cursor = document.createElement('span');
  cursor.className = 'output-block__cursor';
  cursor.textContent = cursorRangeLabel(block);

  header.append(level, source, cursor);

  if (block.roundId) {
    const round = document.createElement('span');
    round.className = 'output-block__meta';
    round.textContent = shortId(block.roundId);
    round.title = block.roundId;
    header.append(round);
  }
  if (block.toolState) {
    const status = document.createElement('span');
    status.className = 'output-block__meta';
    status.textContent = block.toolState;
    header.append(status);
  }
  if (block.eventCount > 1) {
    const chunks = document.createElement('span');
    chunks.className = 'output-block__meta';
    chunks.textContent = text('outputChunks', { value: block.eventCount });
    header.append(chunks);
  }

  const message = document.createElement('div');
  message.className = 'output-block__message';
  message.textContent = block.text || outputKindLabel(block.kind);

  row.append(header, message);
  return row;
}

function renderTurnOutput() {
  const task = runningOutputTask();
  if (task) ensureTurnOutputTarget(task);
  const blocks = compactTurnOutputBlocks(state.turnOutput.events);
  const matches = blocks.filter(turnOutputBlockMatches);
  const visible = matches.slice(-MAX_RENDERED_EVENTS);
  const fragment = document.createDocumentFragment();
  visible.forEach((block) => fragment.append(turnOutputBlockRow(block)));
  view.logList.replaceChildren(fragment);
  const hasOutput = visible.length !== 0;
  view.logEmpty.hidden = hasOutput;
  if (!hasOutput) {
    const message = state.turnOutput.message
      || (task ? text('noLiveOutput') : text('outputNotRunning'));
    view.logEmpty.querySelector('p').textContent = message;
  }
  if (state.followLogs) {
    requestAnimationFrame(() => {
      view.logScroll.scrollTop = view.logScroll.scrollHeight;
      view.newEvents.hidden = true;
    });
  } else if (visible.length) {
    view.newEvents.hidden = false;
  }
  if (task && !state.turnOutput.inFlight && !state.turnOutput.timer) {
    scheduleTurnOutputPoll(state.turnOutput.events.length ? 1200 : 0);
  }
}

function renderLogs() {
  if (state.logMode === 'output') {
    renderTurnOutput();
    return;
  }
  const wasFollowing = state.followLogs;
  const matches = state.events.filter(eventMatches);
  state.visibleEvents = matches;
  const visible = matches.slice(-MAX_RENDERED_EVENTS);
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  const liveTask = state.selectedTaskId ? null : primaryProgressTask(tasks);
  const counts = progressCounts(tasks);
  const fragment = document.createDocumentFragment();
  visible.forEach((event) => fragment.append(eventRow(event)));
  if (liveTask) fragment.append(liveProgressRow(liveTask, counts));
  view.logList.replaceChildren(fragment);
  view.logEmpty.querySelector('p').textContent = activeWorkEmptyMessage();
  view.logEmpty.hidden = visible.length !== 0 || Boolean(liveTask);
  if (wasFollowing) {
    requestAnimationFrame(() => {
      view.logScroll.scrollTop = view.logScroll.scrollHeight;
      view.newEvents.hidden = true;
    });
  } else if (visible.length) {
    view.newEvents.hidden = false;
  }
}

function renderAll() {
  renderExecutionSupport();
  renderEnvironment();
  renderTasks();
  renderLiveness();
  renderLogs();
}

async function hydrateTaskMetadata(taskId) {
  const task = taskForId(taskId);
  const item = task && task.identity && task.identity.item;
  if (!item || identityDescriptionOf(task)) return;
  const metadataKey = itemKey(item);
  if (state.metadataRequests.has(metadataKey)) return;
  state.metadataRequests.add(metadataKey);
  renderLiveness();
  try {
    const response = await app.loopx.resolveIntake({
      input: itemUrl(item),
      modelId: task.modelId || 'auto',
    });
    const candidates = response && response.preview && Array.isArray(response.preview.candidates)
      ? response.preview.candidates
      : [];
    const candidate = candidates.find((entry) => itemKey(entry.key) === metadataKey);
    state.itemMetadata.set(metadataKey, {
      title: candidate && candidate.title ? candidate.title : '',
      description: candidate && candidate.description ? candidate.description : '',
      unavailable: !candidate,
    });
  } catch (_error) {
    state.itemMetadata.set(metadataKey, { unavailable: true });
  } finally {
    state.metadataRequests.delete(metadataKey);
    renderTasks();
    renderLiveness();
  }
}

function selectTask(taskId) {
  state.selectedTaskId = taskId || null;
  if (taskId) view.issueDescriptionPanel.open = false;
  const task = taskId ? taskForId(taskId) : null;
  if (task && task.state === 'running') {
    setLogMode('output');
  } else if (!taskId && state.logMode === 'output') {
    setLogMode('full');
  } else if (state.logMode === 'output') {
    setLogMode('key');
  }
  renderTasks();
  renderLiveness();
  renderLogs();
  if (taskId) void hydrateTaskMetadata(taskId);
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

  updateCreateButton();
}

function renderIntakeWarnings() {
  const preview = state.preview;
  if (!preview) return;
  const candidates = Array.isArray(preview.candidates) ? preview.candidates : [];
  const selectedCount = selectedPreviewItems().length;
  const warnings = [];
  if (selectedCount > 1) warnings.push(text('batchSelection', { value: selectedCount }));
  if (preview.truncated) warnings.push(text('truncatedCandidates'));
  if (
    candidates.some((candidate) => candidate.hasImages)
    && preview.model
    && !preview.model.supportsImages
  ) warnings.push(text('imageWarning'));
  if (preview.model && preview.model.available === false) {
    warnings.push(preview.model.detail || text('modelUnavailable'));
  }
  if (preview.workspace && preview.workspace.disposition === 'unavailable') {
    warnings.push(preview.workspace.detail || text('workspaceUnavailable'));
  }
  view.intakeWarning.hidden = warnings.length === 0;
  view.intakeWarning.textContent = warnings.join(' ');
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

function syncSelectAll() {
  const selectAll = view.candidateSelectAll;
  if (!selectAll) return;
  const enabled = [...view.candidateList.querySelectorAll('input[name="candidate"]:not(:disabled)')];
  const checked = [...view.candidateList.querySelectorAll('input[name="candidate"]:checked')];
  selectAll.checked = enabled.length > 0 && checked.length === enabled.length;
  selectAll.indeterminate = checked.length > 0 && checked.length < enabled.length;
}

function updateCreateButton() {
  const itemCount = selectedPreviewItems().length;
  const candidateCount = state.preview && Array.isArray(state.preview.candidates)
    ? state.preview.candidates.length
    : 0;
  const previewReady = Boolean(state.preview)
    && (!state.preview.model || state.preview.model.available !== false)
    && (!state.preview.workspace || state.preview.workspace.disposition !== 'unavailable');
  view.createButton.disabled = itemCount === 0 || !previewReady;
  view.createButton.textContent = itemCount > 1
    ? `${text('createTasks')} (${itemCount})`
    : text('createTasks');
  view.candidateCount.textContent = text('selectedCandidates', {
    selected: itemCount,
    total: candidateCount,
  });
  syncSelectAll();
  renderIntakeWarnings();
}

async function loadModelCatalog() {
  const select = view.modelSelect;
  if (!select || select.tagName !== 'SELECT') return;
  if (!app || !app.loopx || typeof app.loopx.listModels !== 'function') return;
  if (state.modelCatalogLoading) return;
  if (state.modelCatalogLoaded && select.options.length > 1) return;
  state.modelCatalogLoading = true;
  select.dataset.loading = 'true';
  const loadingAutoOption = [...select.options].find((option) => option.value === 'auto');
  if (!state.modelCatalogLoaded && loadingAutoOption) {
    loadingAutoOption.textContent = text('modelLoading');
  }
  try {
    const models = await app.loopx.listModels();
    const current = currentModelSelection();
    select.replaceChildren();
    const auto = document.createElement('option');
    auto.value = 'auto';
    auto.textContent = text('modelAuto');
    auto.selected = current === 'auto';
    select.appendChild(auto);
    let selectedExists = current === 'auto';
    for (const model of Array.isArray(models) ? models : []) {
      if (!model || !model.id) continue;
      const option = document.createElement('option');
      option.value = model.id;
      const tag = model.isDefault === true ? ` · ${text('modelPrimaryTag')}` : '';
      option.textContent = `${model.name || model.modelName || model.id}${tag}`;
      option.selected = current === model.id;
      selectedExists = selectedExists || option.selected;
      select.appendChild(option);
    }
    if (!selectedExists) select.value = 'auto';
    state.modelCatalogLoaded = select.options.length > 1;
    select.title = text('modelReloadTitle');
  } catch (error) {
    const current = currentModelSelection();
    if (select.options.length === 0) {
      const auto = document.createElement('option');
      auto.value = 'auto';
      auto.textContent = text('modelAuto');
      select.appendChild(auto);
    } else {
      const auto = [...select.options].find((option) => option.value === 'auto');
      if (auto) auto.textContent = text('modelAuto');
    }
    select.value = current === 'auto' ? 'auto' : select.value;
    select.title = errorMessage(error);
    state.modelCatalogLoaded = false;
  } finally {
    state.modelCatalogLoading = false;
    delete select.dataset.loading;
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

function summarizeOutcomes(outcomes) {
  const createdCount = outcomes.filter((outcome) => outcome.kind === 'created').length;
  const messages = [];
  if (createdCount === 1) messages.push(text('taskCreated'));
  if (createdCount > 1) messages.push(text('tasksCreated', { value: createdCount }));

  const grouped = new Map();
  outcomes
    .filter((outcome) => outcome.kind !== 'created')
    .forEach((outcome) => {
      const message = outcomeMessage(outcome);
      grouped.set(message, (grouped.get(message) || 0) + 1);
    });
  grouped.forEach((count, message) => {
    messages.push(count === 1 ? message : text('outcomeCount', { message, value: count }));
  });
  return messages;
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
    const messages = summarizeOutcomes(outcomes);
    const hasError = outcomes.some((outcome) => outcome.kind === 'needs_live_verification');
    showNotice(messages.join(' '), hasError ? 'error' : 'success');
    view.intakeDialog.close();
    view.retryDialog.close();
    state.preview = null;
    state.pendingRetry = null;
    await attachSnapshot(false);
    selectTask(null);
    setLogMode('full');
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

function openResetLoopxDialog() {
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks)
    ? state.snapshot.tasks.length
    : 0;
  view.resetLoopxMessage.textContent = text('resetLoopxMessage', {
    tasks,
    events: state.events.length,
  });
  view.resetLoopxDialog.showModal();
}

async function resetLoopx() {
  if (!state.snapshot || state.resetPending) return;
  state.resetPending = true;
  setButtonBusy(view.resetLoopx, true);
  view.resetLoopxConfirm.disabled = true;
  try {
    const clientRequestId = requestId();
    await attachSnapshot(false);
    let request = {
      action: 'reset_all',
      clientRequestId,
      expectedRevision: Number((state.snapshot && state.snapshot.revision) || 0),
    };
    let response = await app.loopx.action(request);
    for (let attempt = 0; attempt < 2 && response && response.status === 'revision_conflict'; attempt += 1) {
      await attachSnapshot(false);
      const nextRevision = Number(response.currentRevision || (state.snapshot && state.snapshot.revision) || 0);
      if (!Number.isSafeInteger(nextRevision) || nextRevision === request.expectedRevision) break;
      request = { ...request, expectedRevision: nextRevision };
      response = await app.loopx.action(request);
    }
    if (response && response.status === 'revision_conflict') {
      showNotice(response.message || text('revisionConflict'), 'error');
    } else if (response && response.status === 'rejected') {
      showNotice(response.message || text('actionRejected'), 'error');
    } else {
      state.events = [];
      state.eventKeys.clear();
      state.selectedTaskId = null;
      state.repositoryResumeTarget = null;
      showNotice(text('resetLoopxApplied'), 'success');
    }
    view.resetLoopxDialog.close();
    await attachSnapshot(false);
  } catch (error) {
    showNotice(errorMessage(error), 'error');
    view.resetLoopxDialog.close();
    await attachSnapshot(false);
  } finally {
    state.resetPending = false;
    setButtonBusy(view.resetLoopx, false);
    view.resetLoopxConfirm.disabled = false;
  }
}

function openRepositoryResumeDialog() {
  const target = state.repositoryResumeTarget;
  if (!target || !target.tasks.length) return;
  view.repositoryResumeMessage.textContent = text('resumeRepositoryMessage', {
    repository: repositoryLabel(target.repository),
    value: target.tasks.length,
  });
  view.repositoryResumeDialog.showModal();
}

async function resumeRepository() {
  const target = state.repositoryResumeTarget;
  if (!target || !target.tasks.length || !state.snapshot) {
    view.repositoryResumeDialog.close();
    return;
  }
  const count = target.tasks.length;
  state.repositoryResumePending = true;
  renderRepositoryActions(state.snapshot.tasks || []);
  view.repositoryResumeConfirm.disabled = true;
  try {
    const response = await app.loopx.action({
      action: 'resume_repository',
      repository: target.repository,
      clientRequestId: requestId(),
      expectedRevision: Number(state.snapshot.revision || 0),
    });
    if (response && response.status === 'revision_conflict') {
      showNotice(response.message || text('revisionConflict'), 'error');
    } else if (response && response.status === 'rejected') {
      showNotice(response.message || text('actionRejected'), 'error');
    } else {
      showNotice(text('resumeRepositoryApplied', { value: count }), 'success');
    }
    view.repositoryResumeDialog.close();
    await attachSnapshot(false);
  } catch (error) {
    showNotice(errorMessage(error), 'error');
    await attachSnapshot(false);
  } finally {
    state.repositoryResumePending = false;
    view.repositoryResumeConfirm.disabled = false;
    const tasks = state.snapshot && Array.isArray(state.snapshot.tasks)
      ? state.snapshot.tasks
      : [];
    renderRepositoryActions(tasks);
  }
}

function mergeActionTask(response) {
  if (!response || !response.task || !state.snapshot) return;
  const index = state.snapshot.tasks.findIndex((item) => item.taskId === response.task.taskId);
  if (index >= 0) state.snapshot.tasks.splice(index, 1, response.task);
  else state.snapshot.tasks.push(response.task);
  state.snapshot.revision = Math.max(
    Number(state.snapshot.revision || 0),
    Number(response.currentRevision || 0),
    Number(response.task.revision || 0),
  );
}

function latestActionRevision(response, fallback) {
  const taskRevision = Number(response && response.task && response.task.revision);
  if (Number.isSafeInteger(taskRevision)) return taskRevision;
  const currentRevision = Number(response && response.currentRevision);
  return Number.isSafeInteger(currentRevision) ? currentRevision : fallback;
}

async function sendActionRequest(request) {
  const response = await app.loopx.action(request);
  mergeActionTask(response);
  return response;
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
  if (task && task.taskId) {
    state.taskActionPending.set(task.taskId, action);
    renderTasks();
    renderLiveness();
  }
  try {
    let response = await sendActionRequest(request);
    if (
      response
      && response.status === 'revision_conflict'
      && task
      && response.task
      && response.task.taskId === task.taskId
    ) {
      const nextRevision = latestActionRevision(response, request.expectedRevision);
      if (nextRevision !== request.expectedRevision) {
        response = await sendActionRequest({
          ...request,
          expectedRevision: nextRevision,
        });
      }
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
  } finally {
    if (task && task.taskId && state.taskActionPending.get(task.taskId) === action) {
      state.taskActionPending.delete(task.taskId);
      renderTasks();
      renderLiveness();
    }
  }
}

function openGateDialog(task) {
  const gate = latestGate(task.taskId);
  if (!gate) {
    showNotice(text('noGate'), 'error');
    return;
  }
  state.gate = { task, gateId: gate.gateId };
  view.gateTitle.textContent = identityTitleOf(task) || itemLabel(task.identity && task.identity.item);
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
    turnOutput: state.logMode === 'output' ? state.turnOutput.events : [],
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
  const fullMode = mode === 'full';
  const outputMode = mode === 'output';
  view.modeKey.classList.toggle('is-active', keyMode);
  view.modeKey.setAttribute('aria-checked', String(keyMode));
  view.modeFull.classList.toggle('is-active', fullMode);
  view.modeFull.setAttribute('aria-checked', String(fullMode));
  view.modeOutput.classList.toggle('is-active', outputMode);
  view.modeOutput.setAttribute('aria-checked', String(outputMode));
  if (!outputMode) clearTurnOutputTimer();
  renderLogs();
  if (outputMode) scheduleTurnOutputPoll(0);
}

const RAIL_MIN_WIDTH = 180;
const RAIL_MAX_WIDTH = 520;
const RAIL_DEFAULT_WIDTH = 286;
const RAIL_WIDTH_STORAGE_KEY = 'loopx.railWidth';

function setRailWidth(width) {
  const workbench = view.taskRail.parentElement;
  workbench.style.setProperty('--rail-width', `${width}px`);
  state.railWidth = width;
}

function bindRailSplitter() {
  const splitter = view.railSplitter;
  if (!splitter) return;
  let startX = 0;
  let startWidth = 0;
  let active = false;
  const width = () => state.railWidth || RAIL_DEFAULT_WIDTH;

  const move = (event) => {
    if (!active) return;
    const next = startWidth + (event.clientX - startX);
    setRailWidth(Math.min(RAIL_MAX_WIDTH, Math.max(RAIL_MIN_WIDTH, next)));
    event.preventDefault();
  };

  const end = () => {
    if (!active) return;
    active = false;
    splitter.classList.remove('is-dragging');
    splitter.classList.remove('is-focused');
    document.removeEventListener('pointermove', move);
    document.removeEventListener('pointerup', end);
    document.body.style.userSelect = '';
    splitter.setAttribute('aria-valuenow', String(width()));
    persistRailWidth();
  };

  splitter.addEventListener('pointerdown', (event) => {
    if (state.railCollapsed) return;
    active = true;
    startX = event.clientX;
    startWidth = width();
    splitter.classList.add('is-dragging');
    document.body.style.userSelect = 'none';
    document.addEventListener('pointermove', move);
    document.addEventListener('pointerup', end);
    event.preventDefault();
  });

  splitter.addEventListener('focus', () => splitter.classList.add('is-focused'));
  splitter.addEventListener('blur', () => splitter.classList.remove('is-focused'));

  // Double-click or double-activate resets to the default width.
  splitter.addEventListener('dblclick', () => {
    if (state.railCollapsed) return;
    setRailWidth(RAIL_DEFAULT_WIDTH);
    persistRailWidth();
  });
  splitter.addEventListener('keydown', (event) => {
    if (state.railCollapsed) return;
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const step = event.shiftKey ? 48 : 12;
    const next = event.key === 'ArrowRight' ? width() + step : width() - step;
    setRailWidth(Math.min(RAIL_MAX_WIDTH, Math.max(RAIL_MIN_WIDTH, next)));
    persistRailWidth();
  });

  splitter.setAttribute('aria-valuemin', String(RAIL_MIN_WIDTH));
  splitter.setAttribute('aria-valuemax', String(RAIL_MAX_WIDTH));
  splitter.setAttribute('aria-valuenow', String(width()));
  splitter.setAttribute('aria-controls', 'task-rail');

  // Restore persisted width (session-only when storage is unavailable).
  try {
    const stored = window.localStorage.getItem(RAIL_WIDTH_STORAGE_KEY);
    const parsed = stored === null ? NaN : Number(stored);
    if (Number.isFinite(parsed) && parsed >= RAIL_MIN_WIDTH && parsed <= RAIL_MAX_WIDTH) {
      setRailWidth(parsed);
    }
  } catch {
    /* storage unavailable: keep default width for this session */
  }
}

function persistRailWidth() {
  const value = String(state.railWidth || RAIL_DEFAULT_WIDTH);
  try {
    window.localStorage.setItem(RAIL_WIDTH_STORAGE_KEY, value);
  } catch {
    /* storage unavailable: keep width for this session only */
  }
}

function bindEvents() {
  bindRailSplitter();
  view.modelSelect.addEventListener('pointerdown', () => void loadModelCatalog());
  view.modelSelect.addEventListener('focus', () => void loadModelCatalog());
  view.modelSelect.addEventListener('change', () => {
    writeStoredModelSelection(view.modelSelect.value || 'auto');
    if (state.preview) {
      state.preview = null;
      if (view.intakeDialog.open) view.intakeDialog.close();
      showNotice(text('modelSelectionChanged'));
    }
  });
  view.intakeForm.addEventListener('submit', (event) => {
    event.preventDefault();
    void resolveIntake();
  });
  view.candidateSelectAll.addEventListener('change', () => {
    const checked = view.candidateSelectAll.checked;
    view.candidateList.querySelectorAll('input[name="candidate"]:not(:disabled)').forEach((input) => {
      input.checked = checked;
    });
    updateCreateButton();
  });
  view.resetLoopx.addEventListener('click', openResetLoopxDialog);
  view.resetLoopxCancel.addEventListener('click', () => {
    view.resetLoopxDialog.close();
  });
  view.resetLoopxConfirm.addEventListener('click', (event) => {
    event.preventDefault();
    void resetLoopx();
  });
  view.syncButton.addEventListener('click', () => void attachSnapshot(false));
  view.retryEnvironment.addEventListener('click', async () => {
    await performAction('retry_environment', null);
  });
  view.resumeRepository.addEventListener('click', openRepositoryResumeDialog);
  view.repositoryResumeCancel.addEventListener('click', () => {
    view.repositoryResumeDialog.close();
  });
  view.repositoryResumeConfirm.addEventListener('click', (event) => {
    event.preventDefault();
    void resumeRepository();
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
  view.showAllEvents.addEventListener('click', () => selectTask(null));
  view.modeKey.addEventListener('click', () => setLogMode('key'));
  view.modeFull.addEventListener('click', () => setLogMode('full'));
  view.modeOutput.addEventListener('click', () => setLogMode('output'));
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
    clearTurnOutputTimer();
    if (app.loopx && typeof app.loopx.offEvent === 'function') {
      app.loopx.offEvent(onLoopxEvent);
    }
  });
  window.setInterval(updateLivenessClock, 5000);
  await attachSnapshot(true);
}

void start();
