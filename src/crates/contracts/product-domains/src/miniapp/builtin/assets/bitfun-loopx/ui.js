// bitfun-loopx — UI + host heartbeat.
// The heartbeat is a single setTimeout chain armed to the earliest per-goal
// due time; each poll's interval is dictated by loopx's scheduler_hint
// (recommended interval, unchanged-poll backoff, max clamp, reset_token).
// Rendering is fingerprint-throttled: unchanged decisions repaint nothing but
// the 1s countdown, and re-renders are deferred while a card input has focus.

const app = window.app;

// Boot timeline: timestamps every startup step and the first renders so the
// "extra flash" on import can be attributed to a step (or to the host).
const BOOT_T0 = performance.now();
let BOOT_RENDER_COUNT = 0;
const bootMs = () => Math.round(performance.now() - BOOT_T0);
const themeProbe = () => String(getComputedStyle(document.documentElement).getPropertyValue('--bitfun-bg')).trim() || '(none)';

// Theme gate release (double failsafe for the inline script in index.html —
// some compilers relocate inline scripts, so the main bundle guarantees the
// page can never stay hidden): reveal when the host appearance vars are in,
// when the appearanceChange event arrives, or on a deadline.
const releaseThemeGate = () => { document.documentElement.style.visibility = 'visible'; };
if (themeProbe() !== '(none)') releaseThemeGate();
app.onAppearanceChange((payload) => {
  dbgUi('theme:applied', `t=${bootMs()}ms mode=${payload && payload.mode}`);
  releaseThemeGate();
});
setTimeout(releaseThemeGate, 800);

const I18N = {
  'zh-CN': {
    title: 'bitfun-loopx',
    refresh: '刷新目标列表',
    retry: '重试',
    notFoundTitle: '未检测到 loopx CLI',
    notFoundHint: '本机未检测到 loopx。可一键拉取 loopx 源码直接运行（无需 pip 安装），或自行 pip 安装。',
    vendorLoopxBtn: '拉取 loopx 源码',
    vendoringLoopx: '正在拉取…（首次需联网，约 1 分钟）',
    vendorDone: '拉取完成',
    vendorFailed: '拉取失败',
    prereqNeedPython: '缺少 Python 3.11+（loopx 源码运行需要）。安装后点「重试」：python.org/downloads，或 brew install python（macOS）/ winget install Python.Python.3.12（Windows）',
    prereqNeedGit: '缺少 git（拉取源码需要）。安装后点「重试」：git-scm.com 或 winget install Git.Git',
    prereqUnknown: '无法探测运行环境（需要 Python 3.11+ 与 git）。也可以在自己的终端执行 pip install git+https://github.com/huangruiteng/loopx.git',
    issuesProgress: (done, total) => `issues ${done}/${total}`,
    issueDone: '已修复',
    issueOpen: '进行中',
    issueBlocked: '受阻',
    issueDeferred: '已搁置',
    issuePending: '待处理',
    moreIssues: (n) => `+${n}`,
    installLoopxBtn: 'pip 安装 loopx',
    installingLoopx: '正在安装…（可能需要几分钟）',
    installDone: '安装完成',
    installFailed: '安装失败',
    runOnce: '执行一次',
    resumeTask: '继续任务',
    resumeTaskHint: '继续该任务：恢复心跳监控与自动执行',
    stopTask: '中止任务',
    stopTaskHint: '中止该任务：取消本次运行、关闭心跳监控与自动执行',
    deleteTask: '删除任务',
    deleteTaskHint: '删除该任务：归档运行记录并移除注册表条目（注册表会先备份）',
    groupArchived: '已归档',
    statusArchived: '已归档',
    archivedHint: '该任务的运行记录已被移入归档区（loopx 注册表中已无此任务）。恢复后回到「自动已关」的暂停态，点「继续」即可接着跑。',
    restoreTask: '恢复任务',
    restoreTaskHint: '把该任务从归档区恢复到看板（重建注册表条目 + 还原运行目录）',
    restoreDone: (name) => `已恢复：${name}`,
    restoreFailed: '恢复失败',
    taskStopped: (id) => `任务 ${id} 已中止：心跳与自动执行已关闭`,
    taskResumed: (id) => `任务 ${id} 已继续：心跳与自动执行已重新开启`,
    taskDeleted: (id) => `任务 ${id} 已删除`,
    deleteTaskFailed: '删除失败',
    stopConfirmTitle: '中止这个任务？',
    stopConfirmText: (id) => `将取消「${id}」正在进行的运行，并关闭它的心跳监控与自动执行。任务会移入「已停表」，随时可以继续。`,
    confirmStop: '确认中止',
    deleteConfirmTitle: '删除这个任务？',
    deleteConfirmText: (id) => `将归档「${id}」的运行记录并从注册表移除（注册表文件会先备份）。看板将不再显示该任务。`,
    confirmDelete: '确认删除',
    activityEmpty: '暂无日志',
    thinkBlockTitle: '思考过程',
    elapsedLabel: (t) => `已用时 ${t}`,
    waitingOn: (w) => `等待：${w}`,
    waitingAgent: '等待 Agent 执行',
    cancel: '取消',
    needProject: '执行 run-once 需要先选择项目目录',
    needAgent: '该目标没有已注册的 agent，请先填写 agent id',
    detected: (v) => `已检测到 loopx：${v}`,
    copy: '复制',
    close: '关闭',
    raw: 'JSON',
    groupPaused: '已停表',
    groupError: '异常',
    colEmpty: '暂无',
    loadingGoals: '正在读取任务…',
    statusRunning: '执行中',
    statusPaused: '已停表',
    statusErroring: (n) => `失败 ×${n}`,
    statusUnmonitored: '监控已关',
    statusManual: '自动已关',
    statusAuto: '自动运行',
    stageGated: '阶段：等待你的确认',
    stageRunning: '阶段：执行中',
    stageFixing: (n) => `阶段：修复 #${n}`,
    stagePlanning: '阶段：规划中',
    skippedDuplicates: (n) => `（已跳过 ${n} 个重复 Issue）`,
    runCancelled: '运行已取消',
    turnStalled: (m) => `运行僵死：约 ${m} 分钟没有收到 Agent 事件（可能宿主已静默取消本轮），已取消该回合并允许自动重试。`,
    streamTrimmed: (n) => `（前文已折叠 ${n} 字符）`,
    groupBacklog: '待处理',
    groupActive: '进行中',
    groupReview: '等你处理',
    colSubReview: '阻塞 · 需要你批准后继续',
    colSubActive: 'Agent 正在执行',
    detailEmptyHint: '点选「进行中」的条目查看任务详情与实时日志',
    groupDone: '已完成',
    taskPlaceholder: '粘贴 GitHub Issue / 仓库 / Issues 列表链接，可附加修复要求',
    taskGuidePlaceholder: (name) => `正在向「${name}」插话：输入指令引导 Agent 继续（不创建新任务）`,
    taskGoalUnsupported: '请粘贴 GitHub Issue、PR、仓库首页或 Issues 列表链接（自由目标暂未开放）',
    taskUnsupportedPath: (u) => `不支持的 GitHub 链接：${u}。请粘贴 Issue、PR、仓库首页或 Issues 列表链接`,
    guidanceNoRunning: '没有正在运行的任务。粘贴 Issue 链接创建新任务，或等任务开始运行后再输入指令。',
    guidancePickOne: '有多个任务正在运行：请先在「进行中」点选目标任务，再发送指令。',
    guidanceSending: '正在发送指令…',
    guidanceSent: (id) => `指令已发送给任务 ${id}，Agent 将在下一步读取`,
    guidanceLine: (t) => `你：${t}`,
    taskCreate: '创建任务',
    taskCreating: '正在创建任务…',
    taskResolving: '正在识别任务类型…',
    taskPendingLabel: '正在创建',
    taskStageStarting: '任务已创建，正在启动 Agent',
    taskStarted: (id) => `任务 ${id} 已创建并开始执行`,
    stageExpand: '正在获取 issue 列表…',
    stageBootstrap: '正在创建 bitfun-loopx 任务…',
    stageRegister: '正在注册 Agent…',
    stagePlan: '正在解析 Issue 修复计划…',
    stageTodos: (c, t) => `正在写入 todos ${c}/${t}…`,
    stageRefresh: '正在刷新状态…',
    intakeTitleIssue: '确认修复这个 Issue',
    intakeTitleIssues: (n) => `确认修复 ${n} 个 Issues`,
    intakeTitleList: '选择要修复的 Issues',
    intakeTitleGoal: '确认新任务',
    intakeSummaryList: (repo, n) => `${repo} 现有 ${n} 个 open issues，默认全选。任务会逐个修复选中的 issue。`,
    intakeSummaryIssues: (repo) => `以下 issues 将写入同一个任务（${repo}），由 Agent 逐个修复。`,
    intakeSummaryGoal: '当前已有任务在进行中——选择新建，或把这段话作为引导写入现有任务。',
    intakeSelectAll: '全选',
    intakeSelectedCount: (n, m) => `已选 ${n}/${m}`,
    intakeModeNew: '新建任务',
    intakeConfirmNew: '创建任务',
    intakeConfirmIssues: (n) => `开始修复 ${n} 个 Issues`,
    intakeConfirmGuide: '写入现有任务',
    guideTargetNote: (id) => `将把所选 Issues 作为子任务并入现有任务：${id}`,
    composerTargetTitle: '目标：新建任务（完全独立）；选择已有任务时，输入的内容会作为人类反馈发送给该任务',
    deleteShort: '删除',
    deleteGoalNamed: (name) => `删除：${name}`,
    resizeHandleHint: '拖拽调整列宽',
    logBottomHint: '回到底部',
    intakeNoneSelected: '至少选择一个 Issue',
    intakeNoIssues: '该仓库没有 open issues',
    guideStarted: (id) => `已把所选 Issues 作为子任务并入任务 ${id}`,
    gateCount: (n) => `等你处理 ${n} 项`,
    gateEmptyHint: '该门禁暂无可直接批准的事项',
    gateItemTitle: '待确认事项',
    gateItemWithType: (hint) => `待确认事项（${hint}）`,
    gateGroupBlocking: '需要确认 · 阻塞任务',
    gateGroupInfo: '仅知会 · 不阻塞',
    gateTypePublish: '发布 / 提交 PR',
    gateTypeApprove: '审批',
    gateTypeInfo: '知会事项',
    gateCardTask: (name) => `任务：${name}`,
    gateItemInfoLabel: (hint) => `知会事项 · ${hint}`,
    gateExplainWrite: '需要你批准：授予写权限后，Agent 才能实施修改并提交 PR。',
    gateExplainDecide: '需要你决定：同意或拒绝这项改动。',
    gateExplainPublish: '需要你批准：发布 / 提交 PR。',
    gateExplainMerge: '需要你批准：合并操作。',
    gateExplainReview: '需要你批准：外部评审请求。',
    gateExplainPreload: '该改动为桌面插件增加一个最小 Electron preload 桥。',
    gateBackground: '背景：',
    gateSummaryLoading: '正在生成中文摘要…',
    copyCardHint: '复制这张卡片的内容',
    autoApprovedWrite: '已按入库授权自动批准：写权限（离开只读适配器）',
    conclusionMerged: '结论：已修复并合并',
    conclusionCompleted: '结论：修复已完成',
    conclusionNoFollowup: '结论：无需修复（无后续动作）',
    conclusionCancelled: '结论：已取消',
    conclusionClosed: '结论：已关闭 / 重复',
    conclusionFinished: '结论：任务已结束',
    approveGate: '批准',
    completeTodoBtn: '标记完成',
    approveGateTitle: '批准这项操作？',
    todoDoneTitle: '标记为已完成？',
    approveTitle: '确认这项操作？',
    approveNote: '备注（可选，写入 todo 完成记录）',
    approveConfirm: '批准并继续',
    todoDoneConfirm: '标记完成',
    approveGateHint: '这是需要你批准的事项：批准后，任务将按该事项继续执行。',
    todoDoneHint: '这是知会/指示类事项：标记完成即可，不需要批准，也不会触发新操作。',
    approveDone: '已批准，任务将继续推进',
    approveResumed: '批准后任务已恢复自动执行',
    todoDoneFeedback: '已标记完成',
    githubTokenTitle: 'GitHub Token 设置',
    githubTokenExplain: '用于 fork 仓库、推送分支、创建 PR。需要一个 fine-grained Personal Access Token（Repository 读写权限）。Token 仅保存在本机 BitFun 应用存储中，不会上传。如果本机已用 GitHub CLI 登录（gh auth login），发布时会自动复用，无需粘贴 Token。',
    githubTokenPlaceholder: 'ghp_ 或 github_pat_ …',
    githubTokenSave: '保存并验证',
    ghLoginGuideTitle: '方式一（推荐）：用 GitHub CLI 登录',
    ghLoginGuide: '点击下方按钮自动安装 GitHub CLI 并弹出浏览器完成登录，无需手动创建 Token（网络受限时会自动使用系统代理）。',
    ghLoginBtn: '用 GitHub CLI 登录',
    tokenGuideTitle: '方式二：使用已有的 Token',
    tokenGuide: '前往 GitHub 创建 Fine-grained Token（需要 Contents 与 Pull requests 的读写权限）：',
    tokenGuideLink: 'github.com/settings/personal-access-tokens/new',
    ghLoginDone: (login) => `登录完成：${login}`,
    ghLoginFailed: '登录失败',
    githubTokenStatus: '当前状态：',
    githubTokenSaved: (user) => `Token 有效，已保存（登录名：${user}）`,
    githubTokenInvalid: 'Token 无效或已过期',
    githubTokenMissing: '未配置',
    githubTokenSet: '已配置',
    approveAndPr: '批准并提交 PR',
    approveOnly: '仅批准，不提交 PR',
    approveOnlyNote: '用户选择仅批准，不提交 PR',
    gateCredGh: '✓ 本机 GitHub 已登录（gh），可直接提交 PR',
    gateCredToken: (login) => `✓ 已配置 GitHub Token（${login}）`,
    gateCredNone: '⚠ 尚未登录 GitHub：提交 PR 前请先完成登录',
    gateCredSetup: '配置 GitHub 登录',
    gateAfterPublish: '批准后：自动 fork 到你的 GitHub → 推送分支 → 创建 PR（带 [bitfun-loopx] 标记），随后继续剩余 issue',
    gateAfterApprove: '批准后：任务继续自动执行',
    sectionTarget: '修复目标',
    sectionDecision: '需要你决定',
    sectionProgress: '当前进度',
    sectionResult: '结果',
    approvePrTitle: '批准并提交 PR？',
    approvePrHint: '批准后控制台将自动：检查/创建你的 fork → 推送修复分支 → 向原仓库创建 PR（标题带 [bitfun-loopx] 标识，可被 GitHub 搜索统计）。',
    approvePrNeedToken: '⚠ 尚未配置 GitHub Token，点击「批准」后将先打开 Token 设置。',
    publishWorking: '正在发布 PR（首次需要 fork 仓库，可能一两分钟）…',
    publishAnalyzing: '正在分析问题原因与解决方案…',
    publishDone: (url) => `✅ PR 已提交：${url}`,
    publishFailed: 'PR 提交失败',
    publishNeedToken: '发布 PR 需要先配置 GitHub Token',
    resetLoopxTitle: '清除所有 bitfun-loopx 状态？',
    resetLoopxText: '将备份并清除本机全部 loopx 相关状态：所有任务（goal）、todo 与运行历史、全局/项目注册表、控制台的仓库克隆缓存和持久化日志。数据会整体移入控制台数据目录下的 cleared-<时间戳> 备份目录，可手动找回。此操作不可撤销。',
    resetLoopxConfirm: '全部清除',
    resetLoopxWorking: '正在清除…',
    resetLoopxDone: (dir) => `已清除全部 bitfun-loopx 状态（备份保留在 ${dir}）`,
    resetLoopxFailed: '清除失败',
    approveFailed: (e) => `批准失败：${e}`,
    notifGateTitle: 'bitfun-loopx 需要你审批',
    notifGateBody: (id, block, info) => (info > 0
      ? `${id}：${block} 项待确认、${info} 项仅知会`
      : `${id} 有 ${block} 项待确认`),
    autoRunNext: '自动执行下一轮',
    autoRunDisabled: (id) => `${id} 连续失败，已暂停自动执行`,
    activityStarting: '正在启动 Agent…',
    activitySentPrompt: (n) => `▶ 已向 Agent 发送指令（${n} 字符，点击展开）`,
    activityRunning: (elapsed) => `Agent 正在执行 · 已用时 ${elapsed}`,
    activityCommitted: 'bitfun-loopx 已提交本次执行结果',
    activityValidationPassed: '独立校验已通过',
    activityValidationFailed: '独立校验未通过',
    activityStateUpdated: '目标状态已更新',
    activityCompleted: '执行已完成',
    activityCompletedValidated: '执行已完成 · 校验通过',
    activityFailed: '执行失败',
    taskGoal: '普通目标',
    taskRepository: 'GitHub 仓库',
    taskIssue: 'GitHub Issue',
    taskIssues: (n) => `${n} 个 Issue`,
    taskIssuesList: '整仓 Issues',
    taskNeedProject: '请先选择这个任务对应的本地项目目录。',
    taskRepoNotFound: (repo) => `GitHub 上找不到仓库：${repo}。请检查链接拼写。`,
    taskRepoLookupFailed: '无法访问 GitHub 校验仓库，请稍后重试。',
    stageClone: '正在克隆仓库…',
    stageClonePercent: (p) => `正在克隆仓库… ${p}%`,
    intakeCloneNote: (repo) => `将自动克隆 ${repo} 到小应用数据目录并开始修复（无需本地 checkout）。`,
    issueHasImages: '该 issue 描述包含图片（截图），文字可能不足以定位问题',
    intakeVisionWarn: (n) => `⚠ 检测到 ${n} 个 issue 的描述包含图片，而当前模型不具备多模态能力：图片里的关键信息可能无法被理解，仅凭文字不一定能确认问题根源。建议先补充文字说明（错误信息、复现步骤等）再创建任务，或改用支持视觉的模型。仍可继续，但修复质量可能受影响。`,
    intakeReuseNote: (repo) => `已找到 ${repo} 的本地 checkout，无需重新克隆。`,
    intakeWriteNote: '本确认即授权：任务将获得仓库写权限并自动连续执行；仅在需要提 PR/发布时才会再次询问你。',
    taskCloneOtherRepo: (expected, actual) => `本地目录绑定的是 ${actual}；将把 ${expected} 克隆到独立目录处理。`,
    composerModelTitle: '新任务执行模型',
    otherTasksTitle: '本机其它 loopx 任务',
    otherTasksHint: '非本控制台创建，默认不监控。接管后进入看板并开始心跳轮询。',
    adopt: '接管',
    adoptedLabel: '已接管',
    adoptFailed: (e) => `接管失败：${e}`,
    modelAuto: '自动（跟随 BitFun 策略）',
    modelPrimaryTag: '主模型',
    modelFollowGlobal: '跟随全局默认',
    modelChanged: (m) => `执行模型已切换为 ${m}`,
    taskNeedAgent: '请先在设置中配置新任务默认 Agent。',
    taskCreated: (id) => `任务 ${id} 已创建`,
    taskRepoMismatch: (expected, actual) => `链接指向 ${expected}，当前项目是 ${actual}。请切换到正确的本地 checkout。`,
    taskMultipleRepos: '一个任务只能绑定一个本地仓库，请把不同仓库的链接拆成多个任务。',
    taskRepoUnverified: (repo) => `选择的目录不是 ${repo} 的本地 checkout（未找到 GitHub remote）。请先选择正确的仓库目录。`,
    taskPartial: (id, n, e) => `任务 ${id} 已创建，但只写入了 ${n} 个 todos：${e}`,
    intakeTruncated: (n) => `（仅显示前 ${n} 个，列表未取全）`,
  },
  'en-US': {
    title: 'bitfun-loopx',
    refresh: 'Refresh goals',
    retry: 'Retry',
    notFoundTitle: 'loopx CLI not found',
    notFoundHint: 'loopx was not detected on this machine. Fetch its source and run it directly (no pip install), or install it yourself with pip.',
    vendorLoopxBtn: 'Fetch loopx source',
    vendoringLoopx: 'Fetching… (first time needs network, ~1 min)',
    vendorDone: 'Fetch complete',
    vendorFailed: 'Fetch failed',
    prereqNeedPython: 'Python 3.11+ is missing (required to run loopx from source). Install it, then press Retry: python.org/downloads, brew install python (macOS) or winget install Python.Python.3.12 (Windows)',
    prereqNeedGit: 'git is missing (required to fetch the source). Install it, then press Retry: git-scm.com or winget install Git.Git',
    prereqUnknown: 'Could not probe the environment (needs Python 3.11+ and git). You can also run pip install git+https://github.com/huangruiteng/loopx.git in your own terminal.',
    issuesProgress: (done, total) => `issues ${done}/${total}`,
    issueDone: 'fixed',
    issueOpen: 'open',
    issueBlocked: 'blocked',
    issueDeferred: 'deferred',
    issuePending: 'pending',
    moreIssues: (n) => `+${n}`,
    installLoopxBtn: 'pip install loopx',
    installingLoopx: 'Installing… (may take a few minutes)',
    installDone: 'Installation complete',
    installFailed: 'Installation failed',
    runOnce: 'Run once',
    resumeTask: 'Resume task',
    resumeTaskHint: 'Resume this task: restore heartbeat and auto-run',
    stopTask: 'Abort task',
    stopTaskHint: 'Abort this task: cancel the current run, disable heartbeat and auto-run',
    deleteTask: 'Delete task',
    deleteTaskHint: 'Delete this task: archive its runtime and remove the registry entry (the registry is backed up first)',
    groupArchived: 'Archived',
    statusArchived: 'Archived',
    archivedHint: 'This task was archived (its runtime moved out of the loopx registry). Restoring brings it back paused; press Resume to keep working.',
    restoreTask: 'Restore task',
    restoreTaskHint: 'Restore this task from the archive back to the board (registry entry + runtime dir)',
    restoreDone: (name) => `Restored: ${name}`,
    restoreFailed: 'Restore failed',
    taskStopped: (id) => `Task ${id} aborted: heartbeat and auto-run disabled`,
    taskResumed: (id) => `Task ${id} resumed: heartbeat and auto-run re-enabled`,
    taskDeleted: (id) => `Task ${id} deleted`,
    deleteTaskFailed: 'Delete failed',
    stopConfirmTitle: 'Abort this task?',
    stopConfirmText: (id) => `This cancels the running turn of "${id}", switches off its heartbeat monitoring and auto-run. The task moves to "Stopped" and can be resumed anytime.`,
    confirmStop: 'Abort it',
    deleteConfirmTitle: 'Delete this task?',
    deleteConfirmText: (id) => `This archives the runtime records of "${id}" and removes it from the registry (the registry file is backed up first). The task will no longer appear on the board.`,
    confirmDelete: 'Delete it',
    activityEmpty: 'No log yet',
    thinkBlockTitle: 'Reasoning',
    elapsedLabel: (t) => `elapsed ${t}`,
    waitingOn: (w) => `waiting on: ${w}`,
    waitingAgent: 'waiting for the agent',
    cancel: 'Cancel',
    needProject: 'Run-once requires a project directory',
    needAgent: 'This goal has no registered agent — type an agent id first',
    detected: (v) => `loopx detected: ${v}`,
    copy: 'Copy',
    close: 'Close',
    raw: 'JSON',
    groupPaused: 'Stopped',
    groupError: 'Errors',
    colEmpty: 'Nothing here',
    loadingGoals: 'Loading tasks…',
    statusRunning: 'Working',
    statusPaused: 'Stopped',
    statusErroring: (n) => `${n} fail`,
    statusUnmonitored: 'Monitoring off',
    statusManual: 'Auto-run off',
    statusAuto: 'Auto-run on',
    stageGated: 'Stage: waiting for your confirmation',
    stageRunning: 'Stage: working',
    stageFixing: (n) => `Stage: fixing #${n}`,
    stagePlanning: 'Stage: planning',
    skippedDuplicates: (n) => ` (${n} duplicate issue${n > 1 ? 's' : ''} skipped)`,
    runCancelled: 'run cancelled',
    turnStalled: (m) => `Turn stalled: no agent events for about ${m} minutes (the host may have silently cancelled it) — cancelled the turn and allowed an automatic retry.`,
    streamTrimmed: (n) => `(earlier content trimmed: ${n} chars)`,
    groupBacklog: 'Queued',
    groupActive: 'In progress',
    groupReview: 'Needs you',
    colSubReview: 'Blocking · continues after your approval',
    colSubActive: 'The agent is working',
    detailEmptyHint: 'Select an entry in "In progress" to see its details and live log',
    groupDone: 'Done',
    taskPlaceholder: 'Paste a GitHub issue / repository / issues-list link, optionally with fix instructions',
    taskGuidePlaceholder: (name) => `Guiding "${name}": type instructions to steer the agent (no new task is created)`,
    taskGoalUnsupported: 'Paste a GitHub issue, pull request, repository home, or issues-list link (free-form goals are not open yet)',
    taskUnsupportedPath: (u) => `Unsupported GitHub link: ${u}. Paste an issue, a pull request, the repository home, or its issues list.`,
    guidanceNoRunning: 'No task is running. Paste an issue link to create one, or wait until a task runs to send instructions.',
    guidancePickOne: 'Several tasks are running: select the target in "In progress" first, then send the instruction.',
    guidanceSending: 'Sending instruction…',
    guidanceSent: (id) => `Instruction sent to task ${id} — the agent reads it on its next step`,
    guidanceLine: (t) => `You: ${t}`,
    taskCreate: 'Create task',
    taskCreating: 'Creating task…',
    taskResolving: 'Detecting task type…',
    taskPendingLabel: 'Creating',
    taskStageStarting: 'Task created, starting the Agent',
    taskStarted: (id) => `Task ${id} created and started`,
    stageExpand: 'Fetching the issue list…',
    stageBootstrap: 'Creating the bitfun-loopx task…',
    stageRegister: 'Registering the Agent…',
    stagePlan: 'Planning the issue fix…',
    stageTodos: (c, t) => `Writing todos ${c}/${t}…`,
    stageRefresh: 'Refreshing state…',
    intakeTitleIssue: 'Fix this issue?',
    intakeTitleIssues: (n) => `Fix ${n} issues?`,
    intakeTitleList: 'Select issues to fix',
    intakeTitleGoal: 'Confirm new task',
    intakeSummaryList: (repo, n) => `${repo} has ${n} open issues — all selected by default. The task fixes the selected issues one by one.`,
    intakeSummaryIssues: (repo) => `These issues go into one task (${repo}); the agent fixes them one by one.`,
    intakeSummaryGoal: 'Tasks are already running — create a new one, or write this as guidance into an existing task.',
    intakeSelectAll: 'Select all',
    intakeSelectedCount: (n, m) => `${n}/${m} selected`,
    intakeModeNew: 'New task',
    intakeConfirmNew: 'Create task',
    intakeConfirmIssues: (n) => `Start fixing ${n} issues`,
    intakeConfirmGuide: 'Write into existing task',
    guideTargetNote: (id) => `The selected issues will be added as subtasks of the existing task: ${id}`,
    composerTargetTitle: 'Target: a new independent task, or an existing task that receives your input as feedback',
    deleteShort: 'Delete',
    deleteGoalNamed: (name) => `Delete: ${name}`,
    resizeHandleHint: 'Drag to resize the column',
    logBottomHint: 'Back to bottom',
    intakeNoneSelected: 'Select at least one issue',
    intakeNoIssues: 'This repository has no open issues',
    guideStarted: (id) => `Selected issues added as subtasks of task ${id}`,
    gateCount: (n) => `${n} item${n > 1 ? 's' : ''} need you`,
    gateEmptyHint: 'This gate has no directly approvable item yet',
    gateItemTitle: 'Pending confirmation',
    gateItemWithType: (hint) => `Pending confirmation (${hint})`,
    gateGroupBlocking: 'Needs confirmation · blocking',
    gateGroupInfo: 'Informational · not blocking',
    gateTypePublish: 'Publish / submit PR',
    gateTypeApprove: 'Approval',
    gateTypeInfo: 'Informational',
    gateCardTask: (name) => `Task: ${name}`,
    gateItemInfoLabel: (hint) => `Informational · ${hint}`,
    gateExplainWrite: 'Approval needed: grant write access so the agent can implement changes and submit the PR.',
    gateExplainDecide: 'Your decision needed: approve or reject this change.',
    gateExplainPublish: 'Approval needed: publish / submit the PR.',
    gateExplainMerge: 'Approval needed: merge.',
    gateExplainReview: 'Approval needed: external review request.',
    gateExplainPreload: 'This change adds a minimal Electron preload bridge to the desktop plugin.',
    gateBackground: 'Background: ',
    gateSummaryLoading: 'Generating the Chinese summary…',
    copyCardHint: 'Copy this card',
    autoApprovedWrite: 'Auto-approved per intake consent: write access (leaving the read-only adapter)',
    conclusionMerged: 'Conclusion: fixed and merged',
    conclusionCompleted: 'Conclusion: fix completed',
    conclusionNoFollowup: 'Conclusion: no fix needed (no follow-up action)',
    conclusionCancelled: 'Conclusion: cancelled',
    conclusionClosed: 'Conclusion: closed / duplicate',
    conclusionFinished: 'Conclusion: task finished',
    approveGate: 'Approve',
    completeTodoBtn: 'Mark done',
    approveGateTitle: 'Approve this action?',
    todoDoneTitle: 'Mark as done?',
    approveTitle: 'Confirm this action?',
    approveNote: 'Note (optional, recorded on the todo)',
    approveConfirm: 'Approve and continue',
    todoDoneConfirm: 'Mark done',
    approveGateHint: 'This item needs your approval: once approved, the task continues along this action.',
    todoDoneHint: 'This is an informational/instructional item: marking it done is enough — no approval and no new action.',
    approveDone: 'Approved — the task will continue',
    approveResumed: 'auto-run resumed after approval',
    todoDoneFeedback: 'Marked done',
    githubTokenTitle: 'GitHub token settings',
    githubTokenExplain: 'Used to fork the repository, push the branch, and create the PR. Provide a fine-grained Personal Access Token with Repository read/write. The token stays in this machine\'s BitFun app storage only. If the GitHub CLI is already signed in on this machine (gh auth login), publishing reuses it automatically — no token needed.',
    githubTokenPlaceholder: 'ghp_ or github_pat_ …',
    githubTokenSave: 'Save & verify',
    ghLoginGuideTitle: 'Option 1 (recommended): sign in with GitHub CLI',
    ghLoginGuide: 'Click the button below to auto-install GitHub CLI and sign in via the browser — no manual token creation (the system proxy is used automatically when the network is restricted).',
    ghLoginBtn: 'Sign in with GitHub CLI',
    tokenGuideTitle: 'Option 2: use an existing token',
    tokenGuide: 'Create a fine-grained token on GitHub (needs Contents and Pull requests read/write):',
    tokenGuideLink: 'github.com/settings/personal-access-tokens/new',
    ghLoginDone: (login) => `Signed in: ${login}`,
    ghLoginFailed: 'Sign-in failed',
    githubTokenStatus: 'Status: ',
    githubTokenSaved: (user) => `Token valid and saved (login: ${user})`,
    githubTokenInvalid: 'Token invalid or expired',
    githubTokenMissing: 'Not configured',
    githubTokenSet: 'Configured',
    approveAndPr: 'Approve & submit PR',
    approveOnly: 'Approve only (no PR)',
    approveOnlyNote: 'User approved without a PR submission',
    gateCredGh: '✓ GitHub signed in on this machine (gh) — the PR can be submitted directly',
    gateCredToken: (login) => `✓ GitHub token configured (${login})`,
    gateCredNone: '⚠ Not signed in to GitHub yet — sign in before submitting the PR',
    gateCredSetup: 'Sign in to GitHub',
    gateAfterPublish: 'After approval: forks to your GitHub automatically → pushes the branch → opens the PR (tagged [bitfun-loopx]), then continues with the remaining issues',
    gateAfterApprove: 'After approval: the task continues running automatically',
    sectionTarget: 'Target',
    sectionDecision: 'Your decision',
    sectionProgress: 'Progress',
    sectionResult: 'Result',
    approvePrTitle: 'Approve and submit the PR?',
    approvePrHint: 'On approval the console will: check/create your fork → push the fix branch → create a PR against the upstream repository (the title carries the [bitfun-loopx] marker so the tool\'s PRs are searchable).',
    approvePrNeedToken: '⚠ No GitHub token configured yet — approving will open the token settings first.',
    publishWorking: 'Publishing the PR (first fork may take a minute or two)…',
    publishAnalyzing: 'Analyzing the root cause and the solution…',
    publishDone: (url) => `✅ PR submitted: ${url}`,
    publishFailed: 'PR submission failed',
    publishNeedToken: 'A GitHub token is required to publish the PR',
    resetLoopxTitle: 'Clear all bitfun-loopx state?',
    resetLoopxText: 'This backs up and removes every LoopX-related state on this machine: all goals, todos and run history, global/project registries, the console\'s clone cache and persisted logs. Everything moves into a timestamped backup under the console data directory (cleared-<timestamp>) so it stays recoverable. This cannot be undone.',
    resetLoopxConfirm: 'Clear everything',
    resetLoopxWorking: 'Clearing…',
    resetLoopxDone: (dir) => `All bitfun-loopx state cleared (backup kept at ${dir})`,
    resetLoopxFailed: 'Clear failed',
    approveFailed: (e) => `Approval failed: ${e}`,
    notifGateTitle: 'bitfun-loopx needs your approval',
    notifGateBody: (id, block, info) => (info > 0
      ? `${id}: ${block} to confirm, ${info} informational`
      : `${id} has ${block} item${block > 1 ? 's' : ''} to confirm`),
    autoRunNext: 'Auto-running the next turn',
    autoRunDisabled: (id) => `${id} failed repeatedly — auto-run paused`,
    activityStarting: 'Starting the Agent…',
    activitySentPrompt: (n) => `▶ Instructions sent to the agent (${n} chars, click to expand)`,
    activityRunning: (elapsed) => `Agent is working · ${elapsed} elapsed`,
    activityCommitted: 'bitfun-loopx committed this run',
    activityValidationPassed: 'Independent validation passed',
    activityValidationFailed: 'Independent validation failed',
    activityStateUpdated: 'Goal state updated',
    activityCompleted: 'Run completed',
    activityCompletedValidated: 'Run completed · validation passed',
    activityFailed: 'Run failed',
    taskGoal: 'Goal',
    taskRepository: 'GitHub repository',
    taskIssue: 'GitHub Issue',
    taskIssues: (n) => `${n} Issues`,
    taskIssuesList: 'All repo issues',
    taskNeedProject: 'Select the local project directory for this task first.',
    taskRepoNotFound: (repo) => `GitHub repository not found: ${repo}. Check the link spelling.`,
    taskRepoLookupFailed: 'Could not verify the repository on GitHub; try again later.',
    stageClone: 'Cloning repository…',
    stageClonePercent: (p) => `Cloning repository… ${p}%`,
    intakeCloneNote: (repo) => `${repo} will be cloned into the MiniApp data directory (no local checkout needed).`,
    issueHasImages: 'This issue embeds images (screenshots) — text alone may not pinpoint the problem',
    intakeVisionWarn: (n) => `⚠ ${n} issue(s) embed images, but the current model has no multimodal capability: key information in the screenshots may be unreadable, and text alone may not confirm the root cause. Consider adding a text description (error messages, repro steps) before creating the task, or switch to a vision-capable model. You can still continue, but fix quality may suffer.`,
    intakeReuseNote: (repo) => `Found the local checkout of ${repo} — no re-cloning.`,
    intakeWriteNote: 'This confirmation grants the task repository write scope and continuous auto-run; you will only be asked again for PR/publish decisions.',
    taskCloneOtherRepo: (expected, actual) => `The local checkout is bound to ${actual}; ${expected} will be cloned into its own directory instead.`,
    composerModelTitle: 'Execution model for new tasks',
    otherTasksTitle: 'Other local loopx goals',
    otherTasksHint: 'Created by other loopx hosts; not monitored until adopted.',
    adopt: 'Adopt',
    adoptedLabel: 'Adopted',
    adoptFailed: (e) => `Adopt failed: ${e}`,
    modelAuto: 'Auto (follow BitFun policy)',
    modelPrimaryTag: 'primary',
    modelFollowGlobal: 'Follow global default',
    modelChanged: (m) => `Execution model switched to ${m}`,
    taskNeedAgent: 'Configure the default Agent for new tasks in Settings first.',
    taskCreated: (id) => `Task ${id} created`,
    taskRepoMismatch: (expected, actual) => `The link targets ${expected}, but the current project is ${actual}. Select the matching local checkout.`,
    taskMultipleRepos: 'One task can bind only one local repository. Split links from different repositories into separate tasks.',
    taskRepoUnverified: (repo) => `The selected directory is not a checkout of ${repo} (no GitHub remote found). Pick the repository directory first.`,
    taskPartial: (id, n, e) => `Task ${id} was created, but only ${n} todos were written: ${e}`,
    intakeTruncated: (n) => `(showing the first ${n} — the list is incomplete)`,
  },
};

function t(key, ...args) {
  const table = {};
  for (const [loc, entries] of Object.entries(I18N)) table[loc] = entries[key];
  const v = app.t(table, I18N['en-US'][key]);
  return typeof v === 'function' ? v(...args) : v;
}

// ── state ─────────────────────────────────────────────────
const DEFAULT_INTERVAL_MIN = 1;
const ERROR_BACKOFF_CAP_MIN = 30;

const S = {
  config: {
    projectDir: null, argvPrefix: null, srcDir: '', agentByGoal: {}, monitorByGoal: {},
    projectByGoal: {}, ownedGoals: {}, defaultAgentId: 'bitfun-agent', autoRunByGoal: {},
    defaultModel: 'auto', modelByGoal: {},
    // GitHub fine-grained PAT (Repository read/write) for the publish flow
    // (fork → push branch → create PR). Kept in the local app storage only.
    githubToken: '',
    githubLogin: '',
    // Local GitHub CLI credential probe (null = unknown, probed at boot).
    ghAvailable: null,
    // Drag-resizable column widths in px (0 = CSS default).
    reviewZoneWidth: 0,
    railWidth: 0,
    // Explicit user stops: stoppedByGoal persists the parked state across
    // restarts; autoRunBeforeStop remembers the auto-run setting to restore.
    stoppedByGoal: {}, autoRunBeforeStop: {},
    // Host agent session per goal. Persisted (not just in-memory) so a turn
    // after an app restart reuses the SAME hidden session: the host restores
    // it from disk and the agent continues with its full prior context
    // instead of starting from scratch.
    agentSessionByGoal: {},
  },
  detect: null,
  goals: new Map(), // goalId -> G
  bootLoading: true, // initial goals refresh in flight
  agentSessionByGoal: new Map(), // goalId -> host agent sessionId (context reuse)
  timer: null,
  countdownTimer: null,
  paused: false,
  renderPending: false,
  // Selected composer target goal ('' = 新建任务). The picker is a custom
  // popover: native selects cannot host per-option delete buttons.
  composerTargetId: '',
  activeGoalId: null,
  intakeDraft: null,
  pendingIntake: null, // resolveIntake result awaiting sheet confirmation
  moreOpen: new Set(),
  logs: [],
  // Persisted activity logs (goalId -> {lines}) restored on boot so the
  // stream survives console restarts; bounded per goal before each save.
  persistedLogs: {},
  // Persisted gate summaries (goalId -> {todoId -> {status, text}}) so the
  // three-line Chinese summary is ALREADY on the card when it renders —
  // generated once, displayed instantly on every later session.
  persistedGateSummaries: {},
};

// Direction C: a goal created by auto-clone binds to its own clone directory;
// goals bound to the user's selected checkout use the global setting.
function goalProjectDir(goalId) {
  return S.config.projectByGoal[goalId] || S.config.projectDir || null;
}

// v3.2: the board only manages goals this console created (bfx- prefix or an
// explicit adoption record). Goals created by other loopx hosts on this
// machine are shown separately and stay unmonitored until adopted.
function isOwnedGoal(goalId) {
  if (!goalId) return false;
  if (String(goalId).startsWith('bfx-')) return true;
  if (S.config.ownedGoals && S.config.ownedGoals[goalId]) return true;
  return false;
}

// All registries the board should aggregate: the selected checkout plus every
// clone directory recorded for created goals.
function projectRegistryDirs() {
  const dirs = [];
  if (S.config.projectDir) dirs.push(S.config.projectDir);
  for (const dir of Object.values(S.config.projectByGoal || {})) {
    if (dir && !dirs.includes(dir)) dirs.push(dir);
  }
  return dirs;
}

// ── execution model selection ───────────────────────────────
// Long-running fixes let the user pick the host agent model per goal, with a
// global default in Settings. Values: 'auto' (follow the host policy) or a
// concrete model config id from the host's model list. The abstract
// 'primary'/'fast' slot labels are NOT shown: the host marks its primary
// model (isDefault) in the catalog, and listing slots next to the concrete
// models they resolve to just duplicates one model under several labels.
S.modelCatalog = [];
function modelForGoal(goalId) {
  return S.config.modelByGoal[goalId] || S.config.defaultModel || 'auto';
}

function fillModelSelect(select, currentValue, includeFollowGlobal) {
  select.replaceChildren();
  if (includeFollowGlobal) {
    const follow = document.createElement('option');
    follow.value = '';
    follow.textContent = t('modelFollowGlobal');
    select.appendChild(follow);
  }
  const auto = document.createElement('option');
  auto.value = 'auto';
  auto.textContent = t('modelAuto');
  auto.selected = currentValue === 'auto';
  select.appendChild(auto);
  // Legacy slot values ('primary'/'fast') migrate onto the host's primary
  // model id so a persisted config never dangles after the label cleanup.
  const primaryModel = (S.modelCatalog || []).find((m) => m && m.isDefault);
  let effective = currentValue;
  if (effective === 'primary' || effective === 'fast') {
    effective = primaryModel ? primaryModel.id : 'auto';
  }
  for (const model of S.modelCatalog || []) {
    if (!model || !model.id) continue;
    const option = document.createElement('option');
    option.value = model.id;
    const tag = model.isDefault === true ? ` · ${t('modelPrimaryTag')}` : '';
    option.textContent = `${model.name || model.id}${tag}`;
    option.selected = effective === model.id;
    select.appendChild(option);
  }
}

// The composer carries a compact copy of the model default (right where the
// user submits); Settings keeps the full row. Both stay in sync.
function syncComposerModel() {
  fillModelSelect(document.getElementById('composer-model'), S.config.defaultModel || 'auto', false);
}

// The host model catalog currently exposes only supports_text_chat — no
// vision flag — so capability detection falls back to a name heuristic.
// Unknown models are treated as text-only: the conservative reading the
// image guard needs (screenshots may carry the whole problem).
const VISION_MODEL_HINTS = /vision|multimodal|gpt-4o|o1|o3|gemini|claude|qwen[^ ]*-?vl|glm-?4v|pixtral|llava|internvl|moondream/i;
function modelSupportsVision() {
  const catalog = Array.isArray(S.modelCatalog) ? S.modelCatalog : [];
  const currentId = String(S.config.defaultModel || 'auto');
  const entry = catalog.find((m) => m && m.id === currentId)
    || catalog.find((m) => m && m.isDefault)
    || catalog[0];
  if (!entry) return false;
  const flags = String(
    (entry.capabilities && Array.isArray(entry.capabilities) ? entry.capabilities.join(',') : entry.capabilities)
    || entry.capability || ''
  ).toLowerCase();
  if (flags && /vision|multimodal|image/.test(flags)) return true;
  if (flags && /text_chat/.test(flags)) return false;
  const label = `${String(entry.name || '')} ${String(entry.modelName || '')} ${String(entry.id || '')}`;
  return VISION_MODEL_HINTS.test(label);
}

// The composer shows where the next intake lands: a new task (default) or an
// existing goal. The picker is a custom popover so every goal option carries
// its own delete (×) button — native selects cannot host per-option buttons.
function composerTargetValue() {
  return S.composerTargetId || '';
}

// The input hint must match the mode: paste-a-link for new tasks, guide
// wording when an existing goal is selected (interjections, not intake).
function updateTaskPlaceholder() {
  const input = document.getElementById('task-input');
  if (!input) return;
  if (S.composerTargetId) {
    const g = S.goals.get(S.composerTargetId);
    input.placeholder = t('taskGuidePlaceholder', g ? goalDisplayName(g) : S.composerTargetId);
  } else {
    input.placeholder = t('taskPlaceholder');
  }
}

function setComposerTarget(id) {
  S.composerTargetId = id || '';
  updateTaskPlaceholder();
  const label = document.getElementById('composer-target-label');
  if (label) {
    if (S.composerTargetId) {
      const g = S.goals.get(S.composerTargetId);
      label.textContent = g ? goalDisplayName(g) : S.composerTargetId;
      label.title = S.composerTargetId;
    } else {
      label.textContent = t('intakeModeNew');
      label.title = '';
    }
  }
  const chip = document.getElementById('composer-target');
  if (chip) chip.classList.toggle('composer__target--picked', Boolean(S.composerTargetId));
}

function closeComposerTargetMenu() {
  const menu = document.getElementById('composer-target-menu');
  if (menu) menu.hidden = true;
}

// Refills are cheap and idempotent, so callers (render, refresh, boot) may
// invoke this freely; it never depends on the board render having completed.
function refillComposerTarget() {
  const menu = document.getElementById('composer-target-menu');
  if (!menu) return;
  try {
    const previous = composerTargetValue();
    menu.replaceChildren();
    const optNew = document.createElement('button');
    optNew.type = 'button';
    optNew.className = 'composer-target__item' + (previous ? '' : ' is-selected');
    const newLabel = document.createElement('span');
    newLabel.className = 'composer-target__item-label';
    newLabel.textContent = t('intakeModeNew');
    optNew.appendChild(newLabel);
    optNew.onclick = () => {
      setComposerTarget('');
      closeComposerTargetMenu();
    };
    menu.appendChild(optNew);
    for (const g of S.goals.values()) {
      if (isTerminal(g)) continue;
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'composer-target__item' + (g.goalId === previous ? ' is-selected' : '');
      const label = document.createElement('span');
      label.className = 'composer-target__item-label';
      // Status suffix makes same-repo historical siblings distinguishable.
      const pending = Array.isArray(g.userTodos) && g.userTodos.length
        ? ` · ${t('gateCount', g.userTodos.length)}`
        : '';
      label.textContent = `${goalDisplayName(g)} · ${goalStatus(g).text}${pending}`;
      label.title = g.goalId;
      item.appendChild(label);
      const close = document.createElement('span');
      close.className = 'composer-target__item-close';
      close.textContent = '×';
      close.title = `${t('deleteGoalNamed', goalDisplayName(g))}（${g.goalId}）`;
      close.onclick = (ev) => {
        ev.stopPropagation();
        openDeleteConfirm(g);
      };
      item.appendChild(close);
      item.onclick = () => {
        setComposerTarget(g.goalId);
        closeComposerTargetMenu();
      };
      menu.appendChild(item);
    }
    // Keep the pick valid; when its goal vanished (deleted), fall back to new.
    if (previous && !S.goals.has(previous)) setComposerTarget('');
    else setComposerTarget(previous);
    dbgUi('refillTarget', `opts=${menu.children.length} goals=${S.goals.size} value=${JSON.stringify(composerTargetValue())}`);
  } catch (err) {
    dbgUi('refillTarget:error', String(err && (err.stack || err.message) || err).slice(0, 300));
  }
}

function newGoalState(goalId, info) {
  const archived = !!info.archived;
  return {
    goalId,
    objective: info.objective || null,
    agents: info.agents || [],
    agentId: S.config.agentByGoal[goalId] || (info.agents && info.agents[0]) || '',
    state: info.state || null,
    waitingOn: info.waitingOn ?? null,
    // v3.2: only owned goals poll by default; other-host goals stay quiet
    // until the user adopts them. An explicit user stop overrides ownership.
    // Archived goals never poll or auto-run — they live in the quiet 已归档
    // group until the user restores them explicitly.
    monitoring: !archived && (isOwnedGoal(goalId)
      ? (S.config.monitorByGoal[goalId] !== false && S.config.stoppedByGoal[goalId] !== true)
      : S.config.monitorByGoal[goalId] === true),
    userStopped: S.config.stoppedByGoal[goalId] === true,
    // loopx's philosophy is auto-run by default: owned goals execute
    // automatically unless the user explicitly switched auto-run off.
    // (Re-discovered cache goals on a fresh import get a fresh config, so
    // defaulting to ON keeps them running instead of vanishing into the
    // hidden queued state.)
    autoRun: !archived && (isOwnedGoal(goalId)
      ? S.config.autoRunByGoal[goalId] !== false
      : S.config.autoRunByGoal[goalId] === true),
    archived,
    archiveDir: info.archiveDir || null,
    autoFailCount: 0,
    intervalMin: DEFAULT_INTERVAL_MIN,
    nextDueAt: 0,
    unchangedCount: 0,
    errorCount: 0,
    lastError: null,
    lastResetToken: null,
    lastDecisionKey: null,
    hint: null,          // { base, mult, cap } for the interval-math line
    stopped: false,
    polling: false,
    repollQueued: false,
    running: false,
    runStartedAt: 0,
    lastRun: null,       // { exitCode, durationMs, status, ok, cancelled }
    last: null,          // normalized shouldRun result
    userTodos: null,     // open user-lane todos (gate approvals), null = not loaded
    userTodosAt: 0,
    userTodosLoading: false,
    // Per-issue tracker for batch goals (one agent todo per issue): the
    // board projection { issues:[{url,number,title,status,done}], done, total }.
    issues: null,
    issuesAt: 0,
    issuesLoading: false,
    wasGated: false,
    // First gate observation of the session adopts silently: pre-existing
    // gates must not re-notify when the console opens or a new task starts.
    firstGateCheck: true,
    // Gate-item helpers: agent-lane todos (issue titles as background) and
    // per-item Chinese summaries generated by the host agent.
    agentTodos: [],
    gateSummaries: (() => {
      const stored = S.persistedGateSummaries && S.persistedGateSummaries[goalId];
      const map = new Map();
      if (stored && typeof stored === 'object') {
        for (const [todoId, v] of Object.entries(stored)) {
          if (v && v.status === 'done' && v.text) {
            // Self-heal legacy caches: summaries persisted before the
            // parse-don't-filter rule may contain reasoning walls — clean
            // them on restore so dirty data never resurfaces on the card.
            map.set(todoId, { status: 'done', text: cleanGateSummary(String(v.text)) });
          }
        }
      }
      return map;
    })(),
    activityLines: [],
    currentActivity: '',
  };
  // Restore the persisted log so the stream survives console restarts.
  const persisted = S.persistedLogs && S.persistedLogs[goalId];
  if (Array.isArray(persisted) && persisted.length) {
    state.activityLines = persisted.map((e) => ({
      time: e.time || '', line: String(e.line || ''), isErr: !!e.isErr,
      count: e.count || 1, kind: e.kind || null, raw: e.raw || null,
      stream: !!e.stream, isTick: !!e.isTick,
    }));
    // Card activity line = progress, never model prose: prefer the last
    // tool/status line over agent/think stream text when restoring, so cached
    // self-talk ("我需要用三句话…") never resurfaces on the board.
    const last = [...persisted].reverse()
      .find((e) => e.line && !e.isTick && e.kind !== 'agent' && e.kind !== 'think')
      || [...persisted].reverse().find((e) => e.line && !e.isTick);
    if (last) state.currentActivity = activityText(String(last.line));
  }
  return state;
}

// ── log persistence ─────────────────────────────────────────
// The stream is incremental and lives in memory; persist it (debounced, per
// goal, bounded) so a console restart keeps the log. Raw prompt bodies are
// capped before they hit storage.
let logSaveTimer = null;
let logSaveDirty = false;
function scheduleLogSave() {
  logSaveDirty = true;
  if (logSaveTimer) return;
  logSaveTimer = setTimeout(() => {
    logSaveTimer = null;
    if (!logSaveDirty) return;
    logSaveDirty = false;
    saveLogs();
  }, 3000);
}
// Gate summaries persist so the three-line summary renders instantly on the
// card in later sessions — the model only pays latency the first time.
let gateSummarySaveTimer = null;
function scheduleGateSummarySave() {
  if (gateSummarySaveTimer) return;
  gateSummarySaveTimer = setTimeout(async () => {
    gateSummarySaveTimer = null;
    const store = {};
    for (const g of S.goals.values()) {
      if (!g.gateSummaries || !g.gateSummaries.size) continue;
      const obj = {};
      for (const [todoId, v] of g.gateSummaries) {
        if (v && v.status === 'done' && v.text) obj[todoId] = { status: 'done', text: v.text };
      }
      if (Object.keys(obj).length) store[g.goalId] = obj;
    }
    S.persistedGateSummaries = store;
    try { await app.storage.set('gateSummaries', store); } catch (_) {}
  }, 2000);
}

async function saveLogs() {
  const logs = {};
  for (const g of S.goals.values()) {
    if (!Array.isArray(g.activityLines) || !g.activityLines.length) continue;
    // Persistence is a restart-history snapshot, not the raw view: keep the
    // window small (120 lines × 1.2KB) so multi-goal boards never round-trip
    // megabytes of JSON through the worker every few seconds.
    logs[g.goalId] = g.activityLines.slice(-120).map((e) => ({
      time: e.time, line: String(e.line || '').slice(0, 1200),
      isErr: !!e.isErr, count: e.count || 1, kind: e.kind || null,
      raw: e.raw ? String(e.raw).slice(0, 1200) : null,
      stream: !!e.stream, isTick: !!e.isTick,
    }));
  }
  try { await app.storage.set('logs', logs); } catch (_) {}
}

// ── logging ───────────────────────────────────────────────
// ── debug trace (UI side) ──────────────────────────────────
// Written to <appdata>/debug-ui.log through the host fs bridge so host logs
// are not required for diagnosis.
const DEBUG_UI = [];
let DEBUG_UI_BUSY = false;
async function dbgUi(tag, detail) {
  const line = `${new Date().toISOString()} ${tag} ${detail || ''}`;
  DEBUG_UI.push(line);
  if (DEBUG_UI.length > 200) DEBUG_UI.shift();
  if (DEBUG_UI_BUSY || typeof app === 'undefined' || !app.appDataDir || !app.fs) return;
  DEBUG_UI_BUSY = true;
  try {
    await app.fs.writeFile(`${app.appDataDir}/debug-ui.log`, DEBUG_UI.join('\n'));
  } catch (_) {
    // The trace must never break the flow.
  } finally {
    DEBUG_UI_BUSY = false;
  }
}

// Uncaught UI errors must land in the diagnostic log: a silent render crash
// is otherwise indistinguishable from a frozen app. Keep the slices short so
// one noisy frame cannot drown the timeline.
window.addEventListener('error', (e) => {
  dbgUi('uiError', `${e.message || e.error || 'unknown'} @${(e.filename || '').split('/').pop()}:${e.lineno || '?'}`);
});
window.addEventListener('unhandledrejection', (e) => {
  const r = e && e.reason;
  dbgUi('uiRejection', String((r && (r.stack || r.message)) || r).slice(0, 400));
});

// ── logging ───────────────────────────────────────────────
// Diagnostic trace kept in memory for debugging; the user-facing log surface
// is the per-task activity panel, so there is no global log drawer anymore.
function log(msg, isErr = false) {
  const time = new Date().toTimeString().slice(0, 8);
  S.logs.push({ time, msg, isErr });
  if (S.logs.length > 500) S.logs.splice(0, S.logs.length - 500);
}

// ── config persistence ────────────────────────────────────
async function loadConfig() {
  try {
    const stored = await app.storage.get('config');
    if (stored && typeof stored === 'object') Object.assign(S.config, stored);
  } catch (_) {}
  try {
    const logs = await app.storage.get('logs');
    if (logs && typeof logs === 'object') S.persistedLogs = logs;
  } catch (_) {}
  try {
    const summaries = await app.storage.get('gateSummaries');
    if (summaries && typeof summaries === 'object') S.persistedGateSummaries = summaries;
  } catch (_) {}
  // Execution moved to the host agent; drop persisted external-host settings.
  delete S.config.host;
  delete S.config.codexBin;
  delete S.config.hostCommandJson;
  delete S.config.validationCommandJson;
  delete S.config.timeoutSeconds;
  if (!S.config.defaultAgentId) {
    S.config.defaultAgentId = Object.values(S.config.agentByGoal || {}).find(Boolean) || 'bitfun-agent';
  }
}
async function saveConfig() {
  try { await app.storage.set('config', S.config); } catch (_) {}
}

// ── heartbeat scheduling ──────────────────────────────────
function rearmTimer() {
  if (S.timer) { clearTimeout(S.timer); S.timer = null; }
  if (S.paused) return;
  let earliest = Infinity;
  for (const g of S.goals.values()) {
    if (g.monitoring && !g.stopped && !g.polling && g.nextDueAt < earliest) earliest = g.nextDueAt;
  }
  if (earliest === Infinity) return;
  const delay = Math.max(0, earliest - Date.now());
  S.timer = setTimeout(onTimerFire, Math.min(delay, 2147000000));
}

function onTimerFire() {
  S.timer = null;
  const now = Date.now();
  for (const g of S.goals.values()) {
    if (g.monitoring && !g.stopped && !g.polling && g.nextDueAt <= now) pollGoal(g);
  }
  rearmTimer();
}

function valueAtPath(obj, path) {
  return path.split('.').reduce((c, p) => (c && typeof c === 'object' ? c[p] : undefined), obj);
}

// Prefer the contract's unchanged_identity_keys over home-grown fields:
// free-text `reason` embeds live quota fractions and would defeat backoff.
function decisionKey(res) {
  const keys = res.scheduler?.unchangedIdentityKeys;
  if (keys && keys.length && res.raw) {
    return keys.map((k) => String(valueAtPath(res.raw, k))).join('|');
  }
  return [res.shouldRun, res.state, res.effectiveAction].map(String).join('|');
}

function applyPollError(g, message) {
  g.errorCount += 1;
  g.lastError = message;
  g.intervalMin = Math.min(Math.pow(2, g.errorCount), ERROR_BACKOFF_CAP_MIN);
  log(`[${g.goalId}] poll failed ×${g.errorCount}: ${message}`, true);
  // The panel IS the only log surface now: a dead worker / broken loopx
  // must be visible there instead of freezing silently.
  recordGoalActivity(g, `⚠ ${message}`, true);
}

async function pollGoal(g) {
  if (g.polling) return;
  g.polling = true;
  requestRender();
  try {
    const res = await app.call('loopx.shouldRun', {
      argvPrefix: S.config.argvPrefix,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
      agentId: g.agentId || undefined,
    });
    if (res.raw) g.last = res; // keep partial payloads visible (reason, state)
    if (res.ok === false || res.error) {
      // CLI-level failure (bad exit / no JSON) is an error, not a decision.
      applyPollError(g, res.error || res.reason || 'loopx exited non-zero');
      return;
    }
    g.errorCount = 0;
    g.lastError = null;
    // shouldRun is authoritative for the gate: a cleared waiting_on (null)
    // must un-gate the goal rather than stick to the stale listGoals value.
    g.waitingOn = res.waitingOn ?? null;
    const sched = res.scheduler || {};
    const recommended = Number(sched.recommendedIntervalMinutes) || DEFAULT_INTERVAL_MIN;
    const maxIv = Number(sched.maxIntervalMinutes) || Math.max(recommended, 60);
    const backoff = Number(sched.backoffMultiplier) || 2;
    g.hint = { base: recommended, mult: backoff, cap: maxIv };
    const token = sched.resetToken || null;
    const key = decisionKey(res);

    if (token !== g.lastResetToken) {
      // loopx-side goal mutation → reset cadence to the fresh recommendation
      g.lastResetToken = token;
      g.intervalMin = recommended;
      g.unchangedCount = 0;
      g.stopped = false;
      log(`[${g.goalId}] reset_token changed → interval ${g.intervalMin}m`);
    } else if (key !== g.lastDecisionKey) {
      g.intervalMin = recommended;
      g.unchangedCount = 0;
      const reasonBrief = String(res.reason || '').slice(0, 160);
      log(`[${g.goalId}] decision changed (${res.state ?? '?'}/${res.shouldRun}) → interval ${g.intervalMin}m${reasonBrief ? ` · ${reasonBrief}` : ''}`);
    } else {
      g.unchangedCount += 1;
      g.intervalMin = Math.min(g.intervalMin * backoff, maxIv);
      const limit = sched.unchangedPollLimit;
      if (limit != null && g.unchangedCount >= limit && sched.afterLimit === 'stop_tick_loop') {
        g.stopped = true;
        log(`[${g.goalId}] unchanged ×${g.unchangedCount} ≥ limit → tick loop stopped`);
      }
      // Steady-state backoff steps stay invisible on the card; logging every
      // tick would flood the diagnostic log.
    }
    g.lastDecisionKey = key;
    g.intervalMin = Math.min(Math.max(g.intervalMin, recommended), maxIv);
  } catch (err) {
    applyPollError(g, String(err.message || err));
  } finally {
    g.nextDueAt = Date.now() + g.intervalMin * 60000;
    g.polling = false;
    // The goal may have been dropped/replaced mid-poll (project switch,
    // refreshGoals): an orphaned closure must not notify or launch anything.
    if (isLiveGoal(g)) {
      syncGateState(g);
      requestRender();
      rearmTimer();
      maybeAutoRun(g);
      if (g.repollQueued) {
        g.repollQueued = false;
        pollNow(g);
      }
    }
  }
}

function pollNow(g, { force = false } = {}) {
  if (g.polling) {
    // A poll is in flight; queue exactly one follow-up instead of silently
    // dropping the request (matters after run-once completes).
    g.repollQueued = true;
    return;
  }
  g.nextDueAt = 0;
  g.stopped = false;
  // force bypasses the visibility pause: a finished run must still get its
  // decision poll (which chains the next auto-run turn and fires gate
  // notifications) while the window is hidden — the exact moment the batch
  // and the "needs your approval" notification matter most.
  if (S.paused && !force) return; // due immediately once the heartbeat resumes
  pollGoal(g).then(rearmTimer);
}

// ── user gates (approvals) ────────────────────────────────
// A gated goal's concrete asks live in its user-lane todos. Load them lazily
// when a goal enters the review group, cache for 60s, and raise attention
// (system notification) exactly on the not-gated → gated edge. Returns true
// when a fetch actually ran (false = skipped: in-flight or fresh cache) so
// callers can chain a re-evaluation without looping.
async function refreshUserTodos(g, force = false) {
  if (g.userTodosLoading) return false;
  if (!force && g.userTodos && Date.now() - g.userTodosAt < 60000) return false;
  g.userTodosLoading = true;
  try {
    const res = await app.call('loopx.listTodos', {
      argvPrefix: S.config.argvPrefix,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
      role: 'user',
      status: 'open',
    });
    g.userTodos = res.ok ? res.todos : [];
    g.userTodosAt = Date.now();
    // The intake sheet already granted write scope (bootstrap
    // --write-scope write): a "leave the read-only adapter" gate just re-asks
    // for that consent, so complete it automatically and reload — the user
    // keeps only real decisions (design choices) and publish/PR approvals.
    const writeGates = g.userTodos.filter((td) =>
      td.task_class === 'user_gate'
      && /write access|leave the read-?only adapter|connected-read-only/i.test(String(td.text || td.title || '')));
    for (const todo of writeGates) {
      try {
        const done = await app.call('loopx.completeTodo', {
          argvPrefix: S.config.argvPrefix,
          srcDir: S.config.srcDir || null,
          projectDir: goalProjectDir(g.goalId),
          goalId: g.goalId,
          todoId: todo.todo_id,
          note: '由任务入库确认自动批准（写权限已预授）',
          decisionOutcome: 'approve',
        });
        if (done.ok) {
          recordGoalActivity(g, t('autoApprovedWrite'), false, 'agent');
          log(`[${g.goalId}] auto-approved write-access gate (${todo.todo_id})`);
        }
      } catch (err) {
        log(`[${g.goalId}] auto-approve write gate failed: ${err.message || err}`, true);
      }
    }
    if (writeGates.length) {
      const reload = await app.call('loopx.listTodos', {
        argvPrefix: S.config.argvPrefix,
        projectDir: goalProjectDir(g.goalId),
        goalId: g.goalId,
        role: 'user',
        status: 'open',
      });
      g.userTodos = reload.ok ? reload.todos : g.userTodos;
      // The gate may be cleared by the approvals: re-decide immediately so
      // the goal resumes instead of waiting for the next heartbeat.
      pollNow(g, { force: true });
    }
    // Agent-lane todos carry the issue titles ("Fix GitHub issue #N: <title>")
    // used as the gate items' background, and they persist across restarts.
    try {
      const agentRes = await app.call('loopx.listTodos', {
        argvPrefix: S.config.argvPrefix,
        projectDir: goalProjectDir(g.goalId),
        goalId: g.goalId,
        role: 'agent',
      });
      g.agentTodos = agentRes.ok ? agentRes.todos : (g.agentTodos || []);
    } catch (_) {
      if (!g.agentTodos) g.agentTodos = [];
    }
    // Kick off the Chinese summaries for blocking items that lack one.
    for (const todo of g.userTodos || []) {
      if (gateTodoInfo(todo).isBlocking) ensureGateSummary(g, todo);
    }
    // Publish guidance needs the GitHub credential state: probe the local gh
    // CLI once per session when it is still unknown.
    if (S.ghAvailable === null) {
      try {
        const probe = await app.call('loopx.githubGhToken', {});
        S.ghAvailable = Boolean(probe && probe.ok);
      } catch (_) {
        S.ghAvailable = false;
      }
    }
  } catch (err) {
    log(`[${g.goalId}] listTodos error: ${err.message || err}`, true);
    if (!g.userTodos) g.userTodos = [];
  } finally {
    g.userTodosLoading = false;
    renderGoal(g);
  }
  return true;
}

function notifyGate(g) {
  const todos = g.userTodos || [];
  const blocking = todos.filter((td) => gateTodoInfo(td).isBlocking).length;
  const infoOnly = todos.length - blocking;
  const body = t('notifGateBody', goalDisplayName(g), blocking, infoOnly);
  try {
    if (app.notifications?.system) {
      app.notifications.system(t('notifGateTitle'), body);
    }
  } catch (_) {}
  log(`[${g.goalId}] ${body}`, false);
}

function syncGateState(g) {
  const gated = isGated(g);
  if (g.firstGateCheck) {
    // First observation this session (boot / goal load): adopt the state
    // silently. A gate that already existed must not fire a "historical"
    // notification every time the console opens or a new task is created.
    g.firstGateCheck = false;
    if (gated) {
      g.wasGated = true;
      refreshUserTodos(g, true);
    } else if (shouldTrackUserTodos(g)) {
      // waiting_on may say codex while a publish approval is already open:
      // discover it silently, then adopt the post-load state.
      refreshUserTodos(g).then((ran) => {
        if (ran && isLiveGoal(g)) {
          g.wasGated = isGated(g);
          requestRender();
        }
      });
    } else {
      g.wasGated = false;
    }
    return;
  }
  if (gated) {
    if (!g.wasGated) {
      // Load the concrete asks first so the notification names the first one
      // instead of a generic "1 item".
      refreshUserTodos(g, true).then(() => { if (isLiveGoal(g)) notifyGate(g); });
    } else {
      refreshUserTodos(g);
    }
  } else if (shouldTrackUserTodos(g)) {
    // Not gated by waiting_on: keep probing for open user todos so a publish
    // approval surfaces even while loopx reports waiting_on=codex. Chain the
    // re-evaluation only when a fetch actually ran (TTL skips stop the chain).
    refreshUserTodos(g).then((ran) => {
      if (ran && isLiveGoal(g)) syncGateState(g);
    });
  }
  g.wasGated = gated;
}

// User todos only matter for goals this console owns and runs: archived goals
// are restore-only, other-host goals are not ours to approve.
function shouldTrackUserTodos(g) {
  return !g.archived && isOwnedGoal(g.goalId);
}

async function approveTodo(g, todo, note, button, opts = {}) {
  if (button) button.disabled = true;
  try {
    // Publish gates publish by default: the console forks (when needed),
    // pushes the branch and creates the PR first, then completes the todo so
    // loopx reconciles the created PR instead of pushing on its own. The user
    // can opt out of the PR submission (仅批准).
    if (isPublishTodo(todo) && opts.publish !== false) {
      let token = String(S.config.githubToken || '').trim();
      if (!token) {
        // Reuse the machine's GitHub CLI credential when available — BitFun
        // itself does not store GitHub tokens and delegates to gh auth.
        try {
          const probe = await app.call('loopx.githubGhToken', {});
          if (probe && probe.ok && probe.token) {
            token = probe.token;
            log(`[${g.goalId}] using local GitHub CLI credentials for publish`);
          }
        } catch (_) {}
        if (!token) {
          log(`[${g.goalId}] ${t('publishNeedToken')}`, true);
          recordGoalActivity(g, t('publishNeedToken'), true);
          if (button) button.disabled = false;
          openTokenDialog();
          return false;
        }
      }
      recordGoalActivity(g, t('publishWorking'));
      try {
        let analysis = null;
        try {
          recordGoalActivity(g, t('publishAnalyzing'));
          analysis = await generateCauseAnalysis(g, todo);
        } catch (_) { /* publish proceeds without the analysis */ }
        const issueRefs = parseIssueUrls(String(todo.text || todo.title || ''));
        const published = await app.call('loopx.publishPr', {
          projectDir: goalProjectDir(g.goalId),
          goalId: g.goalId,
          token,
          title: prTitleFor(g),
          body: prBodyFor(g),
          branch: branchHintFromText(todo.text || todo.title || ''),
          // The worker composes the real PR content from this: issue number
          // in the title, "Fixes #N" binding, issue link + one-line
          // description, the generated 原因/解决 analysis and the branch's
          // commit subjects.
          issueUrl: issueRefs.length ? issueRefs[0].url : null,
          analysis,
        });
        if (!published.ok) throw new Error(published.error || 'publish failed');
        recordGoalActivity(g, t('publishDone', published.prUrl), false, 'agent');
        log(`[${g.goalId}] PR published: ${published.prUrl} (fork=${published.forkUrl})`);
        note = [note || '', `PR 已由控制台创建：${published.prUrl}`].filter(Boolean).join(' · ');
      } catch (err) {
        const message = String(err && err.message || err);
        log(`[${g.goalId}] publish failed: ${message}`, true);
        recordGoalActivity(g, `${t('publishFailed')}: ${message}`, true);
        if (button) button.disabled = false;
        return false;
      }
    }
    if (isPublishTodo(todo) && opts.publish === false) {
      note = [note || '', t('approveOnlyNote')].filter(Boolean).join(' · ');
    }
    const res = await app.call('loopx.completeTodo', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
      todoId: todo.todo_id,
      note: note || null,
      // user_gate todos hard-require a decision outcome; other classes
      // reject the flag, so send it only where the CLI demands it.
      decisionOutcome: todo.task_class === 'user_gate' ? 'approve' : null,
    });
    if (!res.ok) throw new Error(res.error || 'todo complete failed');
    const todoIsGate = todo.task_class === 'user_gate';
    log(`[${g.goalId}] ${todoIsGate ? t('approveDone') : t('todoDoneFeedback')} (${todo.todo_id})`);
    // Approval is an explicit "go": clear any user stop and re-enable
    // auto-run so the task proceeds to the next step without another click
    // (the paused 继续/删除 buttons must not linger after a decision).
    if (g.userStopped || !g.autoRun || !g.monitoring) {
      g.userStopped = false;
      delete S.config.stoppedByGoal[g.goalId];
      delete S.config.autoRunBeforeStop[g.goalId];
      g.monitoring = true;
      S.config.monitorByGoal[g.goalId] = true;
      g.autoRun = true;
      S.config.autoRunByGoal[g.goalId] = true;
      await saveConfig();
      log(`[${g.goalId}] ${t('approveResumed')}`);
    }
    // Reload the gate list fresh. A rapid second approval can land while the
    // first reload is still in flight — wait it out so the just-completed
    // todo is guaranteed to be reflected (otherwise the stale cached list
    // keeps the second card visible for up to 60s).
    while (g.userTodosLoading) {
      await new Promise((resolve) => setTimeout(resolve, 150));
    }
    await refreshUserTodos(g, true);
    pollNow(g, { force: true }); // approval may clear the gate — re-decide immediately
    return true;
  } catch (err) {
    log(`[${g.goalId}] ${t('approveFailed', err.message || err)}`, true);
    if (button) button.disabled = false;
    return false;
  }
}

// ── auto-run ──────────────────────────────────────────────
// Composer-created tasks run continuously: whenever loopx says should_run and
// no gate is open, fire the next turn without asking. Three consecutive
// failures trip the breaker so a broken setup can't loop forever.
const AUTO_RUN_FAIL_LIMIT = 3;

// A goal object becomes an orphan when refreshGoals/project-switch replaces
// or drops it; async callbacks holding the old reference must go quiet.
function isLiveGoal(g) {
  return S.goals.get(g.goalId) === g;
}

function canAutoRun(g) {
  return g.autoRun && !g.running && isLiveGoal(g) && !isGated(g)
    // Only a FRESH successful decision may launch a turn — a failed poll
    // leaves g.last stale (or ok:false) and must never fire on it.
    && g.errorCount === 0 && g.last?.ok !== false
    && g.last?.shouldRun === true && g.autoFailCount < AUTO_RUN_FAIL_LIMIT
    && !!goalProjectDir(g.goalId) && !!g.agentId;
}

function maybeAutoRun(g) {
  if (!canAutoRun(g)) return;
  log(`[${g.goalId}] ${t('autoRunNext')}`);
  executeRunOnce(g).catch((err) => {
    log(`[${g.goalId}] auto-run error: ${err.message || err}`, true);
    g.running = false;
    renderGoal(g);
  });
}

function setAutoRun(g, enabled) {
  g.autoRun = enabled;
  g.autoFailCount = 0;
  S.config.autoRunByGoal[g.goalId] = enabled;
  saveConfig();
  if (enabled) maybeAutoRun(g);
}

// ── stop / resume task ────────────────────────────────────
// "停止任务" is a FULL stop, not a single-turn cancel: the in-flight run is
// cancelled, the loopx heartbeat (auto-poll) for this goal is switched off,
// auto-run is switched off, and the parked state persists across restarts.
// "取消运行" (cancelGoalRun) remains turn-scoped: it kills only the current
// run and prevents an immediate relaunch; polling continues.
async function stopGoalTask(g) {
  if (g.running) cancelGoalRun(g, null);
  S.config.autoRunBeforeStop[g.goalId] = g.autoRun === true;
  if (g.autoRun) setAutoRun(g, false);
  g.monitoring = false;
  S.config.monitorByGoal[g.goalId] = false;
  g.userStopped = true;
  S.config.stoppedByGoal[g.goalId] = true;
  await saveConfig();
  rearmTimer();
  log(t('taskStopped', g.goalId));
  recordGoalActivity(g, t('taskStopped', g.goalId));
  renderGoalDetails(g);
  renderAllGoals(true);
}

async function resumeGoalTask(g) {
  delete S.config.stoppedByGoal[g.goalId];
  g.userStopped = false;
  g.monitoring = true;
  S.config.monitorByGoal[g.goalId] = true;
  if (S.config.autoRunBeforeStop[g.goalId] === true) {
    g.autoRun = true;
    S.config.autoRunByGoal[g.goalId] = true;
  } else if (S.config.autoRunBeforeStop[g.goalId] === false) {
    g.autoRun = false;
    S.config.autoRunByGoal[g.goalId] = false;
  }
  delete S.config.autoRunBeforeStop[g.goalId];
  await saveConfig();
  log(t('taskResumed', g.goalId));
  recordGoalActivity(g, t('taskResumed', g.goalId));
  renderGoalDetails(g);
  pollNow(g); // immediate fresh decision; auto-run may launch from it
  renderAllGoals(true);
}

// ── pause / resume (lifecycle + visibility) ───────────────
function pauseHeartbeat() {
  if (S.paused) return;
  S.paused = true;
  if (S.timer) { clearTimeout(S.timer); S.timer = null; }
}

function resumeHeartbeat() {
  if (!S.paused) return;
  S.paused = false;
  const now = Date.now();
  for (const g of S.goals.values()) {
    if (g.monitoring && !g.stopped && g.nextDueAt <= now) pollGoal(g);
  }
  rearmTimer();
}

// ── rendering ─────────────────────────────────────────────
function isGated(g) {
  // After a successful poll, its waiting_on is authoritative (may be null);
  // before one, fall back to the listGoals snapshot. waiting_on=controller is
  // NOT a user gate: this console runs as scheduler_owner=outer_controller,
  // i.e. it IS the controller — auto-run should take that turn, not park the
  // goal in "needs you" with nothing approvable.
  const w = g.last && g.last.ok !== false ? g.last.waitingOn : g.waitingOn;
  if (w === 'user') return true;
  const s = String(g.last?.state || g.state || '').toLowerCase();
  if (/gate|user_action|operator/.test(s)) return true;
  // loopx may keep waiting_on=codex while an open user-lane todo (publish
  // approval etc.) sits pending — the todo itself is the authoritative gate.
  // Multi-issue goals run other issues in parallel, so waiting_on alone is
  // NOT enough to surface a publish approval. Informational user todos
  // (guidance, not decisions) must NOT gate: they would park the goal in the
  // review column with nothing actionable to show.
  if (Array.isArray(g.userTodos) && g.userTodos.some((td) => gateTodoInfo(td).isBlocking)) {
    return true;
  }
  return false;
}

// The board mirrors an issue tracker, but attention comes first: ONLY the two
// things that deserve a column exist — work that needs the human (blocking)
// and work that is running. Queued auto-run goals between turns are
// intentionally invisible (they surface the moment they run or need
// approval); paused/stopped/error goals stay visible as dimmed rail cards
// (so a restart can never make a task disappear), and terminal/other-host
// goals collapse into the quiet "more" chips footer.
const PRIMARY_GROUPS = ['review', 'active'];
const ARCHIVE_GROUPS = ['done', 'archived'];
const GROUP_I18N_KEY = {
  backlog: 'groupBacklog', active: 'groupActive', review: 'groupReview',
  done: 'groupDone', paused: 'groupPaused', error: 'groupError',
  archived: 'groupArchived',
};
const GROUP_SUB_KEY = {
  review: 'colSubReview', active: 'colSubActive',
};

function isTerminal(g) {
  const state = String(g.last?.state || g.state || '').toLowerCase();
  return /(^|_)(terminal|completed|complete|done|cancelled|canceled|duplicate|merged|closed)(_|$)/.test(state)
    || state.includes('no_followup');
}

function goalGroup(g) {
  // An archived task sits in the quiet 已归档 group with a 恢复 button —
  // never in a hidden bucket, never mixed into running/paused states.
  if (g.archived) return 'archived';
  if (isTerminal(g)) return 'done';
  if (g.userStopped) return 'paused';
  if (g.errorCount > 0) return 'error';
  if (g.stopped) return 'paused';
  // A human gate outranks a running turn: an approval request must surface in
  // "needs you" the moment it opens, not hide behind "in progress".
  if (isGated(g)) return 'review';
  if (g.running) return 'active';
  // Auto-run off without a running turn is a VISIBLE paused state — the boot
  // sequence pauses every previous task (自动已关) so nothing auto-runs after
  // a restart, and an auto-run disabled by repeated failures parks here too.
  // These must never fall into 'backlog', which has no visible slot: a task
  // that was running before a restart would silently vanish from the board
  // instead of showing its card with 继续.
  if (!g.autoRun) return 'paused';
  return 'backlog';
}

function fmtCountdown(ms) {
  if (ms <= 0) return '0:00';
  const sec = Math.round(ms / 1000);
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

// One-glance answer to "is this task running normally?": a colored status
// chip per card replaces the global header heartbeat readout.
function goalStatus(g) {
  if (g.archived) return { cls: 'goal-status--muted', text: t('statusArchived') };
  if (g.running) return { cls: 'goal-status--live', text: t('statusRunning') };
  if (g.userStopped || g.stopped) return { cls: 'goal-status--muted', text: t('statusPaused') };
  if (g.errorCount > 0) return { cls: 'goal-status--err', text: t('statusErroring', g.errorCount) };
  if (!g.monitoring) return { cls: 'goal-status--muted', text: t('statusUnmonitored') };
  if (!g.autoRun) return { cls: 'goal-status--muted', text: t('statusManual') };
  return { cls: 'goal-status--ok', text: t('statusAuto') };
}

function goalStatusChip(g) {
  const s = goalStatus(g);
  const chip = document.createElement('span');
  chip.className = `goal-status ${s.cls}`;
  chip.textContent = s.text;
  return chip;
}

// Progress stage: which issue the state machine is on — makes "how far from
// the PR confirmation" visible at a glance on the card and in the panel.
function goalStageText(g) {
  if (isTerminal(g)) return null; // the conclusion line covers finished tasks
  if (isGated(g)) return t('stageGated');
  if (g.running) return t('stageRunning');
  const action = String(g.last?.recommendedAction || g.last?.recommended_action || '');
  const m = action.match(/issue #(\d+)/i);
  if (m) return t('stageFixing', m[1]);
  return t('stagePlanning');
}

function showRawJson(g) {
  document.getElementById('raw-title').textContent = `${g.goalId} · ${t('raw')}`;
  document.getElementById('raw-body').textContent = g.last?.raw
    ? JSON.stringify(g.last.raw, null, 2)
    : JSON.stringify(g.last ?? {}, null, 2);
  document.getElementById('dlg-raw').showModal();
}

function renderGoal(_g) {
  renderAllGoals();
}

function goalNarration(g) {
  // Issue goals: the issue chips row IS the narration (count + per-issue
  // status chips with titles on hover). Writing the title/progress again in
  // prose would duplicate both the strip and the gate card's 背景 — human
  // views stay de-duplicated.
  if (objectiveHasIssueSignal(g.objective || '')) return '';
  return g.objective || g.lastError
    || (g.archived ? t('statusArchived') : g.last?.state || g.state || g.goalId || '');
}

// The one-line answer to "was it fixed, and why not?" for finished tasks —
// loopx terminal states mapped to plain Chinese.
function goalConclusion(g) {
  const state = String(g.last?.state || g.state || '').toLowerCase();
  if (/(^|_)merged(_|$)/.test(state)) return t('conclusionMerged');
  if (/(^|_)(cancelled|canceled)(_|$)/.test(state)) return t('conclusionCancelled');
  if (/(^|_)(closed|duplicate)(_|$)/.test(state)) return t('conclusionClosed');
  if (state.includes('no_followup')) return t('conclusionNoFollowup');
  if (/(^|_)(terminal|completed|complete|done)(_|$)/.test(state)) return t('conclusionCompleted');
  return t('conclusionFinished');
}

// Friendly display name: "<repo>#<n>" from the intake link, the bare repo
// slug, or the clone-cache folder name. The raw goalId (loopx's identity)
// always stays reachable as a tooltip, so nothing becomes unfindable.
function goalDisplayName(g) {
  const text = String(g.objective || '');
  const issue = text.match(/github\.com\/[^/\s]+\/([^/\s]+)\/(?:issues|pull)\/(\d+)/i);
  if (issue) return `${issue[1].replace(/\.git$/i, '')}#${issue[2]}`;
  const repo = text.match(/github\.com\/[^/\s]+\/([^/\s?#]+)/i);
  if (repo) return repo[1].replace(/\.git$/i, '');
  const dir = String(goalProjectDir(g.goalId) || '');
  const base = dir.split(/[\\/]/).filter(Boolean).pop() || '';
  if (base) return base;
  return String(g.goalId || '').replace(/^bfx-/, '');
}

// waiting_on values are loopx identifiers ('user', 'controller', …); translate
// the one that means the user instead of leaking raw ids into the UI. 'codex'
// is loopx's legacy label for the agent execution lane — in outer_controller
// mode that lane is BitFun's own agent, so surface it as friendly text too.
function waitingLabel(w) {
  if (!w) return null;
  const v = String(w).toLowerCase();
  if (v === 'user') return t('groupReview');
  if (v === 'codex' || v === 'agent') return t('waitingAgent');
  return String(w);
}

// loopx authors todo texts in its own words; the console frames them with a
// type label in the UI language so the "needs you" list reads clearly without
// needing an LLM to translate arbitrary gate content.
const GATE_ACTION_LABELS = [
  [/publish|external_review|reviewer|pr_|pull_request/i, '发布 / 提 PR / 外部评审'],
  [/approval|approve/i, '审批'],
  [/credential|secret|private/i, '凭据 / 私密材料'],
  [/production|deploy|release/i, '生产 / 发布操作'],
  [/submission|leaderboard|public_claim/i, '提交 / 公开宣称'],
  [/boundary/i, '边界授权'],
];
function gateActionLabel(todo) {
  const kind = String(todo?.action_kind || todo?.actionKind || todo?.task_class || todo?.taskClass || '');
  if (!kind) return null;
  for (const [re, label] of GATE_ACTION_LABELS) if (re.test(kind)) return label;
  return null;
}

// Chinese one-line explanations for the frequent loopx gate wordings; the
// exact original stays available behind 查看原文.
const GATE_TEXT_HINTS = [
  [/write access|read-?only|connected-read-only/i, 'gateExplainWrite'],
  [/approve or reject|approve|reject/i, 'gateExplainDecide'],
  [/publish|pull ?request/i, 'gateExplainPublish'],
  [/merge/i, 'gateExplainMerge'],
  [/review/i, 'gateExplainReview'],
  [/preload|electron/i, 'gateExplainPreload'],
];
function gateExplainHints(raw) {
  const text = String(raw || '');
  const out = [];
  for (const [re, key] of GATE_TEXT_HINTS) if (re.test(text) && !out.includes(key)) out.push(key);
  return out;
}
function gateExplain(raw) {
  const hints = gateExplainHints(raw);
  return hints.length ? hints[0] : null;
}

// Issue titles from the goal's agent todos: the gate text usually only names
// "#164" while the fix todo carries the full issue title (persisted).
function issueTitlesFor(g, raw) {
  const todos = Array.isArray(g.agentTodos) ? g.agentTodos : [];
  const out = [];
  for (const m of String(raw || '').matchAll(/#(\d+)\b/g)) {
    const n = m[1];
    if (out.some((t) => t.startsWith(`#${n} `))) continue;
    const hit = todos.find((td) => String(td.text || td.title || '').includes(`issue #${n}:`));
    const tm = hit && String(hit.text || hit.title || '').match(/issue #\d+:\s*(.+?)\s*\(https?:/i);
    out.push(tm ? `#${n} ${tm[1]}` : `#${n}`);
  }
  return out;
}

// ── Chinese gate summaries (host agent) ─────────────────────
// Each blocking gate item gets a 3-line Chinese summary (背景 / 已完成 /
// 需要你) generated by the host agent itself — the only reliable way to turn
// loopx's English wording into the context the user actually needs. Runs are
// hidden, tool-less, cached per goal, and never touch the goal's own session.
const summaryRuns = new Map(); // sessionId -> { goalId, todoId, buffer }

// The summary model may reason aloud before answering ("The user wants a
// concise 3-line summary… Let me draft.") — the wall of reasoning must never
// reach the human-facing card. Parse the REAL answer instead: the last
// occurrence of each labeled line (背景/已完成/需要你) in order. Extraction
// beats filtering because reasoning text is arbitrary and unfilterable.
const GATE_SUMMARY_LABELS = [/^背景[:：]/, /^已完成[:：]/, /^需要你[:：]/];
function extractGateSummary(text) {
  const lines = String(text || '').split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  const lastIndexOf = (re) => {
    for (let i = lines.length - 1; i >= 0; i -= 1) if (re.test(lines[i])) return i;
    return -1;
  };
  const idx = GATE_SUMMARY_LABELS.map(lastIndexOf);
  if (idx.every((i) => i >= 0) && idx[0] < idx[1] && idx[1] < idx[2]) {
    return [lines[idx[0]], lines[idx[1]], lines[idx[2]]].join('\n');
  }
  return null;
}

// Fallback: drop obvious self-talk lines; if that empties the text, keep the
// raw tail (last few lines) rather than nothing.
const SUMMARY_SELFTALK_RE = /^(我需要|我要|我会|我将|让我|首先|接下来|好的|那么|I need to|I will|I'm going to|Let me|First|Next|Okay|The user wants|Should I|But wait|Need to|Format|Original|Check|Each|Let's|Let me check|Could|Final)[，,.:：\s]/i;
function stripSummarySelfTalk(text) {
  const lines = String(text || '').split(/\r?\n/);
  const kept = lines.filter((line) => {
    const t = line.trim();
    if (!t) return false;
    if (SUMMARY_SELFTALK_RE.test(t)) return false;
    return true;
  });
  if (kept.length) return kept.join('\n').trim();
  const tail = lines.slice(-5).filter((line) => line.trim());
  return tail.length ? tail.join('\n').trim() : String(text || '').trim();
}

// The one gate-summary entry point: structured extraction first, filtering
// as the safety net. Never raw model output.
function cleanGateSummary(text) {
  return extractGateSummary(text) || stripSummarySelfTalk(text);
}

// ── publish-time cause/solution analysis ───────────────────
// PR bodies need "why it broke + how it was fixed". A one-shot agent run
// (no tools) reads the issue title, branch and commit subjects and answers
// with exactly two labeled lines (原因：/解决：). Runs are tracked like the
// gate summaries; a 60s cap resolves null so publish never blocks on it.
const analysisRuns = new Map(); // sessionId -> { goalId, resolve, buffer, timer }
function extractLabeledLines(text, labels) {
  const lines = String(text || '').split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  const out = {};
  for (const key of labels) {
    const re = new RegExp(`^${key}[:：]\\s*(.*)$`);
    for (let i = lines.length - 1; i >= 0; i -= 1) {
      const m = lines[i].match(re);
      if (m) { out[key] = m[1].trim() || null; break; }
    }
  }
  return labels.some((key) => out[key])
    ? { cause: out['原因'] || null, solution: out['解决'] || null }
    : null;
}

function generateCauseAnalysis(g, todo) {
  return new Promise((resolve) => {
    const finish = (value) => resolve(value);
    let timer = null;
    (async () => {
      const raw = String(todo.text || todo.title || '');
      const titles = issueTitlesFor(g, raw);
      const branch = branchHintFromText(raw);
      let subjects = [];
      let files = [];
      let stat = null;
      try {
        const gl = await app.call('loopx.gitLog', { projectDir: goalProjectDir(g.goalId), branch: branch || null });
        if (gl && gl.ok && Array.isArray(gl.subjects)) subjects = gl.subjects.slice(0, 10);
      } catch (_) {}
      try {
        const gd = await app.call('loopx.gitDiff', { projectDir: goalProjectDir(g.goalId), branch: branch || null });
        if (gd && gd.ok) { files = (gd.files || []).slice(0, 30); stat = gd.stat || null; }
      } catch (_) {}
      const prompt = [
        '根据下面的修复任务信息，用中文输出恰好两行：第一行以「原因：」开头（问题出现的根因），第二行以「解决：」开头（如何解决的——具体到改了哪些文件、每处改动解决了什么问题）。不要输出任何其他内容（不要思考过程、开场白或解释）。',
        `Issue：${titles.join('；') || raw.slice(0, 200)}`,
        `分支：${branch || '?'}`,
        files.length ? `涉及文件${stat ? `（${stat}）` : ''}：\n${files.map((f) => `- ${f}`).join('\n')}` : '',
        subjects.length ? `提交记录：\n${subjects.map((s) => `- ${s}`).join('\n')}` : '',
      ].filter(Boolean).join('\n');
      try {
        const run = await app.agent.run(prompt, {
          sessionName: `bitfun-loopx PR 分析 · ${goalDisplayName(g)}`,
          enableTools: false,
          model: S.config.defaultModel || 'auto',
        });
        timer = setTimeout(() => {
          if (analysisRuns.has(run.sessionId)) analysisRuns.delete(run.sessionId);
          finish(null);
        }, 60000);
        analysisRuns.set(run.sessionId, {
          goalId: g.goalId, resolve: finish, buffer: '', timer,
        });
      } catch (_) {
        if (timer) clearTimeout(timer);
        finish(null);
      }
    })();
  });
}

// Renders the three labeled summary lines directly on the card (no gray box):
// each line becomes its own row with the 背景/已完成/需要你 label bolded.
function appendLabeledSummary(container, text) {
  for (const rawLine of String(text || '').split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    const row = document.createElement('div');
    const m = line.match(/^(背景|已完成|需要你)[:：]\s*(.*)$/);
    if (m) {
      const label = document.createElement('strong');
      label.textContent = `${m[1]}：`;
      row.append(label, m[2] || '');
    } else {
      row.textContent = line;
    }
    container.appendChild(row);
  }
}

async function ensureGateSummary(g, todo) {
  if (!isLiveGoal(g) || !todo || !todo.todo_id) return;
  if (!g.gateSummaries) g.gateSummaries = new Map();
  // Done summaries persist across sessions (loaded at boot); loading ones are
  // in flight. Anything else (missing / failed / empty) retries so the card
  // always converges on a ready summary without user interaction.
  const existing = g.gateSummaries.get(todo.todo_id);
  if (existing && (existing.status === 'done' || existing.status === 'loading')) return;
  g.gateSummaries.set(todo.todo_id, { status: 'loading' });
  const titles = issueTitlesFor(g, todo.text || todo.title || '');
  const prompt = [
    '用中文输出恰好三行，每行不超过 60 字；第一行必须以「背景：」开头，第二行必须以「已完成：」开头，第三行必须以「需要你：」开头。不要输出任何其他内容（不要思考过程、开场白、解释或多余换行）：',
    `背景：${titles.length ? titles.join('；') : '（见原文）'}`,
    '已完成：该事项涉及的工作或改动',
    '需要你：用户现在需要做的决定或操作',
    '',
    `原文：${todo.text || todo.title || todo.todo_id}`,
  ].join('\n');
  try {
    const run = await app.agent.run(prompt, {
      sessionName: `bitfun-loopx 摘要 · ${goalDisplayName(g)}`,
      enableTools: false,
      model: S.config.defaultModel || 'auto',
    });
    summaryRuns.set(run.sessionId, {
      goalId: g.goalId, todoId: todo.todo_id, buffer: '',
      sessionId: run.sessionId, turnId: run.turnId,
    });
  } catch (err) {
    dbgUi('gateSummary:runError', String(err && err.message || err));
    if (isLiveGoal(g)) g.gateSummaries.set(todo.todo_id, { status: 'failed' });
  }
}

// Publish-scope gates (external PR creation / review request) trigger the
// console's own PR flow on approval — submitting the PR IS the default.
const PUBLISH_TODO_RE = /publish|external_review|reviewer|pr_|pull_request/i;
function isPublishTodo(todo) {
  const meta = String(todo?.action_kind || todo?.actionKind || todo?.task_class || todo?.taskClass || '');
  const text = String(todo?.title || todo?.text || '');
  // loopx writes publish gates with varying metadata: some carry
  // action_kind=external_pr_creation, others only a user_action todo whose
  // TEXT names the publish/PR ask ("推送 fix/… 分支并为 issue #N 创建 PR
  // （publish 需 owner 审批）"). Match both so the approval never surfaces
  // as a mere informational item without the publish action.
  return PUBLISH_TODO_RE.test(meta)
    || /publish|external_review|pull request|创建\s*(PR|pull)|提交\s*(PR|pull)/i.test(text);
}

// GitHub credential state for the publish guidance: the host-level gh CLI
// credential, a configured PAT, or nothing yet — drives the step-by-step
// 等你处理 guidance.
function githubCredState() {
  if (String(S.config.githubToken || '').trim()) {
    return { mode: 'token', label: t('gateCredToken', S.config.githubLogin || '?') };
  }
  if (S.ghAvailable === true) return { mode: 'gh', label: t('gateCredGh') };
  return { mode: 'none', label: t('gateCredNone') };
}

// PR identity markers — the countability contract: every PR created by this
// tool carries both keywords in its title, searchable on GitHub with
// `"bitfun-loopx" in:title`.
const PR_TITLE_PREFIX = '[bitfun-loopx] ';
const PR_BODY_MARKER = 'Created by bitfun-loopx (BitFun built-in MiniApp).';


function prTitleFor(g) {
  return `${PR_TITLE_PREFIX}${goalDisplayName(g)}`;
}

function prBodyFor(g) {
  const issueMatch = String(g.objective || '').match(/github\.com\/[^/\s]+\/[^/\s]+\/(?:issues|pull)\/(\d+)/i);
  const lines = [];
  if (issueMatch) lines.push(`Fixes #${issueMatch[1]}`);
  lines.push(PR_BODY_MARKER);
  return lines.join('\n\n');
}

// The publish gate's wording usually names the fix branch ("Push
// fix/issue-216-… to a fork…"). Pushing the named branch instead of HEAD
// keeps per-issue branches separate inside the one-repo goal.
function branchHintFromText(text) {
  const s = String(text || '');
  const pushMatch = s.match(/(?:push|branch)\s+([A-Za-z0-9][A-Za-z0-9._/-]{0,120})/i);
  if (pushMatch) {
    const token = pushMatch[1].replace(/[),.;:]+$/, '');
    if (/^(fix|feature|codex|issue|patch)[\/-]/i.test(token)) return token;
  }
  const m = s.match(/(?:fix|feature|codex|issue|patch)[\/-][A-Za-z0-9._-]{2,}/i);
  return m ? m[0].replace(/[),.;:]+$/, '') : null;
}

// Classify one user todo: BLOCKING gates (user_gate / publish scope) need a
// real decision before the task continues; everything else is informational
// (guidance instructions etc.) and only needs to be acknowledged.
function gateTodoInfo(todo) {
  const isPublish = isPublishTodo(todo);
  const isBlocking = todo.task_class === 'user_gate' || isPublish;
  const raw = todo.title || todo.text || todo.todo_id || '';
  const mapped = gateActionLabel(todo);
  const typeLabel = mapped || (isPublish ? t('gateTypePublish') : (isBlocking ? t('gateTypeApprove') : t('gateTypeInfo')));
  // Chinese-first everywhere: the card title is always a Chinese label;
  // loopx's raw wording (or the user's own instruction text) stays as a dim
  // secondary line, so no todo ever surfaces as bare English.
  const title = isBlocking
    ? t('gateItemWithType', typeLabel)
    : (mapped ? t('gateItemInfoLabel', typeLabel) : t('gateTypeInfo'));
  return { isBlocking, isPublish, typeLabel, title, raw };
}

// One gate item as a card: Chinese label + the three-line summary + credential
// state + after-approval note + the action button. No task kicker and no
// separate 背景 line — the goal card's head already names the task, and the
// three-line summary's 背景 covers the issue context (no repeated layers).
function buildGateItemCard(g, todo) {
  const info = gateTodoInfo(todo);
  const card = document.createElement('div');
  card.className = `gate-card ${info.isBlocking ? 'gate-card--block' : 'gate-card--info'}`;
  const title = document.createElement('div');
  title.className = 'gate-card__title';
  const label = document.createElement('span');
  label.className = 'gate-card__label';
  label.textContent = info.title;
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = `btn btn--tiny ${info.isBlocking ? 'btn--approve' : ''}`;
  btn.textContent = info.isPublish
    ? t('approveAndPr')
    : (info.isBlocking ? t('approveGate') : t('completeTodoBtn'));
  btn.onclick = (ev) => { ev.stopPropagation(); openApproveDialog(g, todo); };
  const copyBtn = document.createElement('button');
  copyBtn.type = 'button';
  copyBtn.className = 'btn btn--tiny';
  copyBtn.textContent = t('copy');
  copyBtn.title = t('copyCardHint');
  copyBtn.onclick = async (ev) => {
    ev.stopPropagation();
    const summary = g.gateSummaries && g.gateSummaries.get(todo.todo_id);
    const titles = issueTitlesFor(g, info.raw);
    const text = [
      t('gateCardTask', goalDisplayName(g)),
      info.title,
      titles.length ? `${t('gateBackground')}${titles.join('；')}` : '',
      summary && summary.status === 'done' && summary.text
        ? summary.text
        : gateExplainHints(info.raw).map((key) => t(key)).join('\n'),
      info.raw,
    ].filter(Boolean).join('\n');
    try {
      if (app.clipboard && app.clipboard.writeText) await app.clipboard.writeText(text);
      else await navigator.clipboard.writeText(text);
      copyBtn.textContent = '✓';
      setTimeout(() => { copyBtn.textContent = t('copy'); }, 1200);
    } catch (_) {
      // Clipboard unavailable: leave the button as-is.
    }
  };
  title.append(label, copyBtn, btn);
  card.appendChild(title);
  if (info.isBlocking) {
    // The three-line summary's 背景 line carries the issue titles — no
    // separate background block here (de-duplicated human view).
    const summary = g.gateSummaries && g.gateSummaries.get(todo.todo_id);
    if (summary && summary.status === 'done' && summary.text) {
      const sum = document.createElement('div');
      sum.className = 'gate-card__summary';
      // Plain lines on the card (no gray box); labels bolded for scannability.
      appendLabeledSummary(sum, summary.text);
      card.appendChild(sum);
    } else if (summary && summary.status === 'loading') {
      const loading = document.createElement('div');
      loading.className = 'gate-card__summary gate-card__summary--loading';
      loading.textContent = t('gateSummaryLoading');
      card.appendChild(loading);
    } else {
      // No summary yet: show the pattern-based Chinese hints immediately and
      // generate the full 背景/已完成/需要你 summary in the background.
      for (const key of gateExplainHints(info.raw)) {
        const hintEl = document.createElement('div');
        hintEl.className = 'gate-card__explain';
        hintEl.textContent = t(key);
        card.appendChild(hintEl);
      }
      ensureGateSummary(g, todo);
    }
    // Publish guidance, step by step: show the GitHub credential state and,
    // when nothing is logged in yet, a setup action right on the card.
    if (info.isPublish) {
      const cred = githubCredState();
      const credLine = document.createElement('div');
      credLine.className = `gate-card__cred gate-card__cred--${cred.mode}`;
      credLine.textContent = cred.label;
      card.appendChild(credLine);
      if (cred.mode === 'none') {
        const setup = document.createElement('button');
        setup.type = 'button';
        setup.className = 'btn btn--tiny';
        setup.textContent = t('gateCredSetup');
        setup.onclick = (ev) => { ev.stopPropagation(); openTokenDialog(); };
        card.appendChild(setup);
      }
    }
    // "用户做了什么之后会发生什么": every decision card answers what
    // happens next, so the human never approves blind.
    const after = document.createElement('div');
    after.className = 'gate-card__after';
    after.textContent = info.isPublish ? t('gateAfterPublish') : t('gateAfterApprove');
    card.appendChild(after);
  }
  // The loopx original wording is internal noise for the human template
  // (label + background + three-line summary + after-approval already cover
  // it) — no 查看原文 collapsible. Anyone who needs the exact text can copy
  // it via the 复制 button, which embeds info.raw.
  return card;
}

// Blocking first, informational second — two clearly labelled groups.
function buildGateItemsList(g) {
  const list = document.createElement('div');
  list.className = 'gate-items';
  const todos = g.userTodos || [];
  // The review column is a DECISION queue, not an inbox: only blocking items
  // (user_gate / publish scope) need a human decision. Informational user
  // todos (guidance loopx wrote for the agent/user, "用户需要 xxx" style)
  // stay internal — loopx reconciles them without the console echoing them.
  const blocking = todos.filter((td) => gateTodoInfo(td).isBlocking);
  for (const td of blocking) list.appendChild(buildGateItemCard(g, td));
  return list;
}

function activityText(line) {
  const text = String(line || '')
    .replace(/^\s*(?:\[[^\]]+\]\s*)+/, '')
    .replace(/\s+/g, ' ')
    .trim();
  return text.length > 150 ? `${text.slice(0, 147)}...` : text;
}

// Keep the stream DOM light: multi-KB reasoning blocks are the single biggest
// layout cost in the log panel. Only the newest tail renders; the full text
// stays in the persisted log (and the raw dialog), never in the DOM.
// Per-kind DOM tail windows: reasoning is auxiliary (small window), the
// model's visible output gets a more generous one.
const STREAM_DOM_CAPS = { think: 2000, agent: 6000, prompt: 4000 };
function cappedStreamText(text, cap) {
  const s = String(text || '');
  if (s.length <= cap) return s;
  return `…${t('streamTrimmed', s.length - cap)}…\n${s.slice(-cap)}`;
}

function activityDisplayText(entry) {
  return entry.count > 1 ? `${entry.line} ×${entry.count}` : entry.line;
}

function activityLineElement(entry) {
  const row = document.createElement('div');
  row.className = 'activity-stream__line'
    + (entry.isErr ? ' activity-stream__line--err' : '')
    + (entry.kind ? ` activity-stream__line--${entry.kind}` : '');
  const time = document.createElement('span');
  time.className = 'activity-stream__time';
  time.textContent = entry.time;
  row.appendChild(time);
  if (entry.kind === 'think' && entry.stream) {
    // Reasoning renders as ONE block, streamed in place and expanded by
    // default — never a wall of per-paragraph "thinking" rows. Only the
    // newest tail goes into the DOM (full text stays persisted).
    const details = document.createElement('details');
    details.className = 'activity-prompt activity-prompt--think';
    // Collapsed by default: reasoning is the agent's internal thinking — the
    // human-facing log leads with actions and output; thinking stays one
    // click away.
    details.open = false;
    const summary = document.createElement('summary');
    summary.textContent = t('thinkBlockTitle');
    const pre = document.createElement('pre');
    pre.textContent = cappedStreamText(entry.line, STREAM_DOM_CAPS.think);
    details.append(summary, pre);
    row.appendChild(details);
  } else if (entry.kind === 'prompt' && entry.raw) {
    // The instructions sent to the agent: collapsed by default, expandable.
    // Capped in the DOM so one 6KB+ prompt cannot bloat the stream.
    const details = document.createElement('details');
    details.className = 'activity-prompt';
    const summary = document.createElement('summary');
    summary.textContent = activityDisplayText(entry);
    const pre = document.createElement('pre');
    pre.textContent = cappedStreamText(entry.raw, STREAM_DOM_CAPS.prompt);
    details.append(summary, pre);
    row.appendChild(details);
  } else {
    const text = document.createElement('span');
    text.className = 'activity-stream__text';
    // Streamed model output stays capped too — the raw view keeps the whole
    // text, the DOM only needs a readable tail window.
    text.textContent = entry.stream
      ? cappedStreamText(entry.line, STREAM_DOM_CAPS.agent)
      : activityDisplayText(entry);
    row.appendChild(text);
  }
  return row;
}

function recordGoalActivity(g, line, isErr = false, kind = null, raw = null) {
  const summary = kind === 'agent' ? String(line).trim() : activityText(line);
  if (!summary) return;
  if (!Array.isArray(g.activityLines)) g.activityLines = [];
  if (typeof g.currentActivity !== 'string') g.currentActivity = '';
  const now = new Date().toTimeString().slice(0, 8);
  // Collapse back-to-back repeats (tool streams spam the same verb) into one
  // line with a multiplier instead of a wall of identical rows.
  const last = g.activityLines[g.activityLines.length - 1];
  if (last && !last.isErr && !isErr && !last.isTick && !last.kind && !kind
      && last.line === summary) {
    last.count = (last.count || 1) + 1;
    last.time = now;
    const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
    if (stream && stream.lastElementChild) {
      const textEl = stream.lastElementChild.querySelector('.activity-stream__text');
      if (textEl) textEl.textContent = activityDisplayText(last);
      const timeEl = stream.lastElementChild.querySelector('.activity-stream__time');
      if (timeEl) timeEl.textContent = now;
    }
    g.currentActivity = summary;
    scheduleLogSave();
    return;
  }
  const entry = { time: now, line: summary, isErr, count: 1, kind, raw };
  g.activityLines.push(entry);
  if (g.activityLines.length > 240) g.activityLines.splice(0, g.activityLines.length - 240);
  g.currentActivity = summary;

  const cardText = document.querySelector(`.goal__activity-text[data-goal="${CSS.escape(g.goalId)}"]`);
  if (cardText) cardText.textContent = summary;

  const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
  if (stream) {
    const follow = streamAtTail(stream);
    const emptyEl = stream.querySelector('.activity-empty');
    if (emptyEl) emptyEl.remove();
    stream.appendChild(activityLineElement(entry));
    while (stream.children.length > 240) stream.removeChild(stream.firstChild);
    if (follow) streamFollowTail(stream);
  } else {
    const panel = document.getElementById('goal-detail-panel');
    if (!panel.hidden && S.activeGoalId === g.goalId) renderGoalDetails(g);
  }
  scheduleLogSave();
}

// The once-per-10s running clock is one LINE that updates in place, not a new
// row every tick — the stream stays readable while the card text stays live.
function setGoalActivityTick(g, text) {
  const summary = activityText(text);
  if (!summary) return;
  if (!Array.isArray(g.activityLines)) g.activityLines = [];
  if (typeof g.currentActivity !== 'string') g.currentActivity = '';
  const now = new Date().toTimeString().slice(0, 8);
  g.currentActivity = summary;
  const cardText = document.querySelector(`.goal__activity-text[data-goal="${CSS.escape(g.goalId)}"]`);
  if (cardText) cardText.textContent = summary;
  const last = g.activityLines[g.activityLines.length - 1];
  const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
  if (last && last.isTick) {
    last.line = summary;
    last.time = now;
    if (stream && stream.lastElementChild) {
      const follow = streamAtTail(stream);
      const textEl = stream.lastElementChild.querySelector('.activity-stream__text');
      if (textEl) textEl.textContent = summary;
      const timeEl = stream.lastElementChild.querySelector('.activity-stream__time');
      if (timeEl) timeEl.textContent = now;
      // A tick is an in-place time refresh, not new content: follow only when
      // the user was already at the bottom.
      if (follow) streamFollowTail(stream);
    }
    return;
  }
  const entry = { time: now, line: summary, isErr: false, count: 1, isTick: true };
  g.activityLines.push(entry);
  if (g.activityLines.length > 240) g.activityLines.splice(0, g.activityLines.length - 240);
  if (stream) {
    const follow = streamAtTail(stream);
    const emptyEl = stream.querySelector('.activity-empty');
    if (emptyEl) emptyEl.remove();
    stream.appendChild(activityLineElement(entry));
    // Keep the DOM in lockstep with the array: without this cap the row
    // indices desync after 240 lines and the in-place stream patches would
    // hit older rows (the log visibly "jumping back" to old content).
    while (stream.children.length > 240) stream.removeChild(stream.firstChild);
    if (follow) streamFollowTail(stream);
  } else {
    const panel = document.getElementById('goal-detail-panel');
    if (!panel.hidden && S.activeGoalId === g.goalId) renderGoalDetails(g);
  }
  scheduleLogSave();
}

// Intake draft as a pending directory row in the 进行中 rail; its stage line
// is patched in place by the taskIntake progress events.
function buildIntakeRow(draft) {
  const el = document.createElement('div');
  el.className = 'run-item run-item--pending';
  const dot = document.createElement('span');
  dot.className = 'dot dot--active';
  const meta = document.createElement('span');
  meta.className = 'run-item__meta';
  const id = document.createElement('span');
  id.className = 'run-item__id';
  id.textContent = t('taskPendingLabel');
  const text = document.createElement('span');
  text.className = 'run-item__text';
  text.textContent = draft.objective;
  meta.append(id, text);
  const stage = document.createElement('span');
  stage.className = 'goal__activity-text';
  stage.textContent = draft.stage;
  el.append(dot, meta, stage);
  return el;
}

// The 进行中 rail is a directory: one compact row per running goal. Clicking
// a row selects it and streams its log into the panel beside the rail.
// Lifecycle buttons live on the goal cards/rows themselves: 中止/继续 +
// 删除, with stopPropagation so they never toggle the selection underneath.
// The card's 继续 covers three paused flavours: an explicit stop (restore
// heartbeat + auto-run), a stopped tick loop (fresh poll), and the
// boot-paused state (auto-run off) — re-arm and re-decide immediately.
function resumeCardTask(g) {
  if (g.userStopped) { resumeGoalTask(g); return; }
  if (g.stopped) { pollNow(g); return; }
  setAutoRun(g, true);
  pollNow(g, { force: true });
}

function buildGoalActions(g, column) {
  const box = document.createElement('span');
  box.className = 'goal-actions' + (column ? ' goal-actions--column' : '');
  const primary = (g.userStopped || g.stopped || !g.autoRun)
    ? { label: t('resumeTask'), kind: 'primary', handler: () => resumeCardTask(g) }
    : { label: t('stopTask'), kind: 'danger', handler: () => openStopConfirm(g) };
  const pb = document.createElement('button');
  pb.type = 'button';
  pb.className = `btn btn--tiny ${primary.kind === 'primary' ? 'btn--primary' : 'btn--danger'}`;
  pb.textContent = primary.label;
  pb.title = primary.kind === 'primary' ? t('resumeTaskHint') : t('stopTaskHint');
  pb.onclick = (ev) => { ev.stopPropagation(); primary.handler(); };
  box.appendChild(pb);
  const db = document.createElement('button');
  db.type = 'button';
  db.className = 'btn btn--tiny btn--danger';
  db.textContent = t('deleteTask');
  db.title = t('deleteTaskHint');
  db.onclick = (ev) => { ev.stopPropagation(); openDeleteConfirm(g); };
  box.appendChild(db);
  return box;
}

function buildRunItem(g, parked = false) {
  const el = document.createElement('div');
  el.className = 'run-item' + (parked ? ' run-item--parked' : '')
    + (S.activeGoalId === g.goalId ? ' is-selected' : '');
  el.setAttribute('aria-label', g.goalId);
  el.onclick = () => openGoalDetails(g);
  const dot = document.createElement('span');
  dot.className = parked
    ? `dot dot--${g.errorCount > 0 ? 'error' : 'paused'}`
    : 'dot dot--active';
  const meta = document.createElement('span');
  meta.className = 'run-item__meta';
  const id = document.createElement('span');
  id.className = 'run-item__id';
  id.textContent = goalDisplayName(g);
  id.title = g.goalId;
  const text = document.createElement('span');
  text.className = 'run-item__text';
  text.textContent = goalNarration(g);
  meta.append(id, text);
  // Per-issue progress on the directory row itself: the 进行中 rail is where
  // running goals live, so the issue board must be visible right here (the
  // full card variant only appears in the review column).
  const issueStrip = buildIssueStrip(g, { rail: true });
  if (issueStrip) meta.append(issueStrip);
  el.append(dot, meta, goalStatusChip(g), buildGoalActions(g, true));
  return el;
}

// v3.2: goals created by other loopx hosts on this machine. Listed in a
// collapsed section; each row offers one-click adoption (register agent +
// start heartbeat). Execution turns still require a known project directory.
async function adoptGoal(g, btn) {
  if (btn) btn.disabled = true;
  const agentId = g.agents[0] || resolveDefaultAgent();
  try {
    const res = await app.call('loopx.adoptGoal', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
      projectDir: goalProjectDir(g.goalId) || S.config.projectDir,
      goalId: g.goalId,
      agentId,
    });
    if (!res.ok) throw new Error(res.error || 'adopt failed');
    S.config.ownedGoals[g.goalId] = true;
    S.config.agentByGoal[g.goalId] = agentId;
    S.config.monitorByGoal[g.goalId] = true;
    await saveConfig();
    g.agentId = agentId;
    g.monitoring = true;
    log(`[${g.goalId}] ${t('adoptedLabel')}`);
    renderAllGoals(true);
    pollNow(g);
  } catch (err) {
    log(`[${g.goalId}] ${t('adoptFailed', err.message || err)}`, true);
    if (btn) btn.disabled = false;
    renderAllGoals(true);
  }
}

function buildOtherGoalsRows(goals) {
  const body = document.createElement('div');
  body.className = 'board-more__rows';
  const hint = document.createElement('p');
  hint.className = 'board-more__hint';
  hint.textContent = t('otherTasksHint');
  body.appendChild(hint);
  for (const g of goals) {
    const row = document.createElement('div');
    row.className = 'other-tasks__row';
    const meta = document.createElement('div');
    meta.className = 'other-tasks__meta';
    const id = document.createElement('span');
    id.className = 'other-tasks__id';
    id.textContent = goalDisplayName(g);
    id.title = g.goalId;
    const narration = document.createElement('span');
    narration.className = 'other-tasks__text';
    narration.textContent = goalNarration(g);
    narration.title = goalNarration(g);
    meta.append(id, narration);
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'btn btn--small';
    btn.textContent = t('adopt');
    btn.onclick = () => adoptGoal(g, btn);
    row.append(meta, btn);
    body.appendChild(row);
  }
  return body;
}

// ── per-goal issue tracker (card strip) ────────────────────
// Batch goals fix many issues: intake writes one agent todo per issue
// ("Fix GitHub issue #N: <title> (<url>)"), so the per-issue board is a
// projection over those todos (open / blocked / deferred / done). The strip
// lists the objective's issue URLs with status chips; single-issue goals
// derive their chip state from the goal's own group (no extra RPC needed).

function parseIssueUrls(text) {
  const raw = String(text || '').match(/https:\/\/github\.com\/[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+\/(?:issues|pull)\/\d+/gi) || [];
  const seen = new Set();
  const issues = [];
  for (const candidate of raw) {
    const url = candidate.replace(/[),.;:\]}]+$/g, '');
    if (seen.has(url)) continue;
    seen.add(url);
    const m = url.match(/\/(?:issues|pull)\/(\d+)$/);
    issues.push({ url, number: m ? Number(m[1]) : null });
  }
  return issues;
}

function issueStatusLabel(status) {
  if (status === 'done') return t('issueDone');
  if (status === 'blocked') return t('issueBlocked');
  if (status === 'deferred') return t('issueDeferred');
  if (status === 'open') return t('issueOpen');
  return t('issuePending');
}

async function refreshGoalIssues(g, force = false) {
  if (g.issuesLoading) return;
  if (!force && g.issues && Date.now() - g.issuesAt < 60000) return;
  g.issuesLoading = true;
  try {
    const res = await app.call('loopx.goalIssues', {
      argvPrefix: S.config.argvPrefix,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
    });
    g.issues = res && res.ok ? res : { issues: [], total: 0, done: 0, open: 0 };
    g.issuesAt = Date.now();
  } catch (err) {
    if (!g.issues) g.issues = { issues: [], total: 0, done: 0, open: 0 };
    dbgUi('goalIssues:error', `${g.goalId} ${String(err && (err.message || err)).slice(0, 120)}`);
  } finally {
    g.issuesLoading = false;
    renderGoal(g);
  }
}

const ISSUE_CHIP_LIMIT = 12;
const ISSUE_CHIP_LIMIT_RAIL = 6;

// The objective may carry explicit issue URLs, a bare issues-list URL, or no
// URL at all. The todo projection (loopx.goalIssues) is the authoritative
// issue list for batch goals: intake writes one agent todo per issue.
function objectiveHasIssueSignal(text) {
  const t2 = String(text || '').trim();
  return /https:\/\/github\.com\/[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+\/(?:issues|pull)(?:\/\d+)?\/?/i.test(t2)
    || /https:\/\/github\.com\/[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+\/?$/i.test(t2);
}

// opts.rail: the compact inline variant for 进行中 directory rows (fewer
// chips); opts.skipLoad: do not kick the lazy projection load (compact rows).
function buildIssueStrip(g, opts = {}) {
  if (!objectiveHasIssueSignal(g.objective || '')) return null;
  const objectiveUrls = parseIssueUrls(g.objective || '');
  const single = objectiveUrls.length === 1;
  const chipLimit = opts.rail ? ISSUE_CHIP_LIMIT_RAIL : ISSUE_CHIP_LIMIT;
  const projection = (g.issues && g.issues.issues) || [];
  const byUrl = new Map();
  for (const issue of projection) if (issue.url) byUrl.set(issue.url, issue);

  // Chip rows: explicit objective URLs first; when the objective only names
  // the issues list (batch), the projection IS the issue list. null = batch
  // projection still loading.
  let rows = null;
  if (objectiveUrls.length) {
    rows = objectiveUrls.map((u) => ({ url: u.url, number: u.number, info: byUrl.get(u.url) || null }));
    for (const issue of projection) {
      if (!objectiveUrls.some((u) => u.url === issue.url)) {
        rows.push({ url: issue.url, number: issue.number, info: issue });
      }
    }
  } else if (g.issues) {
    rows = projection.map((issue) => ({ url: issue.url, number: issue.number, info: issue }));
  }

  const strip = document.createElement('div');
  strip.className = 'goal__issues' + (opts.rail ? ' goal__issues--rail' : '');
  if (!single) {
    const head = document.createElement('span');
    head.className = 'goal__issues-head';
    if (rows) {
      const doneCount = rows.filter((r) => r.info && r.info.done).length;
      head.textContent = t('issuesProgress', doneCount, rows.length);
    } else {
      head.textContent = t('issuesProgress', '…', '…');
    }
    strip.appendChild(head);
  }

  if (rows === null) {
    const pending = document.createElement('span');
    pending.className = 'issue-chip issue-chip--pending';
    pending.textContent = '…';
    strip.appendChild(pending);
  } else {
    rows.slice(0, chipLimit).forEach((row) => {
      const status = single
        ? (goalGroup(g) === 'done' ? 'done' : 'open')
        : (row.info ? row.info.status : 'pending');
      const chip = document.createElement('a');
      chip.className = `issue-chip issue-chip--${status}`;
      chip.href = row.url;
      chip.target = '_blank';
      chip.rel = 'noreferrer';
      chip.textContent = `#${row.number}`;
      const label = row.info && row.info.title ? `#${row.number} ${row.info.title}` : `#${row.number}`;
      chip.title = `${label} · ${issueStatusLabel(status)}`;
      // The chip is a link, not a card/row activation.
      chip.onclick = (ev) => ev.stopPropagation();
      strip.appendChild(chip);
    });
    if (rows.length > chipLimit) {
      const more = document.createElement('span');
      more.className = 'issue-chip issue-chip--more';
      const rest = rows.length - chipLimit;
      more.textContent = t('moreIssues', rest);
      more.title = `${rest} more`;
      strip.appendChild(more);
    }
  }

  // Batch goals lazy-load their todo projection, refreshing at most once per
  // 60s window per goal (board re-renders kick expired caches naturally).
  if (!opts.skipLoad && !single && !g.issuesLoading && (!g.issues || Date.now() - g.issuesAt >= 60000)) {
    refreshGoalIssues(g);
  }
  return strip;
}

// A titled card section: label + dashed divider, so each human-facing part
// (target / decision / progress) reads as its own block instead of one wall
// of text.
function buildGoalSection(labelText, extraClass = '') {
  const sec = document.createElement('div');
  sec.className = `goal__section${extraClass ? ` ${extraClass}` : ''}`;
  const label = document.createElement('div');
  label.className = 'goal__section-label';
  label.textContent = labelText;
  sec.appendChild(label);
  return sec;
}

function buildGoalCard(g, compact = false) {
  const group = goalGroup(g);
  const el = document.createElement('div');
  el.className = 'goal' + (compact ? ' goal--terminal' : '')
    + (group === 'review' ? ' goal--gated' : '')
    + (S.activeGoalId === g.goalId ? ' is-selected' : '');
  el.id = `goal-${g.goalId}`;
  el.setAttribute('role', 'button');
  el.setAttribute('aria-label', g.goalId);
  // Archived cards are restore-only: no detail panel, no run controls.
  if (!g.archived) el.onclick = () => openGoalDetails(g);

  const head = document.createElement('div');
  head.className = 'goal__head';
  const dot = document.createElement('span');
  dot.className = `dot dot--${group}`;
  head.appendChild(dot);
  const id = document.createElement('span');
  id.className = 'goal__id';
  id.textContent = goalDisplayName(g);
  id.title = g.goalId;
  head.appendChild(id);
  if (group === 'review') {
    const badge = document.createElement('span');
    badge.className = 'goal__gate-badge';
    // Real count once loaded; no phantom "1" while the todo list is pending.
    // Only BLOCKING items count as pending decisions; informational todos
    // stay visible only as a hover hint (they never render as cards).
    const todos = Array.isArray(g.userTodos) ? g.userTodos : [];
    const blockingN = todos.filter((td) => gateTodoInfo(td).isBlocking).length;
    badge.textContent = blockingN > 0 ? t('gateCount', blockingN) : t('groupReview');
    if (todos.length > blockingN) {
      badge.title = `${blockingN} ${t('gateGroupBlocking')} · ${todos.length - blockingN} ${t('gateGroupInfo')}`;
    }
    head.appendChild(badge);
  }
  head.appendChild(goalStatusChip(g));
  el.appendChild(head);

  if (!compact) {
    // ── Sectioned card: every human-facing part gets its own titled block
    // instead of one wall of text. ──

    // ① 修复目标：issue 胶囊行即叙述（计数+状态，标题在悬停）——无重复文字
    {
      const sec = buildGoalSection(t('sectionTarget'));
      const narrationText = goalNarration(g);
      if (narrationText) {
        const narration = document.createElement('div');
        narration.className = 'goal__reason' + (g.lastError ? ' goal__reason--err' : '');
        narration.textContent = narrationText;
        narration.title = narrationText; // full text on hover
        sec.appendChild(narration);
      }
      const issueStrip = buildIssueStrip(g);
      if (issueStrip) sec.appendChild(issueStrip);
      el.appendChild(sec);
    }

    // ② 需要你决定：阻塞审批项（等你处理列的主内容）
    if (group === 'review') {
      const sec = buildGoalSection(t('sectionDecision'), 'goal__section--decision');
      if (Array.isArray(g.userTodos) && g.userTodos.length) {
        sec.appendChild(buildGateItemsList(g));
      } else if (Array.isArray(g.userTodos)) {
        // A gate with no approvable item would otherwise be a dead end — say
        // so and offer the one action that can move it: run a turn.
        const none = document.createElement('div');
        none.className = 'goal__gate-none';
        const gateWait = (g.last && g.last.ok !== false ? g.last.waitingOn : g.waitingOn) || null;
        none.textContent = gateWait
          ? (waitingLabel(gateWait) === t('groupReview') ? t('groupReview') : t('waitingOn', waitingLabel(gateWait)))
          : t('gateEmptyHint');
        sec.appendChild(none);
        if (!g.running) {
          const runNext = document.createElement('button');
          runNext.type = 'button';
          runNext.className = 'btn btn--tiny btn--primary';
          runNext.textContent = t('runOnce');
          runNext.onclick = (ev) => { ev.stopPropagation(); executeRunOnce(g); };
          sec.appendChild(runNext);
        }
      }
      el.appendChild(sec);
    }

    // ③ 进度 / 结果：live stage line（规划中 / 修复中 / 待发布）或结论
    if (group !== 'review' && group !== 'archived') {
      const sec = buildGoalSection(group === 'done' ? t('sectionResult') : t('sectionProgress'));
      if (group === 'done') {
        const conclusion = document.createElement('div');
        conclusion.className = 'goal__conclusion';
        conclusion.textContent = goalConclusion(g);
        sec.appendChild(conclusion);
      } else {
        const stage = document.createElement('div');
        stage.className = 'goal__stage';
        stage.textContent = goalStageText(g);
        sec.appendChild(stage);
      }
      el.appendChild(sec);
    }

    // ④ 已归档：说明 + 恢复
    if (group === 'archived') {
      const hint = document.createElement('div');
      hint.className = 'goal__conclusion';
      hint.textContent = t('archivedHint');
      el.appendChild(hint);
      const restore = document.createElement('button');
      restore.type = 'button';
      restore.className = 'btn btn--tiny btn--primary';
      restore.textContent = t('restoreTask');
      restore.title = t('restoreTaskHint');
      restore.onclick = (ev) => { ev.stopPropagation(); restoreArchivedGoal(g, restore); };
      el.appendChild(restore);
    }
  } else {
    // Terminal capsule (compact): flat and terse, no section chrome.
    const narration = document.createElement('div');
    narration.className = 'goal__reason' + (g.lastError ? ' goal__reason--err' : '');
    narration.textContent = goalNarration(g);
    narration.title = goalNarration(g);
    el.appendChild(narration);
    if (group === 'done') {
      const conclusion = document.createElement('div');
      conclusion.className = 'goal__conclusion';
      conclusion.textContent = goalConclusion(g);
      el.appendChild(conclusion);
    }
    if (group === 'archived') {
      const hint = document.createElement('div');
      hint.className = 'goal__conclusion';
      hint.textContent = t('archivedHint');
      el.appendChild(hint);
      const restore = document.createElement('button');
      restore.type = 'button';
      restore.className = 'btn btn--tiny btn--primary';
      restore.textContent = t('restoreTask');
      restore.title = t('restoreTaskHint');
      restore.onclick = (ev) => { ev.stopPropagation(); restoreArchivedGoal(g, restore); };
      el.appendChild(restore);
    }
  }

  if (g.running || g.currentActivity) {
    const activity = document.createElement('div');
    activity.className = 'goal__activity' + (g.running ? ' goal__activity--live' : '');
    const pulse = document.createElement('span');
    pulse.className = 'goal__activity-dot';
    const text = document.createElement('span');
    text.className = 'goal__activity-text';
    text.dataset.goal = g.goalId;
    text.textContent = g.currentActivity || t('activityStarting');
    activity.append(pulse, text);
    el.appendChild(activity);
  }

  const meta = document.createElement('div');
  meta.className = 'goal__meta';
  if (g.agentId) {
    const agent = document.createElement('span');
    agent.textContent = g.agentId;
    meta.appendChild(agent);
  }
  if (meta.children.length) el.appendChild(meta);
  // Lifecycle actions on the card itself — more direct than a hidden panel.
  // Archived cards get the dedicated 恢复 button instead.
  if (group !== 'archived') el.appendChild(buildGoalActions(g, false));
  return el;
}

function renderGoalDetails(g) {
  const panel = document.getElementById('goal-detail-panel');
  if (panel.hidden || S.activeGoalId !== g.goalId) return;
  if (!Array.isArray(g.activityLines)) g.activityLines = [];
  if (typeof g.currentActivity !== 'string') g.currentActivity = '';
  const active = document.activeElement;
  if (active && panel.contains(active) && (active.tagName === 'INPUT' || active.tagName === 'SELECT')) return;

  const group = goalGroup(g);
  document.getElementById('goal-detail-kicker').textContent = t(GROUP_I18N_KEY[group]);
  const detailTitle = document.getElementById('goal-detail-title');
  detailTitle.textContent = goalDisplayName(g);
  detailTitle.title = g.goalId;
  const conclusionEl = document.getElementById('goal-detail-conclusion');
  if (conclusionEl) {
    conclusionEl.textContent = isTerminal(g) ? goalConclusion(g) : '';
    conclusionEl.hidden = !isTerminal(g);
  }
  const stageEl = document.getElementById('goal-detail-stage');
  if (stageEl) {
    const stageText = goalStageText(g);
    stageEl.textContent = stageText || '';
    stageEl.hidden = !stageText;
  }
  const body = document.getElementById('goal-detail-body');

  // The panel is the LOG and nothing else: gate items already live on the
  // review column cards (with their action buttons), so repeating them here
  // would just duplicate what the user is acting on. The stream is appended
  // to incrementally while visible (recordGoalActivity / tick /
  // upsertGoalStream); rebuilding it on every state change reset the scroll
  // position — rebuild only when the panel was closed, the goal switched, or
  // the run reset its lines.
  let stream = body.querySelector('.activity-stream');
  const fresh = !stream || stream.dataset.goal !== g.goalId
    || stream.children.length > g.activityLines.length;

  if (fresh) {
    body.replaceChildren();
    stream = document.createElement('div');
    stream.className = 'activity-stream activity-stream--panel';
    stream.dataset.goal = g.goalId;
    if (g.activityLines.length > 0) {
      for (const entry of g.activityLines) stream.appendChild(activityLineElement(entry));
    } else {
      const empty = document.createElement('div');
      empty.className = 'activity-empty';
      empty.textContent = g.running ? t('activityStarting') : t('activityEmpty');
      stream.appendChild(empty);
    }
    body.appendChild(stream);
    // Land on the latest SYNCHRONOUSLY so no frame paints at the top, then
    // settle once more after the next frame for any late layout shifts.
    body.scrollTop = body.scrollHeight;
    updateLogBottomBtn();
    requestAnimationFrame(() => {
      for (const pre of stream.querySelectorAll('.activity-prompt--think pre')) {
        pre.scrollTop = pre.scrollHeight;
      }
      body.scrollTop = body.scrollHeight;
      updateLogBottomBtn();
    });
    return;
  }

  // In-sync stream: append only the missing lines.
  const follow = streamAtTail(stream);
  while (stream.children.length < g.activityLines.length) {
    const row = activityLineElement(g.activityLines[stream.children.length]);
    stream.appendChild(row);
    const pre = row.querySelector('.activity-prompt--think pre');
    if (pre) pre.scrollTop = pre.scrollHeight;
  }
  if (follow) streamFollowTail(stream);
}

// The panel's scroll container is the body, not the stream: the stream grows
// with its content (block flow), so scrollHeight lives on its parent.
function streamScroller(stream) {
  return stream.closest('.detail-panel__body') || stream;
}

// The overflow can live on the body OR the stream itself; whichever one
// actually overflows is the element the user scrolls.
function streamTailTarget(stream) {
  const sc = streamScroller(stream);
  return stream.scrollHeight > stream.clientHeight + 2 ? stream : sc;
}

// "Pinned to the tail" must be measured BEFORE the content mutates: after an
// append the new row's own height would otherwise masquerade as distance
// from the bottom and break the decision. The 8px window makes "at the
// bottom" exact — any real scroll-up disables the follow.
function streamAtTail(stream) {
  const target = streamTailTarget(stream);
  return target.scrollHeight - target.scrollTop - target.clientHeight < 8;
}

let lastScrollTraceAt = 0;
// Chat-style follow: the log pins to the latest line ONLY when the user was
// at the bottom before the new content arrived. A reader scrolled up into
// history stays put — no yanking, no up/down bouncing. The throttled trace
// lands in debug-ui.log so a silent failure can be diagnosed from real values.
function streamFollowTail(stream) {
  const sc = streamScroller(stream);
  const target = streamTailTarget(stream);
  const now = Date.now();
  if (now - lastScrollTraceAt > 5000) {
    lastScrollTraceAt = now;
    dbgUi('scrollTail',
      `body sh=${sc.scrollHeight} ch=${sc.clientHeight} top=${sc.scrollTop} `
      + `stream sh=${stream.scrollHeight} ch=${stream.clientHeight} top=${stream.scrollTop} `
      + `target=${target === stream ? 'stream' : 'body'}`);
  }
  target.scrollTop = target.scrollHeight;
  updateLogBottomBtn();
}

// Gate approval confirmation: full todo text + optional note, one deliberate
// click. The dialog is the only writer of todo complete from the UI.
// Publish-scope gates default to the console's PR flow (fork → push → PR).
function openApproveDialog(g, todo) {
  const dlg = document.getElementById('dlg-approve');
  const isGate = todo.task_class === 'user_gate';
  const isPublish = isPublishTodo(todo);
  const tokenOk = Boolean(String(S.config.githubToken || '').trim());
  const raw = todo.text || todo.title || todo.todo_id;
  const hint = gateActionLabel(todo);
  const approveText = document.getElementById('approve-text');
  approveText.replaceChildren();
  const lead = document.createElement('div');
  lead.textContent = isPublish ? t('approvePrHint') : (isGate ? t('approveGateHint') : t('todoDoneHint'));
  approveText.appendChild(lead);
  if (isPublish && !tokenOk) {
    const warn = document.createElement('div');
    warn.textContent = t('approvePrNeedToken');
    approveText.appendChild(warn);
  }
  const explainKey = (isGate || isPublish) ? gateExplain(raw) : null;
  if (explainKey) {
    const explainEl = document.createElement('div');
    explainEl.className = 'approve-text__explain';
    explainEl.textContent = t(explainKey);
    approveText.appendChild(explainEl);
  }
  if (isGate || isPublish) {
    const summary = g.gateSummaries && g.gateSummaries.get(todo.todo_id);
    if (summary && summary.status === 'done' && summary.text) {
      const sum = document.createElement('div');
      sum.className = 'approve-text__summary';
      appendLabeledSummary(sum, summary.text);
      approveText.appendChild(sum);
    } else if (!summary || summary.status !== 'done') {
      ensureGateSummary(g, todo);
    }
  }
  const typeLine = document.createElement('div');
  typeLine.textContent = hint ? t('gateItemWithType', hint) : t('gateItemTitle');
  approveText.appendChild(typeLine);
  // The decision dialog shows the todo's original wording inline (it IS the
  // decision subject) — no 查看原文 collapsible for a single paragraph.
  const rawEl = document.createElement('div');
  rawEl.className = 'gate-card__raw';
  rawEl.textContent = raw;
  approveText.appendChild(rawEl);
  dlg.querySelector('h2').textContent = isPublish
    ? t('approvePrTitle')
    : (isGate ? t('approveGateTitle') : t('todoDoneTitle'));
  dlg.querySelector('button[value="approve"]').textContent = isPublish
    ? t('approveAndPr')
    : (isGate ? t('approveConfirm') : t('todoDoneConfirm'));
  // Publish gates let the human CHOOSE: submit the PR (default) or approve
  // without a PR — the choice is part of the guided flow, not a setting.
  const approveOnlyBtn = dlg.querySelector('#btn-approve-only');
  if (isPublish) {
    approveOnlyBtn.hidden = false;
    approveOnlyBtn.textContent = t('approveOnly');
    approveOnlyBtn.onclick = () => {
      dlg.returnValue = 'approve-only';
      dlg.close('approve-only');
    };
  } else {
    approveOnlyBtn.hidden = true;
  }
  const noteInput = document.getElementById('approve-note');
  noteInput.value = '';
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {
    if (dlg.returnValue === 'approve') approveTodo(g, todo, noteInput.value.trim(), null, { publish: true });
    else if (dlg.returnValue === 'approve-only') approveTodo(g, todo, noteInput.value.trim(), null, { publish: false });
  };
  dlg.showModal();
}

// ── GitHub token settings ──────────────────────────────────
// The publish flow (fork → push → PR) authenticates with a fine-grained PAT.
// Saved through the regular config storage; nothing leaves the machine.
function openTokenDialog() {
  const dlg = document.getElementById('dlg-token');
  document.getElementById('token-input').value = S.config.githubToken || '';
  document.getElementById('token-status').textContent = `${t('githubTokenStatus')}${
    S.config.githubLogin || (S.config.githubToken ? t('githubTokenSet') : t('githubTokenMissing'))}`;
  dlg.showModal();
}

async function saveGitHubToken() {
  const input = document.getElementById('token-input');
  const status = document.getElementById('token-status');
  const btn = document.getElementById('btn-token-save');
  const token = String(input.value || '').trim();
  if (!token) {
    status.textContent = `${t('githubTokenStatus')}${t('githubTokenMissing')}`;
    return;
  }
  btn.disabled = true;
  try {
    const res = await app.call('loopx.githubUser', { token });
    if (!res.ok || !res.login) throw new Error(res.error || t('githubTokenInvalid'));
    S.config.githubToken = token;
    S.config.githubLogin = res.login;
    await saveConfig();
    status.textContent = t('githubTokenSaved', res.login);
    log(`GitHub token saved (login=${res.login})`);
    document.getElementById('dlg-token').close();
    // Gate cards show the credential state inline ("尚未登录 / 已配置
    // Token") — refresh the board so every card flips immediately; paused
    // goals don't poll, so without this they would stay stale.
    requestRender(true);
  } catch (err) {
    status.textContent = `${t('githubTokenInvalid')}：${String(err && err.message || err)}`;
  } finally {
    btn.disabled = false;
  }
}

async function clearGitHubToken() {
  S.config.githubToken = '';
  S.config.githubLogin = '';
  await saveConfig();
  document.getElementById('token-input').value = '';
  document.getElementById('token-status').textContent = `${t('githubTokenStatus')}${t('githubTokenMissing')}`;
  requestRender(true); // gate cards must reflect the cleared credential
}

// One-click GitHub CLI login: the worker installs gh when missing (winget +
// system proxy), launches `gh auth login --web` (console window shows the
// one-time code, the browser completes the flow) and polls until done.
function appendGhLoginProgress(d) {
  const el = document.getElementById('gh-login-progress');
  if (!el) return;
  el.hidden = false;
  el.textContent += `${d && d.line ? d.line : ''}\n`;
  el.scrollTop = el.scrollHeight;
}

async function runGhLogin() {
  const btn = document.getElementById('btn-gh-login');
  const progress = document.getElementById('gh-login-progress');
  const status = document.getElementById('token-status');
  btn.disabled = true;
  progress.hidden = false;
  progress.textContent = '';
  try {
    const res = await app.call('loopx.ghLogin', {});
    if (!res.ok) throw new Error(res.error || 'gh login failed');
    S.ghAvailable = true;
    status.textContent = t('ghLoginDone', res.login || 'gh');
    log(`[gh] login complete (${res.login || '?'})`);
    requestRender(true); // gate cards flip to the gh credential state
    document.getElementById('dlg-token').close();
  } catch (err) {
    const message = String(err && err.message || err);
    status.textContent = `${t('ghLoginFailed')}：${message}`;
    progress.textContent += `\n${message}\n`;
    log(`[gh] login failed: ${message}`, true);
  } finally {
    btn.disabled = false;
  }
}

// ── one-click loopx state reset ─────────────────────────────
// Historical goals (pre one-repo-one-goal) pollute the board and dropdown;
// this wipes every loopx data location with a timestamped backup so the
// console starts from a clean slate.
function openResetConfirm() {
  const dlg = document.getElementById('dlg-stop');
  document.getElementById('stop-title').textContent = t('resetLoopxTitle');
  document.getElementById('stop-text').textContent = t('resetLoopxText');
  dlg.querySelector('button[value="confirm"]').textContent = t('resetLoopxConfirm');
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {
    if (dlg.returnValue !== 'confirm') return;
    resetLoopxState();
  };
  dlg.showModal();
}

async function resetLoopxState() {
  setComposerBusy(true, t('resetLoopxWorking'));
  try {
    const res = await app.call('loopx.resetAll', {
      projectDirs: [S.config.projectDir, ...Object.values(S.config.projectByGoal || {})].filter(Boolean),
    });
    if (!res.ok) throw new Error(res.error || 'reset failed');
    // Clear every per-goal UI binding and the persisted logs.
    for (const key of ['ownedGoals', 'monitorByGoal', 'agentByGoal', 'autoRunByGoal', 'modelByGoal', 'projectByGoal', 'stoppedByGoal', 'autoRunBeforeStop', 'agentSessionByGoal']) {
      S.config[key] = {};
    }
    S.persistedLogs = {};
    await saveConfig();
    try { await app.storage.set('logs', {}); } catch (_) {}
    S.goals.clear();
    S.agentSessionByGoal.clear();
    S.activeGoalId = null;
    document.getElementById('goal-detail-panel').hidden = true;
    document.getElementById('detail-empty').hidden = false;
    await refreshGoals();
    renderAllGoals(true);
    setComposerBusy(false, '');
    setTaskFeedback(t('resetLoopxDone', res.backupDir || ''), 'ok');
    log(`loopx state reset (backup: ${res.backupDir || '?'})`);
  } catch (err) {
    const message = String(err && err.message || err);
    setComposerBusy(false, '');
    setTaskFeedback(`${t('resetLoopxFailed')}: ${message}`, 'error');
    log(`loopx state reset failed: ${message}`, true);
  }
}

// Stopping is a deliberate, whole-task action: explain what it does before
// doing it, so the task never "vanishes" as a surprise.
function openStopConfirm(g) {
  const dlg = document.getElementById('dlg-stop');
  document.getElementById('stop-title').textContent = t('stopConfirmTitle');
  document.getElementById('stop-text').textContent = t('stopConfirmText', goalDisplayName(g));
  dlg.querySelector('button[value="confirm"]').textContent = t('confirmStop');
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {
    if (dlg.returnValue !== 'confirm') return;
    stopGoalTask(g);
  };
  dlg.showModal();
}

function openDeleteConfirm(g) {
  const dlg = document.getElementById('dlg-stop');
  document.getElementById('stop-title').textContent = t('deleteConfirmTitle');
  document.getElementById('stop-text').textContent = t('deleteConfirmText', goalDisplayName(g));
  dlg.querySelector('button[value="confirm"]').textContent = t('confirmDelete');
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {
    if (dlg.returnValue !== 'confirm') return;
    deleteGoalTask(g);
  };
  dlg.showModal();
}

// Delete a task: archive its runtime and drop it from the registry (the
// worker keeps a backup of the registry file). Irreversible from the board.
async function deleteGoalTask(g) {
  const name = goalDisplayName(g);
  try {
    const res = await app.call('loopx.deleteGoal', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
    });
    if (!res.ok) throw new Error(res.error || 'delete failed');
    log(`[${g.goalId}] ${t('taskDeleted', name)}`);
    if (res.warning) log(`[${g.goalId}] ${res.warning}`, true);
    for (const map of [
      S.config.ownedGoals, S.config.monitorByGoal, S.config.agentByGoal,
      S.config.autoRunByGoal, S.config.modelByGoal, S.config.projectByGoal,
      S.config.stoppedByGoal, S.config.autoRunBeforeStop,
      S.config.agentSessionByGoal,
    ]) {
      if (map) delete map[g.goalId];
    }
    await saveConfig();
    S.activeGoalId = null;
    document.getElementById('goal-detail-panel').hidden = true;
    await refreshGoals();
    saveLogs(); // the removed goal's persisted log drops out of the snapshot
    renderAllGoals(true);
    // Visible confirmation — a silent success reads as "nothing happened".
    setTaskFeedback(t('taskDeleted', name), 'ok');
  } catch (err) {
    const message = String(err && err.message || err);
    log(`[${g.goalId}] delete failed: ${message}`, true);
    recordGoalActivity(g, `${t('deleteTaskFailed')}: ${message}`, true);
    // Failures must be visible even when the panel shows another goal.
    setTaskFeedback(`${t('deleteTaskFailed')}: ${message}`, 'error');
    openAlertDialog(t('deleteTaskFailed'), `${name}：${message}`);
  }
}

// Restore an archived task: rebuild its registry entry and move its runtime
// back, then refresh — the card returns to the board paused (自动已关).
async function restoreArchivedGoal(g, button) {
  if (g.restoring) return;
  g.restoring = true;
  if (button) button.disabled = true;
  const name = goalDisplayName(g);
  try {
    const res = await app.call('loopx.restoreGoal', {
      argvPrefix: S.config.argvPrefix,
      projectDir: goalProjectDir(g.goalId) || S.config.projectDir,
      goalId: g.goalId,
      archiveDir: g.archiveDir || null,
    });
    if (!res.ok) throw new Error(res.error || 'restore failed');
    log(`[${g.goalId}] ${t('restoreDone', name)}`);
    setTaskFeedback(t('restoreDone', name), 'ok');
    await refreshGoals();
  } catch (err) {
    const message = String(err?.message || err);
    log(`[${g.goalId}] restore failed: ${message}`, true);
    setTaskFeedback(`${t('restoreFailed')}: ${message}`, 'error');
    openAlertDialog(t('restoreFailed'), `${name}：${message}`);
  } finally {
    g.restoring = false;
    renderAllGoals(true);
  }
}

// Reusable single-confirm alert (dlg-stop with a neutral confirm label).
function openAlertDialog(title, text) {
  const dlg = document.getElementById('dlg-stop');
  document.getElementById('stop-title').textContent = title;
  document.getElementById('stop-text').textContent = text;
  dlg.querySelector('button[value="confirm"]').textContent = t('close');
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {};
  dlg.showModal();
}

function openGoalDetails(g) {
  S.activeGoalId = g.goalId;
  document.getElementById('goal-detail-panel').hidden = false;
  document.getElementById('detail-empty').hidden = true;
  renderGoalDetails(g);
  renderAllGoals(true); // mark the selected card
}

document.getElementById('btn-close-goal').addEventListener('click', () => {
  S.activeGoalId = null;
  document.getElementById('goal-detail-panel').hidden = true;
  renderAllGoals(true);
});

// Fingerprint of everything the goal list displays except per-second
// countdown text (the countdown loop patches those spans in place).
function displayFingerprint() {
  const parts = [
    String(S.goals.size), app.locale,
    S.bootLoading ? 'loading' : 'ready',
    S.intakeDraft ? `${S.intakeDraft.objective}|${S.intakeDraft.stage}` : '',
  ];
  for (const g of S.goals.values()) {
    parts.push([
      g.goalId, goalGroup(g), g.polling, g.running, g.stopped, g.monitoring,
      g.autoRun, g.autoFailCount,
      g.errorCount, g.unchangedCount, g.intervalMin.toFixed(2),
      g.agents.join(','), g.agentId,
      g.objective ?? '',
      g.last ? decisionKey(g.last) : '',
      g.last?.reason ?? '', g.last?.recommendedAction ?? '',
      g.last?.state ?? g.state ?? '', g.last?.waitingOn ?? g.waitingOn ?? '',
      g.lastError ?? '',
      g.userTodos ? `${g.userTodos.length}|${g.userTodos.map((td) => td.todo_id).join(',')}` : '-',
      g.lastRun ? `${g.lastRun.exitCode}|${g.lastRun.cancelled}|${g.lastRun.durationMs}` : '',
    ].join(''));
  }
  return parts.join('');
}

let lastFingerprint = '';
let lastMoreFingerprint = '';

// Rapid bursts of state changes (task-intake progress, goal refresh, first
// poll) would each rebuild the whole board; coalesce them into one repaint
// per animation frame so the board doesn't flash through intermediate DOMs.
let renderQueued = false;
let renderQueuedForce = false;
function requestRender(force = false) {
  if (force) renderQueuedForce = true;
  if (renderQueued) return;
  renderQueued = true;
  requestAnimationFrame(() => {
    renderQueued = false;
    const flushForce = renderQueuedForce;
    renderQueuedForce = false;
    renderAllGoals(flushForce);
  });
}

// A render skipped because an input/select had focus must run once focus
// leaves the control — otherwise that repaint is silently dropped forever.
document.addEventListener('focusout', (e) => {
  if (!S.renderPending) return;
  const el = e.target;
  if (el && (el.tagName === 'INPUT' || el.tagName === 'SELECT')) {
    S.renderPending = false;
    requestRender(false);
  }
});

function renderAllGoals(force = false) {
  if (BOOT_RENDER_COUNT < 12) {
    BOOT_RENDER_COUNT += 1;
    dbgUi('render', `#${BOOT_RENDER_COUNT} t=${bootMs()}ms force=${force} theme=${themeProbe()}`);
  }
  try {
  const workspace = document.getElementById('workspace-root');
  const active = document.activeElement;
  if (!force && active && workspace.contains(active)
      && (active.tagName === 'INPUT' || active.tagName === 'SELECT')) {
    // Never yank the DOM out from under the user's cursor; re-render on blur.
    S.renderPending = true;
    return;
  }
  const fp = displayFingerprint();
  if (!force && fp === lastFingerprint) return;
  lastFingerprint = fp;

  // v3.2: the board shows only goals this console owns; other-host goals are
  // listed separately and stay unmonitored until adopted.
  const owned = [];
  const other = [];
  for (const g of S.goals.values()) (isOwnedGoal(g.goalId) ? owned : other).push(g);
  // Every key goalGroup() can return must exist here: a missing key made
  // buckets.get() return undefined, which crashed the parked spread (and
  // silently killed every render before the refill/panel steps ran).
  const buckets = new Map(
    [...PRIMARY_GROUPS, 'backlog', 'paused', 'error', ...ARCHIVE_GROUPS].map((k) => [k, []]),
  );
  for (const g of owned) buckets.get(goalGroup(g)).push(g);

  // ── 等你处理: a standalone decision column ────────────────
  const reviewGoals = buckets.get('review');
  document.getElementById('review-zone-title').textContent = t(GROUP_I18N_KEY.review);
  document.getElementById('review-zone-count').textContent = String(reviewGoals.length);
  document.getElementById('review-zone-sub').textContent = t(GROUP_SUB_KEY.review);
  const reviewList = document.getElementById('review-list');
  reviewList.replaceChildren();
  if (reviewGoals.length === 0) {
    const none = document.createElement('div');
    none.className = 'zone-empty' + (S.bootLoading ? ' zone-empty--loading' : '');
    none.textContent = S.bootLoading ? t('loadingGoals') : t('colEmpty');
    reviewList.appendChild(none);
  }
  for (const g of reviewGoals) reviewList.appendChild(buildGoalCard(g));

  // ── 进行中 + log panel: ONE unit; the rail is its directory ──
  const activeGoals = buckets.get('active');
  const pendingCount = S.intakeDraft ? 1 : 0;
  document.getElementById('active-zone-title').textContent = t(GROUP_I18N_KEY.active);
  document.getElementById('active-zone-count').textContent = String(activeGoals.length + pendingCount);
  document.getElementById('active-zone-sub').textContent = t(GROUP_SUB_KEY.active);
  const activeList = document.getElementById('active-list');
  activeList.replaceChildren();
  if (S.intakeDraft) activeList.appendChild(buildIntakeRow(S.intakeDraft));
  if (activeGoals.length === 0 && pendingCount === 0) {
    const none = document.createElement('div');
    none.className = 'zone-empty' + (S.bootLoading ? ' zone-empty--loading' : '');
    none.textContent = S.bootLoading ? t('loadingGoals') : t('colEmpty');
    activeList.appendChild(none);
  }
  for (const g of activeGoals) activeList.appendChild(buildRunItem(g));
  // Parked tasks (aborted / errored) stay in the directory rail, dimmed,
  // so they never "vanish" into an unnoticed corner chip.
  const parked = [...buckets.get('paused'), ...buckets.get('error')];
  if (parked.length > 0) {
    const divider = document.createElement('div');
    divider.className = 'run-rail__divider';
    divider.textContent = `${t('groupPaused')} · ${parked.length}`;
    activeList.appendChild(divider);
    for (const g of parked) activeList.appendChild(buildRunItem(g, true));
  }

  // Terminal goals and other-host goals hide behind quiet chips at the
  // bottom of the review zone.
  const moreGroups = [];
  for (const key of ARCHIVE_GROUPS) {
    if (buckets.get(key).length > 0) moreGroups.push({ key, goals: buckets.get(key) });
  }
  if (other.length > 0) moreGroups.push({ key: 'other', goals: other });
  const moreArea = document.getElementById('more-area');
  // Heartbeat polls flip g.polling every ~1-2s, which changes the display
  // fingerprint and rebuilds the whole board — including an EXPANDED archived
  // panel, resetting its scroll and destroying hover/click targets mid-use
  // ("点开归档后 UI 乱掉"). Rebuild the more footer only when its content
  // actually changed; force renders (chip toggle) still rebuild.
  const moreSig = moreGroups
    .map((mg) => `${mg.key}:${mg.goals.map((goal) => goal.goalId).join(',')}`)
    .join('|');
  if (force || moreSig !== lastMoreFingerprint) {
    moreArea.replaceChildren();
    if (moreGroups.length > 0) moreArea.appendChild(buildMoreFooter(moreGroups));
    lastMoreFingerprint = moreSig;
  }

  // Master-detail: the selected goal's panel rides inside the run unit.
  const panel = document.getElementById('goal-detail-panel');
  const emptyHint = document.getElementById('detail-empty');
  if (S.activeGoalId) {
    const activeGoal = S.goals.get(S.activeGoalId);
    if (activeGoal) {
      panel.hidden = false;
      emptyHint.hidden = true;
      renderGoalDetails(activeGoal);
    } else {
      S.activeGoalId = null;
      panel.hidden = true;
      emptyHint.hidden = false;
    }
  } else {
    panel.hidden = true;
    emptyHint.hidden = false;
  }
  refillComposerTarget();
  dbgUi('renderDone', `targetOpts=${document.getElementById('composer-target-menu')?.children.length ?? 'n/a'}`);
  } catch (err) {
    dbgUi('renderError', String(err && (err.stack || err.message) || err).slice(0, 500));
  }
}

// One chip per hidden group; clicking toggles that group's compact cards.
function buildMoreFooter(groups) {
  const footer = document.createElement('footer');
  footer.className = 'board-more';
  const chips = document.createElement('div');
  chips.className = 'board-more__chips';
  for (const group of groups) {
    const open = S.moreOpen.has(group.key);
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'board-more__chip' + (open ? ' is-open' : '');
    const dot = document.createElement('span');
    dot.className = `dot dot--${group.key === 'other' ? 'backlog' : group.key}`;
    const label = document.createElement('span');
    label.textContent = group.key === 'other' ? t('otherTasksTitle') : t(GROUP_I18N_KEY[group.key]);
    const count = document.createElement('b');
    count.textContent = String(group.goals.length);
    chip.append(dot, label, count);
    chip.onclick = () => {
      if (S.moreOpen.has(group.key)) S.moreOpen.delete(group.key); else S.moreOpen.add(group.key);
      renderAllGoals(true);
    };
    chips.appendChild(chip);
  }
  footer.appendChild(chips);
  for (const group of groups) {
    if (!S.moreOpen.has(group.key)) continue;
    const panel = document.createElement('div');
    panel.className = 'board-more__panel';
    if (group.key === 'other') {
      panel.appendChild(buildOtherGoalsRows(group.goals));
    } else {
      for (const g of group.goals) panel.appendChild(buildGoalCard(g, true));
    }
    footer.appendChild(panel);
  }
  return footer;
}

// The panel header carries a live elapsed timer while the selected task
// runs — constant motion so the user knows the agent is alive. Per-card
// status chips (goalStatusChip) cover the "is it running normally?" answer.
function startCountdownLoop() {
  if (S.countdownTimer) clearInterval(S.countdownTimer);
  S.countdownTimer = setInterval(() => {
    const activeGoal = S.activeGoalId ? S.goals.get(S.activeGoalId) : null;
    const timer = document.getElementById('goal-detail-timer');
    if (timer) {
      timer.textContent = (activeGoal && activeGoal.running)
        ? t('elapsedLabel', fmtCountdown(Date.now() - activeGoal.runStartedAt))
        : '';
    }
  }, 1000);
}

// ── turn execution (host agent) ───────────────────────────
// Turns run on BitFun's own agent (app.agent.run): the worker composes the
// prompt (loopx heartbeat-prompt + repo binding), the host executes it in a
// hidden session, agent:event streams progress. No external CLI host and no
// user-facing execution settings. One agent session per goal is reused so
// follow-up turns keep context.
const agentRuns = new Map(); // goalId -> { sessionId, turnId, startedAt, tick }

// Host agent session ids survive restarts (config.agentSessionByGoal) so the
// next turn reuses the same hidden session and keeps the full prior context.
function agentSessionIdFor(goalId) {
  return S.agentSessionByGoal.get(goalId)
    || (S.config.agentSessionByGoal && S.config.agentSessionByGoal[goalId])
    || undefined;
}

function rememberAgentSession(goalId, sessionId) {
  S.agentSessionByGoal.set(goalId, sessionId);
  S.config.agentSessionByGoal = S.config.agentSessionByGoal || {};
  if (S.config.agentSessionByGoal[goalId] !== sessionId) {
    S.config.agentSessionByGoal[goalId] = sessionId;
    saveConfig();
  }
}

function forgetAgentSession(goalId) {
  S.agentSessionByGoal.delete(goalId);
  if (S.config.agentSessionByGoal && S.config.agentSessionByGoal[goalId]) {
    delete S.config.agentSessionByGoal[goalId];
    saveConfig();
  }
}

// A running turn that produces NO agent events for this long is treated as
// dead (host-side cancel lost the event pipe, webview hiccup, ...). The tick
// watchdog cancels it and lets the poll loop relaunch on the same session.
const STALL_TURN_MS = 5 * 60 * 1000;

function stallRecover(g, run) {
  const idleMin = Math.round((Date.now() - run.lastEventAt) / 60000);
  const message = t('turnStalled', idleMin);
  log(`[${g.goalId}] ${message}`, true);
  recordGoalActivity(g, message, true);
  try { app.agent.cancel(run.sessionId, run.turnId); } catch (_) {}
  finishRun(g, { ok: false, error: message });
}

async function executeRunOnce(g) {
  // Auto-run and the manual confirm dialog can race; whoever arrives second
  // must not reset the live run's state or activity stream.
  if (g.running || !isLiveGoal(g)) return;
  if (!goalProjectDir(g.goalId)) { log(t('needProject'), true); return; }
  if (!g.agentId) { log(`[${g.goalId}] ${t('needAgent')}`, true); return; }
  g.running = true;
  g.runStartedAt = Date.now();
  // The activity stream accumulates across runs (and restarts via the
  // persisted log): the run boundary is the 正在启动 line below. Only the
  // per-turn streaming buffers reset.
  g.agentTextBuffer = '';
  g.thinkBuffer = '';
  recordGoalActivity(g, t('activityStarting'));
  // Auto-focus: a task that starts running becomes the selected task, so its
  // log streams into the right-hand panel without a click.
  if (S.activeGoalId !== g.goalId) {
    S.activeGoalId = g.goalId;
    document.getElementById('goal-detail-panel').hidden = false;
    document.getElementById('detail-empty').hidden = true;
  }
  // The liveness tick starts immediately: the panel keeps moving even while
  // turnPrompt / agent.run are still in flight (or stuck), so a silent
  // freeze is impossible — the elapsed clock visibly stops if it breaks.
  const startedAt = g.runStartedAt;
  const tick = setInterval(() => {
    if (!isLiveGoal(g) || !g.running) return;
    const run = agentRuns.get(g.goalId);
    if (!run) return;
    setGoalActivityTick(g, t('activityRunning', fmtCountdown(Date.now() - startedAt)));
    // Stall watchdog: the host can cancel a turn WITHOUT the console ever
    // seeing the completion event (webview hiccup / stale-run cleanup breaks
    // the event pipe). Without this the card stays "running" forever and
    // auto-run freezes. A turn with zero agent events for this long is dead
    // for practical purposes: cancel it, report it, and let the poll loop
    // re-decide — auto-run fires the next turn on the persisted session.
    if (Date.now() - run.lastEventAt > STALL_TURN_MS) {
      clearInterval(tick);
      stallRecover(g, run);
    }
  }, 10000);
  renderGoal(g);
  log(`[${g.goalId}] turn started (agent=${g.agentId})`);
  let requestedSessionId = null;
  try {
    const composed = await app.call('loopx.turnPrompt', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
      agentId: g.agentId,
    });
    if (!composed.ok) throw new Error(composed.error || 'turn prompt failed');
    dbgUi('turn:promptReady', `chars=${composed.prompt.length}`);
    // The log shows what was sent to the agent — collapsed, expandable.
    recordGoalActivity(g, t('activitySentPrompt', composed.prompt.length), false, 'prompt', composed.prompt);
    // Reuse the goal's host session (persisted across restarts) so the agent
    // continues with its full prior context instead of starting from scratch.
    requestedSessionId = agentSessionIdFor(g.goalId);
    const run = await app.agent.run(composed.prompt, {
      sessionName: `bitfun-loopx · ${g.goalId}`,
      sessionId: requestedSessionId || undefined,
      enableTools: true,
      model: modelForGoal(g.goalId),
    });
    dbgUi('turn:agentStarted', `session=${run.sessionId} turn=${run.turnId} reused=${run.sessionId === requestedSessionId}`);
    rememberAgentSession(g.goalId, run.sessionId);
    agentRuns.set(g.goalId, { sessionId: run.sessionId, turnId: run.turnId, startedAt, tick, lastEventAt: Date.now() });
  } catch (err) {
    const message = String(err?.message || err);
    // A dead session id (host restarted or session data pruned) is retried
    // once on a fresh session — cleared EVERYWHERE so the retry cannot reuse
    // it. The retry path also stops this attempt's liveness tick, otherwise
    // the interval would leak and double with the retry's own tick.
    if (requestedSessionId && /session/i.test(message)) {
      clearInterval(tick);
      forgetAgentSession(g.goalId);
      g.running = false;
      return executeRunOnce(g);
    }
    log(`[${g.goalId}] turn error: ${message}`, true);
    g.running = false;
    recordGoalActivity(g, message, true);
    renderGoal(g);
    finishRun(g, { ok: false, error: message });
  }
}

async function cancelGoalRun(g, button) {
  const run = agentRuns.get(g.goalId);
  if (!run) return;
  if (button) button.disabled = true;
  try {
    await app.agent.cancel(run.sessionId, run.turnId);
  } catch (err) {
    log(`[${g.goalId}] cancel error: ${err.message || err}`, true);
    if (button) button.disabled = false;
  }
}

// Terminal handling shared by completion/failure/cancel events.
function finishRun(g, { ok, cancelled = false, error = null }) {
  const run = agentRuns.get(g.goalId);
  if (run) clearInterval(run.tick);
  agentRuns.delete(g.goalId);
  g.running = false;
  g.lastRun = {
    exitCode: ok ? 0 : 1,
    durationMs: run ? Date.now() - run.startedAt : 0,
    status: cancelled ? 'cancelled' : (ok ? 'completed' : 'failed'),
    ok,
    cancelled,
  };
  if (cancelled) {
    // A manual cancel is an explicit "stop": disable the continuous loop
    // instead of immediately relaunching what the user just killed.
    if (g.autoRun) setAutoRun(g, false);
    log(`[${g.goalId}] ${t('runCancelled')}`);
    recordGoalActivity(g, t('runCancelled'));
  } else if (ok) {
    g.autoFailCount = 0;
    log(`[${g.goalId}] turn completed`);
    recordGoalActivity(g, t('activityCompleted'));
  } else {
    if (g.autoRun) {
      g.autoFailCount += 1;
      if (g.autoFailCount >= AUTO_RUN_FAIL_LIMIT) {
        // Trip the breaker VISIBLY: flip the toggle off so the drawer shows
        // reality and re-enabling it is the documented recovery path.
        g.autoRun = false;
        S.config.autoRunByGoal[g.goalId] = false;
        saveConfig();
        log(t('autoRunDisabled', g.goalId), true);
        try {
          if (app.notifications?.system) app.notifications.system(t('title'), t('autoRunDisabled', g.goalId));
        } catch (_) {}
      }
    }
    log(`[${g.goalId}] turn failed: ${error || '?'}`, true);
    recordGoalActivity(g, error || t('activityFailed'), true);
  }
  requestRender();
  pollNow(g, { force: true }); // fresh decision even while hidden; auto-run re-fires from the poll
}

function goalForAgentSession(sessionId) {
  for (const [goalId, run] of agentRuns) {
    if (run.sessionId === sessionId) return S.goals.get(goalId);
  }
  return null;
}

// Compact progress narration from the agent event stream: the instructions
// sent to the agent, its streamed text, tool calls with brief args, and turn
// boundaries. Pure read/observe tools (Read, Grep, Glob, …) are the agent's
// eyes, not progress — skip them; repeats of the same verb collapse into one
// "×N" line via recordGoalActivity.
const QUIET_AGENT_TOOLS = new Set([
  'read', 'grep', 'glob', 'ls', 'list', 'find', 'cat', 'search',
  'web_search', 'websearch', 'fetch', 'mcp',
]);

// Chat-style streaming: the agent's reply and its reasoning each render as
// ONE live block updated in place as chunks arrive (like the host's chat).
// Chunks arrive at model speed — coalesce all buffer updates of a frame into
// ONE DOM write per goal per frame, so a burst of text-chunk events cannot
// re-layout the (capped) block dozens of times per second.
const streamPending = new Map(); // goalId -> { think, agent, raf }
function streamAgentText(g, text, think = false) {
  if (!isLiveGoal(g) || !text) return;
  const key = think ? 'thinkBuffer' : 'agentTextBuffer';
  if (typeof g[key] !== 'string') g[key] = '';
  g[key] += text;
  let pend = streamPending.get(g.goalId);
  if (!pend) {
    pend = { think: null, agent: null, raf: false };
    streamPending.set(g.goalId, pend);
  }
  pend[think ? 'think' : 'agent'] = g[key];
  if (!pend.raf) {
    pend.raf = true;
    requestAnimationFrame(() => {
      pend.raf = false;
      if (!isLiveGoal(g)) { streamPending.delete(g.goalId); return; }
      if (typeof pend.think === 'string') upsertGoalStream(g, 'think', pend.think);
      if (typeof pend.agent === 'string') upsertGoalStream(g, 'agent', pend.agent);
      pend.think = null;
      pend.agent = null;
    });
  }
}

function flushAgentText(g) {
  if (!isLiveGoal(g)) return;
  const pend = streamPending.get(g.goalId);
  for (const [key, kind] of [['agentTextBuffer', 'agent'], ['thinkBuffer', 'think']]) {
    const latest = pend && typeof pend[kind] === 'string' ? pend[kind] : g[key];
    if (typeof latest === 'string' && latest.trim()) {
      upsertGoalStream(g, kind, latest.trim());
    }
    g[key] = '';
  }
  if (pend) { pend.think = null; pend.agent = null; }
}

// Create or update the single streaming block for a kind. Status/tool lines
// between chunks end the block: walk backwards past ticks and continue only
// the newest stream block of that kind; anything else starts a fresh one.
function upsertGoalStream(g, kind, text) {
  const summary = String(text || '').replace(/\s+$/, '');
  if (!summary) return;
  if (!Array.isArray(g.activityLines)) g.activityLines = [];
  const now = new Date().toTimeString().slice(0, 8);
  let lastIndex = -1;
  for (let i = g.activityLines.length - 1; i >= 0; i -= 1) {
    const e = g.activityLines[i];
    if (e.isTick) continue;
    if (e.kind === kind && e.stream) lastIndex = i;
    break;
  }
  const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
  const patchRow = (row) => {
    if (kind === 'think') {
      const pre = row.querySelector('.activity-prompt--think pre');
      if (pre) {
        // Follow only while the user is near the block's bottom: scrolling
        // up to read reasoning history must not be yanked back down.
        const nearBottom = pre.scrollHeight - pre.scrollTop - pre.clientHeight < 48;
        // In-place updates must respect the same DOM tail cap as creation —
        // writing the full accumulated buffer here was the memory/layout bomb.
        pre.textContent = cappedStreamText(summary, STREAM_DOM_CAPS.think);
        if (nearBottom) pre.scrollTop = pre.scrollHeight;
      }
    } else {
      const textEl = row.querySelector('.activity-stream__text');
      if (textEl) textEl.textContent = cappedStreamText(summary, STREAM_DOM_CAPS.agent);
    }
    const timeEl = row.querySelector('.activity-stream__time');
    if (timeEl) timeEl.textContent = now;
  };
  if (lastIndex >= 0) {
    const entry = g.activityLines[lastIndex];
    entry.line = summary;
    entry.time = now;
    if (stream && stream.children[lastIndex]) {
      const follow = streamAtTail(stream);
      patchRow(stream.children[lastIndex]);
      if (follow) streamFollowTail(stream);
    }
  } else {
    const entry = { time: now, line: summary, isErr: false, count: 1, kind, stream: true };
    g.activityLines.push(entry);
    if (g.activityLines.length > 240) g.activityLines.splice(0, g.activityLines.length - 240);
    if (stream) {
      const follow = streamAtTail(stream);
      const emptyEl = stream.querySelector('.activity-empty');
      if (emptyEl) emptyEl.remove();
      stream.appendChild(activityLineElement(entry));
      while (stream.children.length > 240) stream.removeChild(stream.firstChild);
      if (follow) streamFollowTail(stream);
    } else {
      const panel = document.getElementById('goal-detail-panel');
      if (!panel.hidden && S.activeGoalId === g.goalId) renderGoalDetails(g);
    }
  }
  // Model output is for the log panel only — the card's live line stays on
  // tool/status progress (recordGoalActivity), never raw model prose. A
  // "我需要用三句话…" style self-talk must not land on the human-facing card.
  scheduleLogSave();
}

// Tool-event params stream in partial JSON fragments. Accumulate them per
// tool call and parse the running buffer; never surface raw fragments (they
// slice one command into garbage like "bfx" / "-d" / "eepseek-h").
const toolParamsBuf = new Map(); // toolId -> accumulated raw params text
const toolLinesRecorded = new Map(); // toolId -> true (one line per tool call)

function toolBriefFromText(text, final) {
  const t = String(text || '').trim();
  if (!t) return '';
  try {
    const p = JSON.parse(t);
    if (!p || typeof p !== 'object') return '';
    if (Array.isArray(p)) {
      // A single-element array mid-stream is just the first argv fragment —
      // wait for the rest unless this is the final event.
      if (!final && p.length < 2) return '';
      return p.map(String).join(' ').slice(0, 120);
    }
    const brief = p.command || p.cmd || p.file_path || p.filePath || p.path
      || p.query || p.pattern || p.url || p.target_file
      || (Array.isArray(p.args) ? p.args.map(String).join(' ') : '')
      || (Array.isArray(p.arguments) ? p.arguments.map(String).join(' ') : '')
      || '';
    return String(brief).slice(0, 120);
  } catch (_) {
    // Partial buffer. Never guess mid-stream (that produced garbage like
    // "loop" or "bfx"); only make a best effort on the final event.
    if (!final) return '';
    const cmdMatch = t.match(/"(?:cmd|command)"\s*:\s*"([^"]*)/);
    if (cmdMatch && cmdMatch[1]) return cmdMatch[1].replace(/\\(.)/g, '$1').slice(0, 120);
    const argsMatch = t.match(/"(?:args|arguments)"\s*:\s*\[\s*"([^"]*)/);
    if (argsMatch && argsMatch[1]) return argsMatch[1].replace(/\\(.)/g, '$1').slice(0, 120);
    return '';
  }
}

function toolBrief(e, te, final) {
  const raw = te.params ?? e.params;
  if (raw == null) return '';
  const rawText = typeof raw === 'string' ? raw : JSON.stringify(raw);
  if (!te.tool_id) return toolBriefFromText(rawText, final);
  const buf = (toolParamsBuf.get(te.tool_id) || '') + rawText;
  if (buf.length > 6000) toolParamsBuf.set(te.tool_id, buf.slice(-3000));
  else toolParamsBuf.set(te.tool_id, buf);
  return toolBriefFromText(buf, final);
}

function pruneToolMaps() {
  if (toolLinesRecorded.size <= 300) return;
  const keys = [...toolLinesRecorded.keys()].slice(0, 150);
  for (const key of keys) {
    toolLinesRecorded.delete(key);
    toolParamsBuf.delete(key);
  }
}

app.agent.onEvent((e) => {
  // Publish-time cause/solution runs are collected first: their sessions are
  // one-shot analyses, not goal turns.
  const analysisRun = analysisRuns.get(e.sessionId);
  if (analysisRun) {
    if (e.sourceEvent === 'text-chunk' && typeof e.text === 'string' && e.contentType !== 'thinking') {
      analysisRun.buffer += e.text;
      if (analysisRun.buffer.length > 8000) analysisRun.buffer = analysisRun.buffer.slice(-8000);
    } else if (e.sourceEvent === 'dialog-turn-completed'
      || e.sourceEvent === 'dialog-turn-failed'
      || e.sourceEvent === 'dialog-turn-cancelled') {
      clearTimeout(analysisRun.timer);
      const parsed = extractLabeledLines(analysisRun.buffer, ['原因', '解决']);
      analysisRun.resolve(parsed);
      analysisRuns.delete(e.sessionId);
    }
    return;
  }
  // Chinese gate-summary runs are collected first: their sessions are not
  // goal turns, so the normal goal event flow must not see them.
  const summaryRun = summaryRuns.get(e.sessionId);
  if (summaryRun) {
    if (e.sourceEvent === 'text-chunk' && typeof e.text === 'string') {
      // Reasoning chunks never enter the summary buffer — only the visible
      // output stream is a candidate for the three-line answer.
      if (e.contentType !== 'thinking') {
        summaryRun.buffer += e.text;
        if (summaryRun.buffer.length > 8000) summaryRun.buffer = summaryRun.buffer.slice(-8000);
      }
    } else if (e.sourceEvent === 'dialog-turn-completed') {
      const sg = S.goals.get(summaryRun.goalId);
      if (sg && isLiveGoal(sg)) {
        sg.gateSummaries.set(summaryRun.todoId, {
          status: 'done',
          // Parse the labeled three-line answer out of whatever the model
          // emitted (reasoning walls included) — the card shows ONLY those
          // three lines.
          text: cleanGateSummary(String(summaryRun.buffer || '')),
        });
        scheduleGateSummarySave();
        requestRender(true);
      }
      summaryRuns.delete(e.sessionId);
    } else if (e.sourceEvent === 'dialog-turn-failed' || e.sourceEvent === 'dialog-turn-cancelled') {
      const sg = S.goals.get(summaryRun.goalId);
      if (sg) sg.gateSummaries.set(summaryRun.todoId, { status: 'failed' });
      summaryRuns.delete(e.sessionId);
    }
    return;
  }
  const g = goalForAgentSession(e.sessionId);
  if (!g) return;
  // Any event for the goal refreshes the stall watchdog's clock.
  const liveRun = agentRuns.get(g.goalId);
  if (liveRun) liveRun.lastEventAt = Date.now();
  if (e.sourceEvent === 'tool-event') {
    // New bridge nests the tool payload under `toolEvent` (event_type +
    // tool_name/tool_id fields); the legacy flat shape stays as a fallback.
    const te = e.toolEvent || {};
    const name = te.effectiveToolName || te.effective_tool_name
      || te.toolName || te.tool_name || e.toolName || e.tool_name || e.name;
    const phase = te.event_type || te.phase || e.phase;
    const done = phase === 'Completed' || phase === 'completed';
    if (name && !QUIET_AGENT_TOOLS.has(String(name).toLowerCase())) {
      const brief = toolBrief(e, te, done);
      if (te.tool_id) {
        // One line per tool call. If params have not streamed enough yet to
        // name the command, wait — a bare name with garbage fragments is
        // worse than a line that arrives one event later. A completed call
        // with nothing parseable falls back to the bare name.
        if (toolLinesRecorded.has(te.tool_id)) return;
        if (!brief && !done) return;
        toolLinesRecorded.set(te.tool_id, true);
        pruneToolMaps();
        recordGoalActivity(g, brief ? `${name}：${brief}` : String(name));
      } else {
        recordGoalActivity(g, brief ? `${name}：${brief}` : String(name));
      }
    }
  } else if (e.sourceEvent === 'text-chunk') {
    // Stream the model's visible output AND its reasoning (dimmed) — the
    // user wants to see what the model produces, not just its tools.
    if (typeof e.text === 'string') {
      streamAgentText(g, e.text, e.contentType === 'thinking');
    }
  } else if (e.sourceEvent === 'dialog-turn-completed') {
    flushAgentText(g);
    finishRun(g, { ok: true });
  } else if (e.sourceEvent === 'dialog-turn-failed') {
    flushAgentText(g);
    finishRun(g, { ok: false, error: String(e.error || e.message || 'turn failed') });
  } else if (e.sourceEvent === 'dialog-turn-cancelled') {
    flushAgentText(g);
    finishRun(g, { ok: false, cancelled: true });
  }
});

// ── bootstrap / detection / goals ─────────────────────────
function prefixLabel(p) {
  if (!p) return '';
  if (Array.isArray(p)) return p.join(' ');
  const base = (p.argv || []).join(' ');
  return p.env && p.env.PYTHONPATH ? `${base} (PYTHONPATH=${p.env.PYTHONPATH})` : base;
}

async function detect() {
  const banner = document.getElementById('banner-nodetect');
  try {
    S.detect = await app.call('loopx.detect', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
    });
  } catch (err) {
    S.detect = { found: false, probes: [{ error: String(err.message || err) }] };
  }
  if (S.detect.found) {
    banner.hidden = true;
    document.getElementById('btn-vendor-loopx').hidden = true;
    document.getElementById('btn-install-loopx').hidden = true;
    // Persist the working prefix — and heal a stale one: detect probes the
    // persisted prefix first, so if the winner differs, the persisted one is
    // broken (e.g. venv removed) and every poll would fail while the banner
    // says "detected".
    const detectedJson = JSON.stringify(S.detect.argvPrefix);
    if (!S.config.argvPrefix || JSON.stringify(S.config.argvPrefix) !== detectedJson) {
      S.config.argvPrefix = S.detect.argvPrefix;
      saveConfig();
    }
    log(t('detected', `${prefixLabel(S.detect.argvPrefix)} (${S.detect.version || '?'})`));
    return true;
  }
  banner.hidden = false;
  const detail = document.getElementById('probe-detail');
  detail.hidden = false;
  detail.textContent = (S.detect.probes || [])
    .map((p) => `${(p.argvPrefix || []).join(' ')} → ${p.ok ? p.version : p.error || 'failed'}`)
    .join('\n');
  await renderLoopxMissing();
  return false;
}

// ── universal loopx acquisition ────────────────────────────
// loopx is missing on this machine. Preferred path: fetch its source into the
// user's stable vendor dir and run it via PYTHONPATH (loopx has no runtime
// dependencies — only Python >= 3.11 and git are required). The pip install
// button stays as a fallback. Prerequisites are probed and reported item by
// item, so a machine without Python/git gets a concrete hint instead of a
// silent failure.
async function renderLoopxMissing() {
  const vendorBtn = document.getElementById('btn-vendor-loopx');
  const pipBtn = document.getElementById('btn-install-loopx');
  const hint = document.getElementById('prereq-hint');
  if (!vendorBtn || !pipBtn || !hint) return;
  let prereqs = null;
  try { prereqs = await app.call('loopx.checkPrereqs', {}); } catch (_) { prereqs = null; }
  if (prereqs && prereqs.market) {
    // Market edition: interpreters are forbidden by the sandbox, so the
    // vendor path cannot run. Keep only the pip guidance button.
    vendorBtn.hidden = true;
    pipBtn.hidden = false;
    hint.hidden = true;
    return;
  }
  if (prereqs && prereqs.ready) {
    vendorBtn.hidden = false;
    pipBtn.hidden = false;
    hint.hidden = true;
    return;
  }
  // Not ready: name exactly what is missing; hide the buttons until fixed.
  vendorBtn.hidden = true;
  pipBtn.hidden = true;
  hint.hidden = false;
  const lines = [];
  if (!prereqs) {
    lines.push(t('prereqUnknown'));
  } else {
    if (!prereqs.python || !prereqs.python.ok) {
      lines.push(prereqs.python && prereqs.python.found && prereqs.python.version
        ? `${t('prereqNeedPython')}（检测到 ${prereqs.python.version}）`
        : t('prereqNeedPython'));
    }
    if (!prereqs.git || !prereqs.git.found) lines.push(t('prereqNeedGit'));
  }
  hint.textContent = lines.join('\n');
}

// One-click bootstrap: stream pip install progress into the banner, then
// re-detect and reload goals.
function appendInstallProgress(d) {
  const el = document.getElementById('install-progress');
  if (!el) return;
  el.hidden = false;
  el.textContent += `${d && d.line ? d.line : ''}\n`;
  el.scrollTop = el.scrollHeight;
}
app.on('worker:installLoopx:progress', appendInstallProgress);
app.on('worker:vendorLoopx:progress', appendInstallProgress);

async function runInstallLoopx() {
  const btn = document.getElementById('btn-install-loopx');
  const progress = document.getElementById('install-progress');
  btn.disabled = true;
  btn.textContent = t('installingLoopx');
  progress.hidden = false;
  progress.textContent = '';
  try {
    const res = await app.call('loopx.installLoopx', {});
    if (!res.ok) throw new Error(res.error || 'install failed');
    progress.textContent += `\n${t('installDone')}\n`;
    if (await detect()) await refreshGoals();
  } catch (err) {
    progress.textContent += `\n${t('installFailed')}: ${err.message || err}\n`;
  } finally {
    btn.disabled = false;
    btn.textContent = t('installLoopxBtn');
  }
}

async function runVendorLoopx() {
  const btn = document.getElementById('btn-vendor-loopx');
  const progress = document.getElementById('install-progress');
  btn.disabled = true;
  btn.textContent = t('vendoringLoopx');
  progress.hidden = false;
  progress.textContent = '';
  try {
    const res = await app.call('loopx.ensureVendor', {});
    if (!res.ok) throw new Error(res.error || 'vendor failed');
    progress.textContent += `\n${t('vendorDone')}: loopx ${res.version || '?'}\n`;
    // Persist the vendor checkout as the source dir so later detections keep
    // using it (and it heals itself on the next poll).
    if (res.srcDir && !S.config.srcDir) {
      S.config.srcDir = res.srcDir;
      saveConfig();
    }
    if (await detect()) await refreshGoals();
  } catch (err) {
    progress.textContent += `\n${t('vendorFailed')}: ${err.message || err}\n`;
  } finally {
    btn.disabled = false;
    btn.textContent = t('vendorLoopxBtn');
  }
}

async function refreshGoals() {
  try {
    const res = await app.call('loopx.listGoals', {
      argvPrefix: S.config.argvPrefix,
      projectDir: S.config.projectDir,
      projectDirs: projectRegistryDirs(),
    });
    const fresh = new Set();
    let bindingChanged = false;
    for (const info of res.goals || []) {
      fresh.add(info.goalId);
      const existing = S.goals.get(info.goalId);
      if (existing) {
        existing.state = info.state ?? existing.state;
        existing.waitingOn = info.waitingOn ?? existing.waitingOn;
        existing.agents = info.agents?.length ? info.agents : existing.agents;
        existing.objective = info.objective ?? existing.objective;
        // A restored goal leaves the archived group; a goal that got archived
        // elsewhere (another host / loopx maintenance) moves into it.
        existing.archived = !!info.archived;
        existing.archiveDir = info.archiveDir || existing.archiveDir || null;
        if (!existing.archived) {
          // Just restored (or still active): re-arm monitoring for owned
          // goals unless the user explicitly stopped or switched it off.
          existing.monitoring = isOwnedGoal(info.goalId)
            ? (S.config.monitorByGoal[info.goalId] !== false && S.config.stoppedByGoal[info.goalId] !== true)
            : existing.monitoring;
        }
      } else {
        S.goals.set(info.goalId, newGoalState(info.goalId, info));
      }
      // listGoals reports which project directory each goal lives in (clone
      // cache discovery after a fresh import): bind it so polls/turns know
      // the checkout without a re-clone.
      if (info.projectDir && S.config.projectByGoal[info.goalId] !== info.projectDir) {
        S.config.projectByGoal[info.goalId] = info.projectDir;
        bindingChanged = true;
      }
    }
    for (const goalId of [...S.goals.keys()]) {
      if (!fresh.has(goalId)) S.goals.delete(goalId);
    }
    if (bindingChanged) await saveConfig();
    // The composer target dropdown must not depend on the board render
    // completing: refresh it right here too.
    refillComposerTarget();
    requestRender(true);
    // Gate discovery for goals that never poll (paused / auto-run off):
    // loopx may keep waiting_on=codex while an open user-lane todo (publish
    // approval) sits pending. syncGateState is TTL-guarded, so this is one
    // probe per goal per minute at most — polls keep monitored goals fresh.
    for (const g of S.goals.values()) {
      if (shouldTrackUserTodos(g)) syncGateState(g);
    }
    for (const g of S.goals.values()) {
      if (g.monitoring && g.nextDueAt === 0) pollGoal(g);
    }
    rearmTimer();
    log(`goals refreshed: ${S.goals.size} (registry: ${res.registryPath})`);
  } catch (err) {
    log(`listGoals error: ${err.message || err}`, true);
  }
}

// ── toolbar wiring ────────────────────────────────────────
// There is no header bar anymore: refresh is implicit (heartbeat + boot +
// retry), GitHub credentials are prompted by the publish flow itself
// (openTokenDialog), and per-task deletion lives on each goal card — the
// former top bar carried only the brand, so it was removed entirely.
document.getElementById('btn-token-save').addEventListener('click', saveGitHubToken);
document.getElementById('btn-token-clear').addEventListener('click', clearGitHubToken);
document.getElementById('btn-gh-login').addEventListener('click', runGhLogin);
app.on('worker:ghLogin:progress', appendGhLoginProgress);
// External guide links open in the system browser (sandboxed iframe cannot
// navigate top-level windows on its own).
document.querySelectorAll('.external-link').forEach((a) => {
  a.addEventListener('click', (ev) => {
    ev.preventDefault();
    try {
      if (app.system && app.system.openExternal) app.system.openExternal(a.href);
      else window.open(a.href, '_blank', 'noopener');
    } catch (_) {
      window.open(a.href, '_blank', 'noopener');
    }
  });
});
document.getElementById('btn-retry-detect').addEventListener('click', async () => {
  if (await detect()) refreshGoals();
});
document.getElementById('btn-install-loopx').addEventListener('click', runInstallLoopx);
document.getElementById('btn-vendor-loopx').addEventListener('click', runVendorLoopx);

document.getElementById('btn-copy-raw').addEventListener('click', async (e) => {
  const text = document.getElementById('raw-body').textContent;
  try {
    if (app.clipboard?.writeText) await app.clipboard.writeText(text);
    else await navigator.clipboard.writeText(text);
    e.target.textContent = '✓';
    setTimeout(() => { e.target.textContent = t('copy'); }, 1200);
  } catch (err) {
    log(`copy failed: ${err.message || err}`, true);
  }
});

// ── task intake ───────────────────────────────────────────
// Flow: input → loopx.resolveIntake (read-only classify + expand issues-list)
// → confirmation sheet (issue checklist; new-vs-guide when goals exist)
// → loopx.taskIntake (event-driven) → auto-run takes over.
// Strict intake grammar (docs/product-spec.md): the only supported links are
// a single issue/PR, the issues list, and the repository home. Anything else
// is rejected with a specific message instead of being treated as the repo.
function taskInputKind(text) {
  const urls = String(text || '').match(/https:\/\/github\.com\/[A-Za-z0-9._~:/?#[\]@!$&'()*+,;=%-]+/gi) || [];
  let issues = 0;
  let lists = 0;
  let repos = 0;
  for (const url of urls) {
    try {
      const segments = new URL(url.replace(/[),.;:\]}]+$/g, '')).pathname.split('/').filter(Boolean);
      const type = (segments[2] || '').toLowerCase();
      if (segments.length === 2) repos += 1;
      else if (type === 'issues' && segments.length === 3) lists += 1;
      else if (/^(issues|pull)$/.test(type) && segments.length === 4 && /^\d+$/.test(segments[3] || '')) issues += 1;
    } catch (_) {}
  }
  if (lists || (repos && !issues)) return t('taskIssuesList');
  if (issues > 1) return t('taskIssues', issues);
  if (issues === 1) return t('taskIssue');
  return '';
}

// First github.com URL in the text that does NOT fit the supported grammar
// (used to explain rejections precisely); null when none.
function firstUnsupportedGithubUrl(text) {
  const urls = String(text || '').match(/https:\/\/github\.com\/[A-Za-z0-9._~:/?#[\]@!$&'()*+,;=%-]+/gi) || [];
  for (const url of urls) {
    try {
      const segments = new URL(url.replace(/[),.;:\]}]+$/g, '')).pathname.split('/').filter(Boolean);
      const type = (segments[2] || '').toLowerCase();
      const supported = segments.length === 2
        || (type === 'issues' && segments.length === 3)
        || (/^(issues|pull)$/.test(type) && segments.length === 4 && /^\d+$/.test(segments[3] || ''));
      if (!supported) return url;
    } catch (_) {}
  }
  return null;
}

function setTaskFeedback(message, mode = '') {
  const feedback = document.getElementById('task-feedback');
  feedback.textContent = message || '';
  feedback.hidden = !message;
  feedback.className = `composer__feedback${mode ? ` composer__feedback--${mode}` : ''}`;
}

function updateTaskKind() {
  const input = document.getElementById('task-input');
  const kind = taskInputKind(input.value);
  const badge = document.getElementById('task-kind');
  badge.textContent = kind;
  badge.hidden = !kind;
}

function resolveDefaultAgent() {
  if (S.config.defaultAgentId) return S.config.defaultAgentId;
  for (const goal of S.goals.values()) {
    if (goal.agentId) return goal.agentId;
    if (goal.agents.length) return goal.agents[0];
  }
  return Object.values(S.config.agentByGoal || {}).find(Boolean) || '';
}

function setComposerBusy(busy, message = '') {
  document.getElementById('task-input').disabled = busy;
  document.getElementById('btn-create-task').disabled = busy;
  setTaskFeedback(message, busy ? '' : undefined);
}

// The confirmation sheet is the one deliberate stop before anything is
// written: it shows exactly which issues become todos and where they land.
function openIntakeSheet(resolved, objective, targetGoal = null) {
  S.pendingIntake = {
    resolved,
    objective,
    selected: new Set(resolved.issues.map((i) => i.url)),
    guideGoalId: targetGoal && !isTerminal(targetGoal) ? targetGoal.goalId : null,
  };
  const dlg = document.getElementById('dlg-intake');
  const isList = resolved.kind === 'issues-list';
  const hasIssues = resolved.issues.length > 0;
  const guiding = Boolean(S.pendingIntake.guideGoalId);

  document.getElementById('intake-title').textContent = isList
    ? t('intakeTitleList')
    : (resolved.issues.length > 1 ? t('intakeTitleIssues', resolved.issues.length)
      : (resolved.issues.length === 1 ? t('intakeTitleIssue') : t('intakeTitleGoal')));

  const summary = document.getElementById('intake-summary');
  if (isList) {
    summary.textContent = t('intakeSummaryList', resolved.repo || '?', resolved.issues.length)
      + (resolved.truncated ? ` ${t('intakeTruncated', resolved.issues.length)}` : '');
  } else if (resolved.issues.length > 1) summary.textContent = t('intakeSummaryIssues', resolved.repo || '?');
  else if (guiding && !hasIssues) summary.textContent = t('intakeSummaryGoal');
  else summary.textContent = objective;
  if (guiding) {
    const targetG = S.goals.get(S.pendingIntake.guideGoalId);
    summary.textContent += `\n${t('guideTargetNote', targetG ? goalDisplayName(targetG) : S.pendingIntake.guideGoalId)}`;
  }
  if (resolved.autoClone) {
    summary.textContent += `\n${t('intakeCloneNote', resolved.repo || '?')}`;
  } else if (resolved.reuseDir) {
    summary.textContent += `\n${t('intakeReuseNote', resolved.repo || '?')}`;
  }
  if (resolved.fellBackFromCheckout) {
    summary.textContent += `\n${t('taskCloneOtherRepo', resolved.repo || '?', resolved.fellBackFromCheckout)}`;
  }
  // New tasks carry pre-granted write scope (this confirmation IS the
  // consent); only publish/PR decisions still gate later.
  if (resolved.repo && !guiding) {
    summary.textContent += `\n${t('intakeWriteNote')}`;
  }

  // issue checklist (only for multi/list intake; single issue needs no picking)
  const listEl = document.getElementById('intake-issues');
  listEl.replaceChildren();
  listEl.hidden = !hasIssues || resolved.issues.length < 2;
  const bar = document.getElementById('intake-selectbar');
  bar.hidden = listEl.hidden;
  document.getElementById('intake-select-all').checked = true;
  if (!listEl.hidden) {
    for (const issue of resolved.issues) {
      const row = document.createElement('label');
      row.className = 'intake-issue';
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = true;
      cb.onchange = () => {
        if (cb.checked) S.pendingIntake.selected.add(issue.url);
        else S.pendingIntake.selected.delete(issue.url);
        updateIntakeCount();
      };
      const num = document.createElement('span');
      num.className = 'intake-issue__num';
      num.textContent = `#${issue.number}`;
      const title = document.createElement('span');
      title.className = 'intake-issue__title';
      title.textContent = issue.title === `#${issue.number}` ? issue.url : issue.title;
      title.title = issue.url;
      row.append(cb, num, title);
      // Image-bearing issues get a marker so the user can spot them at a glance.
      if (issue.hasImages) {
        const badge = document.createElement('span');
        badge.className = 'intake-issue__img';
        badge.textContent = '🖼';
        badge.title = t('issueHasImages');
        row.appendChild(badge);
      }
      listEl.appendChild(row);
    }
  }

  // Conservative guard: issues whose key info lives in screenshots + a
  // text-only model = warn instead of silently letting the agent guess.
  const imageCount = resolved.issues.filter((issue) => issue.hasImages).length;
  const visionWarn = document.getElementById('intake-vision-warn');
  if (imageCount > 0 && !modelSupportsVision()) {
    visionWarn.hidden = false;
    visionWarn.textContent = t('intakeVisionWarn', imageCount);
  } else {
    visionWarn.hidden = true;
  }

  updateIntakeCount();
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {
    const pending = S.pendingIntake;
    S.pendingIntake = null;
    if (dlg.returnValue !== 'confirm' || !pending) {
      setComposerBusy(false, '');
      return;
    }
    startTaskIntake(pending, pending.guideGoalId || null);
  };
  dlg.showModal();
}

function updateIntakeCount() {
  const pending = S.pendingIntake;
  if (!pending) return;
  const total = pending.resolved.issues.length;
  const selected = total ? pending.resolved.issues.filter((i) => pending.selected.has(i.url)).length : 0;
  const countEl = document.getElementById('intake-count');
  countEl.textContent = total >= 2 ? t('intakeSelectedCount', selected, total) : '';
  const confirm = document.getElementById('btn-intake-confirm');
  const guiding = Boolean(pending.guideGoalId);
  if (guiding) {
    confirm.textContent = t('intakeConfirmGuide');
    confirm.disabled = total >= 2 && selected === 0;
  } else if (total >= 2) {
    confirm.textContent = t('intakeConfirmIssues', selected);
    confirm.disabled = selected === 0;
  } else {
    confirm.textContent = t('intakeConfirmNew');
    confirm.disabled = false;
  }
}

document.getElementById('intake-select-all').addEventListener('change', (e) => {
  const pending = S.pendingIntake;
  if (!pending) return;
  pending.selected = e.target.checked ? new Set(pending.resolved.issues.map((i) => i.url)) : new Set();
  document.querySelectorAll('#intake-issues input[type="checkbox"]').forEach((cb) => { cb.checked = e.target.checked; });
  updateIntakeCount();
});

function startTaskIntake(pending, guideGoalId) {
  const { resolved, objective } = pending;
  const issues = resolved.issues.filter((i) => pending.selected.has(i.url))
    .map((i) => ({ url: i.url, number: i.number, title: i.title }));
  // Guidance todos are claimed by the target goal's own agent, not the
  // new-task default — a mismatch would leave them unclaimable.
  const agentId = guideGoalId
    ? (S.goals.get(guideGoalId)?.agentId || resolveDefaultAgent())
    : resolveDefaultAgent();
  // Binding order: the guided goal's own checkout, then a reuse directory the
  // resolver matched to this repository, then the global setting. Auto-clone
  // only fires when none of those exists.
  const projectDir = guideGoalId
    ? goalProjectDir(guideGoalId)
    : (resolved.reuseDir || (!resolved.bypassCheckout ? S.config.projectDir : null));
  S.intakeDraft = { objective, stage: guideGoalId ? t('taskCreating') : t('stageBootstrap') };
  setComposerBusy(true, t('taskCreating'));
  renderAllGoals(true);
  dbgUi('intake:call', `mode=${guideGoalId ? 'guide' : 'new'} goalId=${guideGoalId || '(new)'} issues=${issues.length} projectDir=${projectDir || '(none)'}`);
  app.call('loopx.taskIntake', {
    argvPrefix: S.config.argvPrefix,
    srcDir: S.config.srcDir || null,
    projectDir,
    objective,
    agentId,
    mode: guideGoalId ? 'guide' : 'new',
    goalId: guideGoalId,
    autoClone: !guideGoalId && !projectDir && Boolean(resolved.autoClone),
    issues: issues.length ? issues : null,
  }).then((res) => {
    dbgUi('intake:accepted', `started=${Boolean(res && res.started)}`);
    if (res && res.ok === false) {
      // Synchronous refusal (repo checks) — no done event will follow.
      S.intakeDraft = null;
      requestRender(true);
      setComposerBusy(false, '');
      const message = res.code === 'repository_mismatch'
        ? t('taskRepoMismatch', res.requestedRepo, res.projectRepo || '?')
        : (res.code === 'multiple_repositories' ? t('taskMultipleRepos') : (res.error || 'task intake failed'));
      setTaskFeedback(message, 'error');
      return;
    }
    if (!res || !res.started) throw new Error('task intake did not start');
    // progress + completion arrive on worker:taskIntake:* events
  }).catch((err) => {
    dbgUi('intake:rejected', String(err && err.message || err));
    S.intakeDraft = null;
    requestRender(true);
    setComposerBusy(false, '');
    setTaskFeedback(String(err.message || err), 'error');
    log(`task intake error: ${err.message || err}`, true);
  });
}

app.on('worker:taskIntake:progress', (d) => {
  if (!S.intakeDraft) return;
  const stages = {
    expand: t('stageExpand'),
    clone: d.percent != null ? t('stageClonePercent', d.percent) : t('stageClone'),
    bootstrap: t('stageBootstrap'),
    register: t('stageRegister'),
    plan: t('stagePlan'),
    todos: d.total ? t('stageTodos', d.current || 0, d.total) : t('taskCreating'),
    refresh: t('stageRefresh'),
  };
  S.intakeDraft.stage = stages[d.stage] || t('taskCreating');
  setTaskFeedback(S.intakeDraft.stage);
  // Patch the pending rail row's stage line in place — a full board rebuild
  // per progress event (clone percent streams) is exactly the flicker we
  // remove.
  const live = document.querySelector('.run-item--pending .goal__activity-text');
  if (live) live.textContent = S.intakeDraft.stage;
});

app.on('worker:taskIntake:done', async (result) => {
  const input = document.getElementById('task-input');
  // A goal that got created is a goal we manage — even if some todos failed.
  // Treating partial failure as total would hide the goal, leave auto-run
  // off, and invite a retry that mints a duplicate (uniqueGoalId suffixes).
  if (!result.ok && !result.created) {
    let message = result.error || 'task creation failed';
    if (result.code === 'repository_mismatch') {
      message = t('taskRepoMismatch', result.requestedRepo, result.projectRepo || '?');
    } else if (result.code === 'multiple_repositories') {
      message = t('taskMultipleRepos');
    } else if (result.code === 'repository_unverified') {
      message = t('taskRepoUnverified', result.requestedRepo || '?');
    }
    S.intakeDraft = null;
    requestRender(true);
    setComposerBusy(false, '');
    setTaskFeedback(message, 'error');
    log(`task intake: ${message}`, true);
    return;
  }
  if (result.mode === 'new') {
    const agentId = resolveDefaultAgent();
    S.config.defaultAgentId = agentId;
    S.config.agentByGoal[result.goalId] = agentId;
    S.config.monitorByGoal[result.goalId] = true;
    S.config.autoRunByGoal[result.goalId] = true;
    S.config.ownedGoals[result.goalId] = true;
    if (result.projectDir) S.config.projectByGoal[result.goalId] = result.projectDir;
    await saveConfig();
  }
  input.value = '';
  updateTaskKind();
  setComposerBusy(false, '');
  const resultGoal = S.goals.get(result.goalId);
  const resultName = goalDisplayName(resultGoal || { goalId: result.goalId, objective: S.intakeDraft?.objective || '' });
  const skipNote = result.skippedDuplicates > 0 ? ` ${t('skippedDuplicates', result.skippedDuplicates)}` : '';
  if (!result.ok) {
    // Goal exists but some todos failed — adopt it, say so honestly.
    setTaskFeedback(t('taskPartial', resultName, result.writtenOk ?? 0, result.error || '') + skipNote, 'error');
    log(`[${result.goalId}] task intake partial: ${result.error}`, true);
  } else if (result.mode === 'guide') {
    setTaskFeedback(t('guideStarted', resultName) + skipNote, 'ok');
    log(`[${result.goalId}] guidance written (${result.written.length} todos)`);
  } else {
    setTaskFeedback(t('taskStarted', resultName) + skipNote, 'ok');
    log(`[${result.goalId}] task created (${result.intakeKind}, ${result.written.length} todos)`);
  }
  if (S.intakeDraft) S.intakeDraft.stage = t('taskStageStarting');
  await refreshGoals();
  // The registry can lag the intake write: keep the pending row until the
  // goal actually shows up, then swap it for the real row in ONE render —
  // no "task briefly vanished" gap in the 进行中 column.
  let goal = S.goals.get(result.goalId);
  for (let retry = 0; !goal && retry < 6; retry += 1) {
    await new Promise((resolve) => setTimeout(resolve, 700));
    await refreshGoals();
    goal = S.goals.get(result.goalId);
  }
  S.intakeDraft = null;
  if (goal) {
    if (result.mode === 'new') {
      goal.agentId = S.config.agentByGoal[result.goalId] || goal.agentId;
      goal.autoRun = true;
      goal.autoFailCount = 0;
    }
    requestRender(true);
    // refreshGoals already polled the goal; a should_run=true decision fires
    // the first turn through maybeAutoRun — one launch path, no races.
    pollNow(goal, { force: true });
  } else {
    requestRender(true);
  }
});

async function createTaskFromInput() {
  const input = document.getElementById('task-input');
  const objective = input.value.trim();
  if (!objective) { input.focus(); return; }
  dbgUi('createTask:start', `text=${objective.slice(0, 120)}`);
  // Composer semantics:
  //   新建任务 → a COMPLETELY independent new task (link → issue intake).
  //   已有任务 → whatever the human typed (link or free text) is FEEDBACK
  //   injected into that task as guidance — no issue-list sheet, no new
  //   todos; the agent's next turns follow the human's expectations.
  const explicitId = composerTargetValue();
  if (explicitId) {
    const target = S.goals.get(explicitId);
    if (target && !isTerminal(target)) {
      startGuidance(target, objective);
      return;
    }
    // The picked goal vanished (deleted): fall through to a fresh task.
    setComposerTarget('');
  }
  // Free text is NOT a goal type — while a task is running, it becomes
  // mid-task guidance on the active/running goal.
  if (!taskInputKind(objective)) {
    const bad = firstUnsupportedGithubUrl(objective);
    if (bad) {
      dbgUi('createTask:unsupported', bad);
      setTaskFeedback(t('taskUnsupportedPath', bad), 'error');
      return;
    }
    const target = guidanceTargetGoal();
    if (!target) {
      const runningCount = [...S.goals.values()].filter((g) => g.running).length;
      dbgUi('createTask:guidanceRejected', `running=${runningCount}`);
      setTaskFeedback(runningCount > 1 ? t('guidancePickOne') : t('guidanceNoRunning'), 'error');
      return;
    }
    startGuidance(target, objective);
    return;
  }
  if (!resolveDefaultAgent()) {
    setTaskFeedback(t('taskNeedAgent'), 'error');
    return;
  }
  setComposerBusy(true, t('taskResolving'));
  dbgUi('createTask:callingResolve', `projectDir=${S.config.projectDir || '(none)'}`);
  let resolved;
  try {
    resolved = await app.call('loopx.resolveIntake', {
      projectDir: S.config.projectDir,
      projectDirs: projectRegistryDirs(),
      objective,
    });
    // The bound checkout is a different repository than the link: fall back
    // to the clone-directory path (auto-clone or reuse) so one console can
    // work across repositories without touching Settings.
    if (!resolved.ok && resolved.code === 'repository_mismatch') {
      const boundRepo = resolved.projectRepo;
      dbgUi('createTask:mismatchFallback', `${resolved.requestedRepo} vs ${boundRepo}`);
      resolved = await app.call('loopx.resolveIntake', {
        projectDir: null,
        projectDirs: Object.values(S.config.projectByGoal || {}).filter(Boolean),
        objective,
      });
      if (resolved.ok && resolved.repo && boundRepo) {
        // The global checkout must not leak back into binding for THIS repo:
        // bypassCheckout marks the fallback so reuse/clone decisions ignore it.
        resolved.fellBackFromCheckout = boundRepo;
        resolved.bypassCheckout = true;
        log(`repo switch: new task targets ${resolved.repo} (checkout was ${boundRepo})`);
      }
    }
    dbgUi('createTask:resolved', JSON.stringify({ ok: resolved.ok, code: resolved.code, kind: resolved.kind, reuseDir: resolved.reuseDir || null, autoClone: resolved.autoClone, issues: resolved.issues && resolved.issues.length }));
  } catch (err) {
    dbgUi('createTask:resolveError', String(err && err.message || err));
    setComposerBusy(false, '');
    setTaskFeedback(String(err.message || err), 'error');
    return;
  }
  if (!resolved.ok) {
    setComposerBusy(false, '');
    if (resolved.code === 'repository_mismatch') {
      setTaskFeedback(t('taskRepoMismatch', resolved.requestedRepo, resolved.projectRepo || '?'), 'error');
    } else if (resolved.code === 'multiple_repositories') {
      setTaskFeedback(t('taskMultipleRepos'), 'error');
    } else if (resolved.code === 'repository_unverified') {
      setTaskFeedback(t('taskRepoUnverified', resolved.requestedRepo || '?'), 'error');
    } else if (resolved.code === 'repository_not_found') {
      setTaskFeedback(t('taskRepoNotFound', resolved.requestedRepo || '?'), 'error');
    } else if (resolved.code === 'repository_lookup_failed') {
      setTaskFeedback(t('taskRepoLookupFailed'), 'error');
    } else if (resolved.code === 'unsupported_github_path') {
      setTaskFeedback(t('taskUnsupportedPath', resolved.url || '?'), 'error');
    } else if (resolved.code === 'unsupported_input') {
      setTaskFeedback(t('taskGoalUnsupported'), 'error');
    } else {
      setTaskFeedback(resolved.error || 'intake failed', 'error');
    }
    return;
  }
  if (resolved.kind === 'issues-list' && resolved.issues.length === 0) {
    setComposerBusy(false, '');
    setTaskFeedback(t('intakeNoIssues'), 'error');
    return;
  }
  setTaskFeedback('');
  // 新建任务: the intake always creates a completely independent task —
  // no same-repo merging, no target overrides. (An explicitly selected
  // existing task never reaches this path: its input became guidance above.)
  if (resolved.issues.length < 2) {
    startTaskIntake({ resolved, objective, selected: new Set(resolved.issues.map((i) => i.url)) }, null);
    return;
  }
  openIntakeSheet(resolved, objective, null);
}

// Free text targets a RUNNING task as guidance. An explicit composer-target
// pick wins first; otherwise prefer the selected goal, then a single running
// goal; multiple running goals need a pick first.
function guidanceTargetGoal() {
  const pickedId = composerTargetValue();
  if (pickedId) {
    const picked = S.goals.get(pickedId);
    if (picked && !isTerminal(picked)) return picked;
  }
  if (S.activeGoalId) {
    const selected = S.goals.get(S.activeGoalId);
    if (selected && selected.running) return selected;
  }
  const running = [...S.goals.values()].filter((g) => g.running);
  return running.length === 1 ? running[0] : null;
}

async function startGuidance(g, text) {
  const input = document.getElementById('task-input');
  setComposerBusy(true, t('guidanceSending'));
  dbgUi('guidance:start', `goal=${g.goalId}`);
  try {
    const res = await app.call('loopx.guideGoal', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
      projectDir: goalProjectDir(g.goalId),
      goalId: g.goalId,
      agentId: g.agentId || null,
      text,
    });
    if (!res.ok) throw new Error(res.error || 'guidance failed');
    input.value = '';
    updateTaskKind();
    setComposerBusy(false, '');
    setTaskFeedback(t('guidanceSent', goalDisplayName(g)), 'ok');
    recordGoalActivity(g, t('guidanceLine', text), false, 'agent');
    log(`[${g.goalId}] guidance sent (${text.length} chars)`);
    dbgUi('guidance:done', `goal=${g.goalId} todoId=${res.todoId || ''}`);
    pollNow(g, { force: true }); // fresh decision; auto-run picks the message up
  } catch (err) {
    const message = String(err && err.message || err);
    dbgUi('guidance:error', message);
    setComposerBusy(false, '');
    setTaskFeedback(message, 'error');
    log(`guidance error: ${message}`, true);
  }
}

document.getElementById('task-input').addEventListener('input', () => {
  updateTaskKind();
  setTaskFeedback('');
});
document.getElementById('task-input').addEventListener('keydown', (event) => {
  // Enter submits; Shift+Enter inserts a newline (default browser behavior).
  if (event.key !== 'Enter' || event.shiftKey) return;
  event.preventDefault();
  createTaskFromInput();
});
document.getElementById('btn-create-task').addEventListener('click', createTaskFromInput);
document.getElementById('composer-model').addEventListener('change', async () => {
  S.config.defaultModel = document.getElementById('composer-model').value || 'auto';
  await saveConfig();
  log(t('modelChanged', S.config.defaultModel));
});
document.getElementById('composer-target-trigger').addEventListener('click', () => {
  const menu = document.getElementById('composer-target-menu');
  if (!menu) return;
  if (menu.hidden) refillComposerTarget();
  menu.hidden = !menu.hidden;
});
document.addEventListener('click', (e) => {
  const target = document.getElementById('composer-target');
  if (target && !target.contains(e.target)) closeComposerTargetMenu();
});


// Dialog cancel buttons are type=button (a submit-type Cancel placed first
// becomes the form's default button, so Enter in any dialog input would
// silently cancel). Close explicitly instead.
document.querySelectorAll('dialog .dlg-cancel').forEach((btn) => {
  btn.addEventListener('click', () => {
    const dlg = btn.closest('dialog');
    dlg.returnValue = 'cancel';
    dlg.close('cancel');
  });
});

// ── column resize handles ──────────────────────────────────
// Both dividers are draggable: 等你处理/进行中 and 进行中目录/日志面板.
// A dragged column becomes FIXED width (flex: 0 0 Npx) — the others keep
// their equal share of the remaining space. Widths persist in config and
// re-apply on boot.
function applyLayoutWidths() {
  const review = document.getElementById('review-zone');
  const rail = document.getElementById('run-rail');
  if (review && S.config.reviewZoneWidth > 0) {
    review.style.flex = `0 0 ${S.config.reviewZoneWidth}px`;
  }
  if (rail && S.config.railWidth > 0) {
    rail.style.flex = `0 0 ${S.config.railWidth}px`;
  }
}

function makeResizable(handle, target, { min, max, persistKey }) {
  if (!handle || !target) return;
  let dragging = false;
  let startX = 0;
  let startW = 0;
  handle.addEventListener('pointerdown', (event) => {
    dragging = true;
    startX = event.clientX;
    startW = target.getBoundingClientRect().width;
    handle.classList.add('is-dragging');
    document.body.style.userSelect = 'none';
    try { handle.setPointerCapture(event.pointerId); } catch (_) {}
    event.preventDefault();
  });
  handle.addEventListener('pointermove', (event) => {
    if (!dragging) return;
    const width = Math.min(max, Math.max(min, startW + (event.clientX - startX)));
    // Fixed width: flex-grow must stop fighting the drag (with the equal
    // three-column default, grow would re-widen the column immediately).
    target.style.flex = `0 0 ${width}px`;
    target.style.flexGrow = '0';
    target.style.flexShrink = '0';
  });
  const finish = () => {
    if (!dragging) return;
    dragging = false;
    handle.classList.remove('is-dragging');
    document.body.style.userSelect = '';
    S.config[persistKey] = Math.round(target.getBoundingClientRect().width);
    saveConfig();
  };
  handle.addEventListener('pointerup', finish);
  handle.addEventListener('pointercancel', finish);
}

makeResizable(
  document.getElementById('resize-review'),
  document.getElementById('review-zone'),
  { min: 220, max: 560, persistKey: 'reviewZoneWidth' },
);
makeResizable(
  document.getElementById('resize-rail'),
  document.getElementById('run-rail'),
  { min: 180, max: 480, persistKey: 'railWidth' },
);

// ── one-click back to the log tail ─────────────────────────
// Mirrors the chat-style affordance: once the reader scrolls up, a downward
// arrow button appears; clicking it jumps to the bottom (and keeps following).
const logBottomBtn = document.getElementById('btn-log-bottom');
const logBodyEl = document.getElementById('goal-detail-body');
function updateLogBottomBtn() {
  if (!logBottomBtn || !logBodyEl) return;
  const nearBottom = logBodyEl.scrollHeight - logBodyEl.scrollTop - logBodyEl.clientHeight < 48;
  logBottomBtn.hidden = nearBottom || logBodyEl.scrollHeight <= logBodyEl.clientHeight + 8;
}
logBodyEl.addEventListener('scroll', updateLogBottomBtn);
logBottomBtn.addEventListener('click', () => {
  logBodyEl.scrollTop = logBodyEl.scrollHeight;
  updateLogBottomBtn();
});

// ── i18n ──────────────────────────────────────────────────
function applyI18n() {
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    const value = t(key);
    if (typeof value !== 'string') return;
    if (el.getAttribute('data-i18n-attr') === 'title') el.title = value;
    else el.textContent = value;
  });
  document.querySelectorAll('[data-i18n-title]').forEach((el) => {
    const value = t(el.getAttribute('data-i18n-title'));
    if (typeof value === 'string') el.title = value;
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const value = t(el.getAttribute('data-i18n-placeholder'));
    if (typeof value === 'string') el.placeholder = value;
  });
  updateTaskKind();
  // Dynamic text the static pass just clobbered: the intake sheet's
  // count-driven confirm label.
  if (S.pendingIntake) updateIntakeCount();
}

app.onLocaleChange((locale) => {
  if (typeof locale === 'string') document.documentElement.setAttribute('lang', locale);
  applyI18n();
  renderAllGoals(true);
});

// ── lifecycle ─────────────────────────────────────────────
// The host documents onActivate/onDeactivate but does not emit them yet;
// keep the hooks (harmless, future-proof) and add two real signals:
// visibilitychange for window minimise, IntersectionObserver for the
// scene-tab display:none toggle (SceneViewport hides inactive tabs via CSS).
app.onDeactivate(pauseHeartbeat);
app.onActivate(resumeHeartbeat);
let intersecting = true;
document.addEventListener('visibilitychange', () => {
  if (document.hidden) pauseHeartbeat();
  else if (intersecting) resumeHeartbeat();
});
const visObserver = new IntersectionObserver((entries) => {
  intersecting = entries[entries.length - 1].isIntersecting;
  if (!intersecting) pauseHeartbeat();
  else if (!document.hidden) resumeHeartbeat();
});
visObserver.observe(document.body);

// Unified teardown: when the console closes (tab closed, scene unmounted, or
// the whole app exits) everything scheduled or running here must stop with
// it — no host agent turn or timer outliving the console's lifetime.
function teardownConsole() {
  if (S.timer) clearTimeout(S.timer);
  if (S.countdownTimer) clearInterval(S.countdownTimer);
  for (const run of agentRuns.values()) {
    try { app.agent.cancel(run.sessionId, run.turnId); } catch (_) {}
  }
  agentRuns.clear();
  for (const run of summaryRuns.values()) {
    try { app.agent.cancel(run.sessionId, run.turnId); } catch (_) {}
  }
  summaryRuns.clear();
}
window.addEventListener('beforeunload', teardownConsole);
window.addEventListener('pagehide', teardownConsole);

// ── boot ──────────────────────────────────────────────────
(async function boot() {
  dbgUi('boot:start', `t=${bootMs()}ms readyState=${document.readyState} theme=${themeProbe()}`);
  await loadConfig();
  applyLayoutWidths();
  dbgUi('boot:configLoaded', `t=${bootMs()}ms projectDir=${S.config.projectDir || '(none)'} theme=${themeProbe()}`);
  try {
    const catalog = await app.ai.getModels();
    if (Array.isArray(catalog)) S.modelCatalog = catalog;
    dbgUi('boot:models', `t=${bootMs()}ms catalog=${S.modelCatalog.length}`);
  } catch (err) {
    dbgUi('boot:modelsError', String(err && err.message || err));
  }
  syncComposerModel();
  applyI18n();
  dbgUi('boot:i18nApplied', `t=${bootMs()}ms`);
  startCountdownLoop();
  // Detect (banner + prefix persistence) and goal loading run in parallel:
  // listGoals resolves the invocation prefix on its own, so the board no
  // longer waits ~1.4s behind the CLI probe before showing goals.
  const detectedPromise = detect();
  const goalsPromise = refreshGoals();
  const detected = await detectedPromise;
  dbgUi('boot:detected', `t=${bootMs()}ms found=${detected} theme=${themeProbe()}`);
  await goalsPromise;
  S.bootLoading = false;
  // Opening the console never auto-resumes a previous task: everything boots
  // paused (自动已关) and the user starts tasks explicitly with 继续. This
  // also guarantees "UI shows not running ⇒ nothing runs" after a restart —
  // no auto-run can fire until the user opts back in.
  let pausedAny = false;
  for (const g of S.goals.values()) {
    if (g.autoRun) {
      g.autoRun = false;
      S.config.autoRunByGoal[g.goalId] = false;
      pausedAny = true;
    }
  }
  if (pausedAny) saveConfig();
  // Fill the composer target dropdown even before the first board paint.
  refillComposerTarget();
  const paints = performance.getEntriesByType('paint')
    .map((p) => `${p.name}@${Math.round(p.startTime)}ms`).join(' ') || '(no paint entries)';
  dbgUi('boot:done', `t=${bootMs()}ms goals=${S.goals.size} theme=${themeProbe()} paint=${paints}`);
  requestRender(true);
})();
