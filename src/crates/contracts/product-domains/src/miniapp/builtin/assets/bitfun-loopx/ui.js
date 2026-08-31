'use strict';

// LoopX is owned by the BitFun host. This file only projects durable snapshots
// and cursor-addressed events into the MiniApp UI.
const app = window.app;
const byId = (id) => document.getElementById(id);
const MAX_EVENTS = 2000;
const MAX_RENDERED_OUTPUT_BLOCKS = 500;
const MAX_TURN_OUTPUT_EVENTS = 800;
const MAX_OUTPUT_HISTORY_EVENTS = 2400;
const MAX_OUTPUT_EVENT_CHARS = 16000;
const MAX_OUTPUT_BLOCK_CHARS = 120000;
const MAX_OUTPUT_HISTORY_CHARS = 1200000;
const MAX_INTAKE_HISTORY = 12;
const HOST_CLOCK_TICK_MS = 5000;
const HOST_RESUME_GAP_MS = 30000;
const STALE_ACTIVE_REATTACH_MS = 30000;
const MODEL_SELECTION_STORAGE_KEY = 'loopx.modelId';
const INTAKE_HISTORY_STORAGE_KEY = 'loopx.intakeHistory';
const HIGH_RISK_SCOPES = new Set([
  'publish',
  'public_comment',
  'pull_request',
  'merge',
  'production_action',
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
    modelEmpty: '未找到已启用的文本模型',
    modelLoadFailed: '模型列表加载失败',
    modelReloadTitle: '刷新模型列表',
    modelSelectionChanged: '已切换模型，请重新分析链接。',
    modelPrimaryTag: '主模型',
    resolve: '分析链接',
    resolving: '正在实时复核链接',
    refresh: '刷新状态',
    resetLoopx: '重置 LoopX',
    resettingLoopx: '正在重置 LoopX…',
    resettingLoopxBackground: '正在后台清理任务与工作进展；窗口可以继续使用，完成后会自动刷新。',
    destructiveAction: '危险操作',
    resetLoopxTitle: '清空并重新开始',
    resetLoopxMessage: '将停止并删除 {tasks} 个任务、{events} 条日志、全部工作进展和受管工作区。此操作不可撤销。',
    resetLoopxRetained: '模型配置、GitHub 登录、MiniApp 设置和干净的 Git 对象缓存会保留；仍在进行的工作区不会被复用。',
    resetLoopxConfirm: '清空并重新开始',
    resetLoopxApplied: 'LoopX 已清空，可以重新开始测试。',
    unsupportedTitle: '当前执行位置不支持 LoopX',
    unsupportedDefault: 'LoopX 目前只支持本地 Desktop 工作区；远程工作区不会静默改在本机执行。',
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
    activity: '模型实时输出',
    allTaskEvents: '所有 Issue 的模型输出会按时间保留在这里',
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
    liveOutput: '模型实时输出',
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
    sourceScheduler: '任务调度',
    sourceLoopx: 'LoopX 引擎',
    sourceAgent: 'Agent',
    sourceGit: 'Git',
    sourceGithub: 'GitHub',
    sourceSystem: '系统',
    toolExecCommand: '执行命令',
    toolRead: '读取文件',
    toolGrep: '搜索内容',
    toolLs: '浏览目录',
    toolWebFetch: '访问网页',
    toolWebSearch: '搜索网页',
    toolWrite: '写入文件',
    toolEdit: '修改文件',
    toolQueued: '工具已排队：{tool}',
    toolWaiting: '工具等待中：{tool}',
    toolStarted: '正在运行：{tool}',
    toolConfirmation: '工具等待确认：{tool}',
    toolConfirmed: '已确认工具：{tool}',
    toolRejected: '已拒绝工具：{tool}',
    toolCompleted: '工具完成：{tool}',
    toolFailed: '工具失败：{tool}',
    toolCancelled: '工具已取消：{tool}',
    toolStateQueued: '排队',
    toolStateWaiting: '等待',
    toolStateStarted: '运行中',
    toolStateConfirmation: '等待确认',
    toolStateConfirmed: '已确认',
    toolStateRejected: '已拒绝',
    toolStateCompleted: '已完成',
    toolStateFailed: '失败',
    toolStateCancelled: '已取消',
    detailDurationMs: '耗时（毫秒）',
    detailExitCode: '退出码',
    detailMatchCount: '匹配数',
    detailFileCount: '文件数',
    detailEntryCount: '条目数',
    detailLineCount: '读取行数',
    detailContentLength: '内容长度',
    detailTitle: '标题',
    detailQueuePosition: '队列位置',
    detailDependencies: '等待项',
    newEvents: '滚动到最新输出',
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
    decisionRequired: '需要你的决定',
    afterApprove: '批准后',
    afterReject: '拒绝后',
    approvalNote: '审批备注',
    approvalNotePlaceholder: '补充批准或拒绝的原因（可选）',
    reject: '拒绝',
    approve: '批准',
    pause: '暂停',
    resume: '恢复',
    continueRun: '仅继续此任务',
    resumeRepository: '恢复仓库任务（{value}）',
    resumingRepository: '正在恢复异常任务…',
    repositorySerial: '同仓库串行执行',
    batchAction: '批量操作',
    resumeRepositoryTitle: '恢复此仓库的异常任务',
    confirmContinue: '确认继续',
    resumeRepositoryMessage: '将恢复 {repository} 中 {value} 个已暂停、中止或失败的任务。同一时间只运行 1 个，其余任务进入队列。',
    resumeRepositoryApplied: '已将 {value} 个仓库任务加入队列。',
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
    turn: '阶段',
    goal: '目标',
    deadline: '截止',
    noRecentOutput: '已 {duration} 没有新输出',
    lastOutput: '最后输出 {duration} 前',
    updated: '更新于 {duration} 前',
    openInGithub: '在 GitHub 中打开',
    issueProgress: '处理进度',
    currentWork: '当前',
    evidenceAndOutputs: '已完成的工作',
    nextAction: '下一步',
    latestOutcome: '最近进展',
    outcomeUpdated: '{duration}前更新',
    stageWorkspace: '工作区',
    stageAnalysis: '分析与方案',
    stageImplementation: '实施',
    stageValidation: '结果核验',
    stageSettlement: '完成',
    stagePending: '待开始',
    stageActive: '进行中',
    stageComplete: '已完成',
    stageBlocked: '已阻塞',
    progressMeta: '{done}/{total} 个阶段已完成',
    progressPreparing: '正在准备独立 Worktree',
    progressQueued: '等待同仓库前序 Issue',
    progressAnalyzing: '正在分析原因并形成可执行方案',
    progressImplementing: '已进入代码修改阶段',
    progressValidating: '正在核验本轮产出',
    progressSettling: '正在保存本轮进展',
    progressWaiting: '等待你的决定',
    progressRecovery: '本轮执行已中断',
    progressCompleted: '修复流程已完成',
    progressResolvedUpstream: '上游已处理该问题',
    progressResolvedUpstreamDetail: '已确认当前上游代码移除了原始故障路径，不需要再提交额外修复。',
    progressIdle: '等待任务推进',
    evidenceWorkspace: '独立工作区已准备完成',
    evidenceProgressSaved: '当前工作进展已保存，中断后可以继续',
    evidenceAnalysisReady: '已完成问题分析并形成修复方向',
    evidenceCodeChanges: '已完成代码修改（涉及 {value} 个文件）',
    evidenceCodeChangesUnknown: '已完成代码修改',
    evidenceValidationPassed: '已运行项目验证，当前未发现失败',
    evidenceValidationNeedsAttention: '部分验证未通过，仍需继续处理',
    evidenceResolvedUpstream: '已核对当前上游代码，原始故障路径已被移除',
    evidenceNoPatchRequired: '当前没有需要提交的修复补丁',
    evidenceNone: '任务尚未产生可展示的工作结果',
    nextPrepare: '完成 Worktree 后开始分析 Issue。',
    nextQueued: '等待仓库执行槽释放后自动开始。',
    nextAnalyze: '完成原因定位并形成可执行方案。',
    nextImplement: '继续实现并保留可核验的修改证据。',
    nextValidate: '核验当前产出并保存进展。',
    nextSettle: '保存进展后继续下一阶段或完成任务。',
    nextRecover: '处理执行异常后继续任务。',
    nextApproval: '查看审批内容并决定是否继续。',
    nextCompleted: '检查最终结论和产出。',
    nextResolvedUpstream: '无需继续修复，可以保留记录或归档任务。',
    issueDescription: 'Issue 描述',
    loadingIssueDescription: '正在加载 Issue 描述…',
    issueDescriptionUnavailable: '暂时无法加载 Issue 描述。',
    problemBackground: '遇到的问题',
    problemImpact: '造成的影响',
    issueApiProxyTitle: '升级后第三方插件无法启动',
    issueApiProxyBackground: '从 2.0.2 升级到 2.0.4 后，第三方任务看板仍依赖已被新版本移除的 apiProxy 服务，因此插件无法完成启动。',
    issueApiProxyImpact: '一个插件启动失败会使整个插件树加载失败，导致桌面端无法正常使用已安装的第三方插件。',
    issueApiProxyProgress: '已经定位原因并确定修复方案：为旧插件补充兼容能力，同时避免单个第三方插件拖垮整个启动流程。实际代码修改尚未开始。',
    issueInputModalityTitle: '手动添加的多模态模型无法发送图片',
    issueInputModalityBackground: '模型设置保存的图片能力字段与运行引擎读取的字段不一致，导致手动添加的模型丢失图片输入能力。',
    issueInputModalityImpact: '模型虽然支持图片，但聊天界面会按纯文本模型处理，用户无法发送图片。',
    issueInputModalityProgress: '已经核对最新上游代码，原有错误写入路径已被移除，目前不需要再提交修复补丁。',
    issueMacFocusTitle: 'macOS 最小化后窗口意外抢占焦点',
    issueMacFocusBackground: '应用最小化后仍会被系统激活事件重新带到前台，打断用户正在进行的其他操作。',
    issueMacFocusImpact: '窗口会突然出现并抢走键盘焦点，影响正常工作流程。',
    issueMacFocusProgress: '已经完成原因定位、代码修改和现有验证，等待决定是否发布为 Pull Request。',
    issueGenericBackground: '该 Issue 描述了以下问题：{title}',
    issueGenericImpact: '问题会影响相关功能的正常使用，需要确认下一步处理方式。',
    issueGenericProgressAnalyzed: '已经完成初步分析并找到继续处理的方向，但修复和验证尚未完成。',
    issueGenericProgressImplemented: '已经产生修复修改，但仍需继续验证并确认最终结果。',
    publishApprovalTitle: '是否发布修复并创建 Pull Request？',
    publishApprovalSummary: '修复已在分支 {branch} 的提交 {commit} 中准备完成，目标仓库为 {repository}。现在需要你决定是否发布。',
    publishApprovalSummaryGeneric: '修复和发布材料已经准备完成，目标仓库为 {repository}。现在需要你决定是否发布为 Pull Request。',
    publishApprovalApproveEffect: '推送修复分支并创建 Pull Request，随后进入 macOS 真机验证。批准不会自动合并代码。',
    publishApprovalRejectEffect: '不推送分支，也不创建 Pull Request；本地分支、提交和验证结果会保留，任务停在当前步骤。',
    publishApprovalRecommendationReady: '建议批准：当前修改已有验证结果，批准后仍可在 Pull Request 中继续评审，并不会自动合并。',
    publishApprovalRecommendationReview: '建议先确认修改和验证结果；批准只会发布 Pull Request，不会自动合并。',
    publishApprovalApprove: '批准并创建 PR',
    publishApprovalReject: '暂不发布',
    autonomyApprovalTitle: '修复尚未完成，是否继续处理？',
    autonomyApprovalSummaryAnalyzed: '已经完成问题分析并找到继续处理的方向，但修复和验证尚未完成。',
    autonomyApprovalSummaryImplemented: '已经产生修复修改，但还需要继续验证并确认最终结果。',
    autonomyApprovalApproveEffect: '继续修复和验证，完成后汇报结果。不会发布或合并代码。',
    autonomyApprovalRejectEffect: '暂停处理并保留当前工作区、修改和调查结果。',
    autonomyApprovalRecommendation: '建议继续：当前已经形成明确方向，下一步主要是实施和验证。',
    continueRepair: '继续处理',
    pauseProcessing: '暂不继续',
    implementationApprovalTitle: '是否允许开始修改代码？',
    implementationApprovalApproveEffect: '允许在独立工作区修改代码并运行测试。完成后会再次汇报结果；不会自动推送、创建 Pull Request 或合并代码。',
    implementationApprovalRejectEffect: '本次不修改代码，也不会继续执行后续修复步骤。现有调查结果和工作区会保留，之后仍可手动恢复。',
    implementationApprovalRecommendation: '建议允许：问题原因和修复方向已经明确，下一步是实施并验证修复。',
    implementationApprovalApprove: '允许开始修复',
    implementationApprovalReject: '暂不修改',
    genericApprovalTitle: '是否继续处理这个 Issue？',
    genericApprovalApproveEffect: '执行当前操作，然后继续后续处理；完成后会再次汇报结果。',
    genericApprovalRejectEffect: '本次不执行该操作，任务不会继续进入后续步骤。现有修改、调查结果和工作区都会保留。',
    genericApprovalRecommendation: '建议：确认下面的操作符合预期后再继续；不确定时可以暂不执行，并在备注中说明需要补充的信息。',
    outcomeWaiting: '代码修改和已有验证结果已保存，任务正在等待你决定是否执行下一步。',
    outcomeRecovery: '任务执行过程中断。已有调查结果和修改仍然保留，恢复后可以从当前进度继续。',
    outcomeCompleted: '修复流程已经完成，可以检查最终产出和验证结果。',
    outcomeResolvedUpstream: '已确认当前上游代码已经移除导致问题的旧路径，本任务无需再提交修复。',
    outcomeValidated: '修复修改已经完成，并已进入结果核验或等待后续操作。',
    outcomeImplemented: '已经产生代码修改，当前正在补充验证并收敛最终结果。',
    outcomeAnalyzed: '已经完成问题分析，正在形成或实施修复方案。',
    outcomeQueued: '任务已经创建，正在等待工作区或仓库执行槽就绪。',
    outcomeStarted: '任务已经开始，当前进展会持续更新。',
    justNow: '刚刚',
    seconds: '{value} 秒',
    minutes: '{value} 分钟',
    hours: '{value} 小时',
    days: '{value} 天',
    deadlinePassed: '已超过截止时间 {duration}',
    deadlineRemaining: '剩余 {duration}',
    intakeUnavailable: '当前执行位置不支持创建任务。',
    bridgeUnavailable: '宿主没有提供受信任的 LoopX 运行接口。请更新 BitFun 后重试。',
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
    approvalSubmitting: '正在提交审批决定，任务会在宿主确认后继续。',
    approvalSubmittingShort: '正在提交决定…',
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
    continueInvestigation: '继续调查',
    stopTask: '停止任务',
    activityInstallingDependencies: '正在准备项目依赖',
    activityBuildingInstaller: '正在构建 Windows 安装包',
    activityTestingUpgrade: '正在验证安装器升级链路',
    activityWaitingProcess: '正在等待外部进程返回结果',
    activitySyncingProgress: '正在同步工作进展',
    activityCheckingRepository: '正在检查仓库状态',
    activityRunningCommand: '正在执行项目命令',
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
    sidecar: 'LoopX 引擎',
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
    state_recovery_required: '已中止',
    state_completed: '已完成',
    state_failed: '失败',
    state_archived: '已归档',
    state_resolved_upstream: '上游已修复',
    phase_unknown: '等待宿主状态',
    phase_validating_environment: '验证环境',
    phase_resolving_intake: '复核输入',
    phase_preparing_workspace: '准备独立工作区',
    phase_creating_goal: '准备任务目标',
    phase_queued: '等待调度',
    phase_inspecting_goal: '检查任务目标',
    phase_building_turn: '正在准备下一阶段',
    phase_starting_agent: '启动修复任务',
    phase_agent_running: '正在分析或修改',
    phase_validating_progress: '正在核验结果',
    phase_settling_turn: '正在保存进展',
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
    modelEmpty: 'No enabled text models found',
    modelLoadFailed: 'Model list failed to load',
    modelReloadTitle: 'Refresh model list',
    modelSelectionChanged: 'Model changed. Analyze the URL again.',
    modelPrimaryTag: 'Primary',
    resolve: 'Analyze URL',
    resolving: 'Verifying URL against the live source',
    refresh: 'Refresh status',
    resetLoopx: 'Reset LoopX',
    resettingLoopx: 'Resetting LoopX...',
    resettingLoopxBackground: 'Cleaning tasks and saved progress in the background. You can keep using this window; it refreshes when cleanup finishes.',
    destructiveAction: 'Destructive action',
    resetLoopxTitle: 'Clear and start over',
    resetLoopxMessage: 'Stop and delete {tasks} tasks, {events} log events, all saved progress, and all managed workspaces. This cannot be undone.',
    resetLoopxRetained: 'Model configuration, GitHub login, MiniApp settings, and clean Git object caches are retained. Unsettled worktrees are not reused.',
    resetLoopxConfirm: 'Clear and start over',
    resetLoopxApplied: 'LoopX was cleared. You can start a fresh test.',
    unsupportedTitle: 'LoopX is unavailable in this execution location',
    unsupportedDefault: 'LoopX currently supports local Desktop workspaces only. Remote workspaces will not silently run on this device instead.',
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
    activity: 'Live model output',
    allTaskEvents: 'Model output for every issue is retained here in time order',
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
    liveOutput: 'Live model output',
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
    sourceScheduler: 'Task scheduler',
    sourceLoopx: 'LoopX engine',
    sourceAgent: 'Agent',
    sourceGit: 'Git',
    sourceGithub: 'GitHub',
    sourceSystem: 'System',
    toolExecCommand: 'Run command',
    toolRead: 'Read file',
    toolGrep: 'Search content',
    toolLs: 'Browse directory',
    toolWebFetch: 'Fetch web page',
    toolWebSearch: 'Search web',
    toolWrite: 'Write file',
    toolEdit: 'Edit file',
    toolQueued: 'Tool queued: {tool}',
    toolWaiting: 'Tool waiting: {tool}',
    toolStarted: 'Running: {tool}',
    toolConfirmation: 'Tool needs confirmation: {tool}',
    toolConfirmed: 'Tool confirmed: {tool}',
    toolRejected: 'Tool rejected: {tool}',
    toolCompleted: 'Tool completed: {tool}',
    toolFailed: 'Tool failed: {tool}',
    toolCancelled: 'Tool cancelled: {tool}',
    toolStateQueued: 'Queued',
    toolStateWaiting: 'Waiting',
    toolStateStarted: 'Running',
    toolStateConfirmation: 'Needs confirmation',
    toolStateConfirmed: 'Confirmed',
    toolStateRejected: 'Rejected',
    toolStateCompleted: 'Completed',
    toolStateFailed: 'Failed',
    toolStateCancelled: 'Cancelled',
    detailDurationMs: 'Duration (ms)',
    detailExitCode: 'Exit code',
    detailMatchCount: 'Matches',
    detailFileCount: 'Files',
    detailEntryCount: 'Entries',
    detailLineCount: 'Lines read',
    detailContentLength: 'Content length',
    detailTitle: 'Title',
    detailQueuePosition: 'Queue position',
    detailDependencies: 'Waiting for',
    newEvents: 'Scroll to latest output',
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
    decisionRequired: 'Your decision is required',
    afterApprove: 'If approved',
    afterReject: 'If rejected',
    approvalNote: 'Approval note',
    approvalNotePlaceholder: 'Optional reason for approving or rejecting',
    reject: 'Reject',
    approve: 'Approve',
    pause: 'Pause',
    resume: 'Resume',
    continueRun: 'Continue only this task',
    resumeRepository: 'Recover repository tasks ({value})',
    resumingRepository: 'Recovering failed tasks...',
    repositorySerial: 'Runs serially per repository',
    batchAction: 'Batch action',
    resumeRepositoryTitle: 'Recover repository failures',
    confirmContinue: 'Continue tasks',
    resumeRepositoryMessage: 'Recover {value} paused, interrupted, or failed tasks in {repository}. One task runs at a time; the rest remain queued.',
    resumeRepositoryApplied: 'Queued {value} repository tasks.',
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
    turn: 'Stage',
    goal: 'Objective',
    deadline: 'deadline',
    noRecentOutput: 'No new output for {duration}',
    lastOutput: 'Last output {duration} ago',
    updated: 'Updated {duration} ago',
    openInGithub: 'Open in GitHub',
    issueProgress: 'Issue progress',
    currentWork: 'Current',
    evidenceAndOutputs: 'Work completed',
    nextAction: 'Next',
    latestOutcome: 'Recent progress',
    outcomeUpdated: 'Updated {duration} ago',
    stageWorkspace: 'Worktree',
    stageAnalysis: 'Analysis and plan',
    stageImplementation: 'Implementation',
    stageValidation: 'Outcome validation',
    stageSettlement: 'Finish',
    stagePending: 'Pending',
    stageActive: 'Active',
    stageComplete: 'Complete',
    stageBlocked: 'Blocked',
    progressMeta: '{done}/{total} stages complete',
    progressPreparing: 'Preparing an isolated worktree',
    progressQueued: 'Waiting for the previous repository issue',
    progressAnalyzing: 'Analyzing the cause and forming an actionable plan',
    progressImplementing: 'Code changes are in progress',
    progressValidating: 'Validating this outcome',
    progressSettling: 'Saving this stage of progress',
    progressWaiting: 'Waiting for your decision',
    progressRecovery: 'Execution was interrupted',
    progressCompleted: 'The repair workflow is complete',
    progressResolvedUpstream: 'Resolved upstream',
    progressResolvedUpstreamDetail: 'The current upstream code has removed the original failure path, so no additional patch is required.',
    progressIdle: 'Waiting for task progress',
    evidenceWorkspace: 'The isolated workspace is ready',
    evidenceProgressSaved: 'Current progress is saved and can resume after an interruption',
    evidenceAnalysisReady: 'Issue analysis is complete and a repair direction is ready',
    evidenceCodeChanges: 'Code changes are ready across {value} files',
    evidenceCodeChangesUnknown: 'Code changes are ready',
    evidenceValidationPassed: 'Project validation has run without a reported failure',
    evidenceValidationNeedsAttention: 'Some validation did not pass and still needs attention',
    evidenceResolvedUpstream: 'Current upstream code no longer contains the original failure path',
    evidenceNoPatchRequired: 'No repair patch needs to be submitted',
    evidenceNone: 'No user-facing work result is available yet',
    nextPrepare: 'Finish the worktree, then analyze the issue.',
    nextQueued: 'Start automatically when the repository slot is free.',
    nextAnalyze: 'Finish root-cause analysis and form an actionable plan.',
    nextImplement: 'Continue implementation and retain verifiable change evidence.',
    nextValidate: 'Validate the current output and save progress.',
    nextSettle: 'Save progress, then continue to the next stage or finish.',
    nextRecover: 'Resolve the execution failure, then continue the task.',
    nextApproval: 'Review the approval request and decide whether to continue.',
    nextCompleted: 'Review the final outcome and outputs.',
    nextResolvedUpstream: 'No further repair is required. Keep the record or archive this task.',
    issueDescription: 'Issue description',
    loadingIssueDescription: 'Loading issue description...',
    issueDescriptionUnavailable: 'Issue description is temporarily unavailable.',
    problemBackground: 'Problem',
    problemImpact: 'Impact',
    issueApiProxyTitle: 'Third-party plugins fail to start after upgrading',
    issueApiProxyBackground: 'After upgrading from 2.0.2 to 2.0.4, the third-party task board still depends on the apiProxy service removed by the new version, so the plugin cannot start.',
    issueApiProxyImpact: 'One failed plugin prevents the entire plugin tree from loading, blocking normal use of installed third-party plugins.',
    issueApiProxyProgress: 'The cause and repair direction are clear: add compatibility for older plugins and prevent one third-party plugin from breaking the whole startup flow. Code changes have not started.',
    issueInputModalityTitle: 'Manually added multimodal models cannot send images',
    issueInputModalityBackground: 'The model settings save image capability under a different field from the one read by the runtime, so manually added models lose image input support.',
    issueInputModalityImpact: 'The chat UI treats an image-capable model as text-only and prevents users from sending images.',
    issueInputModalityProgress: 'The latest upstream code no longer contains the incorrect writer, so no additional repair patch is currently required.',
    issueMacFocusTitle: 'The macOS window steals focus after minimization',
    issueMacFocusBackground: 'After minimization, an activation event brings the app back to the foreground and interrupts other work.',
    issueMacFocusImpact: 'The window appears unexpectedly and takes keyboard focus from the active application.',
    issueMacFocusProgress: 'Root cause analysis, code changes, and existing validation are complete. Publishing the pull request still requires a decision.',
    issueGenericBackground: 'This Issue reports the following problem: {title}',
    issueGenericImpact: 'The problem affects normal use of the related feature and requires a decision on the next step.',
    issueGenericProgressAnalyzed: 'Initial analysis is complete and a direction is available, but repair and validation are not finished.',
    issueGenericProgressImplemented: 'Repair changes are available, but validation and final confirmation are not complete.',
    publishApprovalTitle: 'Publish the fix and create a pull request?',
    publishApprovalSummary: 'The fix is prepared on branch {branch} at commit {commit} for {repository}. Your approval is required before publishing it.',
    publishApprovalSummaryGeneric: 'The fix and publishing materials are ready for {repository}. Your approval is required before creating the pull request.',
    publishApprovalApproveEffect: 'Push the fix branch, create a pull request, then continue with macOS host verification. Approval does not merge code automatically.',
    publishApprovalRejectEffect: 'Do not push the branch or create a pull request. Keep the local branch, commit, and validation results, and stop at this step.',
    publishApprovalRecommendationReady: 'Recommended: approve. The change has validation results and remains reviewable in the pull request; it will not be merged automatically.',
    publishApprovalRecommendationReview: 'Review the change and validation results first. Approval publishes a pull request but does not merge it automatically.',
    publishApprovalApprove: 'Approve and create PR',
    publishApprovalReject: 'Keep local only',
    autonomyApprovalTitle: 'The repair is not finished. Continue?',
    autonomyApprovalSummaryAnalyzed: 'The issue has been analyzed and a clear direction is available, but implementation and validation are not complete.',
    autonomyApprovalSummaryImplemented: 'Repair changes are available, but validation and final confirmation are not complete.',
    autonomyApprovalApproveEffect: 'Continue repairing and validating, then report the result. This will not publish or merge code.',
    autonomyApprovalRejectEffect: 'Pause work and preserve the current workspace, changes, and investigation results.',
    autonomyApprovalRecommendation: 'Recommendation: continue. The direction is clear and the remaining work is implementation and validation.',
    continueRepair: 'Continue repair',
    pauseProcessing: 'Pause for now',
    implementationApprovalTitle: 'Allow code changes to begin?',
    implementationApprovalApproveEffect: 'Allow code changes and tests in the isolated workspace. Results will be reported again; code will not be pushed, published as a pull request, or merged automatically.',
    implementationApprovalRejectEffect: 'Do not modify code or continue to later repair steps. Keep the investigation results and workspace so the task can be resumed manually.',
    implementationApprovalRecommendation: 'Recommendation: allow it. The cause and repair direction are clear; implementation and validation remain.',
    implementationApprovalApprove: 'Allow repair to begin',
    implementationApprovalReject: 'Do not modify yet',
    genericApprovalTitle: 'Continue handling this Issue?',
    genericApprovalApproveEffect: 'Perform the current operation and continue processing. Results will be reported again afterward.',
    genericApprovalRejectEffect: 'Do not perform this operation or continue to later steps. Keep existing changes, investigation results, and the workspace.',
    genericApprovalRecommendation: 'Recommendation: continue only when the operation below matches your expectation. Otherwise pause and note what information is missing.',
    outcomeWaiting: 'Code changes and available validation results are saved. The task is waiting for your decision before taking the next step.',
    outcomeRecovery: 'Execution was interrupted. Existing investigation results and changes are preserved and can resume from the current progress.',
    outcomeCompleted: 'The repair workflow is complete. Review the final outputs and validation results.',
    outcomeResolvedUpstream: 'Current upstream code has removed the old failure path, so this task does not need another repair patch.',
    outcomeValidated: 'The repair changes are ready and have reached validation or a follow-up operation.',
    outcomeImplemented: 'Code changes are ready. Validation and final outcome work are still in progress.',
    outcomeAnalyzed: 'Issue analysis is complete. The repair approach is being finalized or implemented.',
    outcomeQueued: 'The task is created and waiting for its workspace or repository execution slot.',
    outcomeStarted: 'The task has started. Progress will continue to update here.',
    justNow: 'just now',
    seconds: '{value}s',
    minutes: '{value}m',
    hours: '{value}h',
    days: '{value}d',
    deadlinePassed: 'Deadline passed by {duration}',
    deadlineRemaining: '{duration} remaining',
    intakeUnavailable: 'Tasks cannot be created from this execution location.',
    bridgeUnavailable: 'The host did not expose the trusted LoopX runtime interface. Update BitFun and try again.',
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
    approvalSubmitting: 'Submitting the decision. The task will continue after host confirmation.',
    approvalSubmittingShort: 'Submitting decision...',
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
    continueInvestigation: 'Continue investigation',
    stopTask: 'Stop task',
    activityInstallingDependencies: 'Preparing project dependencies',
    activityBuildingInstaller: 'Building the Windows installer',
    activityTestingUpgrade: 'Validating the installer upgrade path',
    activityWaitingProcess: 'Waiting for an external process to finish',
    activitySyncingProgress: 'Synchronizing durable progress',
    activityCheckingRepository: 'Checking repository state',
    activityRunningCommand: 'Running a project command',
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
    sidecar: 'LoopX engine',
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
    state_recovery_required: 'Interrupted',
    state_completed: 'Completed',
    state_failed: 'Failed',
    state_archived: 'Archived',
    state_resolved_upstream: 'Resolved upstream',
    phase_unknown: 'Waiting for host state',
    phase_validating_environment: 'Validating environment',
    phase_resolving_intake: 'Resolving intake',
    phase_preparing_workspace: 'Preparing an isolated workspace',
    phase_creating_goal: 'Preparing the task objective',
    phase_queued: 'Waiting for scheduler',
    phase_inspecting_goal: 'Reviewing the task objective',
    phase_building_turn: 'Building turn',
    phase_starting_agent: 'Starting the repair task',
    phase_agent_running: 'Analyzing or modifying code',
    phase_validating_progress: 'Validating results',
    phase_settling_turn: 'Saving progress',
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
  intakeHistory: byId('intake-history'),
  modelSelect: byId('model-select'),
  resolveButton: byId('resolve-button'),
  resetLoopx: byId('reset-loopx'),
  notice: byId('notice'),
  unsupportedBanner: byId('unsupported-banner'),
  unsupportedReason: byId('unsupported-reason'),
  approvalAlert: byId('approval-alert'),
  approvalAlertTitle: byId('approval-alert-title'),
  approvalAlertMessage: byId('approval-alert-message'),
  approvalAlertOpen: byId('approval-alert-open'),
  approvalAlertReject: byId('approval-alert-reject'),
  approvalAlertApprove: byId('approval-alert-approve'),
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
  repositoryActions: byId('repository-actions'),
  resumeRepository: byId('resume-repository'),
  repositoryActionsMeta: byId('repository-actions-meta'),
  taskItems: byId('task-items'),
  taskEmpty: byId('task-empty'),
  showAllEvents: byId('show-all-events'),
  logTitle: byId('log-title'),
  selectedState: byId('selected-state'),
  selectedSummary: byId('selected-summary'),
  issueLink: byId('issue-link'),
  issueNumber: byId('issue-number'),
  issueApprovalPanel: byId('issue-approval-panel'),
  issueApprovalTitle: byId('issue-approval-title'),
  issueApprovalSummary: byId('issue-approval-summary'),
  issueApprovalBackground: byId('issue-approval-background'),
  issueApprovalImpact: byId('issue-approval-impact'),
  issueApprovalApproveEffect: byId('issue-approval-approve-effect'),
  issueApprovalRejectEffect: byId('issue-approval-reject-effect'),
  issueApprovalRecommendation: byId('issue-approval-recommendation'),
  issueApprovalNote: byId('issue-approval-note'),
  issueApprovalReject: byId('issue-approval-reject'),
  issueApprovalApprove: byId('issue-approval-approve'),
  issueProgressPanel: byId('issue-progress-panel'),
  issueProgressMeta: byId('issue-progress-meta'),
  issueStageList: byId('issue-stage-list'),
  issueCurrentHeading: byId('issue-current-heading'),
  issueCurrentDetail: byId('issue-current-detail'),
  issueEvidenceList: byId('issue-evidence-list'),
  issueNextAction: byId('issue-next-action'),
  issueOutcomePanel: byId('issue-outcome-panel'),
  issueOutcomeMeta: byId('issue-outcome-meta'),
  issueOutcome: byId('issue-outcome'),
  issueDescriptionPanel: byId('issue-description-panel'),
  issueDescription: byId('issue-description'),
  taskActions: byId('task-actions'),
  logScroll: byId('log-scroll'),
  logEmpty: byId('log-empty'),
  logList: byId('log-list'),
  newEvents: byId('new-events'),
  issueDetailDialog: byId('issue-detail-dialog'),
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
  followLogs: true,
  preview: null,
  pendingCreate: null,
  pendingRetry: null,
  approvalTaskId: null,
  promptedGateIds: new Set(),
  pendingApprovalPrompt: false,
  syncing: false,
  syncRequested: false,
  pendingResumeSignal: false,
  lastClockSampleAt: Date.now(),
  lastHostSignalAt: Date.now(),
  lastReattachAt: 0,
  gapRecovery: null,
  connected: false,
  railCollapsed: false,
  intakeHistory: [],
  outputHistory: [],
  outputKeys: new Set(),
  outputCharacters: 0,
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

function isResolvedUpstream(task) {
  const summary = String(task && task.lastAgentSummary || '');
  return /covered[-_ ]?upstream.{0,80}no[-_ ]?follow[-_ ]?up/is.test(summary)
    || /原始故障路径.{0,40}(?:消失|移除).{0,120}(?:不开\s*PR|无需.{0,20}修复)/is.test(summary);
}

function taskStateLabel(task) {
  if (isResolvedUpstream(task)) return stateLabel('resolved_upstream');
  return isWorkspacePreparationFailure(task) ? stateLabel('failed') : stateLabel(task && task.state);
}

function pendingActionFor(task) {
  return task && task.taskId ? state.taskActionPending.get(task.taskId) : '';
}

function taskVisualState(task) {
  const pending = pendingActionFor(task);
  if (pending === 'pause' || pending === 'abort') return 'cancelling';
  if (isResolvedUpstream(task)) return 'completed';
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

function renderIntakeHistory() {
  if (!view.intakeHistory) return;
  const fragment = document.createDocumentFragment();
  state.intakeHistory.forEach((url) => {
    const option = document.createElement('option');
    option.value = url;
    fragment.append(option);
  });
  view.intakeHistory.replaceChildren(fragment);
}

async function loadIntakeHistory() {
  if (!app || !app.storage || typeof app.storage.get !== 'function') return;
  try {
    const stored = await app.storage.get(INTAKE_HISTORY_STORAGE_KEY);
    state.intakeHistory = Array.isArray(stored)
      ? stored.filter((value) => typeof value === 'string' && value.trim()).slice(0, MAX_INTAKE_HISTORY)
      : [];
    renderIntakeHistory();
  } catch (_error) {
    state.intakeHistory = [];
  }
}

async function rememberIntake(value) {
  const url = String(value || '').trim();
  if (!url) return;
  state.intakeHistory = [
    url,
    ...state.intakeHistory.filter((entry) => entry.toLowerCase() !== url.toLowerCase()),
  ].slice(0, MAX_INTAKE_HISTORY);
  renderIntakeHistory();
  if (!app || !app.storage || typeof app.storage.set !== 'function') return;
  try {
    await app.storage.set(INTAKE_HISTORY_STORAGE_KEY, state.intakeHistory);
  } catch (_error) {
    // Input history remains available for the current session.
  }
}

function currentModelSelection() {
  const stored = readStoredModelSelection();
  const selected = view.modelSelect && view.modelSelect.value ? view.modelSelect.value : '';
  if (selected && (selected !== 'auto' || !stored || stored === 'auto')) return selected;
  return stored || selected || 'auto';
}

function describeModelOption(model) {
  const displayName = String(model.name || model.modelName || model.id || '').trim();
  const modelName = String(model.modelName || '').trim();
  const provider = String(model.provider || '').trim();
  const primary = displayName || modelName || model.id;
  const details = [];
  if (modelName && modelName !== primary) details.push(modelName);
  if (provider && provider !== primary && provider !== modelName) details.push(provider);
  return details.length > 0 ? `${primary} (${details.join(' · ')})` : primary;
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

function compactHumanTitle(rawTitle, fallback) {
  const cleaned = String(rawTitle || '')
    .replace(/^\s*[【[]\s*(?:bug|问题)\s*[】\]]\s*/i, '')
    .replace(/^\s*\d{1,2}[./-]\d{1,2}日?\s*/, '')
    .trim();
  if (!cleaned) return fallback;
  return cleaned.length > 88 ? `${cleaned.slice(0, 87)}…` : cleaned;
}

function issueContext(task) {
  const item = task && task.identity && task.identity.item;
  const fallback = item ? compactItemLabel(item) : '--';
  const rawTitle = identityTitleOf(task);
  const evidence = taskProgressEvidence(task);
  const source = `${rawTitle}\n${String(task && task.lastAgentSummary || '')}`;

  if (/plugin tree failed to load|waiting for service:\s*apiProxy/i.test(source)) {
    return {
      title: text('issueApiProxyTitle'),
      background: text('issueApiProxyBackground'),
      impact: text('issueApiProxyImpact'),
      progress: text('issueApiProxyProgress'),
    };
  }
  if (/inputModalities.{0,80}\binput\b|多模态模型无法发送图片/is.test(source)) {
    return {
      title: text('issueInputModalityTitle'),
      background: text('issueInputModalityBackground'),
      impact: text('issueInputModalityImpact'),
      progress: text('issueInputModalityProgress'),
    };
  }
  if (/macOS.{0,80}(?:焦点|focus)|最小化后.{0,80}(?:前台|焦点)/is.test(source)) {
    return {
      title: text('issueMacFocusTitle'),
      background: text('issueMacFocusBackground'),
      impact: text('issueMacFocusImpact'),
      progress: text('issueMacFocusProgress'),
    };
  }

  const title = compactHumanTitle(rawTitle, fallback);
  return {
    title,
    background: text('issueGenericBackground', { title }),
    impact: text('issueGenericImpact'),
    progress: text(evidence.changes > 0
      ? 'issueGenericProgressImplemented'
      : 'issueGenericProgressAnalyzed'),
  };
}

function issueDisplayTitle(task) {
  return issueContext(task).title;
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
  if (selected && selected.state === 'running' && selected.phase === 'agent_running') return selected;
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks)
    ? state.snapshot.tasks
    : [];
  return tasks.find((task) => task.state === 'running' && task.phase === 'agent_running') || null;
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

function clearRunUiState() {
  state.events = [];
  state.eventKeys.clear();
  state.selectedTaskId = null;
  state.preview = null;
  state.pendingCreate = null;
  state.pendingRetry = null;
  state.approvalTaskId = null;
  state.promptedGateIds.clear();
  state.pendingApprovalPrompt = false;
  state.repositoryResumeTarget = null;
  state.repositoryResumePending = false;
  state.taskActionPending.clear();
  state.itemMetadata.clear();
  state.metadataRequests.clear();
  state.outputHistory = [];
  state.outputKeys.clear();
  state.outputCharacters = 0;
  clearTurnOutputTimer();
  resetTurnOutput(null);
  if (view.issueDetailDialog.open) view.issueDetailDialog.close();
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
  if (state.turnOutput.inFlight) return;
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
      const turnId = event.turnId || page.turnId || state.turnOutput.turnId || '';
      const outputKey = `${task.taskId}:${turnId}:${event.cursor}`;
      if (!state.outputKeys.has(outputKey)) {
        const rawText = event.text == null ? '' : String(event.text);
        const boundedText = rawText.length <= MAX_OUTPUT_EVENT_CHARS
          ? rawText
          : `${rawText.slice(0, MAX_OUTPUT_EVENT_CHARS / 2)}\n...\n${rawText.slice(-MAX_OUTPUT_EVENT_CHARS / 2)}`;
        state.outputKeys.add(outputKey);
        state.outputHistory.push({
          ...event,
          text: boundedText,
          taskId: task.taskId,
          turnId,
        });
        state.outputCharacters += boundedText.length;
      }
    });
    state.turnOutput.events.sort((left, right) => left.cursor - right.cursor);
    while (state.turnOutput.events.length > MAX_TURN_OUTPUT_EVENTS) {
      state.turnOutput.events.shift();
    }
    while (
      state.outputHistory.length > MAX_OUTPUT_HISTORY_EVENTS
      || state.outputCharacters > MAX_OUTPUT_HISTORY_CHARS
    ) {
      const removed = state.outputHistory.shift();
      state.outputKeys.delete(`${removed.taskId}:${removed.turnId || ''}:${removed.cursor}`);
      state.outputCharacters = Math.max(0, state.outputCharacters - String(removed.text || '').length);
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
  const streamChanged = previousStreamId && previousStreamId !== snapshot.streamId;
  state.snapshot = snapshot;
  if (streamChanged) {
    clearRunUiState();
  } else {
    replaceStreamEvents(snapshot.streamId);
  }
  if (state.selectedTaskId && !taskForId(state.selectedTaskId)) {
    state.selectedTaskId = null;
    if (view.issueDetailDialog.open) view.issueDetailDialog.close();
  }
  state.connected = true;
  state.lastHostSignalAt = Date.now();
  view.connectionLabel.textContent = text('connected');
  view.root.setAttribute('aria-busy', 'false');
  renderAll();
  if (state.pendingApprovalPrompt) {
    syncApprovalAttention(true);
    if (currentApprovalAttention()) state.pendingApprovalPrompt = false;
  }
}

async function attachSnapshot(loadHistory = false, resumeDetected = false) {
  if (!app || !app.loopx) {
    showBridgeUnavailable();
    return;
  }
  if (resumeDetected) state.pendingResumeSignal = true;
  if (state.syncing) {
    state.syncRequested = true;
    return;
  }
  state.syncing = true;
  if (!view.repositoryActions.hidden) view.resumeRepository.disabled = true;
  try {
    do {
      state.syncRequested = false;
      const reportResume = state.pendingResumeSignal;
      state.pendingResumeSignal = false;
      const knownStreamId = state.snapshot && state.snapshot.streamId;
      const afterCursor = state.snapshot && state.snapshot.cursor;
      if (!state.connected) view.connectionLabel.textContent = text('connecting');
      const response = await app.loopx.attach({
        ...(knownStreamId ? { knownStreamId } : {}),
        ...(Number.isSafeInteger(afterCursor) ? { afterCursor } : {}),
        ...(reportResume ? { resumeDetected: true } : {}),
      });
      state.lastReattachAt = Date.now();
      applySnapshot(response && response.snapshot);
      void loadModelCatalog();
      const snapshot = state.snapshot;
      if (loadHistory && state.events.length === 0 && snapshot.cursor > 0) {
        const replay = await replayEvents(snapshot.streamId, 0, true);
        if (replay.changed) {
          renderLogs();
          syncApprovalAttention(true);
        }
      }
    } while (state.syncRequested);
  } catch (error) {
    state.connected = false;
    view.connectionLabel.textContent = text('connectionFailed');
    showNotice(errorMessage(error), 'error');
  } finally {
    state.syncing = false;
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
  state.lastHostSignalAt = Date.now();
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
  if (event.kind === 'approval_required') state.pendingApprovalPrompt = true;
  state.snapshot.cursor = Math.max(cursor, event.cursor);
  if (changed) {
    renderLogs();
    if (event.kind === 'approval_required') syncApprovalAttention(true);
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
  const environmentBusyOrBlocked = environmentStatus === 'checking' || environmentStatus === 'blocked';
  view.unsupportedBanner.hidden = !snapshot || supported;
  if (snapshot && !supported) {
    view.unsupportedReason.textContent = snapshot.unsupportedReason || text('unsupportedDefault');
  }
  view.resolveButton.disabled = !supported || environmentBusyOrBlocked || state.resetPending;
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

const ERROR_TASK_STATES = new Set(['recovery_required', 'failed']);
const RECOVERABLE_TASK_STATES = new Set(['stopped', ...ERROR_TASK_STATES]);
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

function taskSortPriority(task) {
  if (isResolvedUpstream(task)) return 20;
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
  return issueDisplayTitle(task) || compactItemLabel(item);
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

function recoverableTasksForRepository(tasks, repository) {
  const key = repositoryKey(repository);
  return tasks.filter((task) =>
    RECOVERABLE_TASK_STATES.has(task.state)
    && !isResolvedUpstream(task)
    && repositoryKey(task.identity && task.identity.item && task.identity.item.repository) === key);
}

function renderRepositoryActions(tasks) {
  const selected = selectedTask();
  const eligibleRepositories = new Map();
  tasks.forEach((task) => {
    if (!RECOVERABLE_TASK_STATES.has(task.state) || isResolvedUpstream(task)) return;
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
    : ([...eligibleRepositories.values()][0] || null);
  const eligible = repository ? recoverableTasksForRepository(tasks, repository) : [];
  state.repositoryResumeTarget = repository && eligible.length > 0
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

  const main = document.createElement('span');
  main.className = 'task-item__main';
  const label = document.createElement('strong');
  const item = task.identity && task.identity.item;
  const identityTitle = issueDisplayTitle(task);
  label.textContent = identityTitle || compactItemLabel(item);
  const meta = document.createElement('small');
  const activity = task.lastOutputAt || task.updatedAt;
  meta.textContent = `${repositoryLabel(item && item.repository)} · ${compactItemLabel(item)} · ${relativeLabel(activity)}`;
  main.append(label, meta);
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
  } else {
    taskState.classList.add('task-item__hint');
    if (visualState === 'running') taskState.classList.add('task-item__hint--running');
    taskState.textContent = taskStateDisplayLabel(task);
  }
  taskState.title = taskStateDisplayLabel(task);
  button.setAttribute('aria-label', `${label.textContent}, ${taskStateDisplayLabel(task)}`);
  const compact = document.createElement('span');
  compact.className = 'task-item__compact';
  compact.textContent = item && item.number ? `#${item.number}` : shortId(task.taskId);
  button.append(main, compact, taskState);
  button.addEventListener('click', () => selectTask(task.taskId));
  return button;
}

function renderTasks() {
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  const fragment = document.createDocumentFragment();
  sortedTaskList(tasks).forEach((task) => fragment.append(taskButton(task)));
  view.taskItems.replaceChildren(fragment);
  view.taskCount.textContent = String(tasks.length);
  view.taskEmpty.hidden = tasks.length !== 0;
  renderRepositoryActions(tasks);
  syncApprovalAttention(false);
}

function latestGate(taskId) {
  const task = taskForId(taskId);
  if (task && task.pendingGateId) {
    const actionKind = task.pendingGateActionKind || '';
    return {
      gateId: task.pendingGateId,
      actionKind,
      event: {
        message: task.pendingGateMessage || text('approvalNeeded'),
        details: {
          gateId: task.pendingGateId,
          actionKind,
          autonomousTurns: String(task.autonomousTurnsSinceReview || 4),
        },
      },
    };
  }
  for (let index = state.events.length - 1; index >= 0; index -= 1) {
    const event = state.events[index];
    if (event.taskId !== taskId || event.kind !== 'approval_required') continue;
    const details = event.details || {};
    const gateId = details.gateId || details.gate_id || details.id;
    if (gateId) {
      return {
        event,
        gateId,
        actionKind: details.actionKind || details.action_kind || '',
      };
    }
  }
  return null;
}

function currentApprovalAttention() {
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks)
    ? sortedTaskList(state.snapshot.tasks)
    : [];
  const task = tasks.find((candidate) => candidate.state === 'waiting_for_user') || null;
  if (!task) return null;
  return { task, gate: latestGate(task.taskId) };
}

function gateRawMessage(gate) {
  return String(gate && gate.event && gate.event.message || '').trim();
}

function approvalPresentation(task, gate) {
  const rawMessage = gateRawMessage(gate);
  const actionKind = String(gate && gate.actionKind || '').toLowerCase();
  const context = issueContext(task);
  if (actionKind === 'autonomous_budget_review') {
    return {
      title: text('autonomyApprovalTitle'),
      summary: context.progress,
      approveEffect: text('autonomyApprovalApproveEffect'),
      rejectEffect: text('autonomyApprovalRejectEffect'),
      recommendation: text('autonomyApprovalRecommendation'),
      approveLabel: text('continueRepair'),
      rejectLabel: text('pauseProcessing'),
    };
  }

  const gateSource = `${rawMessage}\n${String(task && task.lastAgentSummary || '')}`;
  const implementationApproval = /仓库写入|repo(?:sitory)?[- ]write|write scope|实施仍未开始|parent approval for repo write/i.test(gateSource);
  if (implementationApproval) {
    return {
      title: text('implementationApprovalTitle'),
      summary: context.progress,
      approveEffect: text('implementationApprovalApproveEffect'),
      rejectEffect: text('implementationApprovalRejectEffect'),
      recommendation: text('implementationApprovalRecommendation'),
      approveLabel: text('implementationApprovalApprove'),
      rejectLabel: text('implementationApprovalReject'),
    };
  }

  const publishPullRequest = actionKind.includes('publish')
    || actionKind.includes('pull_request')
    || /\bpr bundle\b|(?:publish|push|creat(?:e|ing|ion)).{0,100}(?:pull request|\bpr\b)/i.test(rawMessage);
  if (publishPullRequest) {
    const branch = rawMessage.match(/\bbranch\s+([^,\s)]+)/i)?.[1] || '';
    const commit = rawMessage.match(/\bcommit\s+([0-9a-f]{7,40})\b/i)?.[1] || '';
    const messageRepository = rawMessage.match(/\bto\s+([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+)(?=;|[\s.,]|$)/i)?.[1] || '';
    const item = task && task.identity && task.identity.item;
    const repository = messageRepository || repositoryLabel(item && item.repository) || '--';
    const evidence = taskProgressEvidence(task);
    const validated = evidence.validated || evidence.settled || /\b(?:validated|verified)\b/i.test(rawMessage);
    return {
      title: text('publishApprovalTitle'),
      summary: branch && commit
        ? text('publishApprovalSummary', { branch, commit, repository })
        : text('publishApprovalSummaryGeneric', { repository }),
      approveEffect: text('publishApprovalApproveEffect'),
      rejectEffect: text('publishApprovalRejectEffect'),
      recommendation: text(validated ? 'publishApprovalRecommendationReady' : 'publishApprovalRecommendationReview'),
      approveLabel: text('publishApprovalApprove'),
      rejectLabel: text('publishApprovalReject'),
    };
  }

  return {
    title: text('genericApprovalTitle'),
    summary: context.progress,
    approveEffect: text('genericApprovalApproveEffect'),
    rejectEffect: text('genericApprovalRejectEffect'),
    recommendation: text('genericApprovalRecommendation'),
    approveLabel: text('approve'),
    rejectLabel: text('reject'),
  };
}

function syncApprovalAttention(autoOpen = false) {
  const attention = currentApprovalAttention();
  state.approvalTaskId = attention ? attention.task.taskId : null;
  view.approvalAlert.hidden = !attention;
  if (!attention) return;

  const { task, gate } = attention;
  const item = task.identity && task.identity.item;
  const presentation = approvalPresentation(task, gate);
  view.approvalAlertTitle.textContent = `${issueDisplayTitle(task) || itemLabel(item)} · ${text('decisionRequired')}`;
  view.approvalAlertOpen.title = presentation.summary;
  view.approvalAlertOpen.setAttribute('aria-label', `${presentation.title} ${presentation.summary}`);
  const pending = Boolean(pendingActionFor(task));
  view.approvalAlertMessage.textContent = pending
    ? text('approvalSubmitting')
    : `${presentation.title} ${presentation.summary}`;
  view.approvalAlertReject.textContent = presentation.rejectLabel;
  view.approvalAlertApprove.textContent = pending ? text('approvalSubmittingShort') : presentation.approveLabel;
  view.approvalAlertReject.disabled = pending || !gate;
  view.approvalAlertApprove.disabled = pending || !gate;

  if (
    autoOpen
    && gate
    && !state.promptedGateIds.has(gate.gateId)
  ) {
    state.promptedGateIds.add(gate.gateId);
    const anotherDialogOpen = [...document.querySelectorAll('dialog[open]')]
      .some((dialog) => dialog !== view.issueDetailDialog);
    if (!anotherDialogOpen) selectTask(task.taskId);
  }
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
    void performAction(action, task);
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
  if (isResolvedUpstream(task)) {
    return;
  }
  if (['preparing', 'queued', 'running', 'retry_wait'].includes(task.state)) {
    fragment.append(makeActionButton(text('pause'), 'pause', task));
  }
  if (['stopped', 'completed', 'failed'].includes(task.state)) {
    fragment.append(makeActionButton(text('archive'), 'archive', task));
  }
  if (task.state === 'archived') {
    fragment.append(makeActionButton(text('restore'), 'restore', task));
  }
  view.taskActions.append(fragment);
}

function safeMarkdownUrl(rawUrl, baseUrl) {
  try {
    const url = new URL(rawUrl, baseUrl || undefined);
    return url.protocol === 'https:' || url.protocol === 'http:' ? url.href : '';
  } catch (_error) {
    return '';
  }
}

function appendInlineMarkdown(parent, source, baseUrl) {
  const pattern = /(`[^`\n]+`|\[[^\]\n]+\]\([^\s)]+\)|\*\*[^*\n]+\*\*|__[^_\n]+__)/g;
  let cursor = 0;
  for (const match of source.matchAll(pattern)) {
    if (match.index > cursor) parent.append(document.createTextNode(source.slice(cursor, match.index)));
    const token = match[0];
    if (token.startsWith('`')) {
      const code = document.createElement('code');
      code.textContent = token.slice(1, -1);
      parent.append(code);
    } else if (token.startsWith('[')) {
      const parts = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      const href = parts ? safeMarkdownUrl(parts[2], baseUrl) : '';
      if (parts && href) {
        const link = document.createElement('a');
        link.href = href;
        link.target = '_blank';
        link.rel = 'noopener noreferrer';
        link.textContent = parts[1];
        parent.append(link);
      } else {
        parent.append(document.createTextNode(token));
      }
    } else {
      const strong = document.createElement('strong');
      strong.textContent = token.slice(2, -2);
      parent.append(strong);
    }
    cursor = match.index + token.length;
  }
  if (cursor < source.length) parent.append(document.createTextNode(source.slice(cursor)));
}

function renderMarkdown(target, source, baseUrl) {
  const fragment = document.createDocumentFragment();
  const lines = String(source || '').replace(/\r\n?/g, '\n').split('\n');
  let list = null;
  let code = null;
  let paragraph = null;
  const closeParagraph = () => { paragraph = null; };
  const closeList = () => { list = null; };
  lines.forEach((line) => {
    if (/^```/.test(line)) {
      closeParagraph();
      closeList();
      if (code) {
        code = null;
      } else {
        const pre = document.createElement('pre');
        code = document.createElement('code');
        pre.append(code);
        fragment.append(pre);
      }
      return;
    }
    if (code) {
      code.append(document.createTextNode(`${code.textContent ? '\n' : ''}${line}`));
      return;
    }
    if (!line.trim()) {
      closeParagraph();
      closeList();
      return;
    }
    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      closeParagraph();
      closeList();
      const element = document.createElement(`h${Math.min(heading[1].length + 2, 6)}`);
      appendInlineMarkdown(element, heading[2], baseUrl);
      fragment.append(element);
      return;
    }
    const listItem = line.match(/^\s*(?:[-*+] |\d+\. )(.+)$/);
    if (listItem) {
      closeParagraph();
      if (!list) {
        list = document.createElement(/^\s*\d+\./.test(line) ? 'ol' : 'ul');
        fragment.append(list);
      }
      const item = document.createElement('li');
      appendInlineMarkdown(item, listItem[1], baseUrl);
      list.append(item);
      return;
    }
    closeList();
    const quote = line.match(/^>\s?(.*)$/);
    if (quote) {
      closeParagraph();
      const element = document.createElement('blockquote');
      appendInlineMarkdown(element, quote[1], baseUrl);
      fragment.append(element);
      return;
    }
    if (!paragraph) {
      paragraph = document.createElement('p');
      fragment.append(paragraph);
    } else {
      paragraph.append(document.createElement('br'));
    }
    appendInlineMarkdown(paragraph, line, baseUrl);
  });
  target.replaceChildren(fragment);
}

function progressTaskEvents(task) {
  return state.events.filter((event) => (
    event.taskId === task.taskId
    && (event.generation == null || Number(event.generation) === Number(task.generation))
  ));
}

function compactArtifactPath(value) {
  const parts = String(value || '').split(/[\\/]/).filter(Boolean);
  return parts.slice(-3).join('/');
}

function isProductArtifact(value) {
  const normalized = String(value || '').replace(/\\/g, '/').toLowerCase();
  if (!normalized) return false;
  return !normalized.startsWith('.loopx/')
    && !normalized.includes('/.loopx/')
    && !normalized.startsWith('.codex/')
    && !normalized.includes('/.codex/')
    && !normalized.startsWith('.bitfun/')
    && !normalized.includes('/.bitfun/')
    && !normalized.includes('/appdata/local/temp/');
}

function taskProgressEvidence(task) {
  const events = progressTaskEvents(task);
  const started = events.filter((event) => event.details && event.details.activity === 'started');
  const countTools = (names) => started.filter((event) => names.has(event.toolName)).length;
  const reads = countTools(new Set(['Read', 'LS']));
  const searches = countTools(new Set(['Grep']));
  const web = countTools(new Set(['WebFetch', 'WebSearch']));
  const changes = started.filter((event) => (
    ['Write', 'Edit', 'ApplyPatch'].includes(event.toolName)
    && isProductArtifact(event.details && event.details.summary)
  ));
  const commands = countTools(new Set(['ExecCommand']));
  const failures = events.filter((event) => event.details && event.details.activity === 'failed').length;
  const artifacts = [...new Set(changes
    .map((event) => compactArtifactPath(event.details && event.details.summary))
    .filter(Boolean))].slice(-3);
  const validated = events.some((event) => event.phase === 'validating_progress');
  const settled = Boolean(task.settlement && task.settlement.receiptId);
  return {
    reads,
    searches,
    web,
    analysisCount: reads + searches + web,
    changes: changes.length,
    commands,
    failures,
    artifacts,
    validated,
    settled,
  };
}

function progressStageStatus(task, evidence) {
  if (isResolvedUpstream(task)) {
    return [
      ['stageWorkspace', 'complete'],
      ['stageAnalysis', 'complete'],
      ['stageImplementation', 'complete'],
      ['stageValidation', 'complete'],
      ['stageSettlement', 'complete'],
    ];
  }
  const hasSummary = Boolean(task.lastAgentSummary);
  const afterAgent = hasSummary || evidence.validated || evidence.settled
    || ['validating_progress', 'settling_turn', 'recovering', 'finished'].includes(task.phase);
  const workspace = task.workspacePath
    ? 'complete'
    : (task.phase === 'preparing_workspace' ? 'active' : (task.error ? 'blocked' : 'pending'));
  const analysis = hasSummary
    ? 'complete'
    : (evidence.analysisCount > 0
    ? (afterAgent || evidence.changes > 0 ? 'complete' : 'active')
    : (task.phase === 'agent_running' ? 'active' : 'pending'));
  const implementation = evidence.changes > 0
    ? (afterAgent ? 'complete' : 'active')
    : 'pending';
  const validation = evidence.validated || evidence.settled
    ? 'complete'
    : (['validating_progress', 'settling_turn'].includes(task.phase) ? 'active' : 'pending');
  let settlement = 'pending';
  if (task.state === 'completed') settlement = 'complete';
  else if (task.state === 'recovery_required' || task.state === 'failed') settlement = 'blocked';
  else if (task.phase === 'settling_turn' || task.state === 'waiting_for_user') settlement = 'active';
  return [
    ['stageWorkspace', workspace],
    ['stageAnalysis', analysis],
    ['stageImplementation', implementation],
    ['stageValidation', validation],
    ['stageSettlement', settlement],
  ];
}

function currentProgressHeading(task, evidence) {
  if (isResolvedUpstream(task)) return text('progressResolvedUpstream');
  if (task.state === 'completed') return text('progressCompleted');
  if (task.state === 'waiting_for_user') return text('progressWaiting');
  if (task.state === 'recovery_required' || task.state === 'failed') return text('progressRecovery');
  if (task.phase === 'preparing_workspace') return text('progressPreparing');
  if (task.state === 'queued' || task.phase === 'queued') return text('progressQueued');
  if (task.phase === 'validating_progress') return text('progressValidating');
  if (task.phase === 'settling_turn') return text('progressSettling');
  if (task.phase === 'agent_running') {
    return text(evidence.changes > 0 ? 'progressImplementing' : 'progressAnalyzing');
  }
  return text('progressIdle');
}

function currentProgressDetail(task, events) {
  if (isResolvedUpstream(task)) return text('progressResolvedUpstreamDetail');
  if (task.state === 'queued') return latestTaskWaitReason(task);
  if (task.currentTool) {
    const activity = [...events].reverse().find((event) => (
      event.toolName === task.currentTool
      && event.details
      && event.details.activity === 'started'
    ));
    const summary = activity && activity.details && activity.details.summary;
    return activitySummary(task.currentTool, summary);
  }
  if (task.state === 'recovery_required' || task.state === 'failed') return text('nextRecover');
  if (task.workspacePath && task.phase === 'creating_goal') {
    return `${text('evidenceWorkspace')} · ${compactArtifactPath(task.workspacePath)}`;
  }
  return taskPhaseLabel(task);
}

function activitySummary(toolName, rawSummary) {
  const summary = String(rawSummary || '');
  if (toolName !== 'ExecCommand') {
    const path = compactArtifactPath(summary);
    return path ? `${toolLabel(toolName)} · ${path}` : toolLabel(toolName);
  }
  if (/yarn\s+(?:workspace\s+\S+\s+)?install|pnpm\s+install|npm\s+(?:ci|install)/i.test(summary)) {
    return text('activityInstallingDependencies');
  }
  if (/dist:win|electron-builder|package-win|makensis|nsis/i.test(summary)) {
    return text('activityBuildingInstaller');
  }
  if (/smoke-windows-installer-upgrade|installer-upgrade|overwrite-marker|repro-overwrite/i.test(summary)) {
    return text('activityTestingUpgrade');
  }
  if (/Start-Sleep|Wait-Process|Get-Process|Get-CimInstance/i.test(summary)) {
    return text('activityWaitingProcess');
  }
  if (/loopx(?:\.exe)?[^\n]*(?:refresh-state|todo|heartbeat-prompt|quota)/i.test(summary)) {
    return text('activitySyncingProgress');
  }
  if (/\bgit\b/i.test(summary)) return text('activityCheckingRepository');
  return text('activityRunningCommand');
}

function nextProgressAction(task, evidence) {
  if (isResolvedUpstream(task)) return text('nextResolvedUpstream');
  if (task.state === 'completed') return text('nextCompleted');
  if (task.state === 'waiting_for_user') return text('nextApproval');
  if (task.state === 'recovery_required' || task.state === 'failed') return text('nextRecover');
  if (task.phase === 'preparing_workspace') return text('nextPrepare');
  if (task.state === 'queued' || task.phase === 'queued') return text('nextQueued');
  if (task.phase === 'validating_progress') return text('nextValidate');
  if (task.phase === 'settling_turn') return text('nextSettle');
  if (task.phase === 'agent_running') return text(evidence.changes > 0 ? 'nextImplement' : 'nextAnalyze');
  return text('nextAnalyze');
}

function renderIssueApproval(task) {
  const gate = task && task.state === 'waiting_for_user' ? latestGate(task.taskId) : null;
  view.issueApprovalPanel.hidden = !gate;
  if (!gate) return;
  const presentation = approvalPresentation(task, gate);
  const context = issueContext(task);
  view.issueApprovalTitle.textContent = presentation.title;
  view.issueApprovalSummary.textContent = presentation.summary;
  view.issueApprovalBackground.textContent = context.background;
  view.issueApprovalImpact.textContent = context.impact;
  view.issueApprovalApproveEffect.textContent = presentation.approveEffect;
  view.issueApprovalRejectEffect.textContent = presentation.rejectEffect;
  view.issueApprovalRecommendation.textContent = presentation.recommendation;
  const pending = Boolean(pendingActionFor(task));
  view.issueApprovalApprove.textContent = pending ? text('approvalSubmittingShort') : presentation.approveLabel;
  view.issueApprovalReject.textContent = presentation.rejectLabel;
  view.issueApprovalApprove.disabled = pending;
  view.issueApprovalReject.disabled = pending;
  view.issueApprovalNote.disabled = pending;
}

function issueOutcomeSummary(task, evidence) {
  if (isResolvedUpstream(task)) return text('outcomeResolvedUpstream');
  if (task.state === 'waiting_for_user') return issueContext(task).progress;
  if (task.state === 'recovery_required' || task.state === 'failed') return text('outcomeRecovery');
  if (task.state === 'completed') return text('outcomeCompleted');
  if (evidence.validated || evidence.settled) return text('outcomeValidated');
  if (evidence.changes > 0) return text('outcomeImplemented');
  if (evidence.analysisCount > 0 || String(task.lastAgentSummary || '').trim()) {
    return text('outcomeAnalyzed');
  }
  if (task.state === 'queued' || task.state === 'preparing') return text('outcomeQueued');
  return text('outcomeStarted');
}

function renderIssueProgress(task) {
  const evidence = taskProgressEvidence(task);
  const stages = progressStageStatus(task, evidence);
  const stageFragment = document.createDocumentFragment();
  stages.forEach(([labelKey, status]) => {
    const item = document.createElement('li');
    item.dataset.status = status;
    const label = document.createElement('strong');
    label.textContent = text(labelKey);
    const statusLabelElement = document.createElement('span');
    statusLabelElement.textContent = text(`stage${status[0].toUpperCase()}${status.slice(1)}`);
    item.append(label, statusLabelElement);
    stageFragment.append(item);
  });
  view.issueStageList.replaceChildren(stageFragment);
  view.issueProgressMeta.textContent = text('progressMeta', {
    done: stages.filter(([, status]) => status === 'complete').length,
    total: stages.length,
  });

  const taskEvents = progressTaskEvents(task);
  view.issueCurrentHeading.textContent = currentProgressHeading(task, evidence);
  view.issueCurrentDetail.textContent = currentProgressDetail(task, taskEvents);
  view.issueNextAction.textContent = nextProgressAction(task, evidence);

  const facts = isResolvedUpstream(task)
    ? [text('evidenceResolvedUpstream'), text('evidenceNoPatchRequired')]
    : [];
  if (!isResolvedUpstream(task)) {
    if (evidence.validated || evidence.settled) facts.push(text('evidenceProgressSaved'));
    if (task.workspacePath) facts.push(text('evidenceWorkspace'));
    if (evidence.analysisCount > 0) facts.push(text('evidenceAnalysisReady'));
    if (evidence.changes > 0) {
      facts.push(text(
        evidence.artifacts.length > 0 ? 'evidenceCodeChanges' : 'evidenceCodeChangesUnknown',
        { value: evidence.artifacts.length },
      ));
    }
    if (evidence.commands > 0) {
      facts.push(text(evidence.failures > 0 ? 'evidenceValidationNeedsAttention' : 'evidenceValidationPassed'));
    }
  }
  if (facts.length === 0) facts.push(text('evidenceNone'));
  const factFragment = document.createDocumentFragment();
  facts.slice(0, 5).forEach((fact) => {
    const item = document.createElement('li');
    item.textContent = fact;
    factFragment.append(item);
  });
  view.issueEvidenceList.replaceChildren(factFragment);

  const outcome = issueOutcomeSummary(task, evidence);
  view.issueOutcomePanel.hidden = false;
  view.issueOutcomeMeta.textContent = task.lastAgentSummaryAt
    ? text('outcomeUpdated', { duration: relativeLabel(task.lastAgentSummaryAt) })
    : '';
  view.issueOutcome.textContent = outcome;
}

function renderLiveness() {
  const task = selectedTask();
  view.selectedState.hidden = !task;
  view.showAllEvents.hidden = !task;
  if (!task) {
    view.logTitle.textContent = text('liveOutput');
    view.selectedSummary.hidden = false;
    view.selectedSummary.textContent = text('allTaskEvents');
    view.issueLink.hidden = true;
    view.issueLink.removeAttribute('href');
    view.issueNumber.textContent = '';
    view.issueApprovalPanel.hidden = true;
    view.issueProgressPanel.hidden = true;
    view.issueStageList.replaceChildren();
    view.issueEvidenceList.replaceChildren();
    view.issueOutcomePanel.hidden = true;
    view.issueOutcome.replaceChildren();
    view.issueDescriptionPanel.hidden = true;
    view.issueDescription.replaceChildren();
    renderTaskActions(null);
    return;
  }

  const visualState = taskVisualState(task);
  const item = task.identity && task.identity.item;
  const url = itemUrl(item);
  const itemLabelText = itemLabel(item);
  view.logTitle.textContent = issueDisplayTitle(task) || itemLabelText;
  view.selectedSummary.hidden = true;
  view.selectedState.hidden = false;
  view.selectedState.dataset.state = visualState;
  view.selectedState.textContent = taskStateDisplayLabel(task);
  view.issueLink.hidden = !url;
  view.issueLink.textContent = itemLabelText;
  if (url) {
    view.issueLink.href = url;
    view.issueLink.setAttribute('aria-label', `${text('openInGithub')}: ${itemLabelText}`);
  } else {
    view.issueLink.removeAttribute('href');
    view.issueLink.removeAttribute('aria-label');
  }
  view.issueNumber.textContent = item && item.number
    ? `${item.kind === 'pr' ? 'PR' : 'Issue'} #${item.number}`
    : '';
  renderIssueApproval(task);
  view.issueProgressPanel.hidden = false;
  renderIssueProgress(task);

  const description = identityDescriptionOf(task);
  const metadataKey = itemKey(item);
  const loadingDescription = state.metadataRequests.has(metadataKey);
  const descriptionUnavailable = (state.itemMetadata.get(metadataKey) || {}).unavailable === true;
  view.issueDescriptionPanel.hidden = false;
  renderMarkdown(
    view.issueDescription,
    description || (descriptionUnavailable
      ? text('issueDescriptionUnavailable')
      : text('loadingIssueDescription')),
    url,
  );
  renderTaskActions(task);
}

function eventSourceLabel(source) {
  const keys = {
    controller: 'sourceScheduler',
    sidecar: 'sourceLoopx',
    agent: 'sourceAgent',
    git: 'sourceGit',
    github: 'sourceGithub',
    system: 'sourceSystem',
  };
  return keys[source] ? text(keys[source]) : (source || text('sourceScheduler'));
}

function toolLabel(toolName) {
  const keys = {
    ExecCommand: 'toolExecCommand',
    Read: 'toolRead',
    Grep: 'toolGrep',
    LS: 'toolLs',
    WebFetch: 'toolWebFetch',
    WebSearch: 'toolWebSearch',
    Write: 'toolWrite',
    Edit: 'toolEdit',
  };
  return keys[toolName] ? text(keys[toolName]) : (toolName || text('outputTool'));
}

function toolStateLabel(stateValue) {
  const key = {
    queued: 'toolStateQueued',
    waiting: 'toolStateWaiting',
    started: 'toolStateStarted',
    confirmation: 'toolStateConfirmation',
    confirmed: 'toolStateConfirmed',
    rejected: 'toolStateRejected',
    completed: 'toolStateCompleted',
    failed: 'toolStateFailed',
    cancelled: 'toolStateCancelled',
  }[stateValue];
  return key ? text(key) : stateValue;
}

function eventMessage(event) {
  const activity = event.details && event.details.activity;
  const key = {
    queued: 'toolQueued',
    waiting: 'toolWaiting',
    started: 'toolStarted',
    confirmation: 'toolConfirmation',
    confirmed: 'toolConfirmed',
    rejected: 'toolRejected',
    completed: 'toolCompleted',
    failed: 'toolFailed',
    cancelled: 'toolCancelled',
  }[activity];
  if (key) {
    return text(key, { tool: toolLabel(event.toolName || event.details.toolName) });
  }
  return event.message || event.kind || 'event';
}

function eventDetailLabel(key) {
  const keys = {
    durationMs: 'detailDurationMs',
    exitCode: 'detailExitCode',
    matchCount: 'detailMatchCount',
    fileCount: 'detailFileCount',
    entryCount: 'detailEntryCount',
    lineCount: 'detailLineCount',
    contentLength: 'detailContentLength',
    title: 'detailTitle',
    queuePosition: 'detailQueuePosition',
    dependencies: 'detailDependencies',
  };
  return keys[key] ? text(keys[key]) : key;
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
  source.textContent = eventSourceLabel(event.source);

  const content = document.createElement('div');
  content.className = 'log-content';
  const message = document.createElement('div');
  message.className = 'log-message';
  message.textContent = eventMessage(event);
  content.append(message);

  const toolSummary = event.details && event.details.summary;
  if (toolSummary) {
    const summary = document.createElement('div');
    summary.className = 'log-tool-summary';
    summary.textContent = String(toolSummary);
    content.append(summary);
  }

  const metaValues = [];
  if (!state.selectedTaskId && event.taskId) {
    const task = taskForId(event.taskId);
    const item = task && task.identity && task.identity.item;
    metaValues.push(item ? compactItemLabel(item) : event.taskId);
  }
  if (event.phase) metaValues.push(phaseLabel(event.phase));
  if (event.toolName) metaValues.push(`${text('currentTool')}: ${toolLabel(event.toolName)}`);
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

  const detailEntries = Object.entries(event.details || {})
    .filter(([key]) => !['activity', 'toolName', 'summary'].includes(key));
  if (detailEntries.length) {
    const details = document.createElement('details');
    details.className = 'log-details';
    const summary = document.createElement('summary');
    summary.textContent = text('details');
    const list = document.createElement('dl');
    detailEntries.forEach(([key, value]) => {
      const term = document.createElement('dt');
      term.textContent = eventDetailLabel(key);
      const description = document.createElement('dd');
      description.textContent = String(value);
      list.append(term, description);
    });
    details.append(summary, list);
    content.append(details);
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
  source.textContent = eventSourceLabel('controller');

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
  const combined = existing ? `${existing}${value}` : value;
  if (combined.length <= MAX_OUTPUT_BLOCK_CHARS) return combined;
  return `${combined.slice(0, MAX_OUTPUT_BLOCK_CHARS / 2)}\n...\n${combined.slice(-MAX_OUTPUT_BLOCK_CHARS / 2)}`;
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
      && last.taskId === event.taskId
      && last.turnId === event.turnId
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
      taskId: event.taskId || '',
      turnId: event.turnId || '',
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

function cursorRangeLabel(block) {
  return block.startCursor === block.endCursor
    ? `#${block.startCursor}`
    : `#${block.startCursor}-${block.endCursor}`;
}

function outputBlockDomKey(block) {
  return `${block.taskId}:${block.turnId}:${block.kind}:${block.startCursor}`;
}

function outputBlockDomVersion(block) {
  return `${block.endCursor}:${block.eventCount}:${block.toolState}:${String(block.text || '').length}`;
}

function turnOutputBlockRow(block) {
  const row = document.createElement('li');
  row.className = 'log-row turn-output-row';
  row.dataset.kind = block.kind;
  row.dataset.level = block.toolState === 'failed' ? 'error' : 'info';
  row.dataset.cursor = String(block.endCursor);
  row.dataset.taskId = block.taskId;
  row.dataset.blockKey = outputBlockDomKey(block);
  row.dataset.blockVersion = outputBlockDomVersion(block);

  const header = document.createElement('div');
  header.className = 'output-block__header';

  const task = taskForId(block.taskId);
  const item = task && task.identity && task.identity.item;
  const issue = document.createElement(itemUrl(item) ? 'a' : 'span');
  issue.className = 'output-block__issue';
  issue.textContent = item ? compactItemLabel(item) : text('taskNumber', { value: shortId(block.taskId) });
  if (issue.tagName === 'A') {
    issue.href = itemUrl(item);
    issue.target = '_blank';
    issue.rel = 'noopener noreferrer';
    issue.title = issueDisplayTitle(task) || itemLabel(item);
  }

  const level = document.createElement('span');
  level.className = 'event-level';
  level.dataset.level = block.toolState === 'failed' ? 'error' : 'info';
  level.textContent = outputKindLabel(block.kind);

  const source = document.createElement('span');
  source.className = 'output-block__source';
  source.textContent = block.toolName ? toolLabel(block.toolName) : eventSourceLabel('agent');

  const cursor = document.createElement('span');
  cursor.className = 'output-block__cursor';
  cursor.textContent = cursorRangeLabel(block);

  header.append(issue, level, source, cursor);

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
    status.textContent = toolStateLabel(block.toolState);
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
  const blocks = compactTurnOutputBlocks(state.outputHistory);
  const visible = blocks.slice(-MAX_RENDERED_OUTPUT_BLOCKS);
  const existing = new Map(
    [...view.logList.children]
      .filter((node) => node.dataset && node.dataset.blockKey)
      .map((node) => [node.dataset.blockKey, node]),
  );
  const desired = visible.map((block) => {
    const key = outputBlockDomKey(block);
    const node = existing.get(key);
    return node && node.dataset.blockVersion === outputBlockDomVersion(block)
      ? node
      : turnOutputBlockRow(block);
  });
  desired.forEach((node, index) => {
    const current = view.logList.children[index];
    if (current !== node) view.logList.insertBefore(node, current || null);
  });
  const desiredNodes = new Set(desired);
  [...view.logList.children].forEach((node) => {
    if (!desiredNodes.has(node)) node.remove();
  });
  const hasOutput = visible.length !== 0;
  view.logEmpty.hidden = hasOutput;
  if (!hasOutput) {
    const message = state.turnOutput.message || text('noLiveOutput');
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
  renderTurnOutput();
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
  const changed = state.selectedTaskId !== (taskId || null);
  state.selectedTaskId = taskId || null;
  if (changed) view.issueApprovalNote.value = '';
  renderTasks();
  renderLiveness();
  renderLogs();
  if (taskId) {
    if (!view.issueDetailDialog.open) view.issueDetailDialog.showModal();
    void hydrateTaskMetadata(taskId);
  } else if (view.issueDetailDialog.open) {
    view.issueDetailDialog.close();
  }
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
    const availableModels = Array.isArray(models) ? models : [];
    let renderedModelCount = 0;
    for (const model of availableModels) {
      if (!model || !model.id) continue;
      const option = document.createElement('option');
      option.value = model.id;
      const tag = model.isDefault === true ? ` · ${text('modelPrimaryTag')}` : '';
      option.textContent = `${describeModelOption(model)}${tag}`;
      option.selected = current === model.id;
      selectedExists = selectedExists || option.selected;
      select.appendChild(option);
      renderedModelCount += 1;
    }
    if (select.options.length === 1) {
      const empty = document.createElement('option');
      empty.value = '';
      empty.textContent = text('modelEmpty');
      empty.disabled = true;
      empty.dataset.status = 'empty';
      select.appendChild(empty);
    }
    if (!selectedExists) select.value = 'auto';
    state.modelCatalogLoaded = renderedModelCount > 0;
    select.title = text('modelReloadTitle');
  } catch (error) {
    const current = currentModelSelection();
    [...select.options].forEach((option) => {
      if (option.dataset.status) option.remove();
    });
    if (select.options.length === 0) {
      const auto = document.createElement('option');
      auto.value = 'auto';
      auto.textContent = text('modelAuto');
      select.appendChild(auto);
    } else {
      const auto = [...select.options].find((option) => option.value === 'auto');
      if (auto) auto.textContent = text('modelAuto');
    }
    if (![...select.options].some((option) => option.dataset.status === 'load-failed')) {
      const failed = document.createElement('option');
      failed.value = '';
      failed.textContent = text('modelLoadFailed');
      failed.disabled = true;
      failed.dataset.status = 'load-failed';
      select.appendChild(failed);
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
    await rememberIntake(input);
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
    const focusedTaskId = outcomes
      .find((outcome) => outcome.taskId && ['created', 'opened_existing'].includes(outcome.kind))
      ?.taskId;
    state.followLogs = true;
    selectTask(focusedTaskId || null);
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
  view.resetLoopxDialog.close();
  view.root.setAttribute('aria-busy', 'true');
  showNotice(text('resettingLoopxBackground'));
  renderExecutionSupport();
  try {
    const clientRequestId = requestId();
    await attachSnapshot(false);
    view.root.setAttribute('aria-busy', 'true');
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
      clearRunUiState();
      showNotice(text('resetLoopxApplied'), 'success');
    }
    await attachSnapshot(false);
  } catch (error) {
    showNotice(errorMessage(error), 'error');
    await attachSnapshot(false);
  } finally {
    state.resetPending = false;
    setButtonBusy(view.resetLoopx, false);
    view.resetLoopxConfirm.disabled = false;
    view.root.setAttribute('aria-busy', 'false');
    renderExecutionSupport();
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

async function answerTaskGate(task, action, note = '') {
  const gate = task && latestGate(task.taskId);
  if (!task || !gate) {
    showNotice(text('noGate'), 'error');
    return;
  }
  showNotice(text('approvalSubmitting'));
  try {
    const applied = await performAction(action, task, { gateId: gate.gateId, note: note.trim() });
    if (applied && state.selectedTaskId === task.taskId) view.issueApprovalNote.value = '';
  } finally {
    syncApprovalAttention(false);
  }
}

function approvalAlertGate() {
  const task = state.approvalTaskId ? taskForId(state.approvalTaskId) : null;
  const gate = task ? latestGate(task.taskId) : null;
  return task && gate ? { task, gate } : null;
}

function openApprovalAlertGate() {
  const attention = approvalAlertGate();
  if (attention) selectTask(attention.task.taskId);
}

function answerApprovalAlertGate(action) {
  const attention = approvalAlertGate();
  if (!attention) return;
  void answerTaskGate(attention.task, action);
}

function answerSelectedTaskGate(action) {
  const task = selectedTask();
  if (!task) return;
  void answerTaskGate(task, action, view.issueApprovalNote.value);
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

function setRailCollapsed(collapsed) {
  state.railCollapsed = Boolean(collapsed);
  view.taskRail.classList.toggle('is-collapsed', state.railCollapsed);
  view.taskRail.parentElement.classList.toggle('tasks-collapsed', state.railCollapsed);
  view.collapseTasks.setAttribute('aria-expanded', String(!state.railCollapsed));
  view.collapseTasks.setAttribute(
    'title',
    state.railCollapsed ? text('expandTasks') : text('collapseTasks'),
  );
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
    splitter.setAttribute('aria-valuenow', String(width()));
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
    if (state.railCollapsed) {
      setRailCollapsed(false);
      setRailWidth(Math.max(RAIL_MIN_WIDTH, width()));
    }
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
    if (state.railCollapsed) setRailCollapsed(false);
    setRailWidth(RAIL_DEFAULT_WIDTH);
    splitter.setAttribute('aria-valuenow', String(width()));
    persistRailWidth();
  });
  splitter.addEventListener('keydown', (event) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    if (state.railCollapsed) setRailCollapsed(false);
    const step = event.shiftKey ? 48 : 12;
    const next = event.key === 'ArrowRight' ? width() + step : width() - step;
    setRailWidth(Math.min(RAIL_MAX_WIDTH, Math.max(RAIL_MIN_WIDTH, next)));
    splitter.setAttribute('aria-valuenow', String(width()));
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
  view.retryEnvironment.addEventListener('click', async () => {
    await performAction('retry_environment', null);
  });
  view.resumeRepository.addEventListener('click', openRepositoryResumeDialog);
  view.approvalAlertOpen.addEventListener('click', openApprovalAlertGate);
  view.approvalAlertApprove.addEventListener('click', () => answerApprovalAlertGate('approve'));
  view.approvalAlertReject.addEventListener('click', () => answerApprovalAlertGate('reject'));
  view.issueApprovalApprove.addEventListener('click', () => answerSelectedTaskGate('approve'));
  view.issueApprovalReject.addEventListener('click', () => answerSelectedTaskGate('reject'));
  view.repositoryResumeCancel.addEventListener('click', () => {
    view.repositoryResumeDialog.close();
  });
  view.repositoryResumeConfirm.addEventListener('click', (event) => {
    event.preventDefault();
    void resumeRepository();
  });
  view.collapseTasks.addEventListener('click', () => {
    setRailCollapsed(!state.railCollapsed);
  });
  view.showAllEvents.addEventListener('click', () => selectTask(null));
  view.issueDetailDialog.addEventListener('cancel', (event) => {
    event.preventDefault();
    selectTask(null);
  });
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
}

function updateLivenessClock() {
  renderLiveness();
  renderTasks();
  const resumed = sampleHostClock();
  if (resumed) {
    void attachSnapshot(false, true);
    return;
  }
  const tasks = state.snapshot && Array.isArray(state.snapshot.tasks) ? state.snapshot.tasks : [];
  const hasActiveWork = tasks.some((task) => [
    'preparing',
    'running',
    'cancelling',
    'retry_wait',
  ].includes(task.state));
  const now = Date.now();
  if (
    hasActiveWork
    && document.visibilityState === 'visible'
    && now - state.lastHostSignalAt >= STALE_ACTIVE_REATTACH_MS
    && now - state.lastReattachAt >= STALE_ACTIVE_REATTACH_MS
  ) {
    void attachSnapshot(false);
  }
}

function sampleHostClock() {
  const now = Date.now();
  const elapsed = now - state.lastClockSampleAt;
  state.lastClockSampleAt = now;
  return elapsed >= HOST_RESUME_GAP_MS;
}

function handleHostSurfaceReturn() {
  if (document.visibilityState !== 'visible') return;
  void attachSnapshot(false, sampleHostClock());
}

async function start() {
  bindEvents();
  applyLocale();
  void loadIntakeHistory();
  void loadModelCatalog();
  if (!app || !app.loopx) {
    showBridgeUnavailable();
    return;
  }
  app.loopx.onEvent(onLoopxEvent);
  if (typeof app.onLocaleChange === 'function') app.onLocaleChange(applyLocale);
  if (typeof app.onActivate === 'function') app.onActivate(handleHostSurfaceReturn);
  document.addEventListener('visibilitychange', handleHostSurfaceReturn);
  window.addEventListener('focus', handleHostSurfaceReturn);
  window.addEventListener('pageshow', handleHostSurfaceReturn);
  window.addEventListener('online', handleHostSurfaceReturn);
  window.addEventListener('beforeunload', () => {
    clearTurnOutputTimer();
    document.removeEventListener('visibilitychange', handleHostSurfaceReturn);
    window.removeEventListener('focus', handleHostSurfaceReturn);
    window.removeEventListener('pageshow', handleHostSurfaceReturn);
    window.removeEventListener('online', handleHostSurfaceReturn);
    if (app.loopx && typeof app.loopx.offEvent === 'function') {
      app.loopx.offEvent(onLoopxEvent);
    }
  });
  window.setInterval(updateLivenessClock, HOST_CLOCK_TICK_MS);
  await attachSnapshot(true);
}

void start();
