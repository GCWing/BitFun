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
    issueOpen: '修复中',
    issueBlocked: '无法继续',
    issueDeferred: '暂不修复',
    issuePending: '待处理',
    issueResolved: '已解决',
    issueResolvedExternally: (n) => `以下 issue 已被维护者解决，无需再修复：${n}`,
    issueResolvedExternallyShort: '已解决(外部)',
    issueComments: (n) => `💬 ${n}`,
    issueAgeNow: '刚刚',
    issueAgeMin: (n) => `${n} 分钟前`,
    issueAgeHour: (n) => `${n} 小时前`,
    issueAgeDay: (n) => `${n} 天前`,
    issueAgeMonth: (n) => `${n} 个月前`,
    moreIssues: (n) => `+${n}`,
    moreIssuesHint: (n) => `查看全部 issue（还有 ${n} 个）`,
    resumeNothingToDo: '该目标下已没有可继续推进的 issue（全部已完成或已被维护者解决）。',
    installLoopxBtn: 'pip 安装 loopx',
    envCheckLoading: '正在检查环境…',
    envOnboardTitle: '首次使用 · 配置环境',
    envOnboardHint: '自动修复 issue 需要下面几项依赖，一键装好后即可开始。',
    envConfigSummary: '高级配置（模型，默认已填好）',
    envVlmModel: 'VLM 模型',
    envVlmBase: 'VLM 地址',
    envVlmKey: 'VLM API Key',
    envInstallBtn: '一键安装并配置',
    envProgressTitle: '正在安装…',
    envDoneTitle: '环境就绪',
    envDoneHint: '可以开始自动修复 issue 了。',
    envDoneBtn: '开始使用',
    envItemPython: 'Python',
    envItemGit: 'Git',
    envItemLoopx: 'loopx',
    envItemOpenViking: 'OpenViking',
    envItemOpenVikingServer: 'OpenViking 服务',
    envItemOvCli: 'ov CLI 连接',
    envItemGh: 'GitHub 登录',
    envInstallLinkBtn: '打开下载页',
    envInstallCmdHint: '或复制此命令在终端安装（装好后点「重新检测」）',
    envOvCliReconnect: '重连',
    envVerifying: '正在复检环境…',
    envVerifyOk: '复检通过，环境就绪',
    envVerifyFailed: '复检未全部通过，请检查下方清单',
    envVlmKeyEmpty: 'VLM API Key 为空：图像理解相关能力（含 OpenViking VLM 调用）将不可用，可稍后在高级配置中补填',
    bootPausedBanner: (n) => `上次有 ${n} 个任务在运行（重启后已暂停）`,
    bootPausedResumeAll: '全部继续',
    bootPausedDismiss: '忽略',
    bootPausedResumed: (n) => `已继续 ${n} 个任务`,
    diffBranchLabel: (b) => `分支：${b}`,
    installingLoopx: '正在安装…（可能需要几分钟）',
    installDone: '安装完成',
    installFailed: '安装失败',
    runOnce: '执行一次',
    resumeTask: '继续任务',
    resumeTaskHint: '继续该任务：恢复心跳监控与自动执行',
    runShort: '运行',
    stopShort: '停止',
    stopTask: '中止任务',
    stopTaskHint: '中止整个任务：取消本次运行、关闭心跳监控与自动执行（任务保留，可继续）',
    deleteTask: '删除任务',
    deleteTaskHint: '删除整个任务（该任务下所有 issue 一起删）：归档运行记录并移除注册表条目（注册表会先备份）。单个 issue 请在 issue 行点「移除」',
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
    stopConfirmText: (id) => `将取消「${id}」正在进行的运行，并关闭它的心跳监控与自动执行。任务会移入「已停表」并保留下来，随时可以继续。`,
    confirmStop: '确认中止',
    deleteConfirmTitle: '删除整个任务？',
    deleteConfirmText: (id) => `将删除整个任务「${id}」：归档运行记录、移除注册表条目（注册表会先备份）。该任务下的所有 issue 会一并删除，看板不再显示该任务。只想删单个 issue？请在 issue 行点「移除」。`,
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
    statusAutoTripped: '连续失败已暂停（悬停看原因）',
    statusAuto: '自动运行',
    statusGated: '等待审批',
    statusCooldown: (n) => `冷却中 · ${n} 分钟后重试`,
    statusExternalResolved: (n) => `${n} 项已被维护者解决`,
    stageGated: '阶段：等待你的确认',
    stageRunning: '阶段：执行中',
    stageFixing: (n) => `阶段：修复 #${n}`,
    stagePlanning: '阶段：规划中',
    skippedDuplicates: (n) => `（已跳过 ${n} 个重复 Issue）`,
    skippedResolved: (n) => `（已跳过 ${n} 个已解决的 Issue）`,
    issueResolvedLog: (num, reason) => `Issue #${num} 已被解决（${reason}），跳过修复`,
    issueResolvedStopped: (num, reason) => `Issue #${num} 已被维护者解决（${reason}），已自动跳过后续修复`,
    rewardMemoryOn: 'reward memory 已启用（issue-fix 补丁规划面）',
    rewardMemoryOff: (e) => `reward memory 未启用：${e}`,
    repoMemorySyncOn: (scope) => `仓库记忆已导入 OpenViking（${scope}）`,
    repoMemorySyncOff: (e) => `仓库记忆导入跳过：${e}`,
    repoMemorySyncStarted: '仓库记忆后台索引中（OpenViking），不影响修复进行',
    memoryIndexedOk: '仓库记忆已索引（OpenViking）',
    memoryIndexedMissing: '仓库记忆未索引：同步可能超时/失败，Agent 暂无可检索的仓库上下文',
    memoryServerDown: 'OpenViking 服务离线，仓库记忆不可用',
    runCancelled: '运行已取消',
    runCancelledByHost: '本轮被宿主取消（非手动停止），将按失败退避自动重试',
    turnStalled: (m) => `运行僵死：约 ${m} 分钟没有收到 Agent 事件（可能宿主已静默取消本轮），已取消该回合并允许自动重试。`,
    turnModelHang: (m) => `模型 API 未响应：约 ${m} 分钟没有返回第一个字（可能模型服务超时/过载），已取消该回合并允许自动重试。`,
    modelHangTitle: '模型未响应 — 需要你决定',
    modelHangText: (m) => `当前模型约 ${m} 分钟没有返回第一个字（可能限流/过载）。切换模型立即重试，或继续等待自动重试。`,
    modelHangSwitch: '切换模型',
    modelHangSwitchApply: '切换并重试',
    modelHangKeepWaiting: '继续等待',
    modelHangSwitched: (m) => `已切换为 ${m}，立即重试`,
    modelHangKeptWaiting: '继续使用当前模型，等待自动重试',
    notifModelHangTitle: '模型未响应',
    notifModelHangBody: (id, m) => `${id}：模型约 ${m} 分钟未响应，可切换模型或继续等待`,
    streamTrimmed: (n) => `（前文已折叠 ${n} 字符）`,
    groupBacklog: '排队中',
    groupActive: '进行中',
    groupReview: '等你处理',
    colSubReview: '阻塞 · 需要你批准后继续',
    colSubActive: 'Agent 正在执行',
    groupDecisions: '需决策',
    groupNotices: '知会',
    colSubDecisions: '阻塞 · 需要你批准后继续',
    colSubNotices: '受阻或暂不修复 · 仅知会',
    detailEmptyHint: '在上方选择进行中的目标查看日志；或粘贴 Issue 链接创建新任务',
    goalPickerLabel: (n) => `进行中的目标 (${n})`,
    goalPickerIssues: '正在修复',
    goalPickerIssuesHint: '查看正在修复的 Issues',
    goalPickerNoIssues: '暂无 issue 信息',
    goalPickerOpenLog: '打开日志',
    goalPickerActions: '操作',
    issueSummary: (total, done) => `${total} 个 issue · ${done} 完成`,
    issuePause: '暂停',
    issueResume: '恢复',
    issueRemove: '移除',
    issueReopen: '重开',
    issueAdd: '新增 issue',
    issueAddPlaceholder: 'Issue 链接或 #编号',
    issueAddBtn: '追加',
    issueSelected: '计划中修复的 Issue',
    issueAvailable: '仓库其它 Issue',
    issueLoading: '正在读取 Issues…',
    issueApply: '应用更改',
    issueApplyCount: (n) => `应用更改 (${n})`,
    issueNoMore: '没有其它 open issues',
    groupDone: '已完成',
    taskPlaceholder: '粘贴 GitHub Issue / 仓库 / Issues 列表链接，可附加修复要求',
    taskGuidePlaceholder: (name) => `正在向「${name}」插话：输入指令引导 Agent 继续（不创建新任务）`,
    taskLinkPlaceholder: '粘贴 GitHub Issue / 仓库 / Issues 列表链接',
    taskNotePlaceholder: '附加修复要求（可选）',
    taskGuideEmpty: '输入指令引导 Agent 继续（先在上方选择要引导的任务）',
    taskNeedLink: '请粘贴 GitHub Issue / 仓库 / Issues 列表链接',
    taskSend: '发送',
    composerModeNew: '新建修复任务',
    composerModeGuide: '引导运行中任务',
    taskGoalUnsupported: '请粘贴 GitHub Issue、PR、仓库首页或 Issues 列表链接（自由目标暂未开放）',
    taskUnsupportedPath: (u) => `不支持的 GitHub 链接：${u}。请粘贴 Issue、PR、仓库首页或 Issues 列表链接`,
    guidanceNoRunning: '没有正在运行的任务。粘贴 Issue 链接创建新任务，或等任务开始运行后再输入指令。',
    guidancePickOne: '有多个任务正在运行：请在上方「进行中的目标」里选择要纠正的目标（或先在日志面板打开它），再发送指令。',
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
    stageWriteTodos: '写入修复 todos',
    stageResolved: '跳过已解决的 issue…',
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
    notifyCount: '知会',
    decisionCount: (n) => `${n} 待决策`,
    noticeCountN: (n) => `${n} 知会`,
    noticeAck: '已读',
    notifyGroup: '知会',
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
    viewOriginal: '查看原文',
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
    rejectGate: '拒绝',
    rejectNote: '用户已拒绝该项改动',
    rejectDone: '已拒绝该项改动，任务将重新规划',
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
    ghLoginWorking: '登录中…',
    ghLoginDoneShort: '已登录',
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
    sectionNotify: '知会（无需操作）',
    notifyAck: '标记已读',
    notifyAckAll: '全部标记已读',
    notifyAckHint: '仅消除提醒，不改变 issue 状态',
    notifyResumeHint: '把该 issue 放回执行队列，让 Agent 继续修',
    notifyNoteLabel: 'Agent 说明（做了什么 + 结论）',
    notifyMore: (n) => `还有 ${n} 个知会（点击展开）`,
    notifyGroupResolved: '已解决（无需处理）',
    notifyGroupBlocked: '无法继续（受阻）',
    notifyGroupDeferred: '暂不修复',
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
    activityStarting: '正在启动 Agent…（新会话，从 loopx 注册表续跑）',
    activityResumeSession: '恢复之前的会话上下文，继续执行…',
    resumeReconcileTitle: '续跑对账（loopx 注册表，不复用会话）',
    cancelTurnLogged: '已请求取消本轮执行',
    activityWaitingModel: '等待模型响应…',
    activityWaitingModelCtx: (ctx) => `等待模型响应…（上下文约 ${ctx}）`,
    activityWaitingModelCtxEta: (ctx, eta) => `等待模型响应…（上下文约 ${ctx}，上次约 ${eta}）`,
    activityStartingDone: 'Agent 已启动',
    activityResumeDone: '会话上下文已恢复',
    activityModelResponded: '模型已响应',
    activityModelRespondedEta: (eta) => `模型已响应（${eta}）`,
    toolShell: '执行命令',
    toolRead: '读取',
    toolSearch: '搜索',
    toolWrite: '写入',
    toolEdit: '编辑',
    toolCode: '运行代码',
    toolCall: '工具',
    activityDiving: 'Bitfun努力解bug中...',
    activityStalled: '可能卡住了（无输出）',
    activityModelHang: '模型未响应（等待返回）',
    activityToolRunning: (name, dur) => `工具运行中 · ${name} · 已 ${dur}`,
    idleFor: (t) => `距上次输出 ${t}`,
    durationMinutes: (m, s) => `${m}分${s}秒`,
    durationSeconds: (s) => `${s}秒`,
    activitySentPrompt: (n) => `已向 Agent 发送指令（${n} 字符，点击展开）`,
    activityRunning: (elapsed) => `Agent 正在执行 · 已用时 ${elapsed}`,
    activityCommitted: 'bitfun-loopx 已提交本次执行结果',
    activityValidationPassed: '独立校验已通过',
    activityValidationFailed: '独立校验未通过',
    activityStateUpdated: '目标状态已更新',
    activityCompleted: '执行已完成',
    activityCompletedValidated: '执行已完成 · 校验通过',
    activityFailed: '执行失败',
    turnConclusionLabel: '本轮结论：',
    feedbackAsk: '对最近这轮满意吗？',
    feedbackGood: '有用',
    feedbackBad: '没用',
    feedbackDone: (r) => (r === 'positive' ? '已记录：有用' : '已记录：没用'),
    feedbackError: (e) => `反馈记录失败：${e}`,
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
    issueResolvedBadge: '已解决',
    issueResolvedBadgeTitle: (reason) => `该 issue 上游已关闭（${reason || 'closed'}），创建任务时会自动跳过、不再修复`,
    intakeResolvedWarn: (n) => `✓ 检测到 ${n} 个 issue 上游已解决：创建任务时会自动跳过这些 issue，不会重复修复。`,
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
    diffFilesCount: (n) => `${n} 个改动文件`,
    diffStatLabel: (a, d) => `+${a} −${d}`,
    diffLoading: '正在加载代码改动…',
    diffEmpty: '尚未检测到代码改动（分支可能尚未提交，或与主分支无差异）',
    diffViewHunk: '查看完整差异',
    diffTruncated: '（diff 已裁剪，仅显示前 40000 字符）',
    stepperPlan: '规划修复方案',
    stepperFix: '修复 Issue',
    stepperPublish: '发布 / 提交 PR',
    stepperIssuesDone: (done, total) => `Issue ${done}/${total}`,
    cancelTurn: '取消本轮',
    turnLabel: (n, time) => `第 ${n} 轮 · ${time}`,
    turnLines: (n) => `${n} 行`,
    emptyBoardTitle: '开始修复你的第一个 Issue',
    emptyBoardHint: '粘贴 GitHub Issue / 仓库链接；或输入 owner/repo 浏览该仓库的 open issues。',
    emptySampleIssue: '示例：单个 Issue',
    emptySampleRepo: '示例：仓库主页',
    emptyBrowseBtn: '浏览',
    emptyBrowsePlaceholder: 'owner/repo',
    kbHint: '快捷键：j/k 移动 · Enter 查看 · a 批准 · x 选中',
    errorBannerTitle: '最近一次错误',
    errorRetry: '重试',
    errorClearState: '清除该任务状态',
    modelVisionYes: '视觉 ✓',
    modelVisionNo: '视觉 ✗',
    composerVisionHint: '当前模型不支持读取截图：若该 Issue 依赖图片，修复质量可能受影响。',
    sbRunning: '运行中',
    sbNeedsYou: '等你',
    sbQueued: '排队中',
    sbStopped: '已停',
    sbError: '异常',
    sbDone: '已完成',
    sbArchived: '已归档',
    queuedHint: '排队中：等待 loopx 调度下一轮自动执行',
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
    issueOpen: 'fixing',
    issueBlocked: 'cannot continue',
    issueDeferred: "won't fix now",
    issuePending: 'pending',
    issueResolved: 'Resolved',
    issueResolvedExternally: (n) => `Resolved upstream, no fix needed: ${n}`,
    issueResolvedExternallyShort: 'resolved upstream',
    issueComments: (n) => `💬 ${n}`,
    issueAgeNow: 'just now',
    issueAgeMin: (n) => `${n}m ago`,
    issueAgeHour: (n) => `${n}h ago`,
    issueAgeDay: (n) => `${n}d ago`,
    issueAgeMonth: (n) => `${n}mo ago`,
    moreIssues: (n) => `+${n}`,
    moreIssuesHint: (n) => `View all issues (${n} more)`,
    resumeNothingToDo: 'No actionable issues remain in this task (all done or resolved upstream).',
    installLoopxBtn: 'pip install loopx',
    envCheckLoading: 'Checking environment…',
    envOnboardTitle: 'First-time setup',
    envOnboardHint: 'Auto-fixing issues needs these dependencies. Install them to get started.',
    envConfigSummary: 'Advanced config (model, pre-filled)',
    envVlmModel: 'VLM model',
    envVlmBase: 'VLM base URL',
    envVlmKey: 'VLM API key',
    envInstallBtn: 'Install & configure',
    envProgressTitle: 'Installing…',
    envDoneTitle: 'Environment ready',
    envDoneHint: 'You can start auto-fixing issues now.',
    envDoneBtn: 'Get started',
    envItemPython: 'Python',
    envItemGit: 'Git',
    envItemLoopx: 'loopx',
    envItemOpenViking: 'OpenViking',
    envItemOpenVikingServer: 'OpenViking server',
    envItemOvCli: 'ov CLI connection',
    envItemGh: 'GitHub sign-in',
    envInstallLinkBtn: 'Open download page',
    envInstallCmdHint: 'Or copy this command into a terminal (then press Re-check)',
    envOvCliReconnect: 'Reconnect',
    envVerifying: 'Re-checking the environment…',
    envVerifyOk: 'Re-check passed, environment ready',
    envVerifyFailed: 'Re-check did not fully pass — see the checklist',
    envVlmKeyEmpty: 'VLM API key is empty: vision features (incl. OpenViking VLM calls) will be unavailable until you fill it in under advanced config',
    bootPausedBanner: (n) => `${n} task${n > 1 ? 's were' : ' was'} running before the restart (now paused)`,
    bootPausedResumeAll: 'Resume all',
    bootPausedDismiss: 'Dismiss',
    bootPausedResumed: (n) => `${n} task${n > 1 ? 's' : ''} resumed`,
    diffBranchLabel: (b) => `branch: ${b}`,
    installingLoopx: 'Installing… (may take a few minutes)',
    installDone: 'Installation complete',
    installFailed: 'Installation failed',
    runOnce: 'Run once',
    resumeTask: 'Resume task',
    resumeTaskHint: 'Resume this task: restore heartbeat and auto-run',
    runShort: 'Run',
    stopShort: 'Stop',
    stopTask: 'Abort task',
    stopTaskHint: 'Abort the whole task: cancel the current run, disable heartbeat and auto-run (the task is kept and can be resumed)',
    deleteTask: 'Delete task',
    deleteTaskHint: 'Delete the whole task (all its issues together): archive its runtime and remove the registry entry (backed up first). To remove one issue, use Remove on that issue row',
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
    stopConfirmText: (id) => `This cancels the running turn of "${id}", switches off its heartbeat monitoring and auto-run. The task moves to "Stopped", is kept, and can be resumed anytime.`,
    confirmStop: 'Abort it',
    deleteConfirmTitle: 'Delete the whole task?',
    deleteConfirmText: (id) => `This deletes the whole task "${id}": its runtime is archived and its registry entry removed (the registry is backed up first). All issues under it are deleted too, and it no longer appears on the board. To remove a single issue, use Remove on that issue row instead.`,
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
    statusAutoTripped: 'Paused after repeated failures (hover for reason)',
    statusAuto: 'Auto-run on',
    statusGated: 'Needs approval',
    statusCooldown: (n) => `cooldown · retry in ${n}m`,
    statusExternalResolved: (n) => `${n} resolved upstream`,
    stageGated: 'Stage: waiting for your confirmation',
    stageRunning: 'Stage: working',
    stageFixing: (n) => `Stage: fixing #${n}`,
    stagePlanning: 'Stage: planning',
    skippedDuplicates: (n) => ` (${n} duplicate issue${n > 1 ? 's' : ''} skipped)`,
    skippedResolved: (n) => ` (${n} already-resolved issue${n > 1 ? 's' : ''} skipped)`,
    issueResolvedLog: (num, reason) => `Issue #${num} was already resolved (${reason}) — skipping`,
    issueResolvedStopped: (num, reason) => `Issue #${num} was resolved upstream (${reason}) — further fixing skipped automatically`,
    rewardMemoryOn: 'reward memory enabled (issue-fix patch planning surface)',
    rewardMemoryOff: (e) => `reward memory unavailable: ${e}`,
    repoMemorySyncOn: (scope) => `repository memory seeded into OpenViking (${scope})`,
    repoMemorySyncOff: (e) => `repository memory seed skipped: ${e}`,
    repoMemorySyncStarted: 'repository memory indexing in the background (OpenViking) — fixes proceed meanwhile',
    memoryIndexedOk: 'repository memory indexed (OpenViking)',
    memoryIndexedMissing: 'repository memory not indexed: sync may have timed out; agent has no retrievable repo context',
    memoryServerDown: 'OpenViking offline: repository memory unavailable',
    runCancelled: 'run cancelled',
    runCancelledByHost: 'turn cancelled by the host (not a manual stop) — auto-run retries with backoff',
    turnStalled: (m) => `Turn stalled: no agent events for about ${m} minutes (the host may have silently cancelled it) — cancelled the turn and allowed an automatic retry.`,
    turnModelHang: (m) => `Model API not responding: no first token for about ${m} minutes (the model service may have timed out / be overloaded) — cancelled the turn and allowed an automatic retry.`,
    modelHangTitle: 'Model not responding — needs you',
    modelHangText: (m) => `The current model has not produced a first token for about ${m} minutes (possibly rate-limited / overloaded). Switch models to retry now, or keep waiting for the automatic retry.`,
    modelHangSwitch: 'Switch model',
    modelHangSwitchApply: 'Switch & retry',
    modelHangKeepWaiting: 'Keep waiting',
    modelHangSwitched: (m) => `Switched to ${m}; retrying now`,
    modelHangKeptWaiting: 'Keeping the current model; waiting for the automatic retry',
    notifModelHangTitle: 'Model not responding',
    notifModelHangBody: (id, m) => `${id}: no model response for about ${m} minutes — switch model or keep waiting`,
    streamTrimmed: (n) => `(earlier content trimmed: ${n} chars)`,
    groupBacklog: 'Queued',
    groupActive: 'In progress',
    groupReview: 'Needs you',
    colSubReview: 'Blocking · continues after your approval',
    colSubActive: 'The agent is working',
    groupDecisions: 'Decisions',
    groupNotices: 'Notices',
    colSubDecisions: 'Blocking · continues after your approval',
    colSubNotices: 'Blocked or deferred · for your awareness',
    detailEmptyHint: 'Pick an in-progress goal above to view its log, or paste an issue link to create a task',
    goalPickerLabel: (n) => `In-progress goals (${n})`,
    goalPickerIssues: 'Fixing',
    goalPickerIssuesHint: 'View the issues being fixed',
    goalPickerNoIssues: 'No issue info yet',
    goalPickerOpenLog: 'Open log',
    goalPickerActions: 'Actions',
    issueSummary: (total, done) => `${total} issues · ${done} done`,
    issuePause: 'Pause',
    issueResume: 'Resume',
    issueRemove: 'Remove',
    issueReopen: 'Reopen',
    issueAdd: 'Add issue',
    issueAddPlaceholder: 'Issue URL or #number',
    issueAddBtn: 'Append',
    issueSelected: 'Issues in the fix plan',
    issueAvailable: 'Other repo issues',
    issueLoading: 'Loading issues…',
    issueApply: 'Apply changes',
    issueApplyCount: (n) => `Apply (${n})`,
    issueNoMore: 'No other open issues',
    groupDone: 'Done',
    taskPlaceholder: 'Paste a GitHub issue / repository / issues-list link, optionally with fix instructions',
    taskGuidePlaceholder: (name) => `Guiding "${name}": type instructions to steer the agent (no new task is created)`,
    taskLinkPlaceholder: 'Paste a GitHub issue / repository / issues-list link',
    taskNotePlaceholder: 'Additional fix requirements (optional)',
    taskGuideEmpty: 'Type guidance for the agent (pick a task above first)',
    taskNeedLink: 'Paste a GitHub issue / repository / issues-list link',
    taskSend: 'Send',
    composerModeNew: 'New fix task',
    composerModeGuide: 'Guide a running task',
    taskGoalUnsupported: 'Paste a GitHub issue, pull request, repository home, or issues-list link (free-form goals are not open yet)',
    taskUnsupportedPath: (u) => `Unsupported GitHub link: ${u}. Paste an issue, a pull request, the repository home, or its issues list.`,
    guidanceNoRunning: 'No task is running. Paste an issue link to create one, or wait until a task runs to send instructions.',
    guidancePickOne: 'Several tasks are running: pick the target in the goal dropdown above (or open it in the log panel) first, then send the instruction.',
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
    stageWriteTodos: 'Writing fix todos',
    stageResolved: 'Skipping already-resolved issue(s)…',
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
    notifyCount: 'FYI',
    decisionCount: (n) => `${n} to decide`,
    noticeCountN: (n) => `${n} FYI`,
    noticeAck: 'Mark read',
    notifyGroup: 'FYI',
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
    viewOriginal: 'View original',
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
    rejectGate: 'Reject',
    rejectNote: 'User rejected this change',
    rejectDone: 'Rejected this change — the task will re-plan',
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
    ghLoginWorking: 'Signing in…',
    ghLoginDoneShort: 'Signed in',
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
    sectionNotify: 'FYI (no action needed)',
    notifyAck: 'Mark read',
    notifyAckAll: 'Mark all read',
    notifyAckHint: 'Only clears the reminder; does not change the issue status',
    notifyResumeHint: 'Put the issue back in the queue for the agent to retry',
    notifyNoteLabel: 'Agent note (what it did + conclusion)',
    notifyMore: (n) => `${n} more FYI (click to expand)`,
    notifyGroupResolved: 'Resolved (no action needed)',
    notifyGroupBlocked: 'Blocked (cannot continue)',
    notifyGroupDeferred: "Won't fix now",
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
    activityStarting: 'Starting the Agent… (fresh session, resuming from the loopx registry)',
    activityResumeSession: 'Resuming the previous session context…',
    resumeReconcileTitle: 'Resume reconciliation (loopx registry, no session reuse)',
    cancelTurnLogged: 'Turn cancel requested',
    activityWaitingModel: 'Waiting for the model response…',
    activityWaitingModelCtx: (ctx) => `Waiting for the model response… (~${ctx} context)`,
    activityWaitingModelCtxEta: (ctx, eta) => `Waiting for the model response… (~${ctx} context, last ~${eta})`,
    activityStartingDone: 'Agent started',
    activityResumeDone: 'Session context restored',
    activityModelResponded: 'Model responded',
    activityModelRespondedEta: (eta) => `Model responded (${eta})`,
    toolShell: 'Run command',
    toolRead: 'Read',
    toolSearch: 'Search',
    toolWrite: 'Write',
    toolEdit: 'Edit',
    toolCode: 'Run code',
    toolCall: 'Tool',
    activityDiving: 'Bitfun is working on the bug…',
    activityStalled: 'Possibly stalled (no output)',
    activityModelHang: 'Model not responding (waiting)',
    activityToolRunning: (name, dur) => `Tool running · ${name} · ${dur}`,
    idleFor: (t) => `last output ${t} ago`,
    durationMinutes: (m, s) => `${m}m ${s}s`,
    durationSeconds: (s) => `${s}s`,
    activitySentPrompt: (n) => `Instructions sent to the agent (${n} chars, click to expand)`,
    activityRunning: (elapsed) => `Agent is working · ${elapsed} elapsed`,
    activityCommitted: 'bitfun-loopx committed this run',
    activityValidationPassed: 'Independent validation passed',
    activityValidationFailed: 'Independent validation failed',
    activityStateUpdated: 'Goal state updated',
    activityCompleted: 'Run completed',
    activityCompletedValidated: 'Run completed · validation passed',
    activityFailed: 'Run failed',
    turnConclusionLabel: 'This turn: ',
    feedbackAsk: 'Was this last turn useful?',
    feedbackGood: 'Useful',
    feedbackBad: 'Not useful',
    feedbackDone: (r) => (r === 'positive' ? 'Recorded: useful' : 'Recorded: not useful'),
    feedbackError: (e) => `Feedback failed: ${e}`,
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
    issueResolvedBadge: 'resolved',
    issueResolvedBadgeTitle: (reason) => `This issue is already closed upstream (${reason || 'closed'}) — it will be skipped and not re-fixed`,
    intakeResolvedWarn: (n) => `✓ ${n} issue(s) are already resolved upstream: they will be skipped automatically and not re-fixed.`,
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
    diffFilesCount: (n) => `${n} changed file${n > 1 ? 's' : ''}`,
    diffStatLabel: (a, d) => `+${a} −${d}`,
    diffLoading: 'Loading the code changes…',
    diffEmpty: 'No code changes detected yet (nothing committed, or the branch has no diff from main).',
    diffViewHunk: 'View full diff',
    diffTruncated: '(diff truncated to the first 40000 chars)',
    stepperPlan: 'Plan the fix',
    stepperFix: 'Fix issue',
    stepperPublish: 'Publish / submit PR',
    stepperIssuesDone: (done, total) => `Issue ${done}/${total}`,
    cancelTurn: 'Cancel turn',
    turnLabel: (n, time) => `Turn #${n} · ${time}`,
    turnLines: (n) => `${n} line${n > 1 ? 's' : ''}`,
    emptyBoardTitle: 'Fix your first issue',
    emptyBoardHint: 'Paste a GitHub issue / repository link, or enter owner/repo to browse its open issues.',
    emptySampleIssue: 'Sample: single issue',
    emptySampleRepo: 'Sample: repository',
    emptyBrowseBtn: 'Browse',
    emptyBrowsePlaceholder: 'owner/repo',
    kbHint: 'Shortcuts: j/k move · Enter open · a approve · x select',
    errorBannerTitle: 'Last error',
    errorRetry: 'Retry',
    errorClearState: 'Clear task state',
    modelVisionYes: 'vision ✓',
    modelVisionNo: 'vision ✗',
    composerVisionHint: 'The current model cannot read screenshots; if this issue depends on images, fix quality may suffer.',
    sbRunning: 'Running',
    sbNeedsYou: 'Needs you',
    sbQueued: 'Queued',
    sbStopped: 'Stopped',
    sbError: 'Errors',
    sbDone: 'Done',
    sbArchived: 'Archived',
    queuedHint: 'Queued: waiting for loopx to schedule the next auto-run turn',
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
    // Cumulative input characters sent to each goal's reused agent session, and
    // the observed time-to-first-response of its last turn. Both feed the
    // "等待模型响应" hint so the user gets a rough ETA instead of a bare spinner.
    agentInputCharsByGoal: {},
    agentTtftMsByGoal: {},
    // Authoritative context size from the host's token-usage event (inputTokens
    // + maxContextTokens). This is the REAL session context — prompts + prior
    // model output + tool results — not just the sum of the prompts we sent.
    agentInputTokensByGoal: {},
    agentMaxContextTokensByGoal: {},
    // Non-blocking notification ack: goalId -> [acknowledged issue todoIds].
    // Blocked/deferred issues surface in the review column as FYI cards (no
    // approve button); "标记已读" records the todoId here so it stops re-surfacing.
    notifyAckByGoal: {},
    // Env gate: timestamp of the last time the environment (python/git/loopx/
    // OpenViking) was confirmed ready. 0 = never confirmed → boot shows the
    // setup guide; > 0 = confirmed → boot skips the overlay and re-checks
    // silently in the background (a later breakage re-surfaces the guide).
    envReadyAt: 0,
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
  didAutoSelect: false, // 启动后自动选中第一个可操作目标，只做一次
  intakeDraft: null,
  pendingIntake: null, // resolveIntake result awaiting sheet confirmation
  moreOpen: new Set(),
  // Which group the left board column currently shows; set by the status-bar
  // tabs. Session-only UI state (not persisted).
  activeBoardTab: 'decisions',
  // Composer mode: 'new' (link-first intake) or 'guide' (steer a running task).
  composerMode: 'new',
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
  updateVisionHint();
}

// The host model catalog currently exposes only supports_text_chat — no
// vision flag — so capability detection falls back to a name heuristic.
// Unknown models are treated as text-only: the conservative reading the
// image guard needs (screenshots may carry the whole problem).
const VISION_MODEL_HINTS = /vision|multimodal|gpt-4o|o1|o3|gemini|claude|qwen[^ ]*-?vl|glm-?4v|pixtral|llava|internvl|moondream/i;
function modelEntrySupportsVision(entry) {
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
function modelSupportsVision() {
  const catalog = Array.isArray(S.modelCatalog) ? S.modelCatalog : [];
  const currentId = String(S.config.defaultModel || 'auto');
  const entry = catalog.find((m) => m && m.id === currentId)
    || catalog.find((m) => m && m.isDefault)
    || catalog[0];
  return modelEntrySupportsVision(entry);
}

// #9 视觉警告提前：输入已含 GitHub 链接且当前模型无视觉能力时，在发送前就地提示。
function updateVisionHint() {
  const el = document.getElementById('composer-vision-hint');
  if (!el) return;
  const input = document.getElementById('task-input');
  const text = input ? String(input.value || '') : '';
  const show = /https:\/\/github\.com\//i.test(text) && !modelSupportsVision();
  el.hidden = !show;
  if (show) el.textContent = t('composerVisionHint');
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
  if (S.composerMode === 'guide') {
    const g = S.composerTargetId ? S.goals.get(S.composerTargetId) : null;
    input.placeholder = g ? t('taskGuidePlaceholder', goalDisplayName(g)) : t('taskGuideEmpty');
  } else {
    input.placeholder = t('taskNotePlaceholder');
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

// ── 双态 composer ────────────────────────────────────────────
// 'new' = link-first intake (link input + optional note); 'guide' = steer a
// running task (textarea + target picker). The underlying resolveIntake /
// startGuidance flows are unchanged; this only swaps which input is visible.
function setComposerMode(mode) {
  S.composerMode = mode === 'guide' ? 'guide' : 'new';
  const newTab = document.getElementById('mode-new');
  const guideTab = document.getElementById('mode-guide');
  const linkRow = document.getElementById('composer-link-row');
  const target = document.getElementById('composer-target');
  const submitLabel = document.getElementById('btn-create-task-label');
  const submit = document.getElementById('btn-create-task');
  if (newTab) newTab.classList.toggle('is-active', S.composerMode === 'new');
  if (guideTab) guideTab.classList.toggle('is-active', S.composerMode === 'guide');
  if (linkRow) linkRow.hidden = S.composerMode === 'guide';
  if (target) target.hidden = S.composerMode === 'new';
  if (submitLabel) submitLabel.textContent = S.composerMode === 'guide' ? t('taskSend') : t('taskCreate');
  if (submit) submit.title = S.composerMode === 'guide' ? t('taskSend') : t('taskCreate');
  if (S.composerMode === 'guide' && !S.composerTargetId) {
    const running = [...S.goals.values()].filter((g) => g.running);
    if (running.length === 1) setComposerTarget(running[0].goalId);
  }
  updateTaskPlaceholder();
  updateTaskKind();
  renderLinkUnfurl();
  setTaskFeedback('');
}

// Combined objective for the 'new' mode: the pasted link plus an optional
// human note. resolveIntake / parseIssueUrls already tolerate a note after
// the URL, so this preserves the old single-textarea behavior.
function composerObjective() {
  const link = ((document.getElementById('composer-link-input') || {}).value || '').trim();
  const note = ((document.getElementById('task-input') || {}).value || '').trim();
  return note ? `${link}\n${note}` : link;
}

// Lightweight link "unfurl": parse the repo + classify the link kind without a
// network round-trip (the intake sheet fetches full issue titles later).
function parseRepoLabel(text) {
  const m = String(text || '').match(/github\.com\/([^/\s]+)\/([^/\s?#]+)/i);
  if (!m) return null;
  return `${m[1]}/${m[2].replace(/\.git$/i, '')}`;
}
function renderLinkUnfurl() {
  const input = document.getElementById('composer-link-input');
  const unfurl = document.getElementById('composer-unfurl');
  if (!input || !unfurl) return;
  const text = input.value.trim();
  if (!text || S.composerMode === 'guide') {
    unfurl.hidden = true;
    unfurl.replaceChildren();
    return;
  }
  const kind = taskInputKind(text);
  if (!kind) {
    unfurl.hidden = true;
    unfurl.replaceChildren();
    return;
  }
  unfurl.hidden = false;
  unfurl.replaceChildren();
  const repo = parseRepoLabel(text);
  const line = document.createElement('span');
  line.className = 'composer__unfurl-text';
  line.textContent = repo ? `${repo} · ${kind}` : kind;
  unfurl.appendChild(line);
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
  const state = {
    goalId,
    objective: info.objective || null,
    agents: info.agents || [],
    agentId: S.config.agentByGoal[goalId] || (info.agents && info.agents[0]) || '',
    state: info.state || null,
    waitingOn: info.waitingOn ?? null,
    // #4 turn 分组：每条活动日志打上所属轮次（executeRunOnce 每次 +1）。
    turnNumber: 0,
    // #3 当前动作：最近一次非静默工具/状态行，显示在详情面板标题下。
    currentAction: '',
    // #7 未读门禁：本会话内新出现的审批未读标记（review 列头角标 + 脉冲）。
    gateUnread: false,
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
    retryAfter: 0, // 失败后自动重试的冷却截止时间戳
    lastFailReason: '', // 上一轮失败原因，注入到下一轮 prompt 供模型参考
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
    // 外部已解决（维护者关闭 / 合并）的 issue URL 列表 + 上次实时复核时间戳。
    externalResolved: [],
    liveIssueCheckAt: 0,
    memoryStatusAt: 0, // 上次仓库记忆索引进度检查时间戳
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
      stream: !!e.stream, isTick: !!e.isTick, turn: e.turn || 0,
      done: !!e.done, failed: !!e.failed,
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
      stream: !!e.stream, isTick: !!e.isTick, turn: e.turn || 0,
      done: !!e.done, failed: !!e.failed,
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
// A tick loop the scheduler stopped (unchanged×limit → stop_tick_loop) is NOT
// permanently dead: time-based conditions (quota window reset, an issue
// closed upstream, a gate approved elsewhere) can only be observed by
// polling, so stopped goals keep a low-frequency fallback poll instead of a
// "stopped ⇒ never observes change ⇒ stays stopped" deadlock. User-stopped
// goals are excluded (monitoring=false).
const STOPPED_FALLBACK_MIN = 45;

function rearmTimer() {
  if (S.timer) { clearTimeout(S.timer); S.timer = null; }
  if (S.paused) return;
  let earliest = Infinity;
  for (const g of S.goals.values()) {
    if (g.monitoring && !g.polling && g.nextDueAt < earliest) earliest = g.nextDueAt;
  }
  if (earliest === Infinity) return;
  const delay = Math.max(0, earliest - Date.now());
  S.timer = setTimeout(onTimerFire, Math.min(delay, 2147000000));
}

function onTimerFire() {
  S.timer = null;
  const now = Date.now();
  for (const g of S.goals.values()) {
    if (g.monitoring && !g.polling && g.nextDueAt <= now) pollGoal(g);
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
    // Stopped tick loops re-poll on the slow fallback cadence (see
    // STOPPED_FALLBACK_MIN) so time-based state changes are still observed.
    g.nextDueAt = Date.now() + (g.stopped ? STOPPED_FALLBACK_MIN : g.intervalMin) * 60000;
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
  // 非阻塞知会也依赖 issue 状态：为 owned goal 加载 issue 投影（60s TTL 限流），
  // 让 blocked/deferred issue 能在 review 列出现为「知会」卡片。
  if (shouldTrackUserTodos(g) && goalProjectDir(g.goalId)) {
    refreshGoalIssues(g);
    maybeLiveIssueCheck(g);
    maybeMemoryStatus(g);
  }
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
      // #7 未读门禁：本会话内新出现的审批标记为未读（review 列头角标 + 脉冲）。
      g.gateUnread = true;
      // Load the concrete asks first so the notification names the first one
      // instead of a generic "1 item".
      refreshUserTodos(g, true).then(() => {
        if (isLiveGoal(g)) { notifyGate(g); requestRender(true); }
      });
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
  // Decision outcome for user_gate todos: approve (default) or reject.
  const outcome = opts.outcome || 'approve';
  if (button) button.disabled = true;
  try {
    // Publish gates publish by default on approve: the console forks (when
    // needed), pushes the branch and creates the PR first, then completes the
    // todo so loopx reconciles the created PR instead of pushing on its own.
    // 仅批准 skips the PR; 拒绝 neither publishes nor approves.
    if (isPublishTodo(todo) && outcome === 'approve' && opts.publish !== false) {
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
    if (isPublishTodo(todo) && (opts.publish === false || outcome === 'reject')) {
      note = [note || '', outcome === 'reject' ? t('rejectNote') : t('approveOnlyNote')].filter(Boolean).join(' · ');
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
      decisionOutcome: todo.task_class === 'user_gate' ? outcome : null,
    });
    if (!res.ok) throw new Error(res.error || 'todo complete failed');
    const todoIsGate = todo.task_class === 'user_gate';
    const doneMsg = outcome === 'reject' ? t('rejectDone')
      : (todoIsGate ? t('approveDone') : t('todoDoneFeedback'));
    log(`[${g.goalId}] ${doneMsg} (${todo.todo_id})`);
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
    g.gateUnread = false; // #7 批准即视为已读。
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
// no gate is open, fire the next turn without asking. Failures back off
// progressively (1m/2m/3m/4m/5m) before the breaker trips at 5, so a transient
// model/network hiccup no longer parks the task after just 3 misses.
const AUTO_RUN_FAIL_LIMIT = 5;
// Auto-run concurrency cap: each goal polls independently, so N repos could
// otherwise spawn N simultaneous host agent turns (API cost + machine load
// with zero feedback). Manual 执行 is never capped — only the automatic path;
// capped-out goals simply retry on their next poll.
const MAX_CONCURRENT_AUTO_RUNS = 3;
function runningTurnCount() {
  let n = 0;
  for (const g of S.goals.values()) if (g.running) n += 1;
  return n;
}

// A goal object becomes an orphan when refreshGoals/project-switch replaces
// or drops it; async callbacks holding the old reference must go quiet.
function isLiveGoal(g) {
  return S.goals.get(g.goalId) === g;
}

function canAutoRun(g) {
  // NOTE isHardGated, not isGated: an open publish approval gates only its own
  // issue (surfaced in 需决策), while loopx keeps should_run=true so the
  // goal's OTHER issues keep advancing. Blocking the whole goal here froze
  // multi-issue goals on the first PR approval.
  return g.autoRun && !g.running && isLiveGoal(g) && !isHardGated(g)
    // Only a FRESH successful decision may launch a turn — a failed poll
    // leaves g.last stale (or ok:false) and must never fire on it.
    && g.errorCount === 0 && g.last?.ok !== false
    && g.last?.shouldRun === true && g.autoFailCount < AUTO_RUN_FAIL_LIMIT
    // Failure cooldown: after a failed turn, wait before auto-retrying.
    && (!g.retryAfter || Date.now() >= g.retryAfter)
    && !!goalProjectDir(g.goalId) && !!g.agentId;
}

function maybeAutoRun(g) {
  if (!canAutoRun(g)) return;
  if (runningTurnCount() >= MAX_CONCURRENT_AUTO_RUNS) {
    // Capped: the goal stays in backlog and fires on a later poll once a
    // slot frees up (finishRun triggers pollNow on every completion).
    return;
  }
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
  g.retryAfter = 0;
  S.config.autoRunByGoal[g.goalId] = enabled;
  saveConfig();
  if (enabled) maybeAutoRun(g);
}

// ── stop / resume task ────────────────────────────────────
// "中止任务" is a FULL stop: the in-flight run is cancelled and its local run
// state is finalized immediately (the timer stops now, not when the async
// cancel event arrives), the loopx heartbeat (auto-poll) for this goal is
// switched off, auto-run is switched off, and the parked state persists
// across restarts.
async function stopGoalTask(g) {
  S.config.autoRunBeforeStop[g.goalId] = g.autoRun === true;
  if (g.running) {
    // 立即停止：取消 host turn（尽力而为）并马上收尾本地运行态——计时器必须
    // 当下停住，不能等异步的 dialog-turn-cancelled 事件（可能延迟或丢失）。
    const run = agentRuns.get(g.goalId);
    if (run) { try { app.agent.cancel(run.sessionId, run.turnId); } catch (_) {} }
    finishRun(g, { ok: false, cancelled: true, byUser: true });
  }
  if (g.autoRun) setAutoRun(g, false);
  g.monitoring = false;
  S.config.monitorByGoal[g.goalId] = false;
  g.userStopped = true;
  S.config.stoppedByGoal[g.goalId] = true;
  await saveConfig();
  rearmTimer();
  log(t('taskStopped', g.goalId));
  recordGoalActivity(g, t('taskStopped', g.goalId), true);
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
    if (g.monitoring && g.nextDueAt <= now) pollGoal(g);
  }
  rearmTimer();
}

// ── rendering ─────────────────────────────────────────────
// HARD gate: loopx itself says the goal cannot advance without a human
// (waiting_on=user / a gate state). This is the only condition that blocks
// auto-run — an open publish approval on ONE issue must not freeze the other
// issues of a multi-issue goal (loopx keeps waiting_on=codex + should_run=true
// precisely so the remaining issues keep moving).
function isHardGated(g) {
  const w = g.last && g.last.ok !== false ? g.last.waitingOn : g.waitingOn;
  if (w === 'user') return true;
  const s = String(g.last?.state || g.state || '').toLowerCase();
  return /gate|user_action|operator/.test(s);
}

function isGated(g) {
  // After a successful poll, its waiting_on is authoritative (may be null);
  // before one, fall back to the listGoals snapshot. waiting_on=controller is
  // NOT a user gate: this console runs as scheduler_owner=outer_controller,
  // i.e. it IS the controller — auto-run should take that turn, not park the
  // goal in "needs you" with nothing approvable.
  if (isHardGated(g)) return true;
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

// ── Issue 总览（方案 A：状态条 + 分组小卡）─────────────────────────
// 一个 goal 的 issue 板：把「该 goal 下所有 issue 现在各自什么状态」合并成一张
// 可扫读的看板。loopx 的 todo 状态 open/done/blocked/deferred 是主数据，外部
// 已解决（维护者关闭/合并）会覆盖成 resolved。

// 统一 issue 列表：projection 的每条 issue + externalResolved 合并去重。
function issueBoard(g) {
  const issues = (g.issues && g.issues.issues) || [];
  const extByUrl = new Map();
  for (const r of (g.externalResolved || [])) if (r.url) extByUrl.set(r.url, r);
  const board = [];
  for (const it of issues) {
    const ext = it.url ? extByUrl.get(it.url) : null;
    board.push({
      url: it.url, number: it.number, title: it.title,
      status: ext ? 'resolved' : (it.status || 'open'),
      todoId: it.todoId, reason: it.reason, resumeWhen: it.resumeWhen, note: it.note,
      external: !!ext,
    });
  }
  for (const r of (g.externalResolved || [])) {
    if (!issues.some((it) => it.url === r.url)) {
      board.push({
        url: r.url, number: r.number, title: r.title, status: 'resolved',
        todoId: null, reason: null, resumeWhen: null,
        note: r.stateReason ? `closed: ${r.stateReason}` : null, external: true,
      });
    }
  }
  return board;
}

// 分组：按「要不要用户关注」排序——无法继续/暂不修复默认展开，修复中/已解决
// 默认折叠（数量大、无需操作）。
function issueBoardGroups(g) {
  const board = issueBoard(g);
  const groups = [
    { key: 'blocked', statuses: ['blocked'], label: t('issueBlocked'), expanded: true, items: [] },
    { key: 'deferred', statuses: ['deferred'], label: t('issueDeferred'), expanded: true, items: [] },
    { key: 'open', statuses: ['open'], label: t('issueOpen'), expanded: false, items: [] },
    { key: 'resolved', statuses: ['done', 'resolved'], label: t('issueResolved'), expanded: true, items: [] },
  ];
  for (const it of board) {
    const grp = groups.find((x) => x.statuses.includes(it.status));
    if (grp) grp.items.push(it);
  }
  return groups.filter((x) => x.items.length > 0);
}

// 知会 exists when there is a blocked/deferred issue the human has not yet
// acknowledged. "标记已读" records the issue urls in notifyAckByGoal; a status
// change (or a brand-new blocked/deferred issue) re-surfaces it.
function noticeIssues(g) {
  const acked = new Set(S.config.notifyAckByGoal[g.goalId] || []);
  return issueBoard(g).filter((i) => (i.status === 'blocked' || i.status === 'deferred') && !acked.has(i.url));
}
function hasAttentionIssues(g) {
  return noticeIssues(g).length > 0;
}
function ackNotices(g) {
  const urls = issueBoard(g)
    .filter((i) => i.status === 'blocked' || i.status === 'deferred')
    .map((i) => i.url)
    .filter(Boolean);
  if (!urls.length) return;
  if (!S.config.notifyAckByGoal) S.config.notifyAckByGoal = {};
  S.config.notifyAckByGoal[g.goalId] = urls;
  saveConfig();
  renderAllGoals(true);
}

// 小卡第二行的结论：只对「有原因可说」的状态给一句中文，open 不额外占行。
function issueCardConclusion(issue) {
  if (issue.status === 'blocked' || issue.status === 'deferred') return notifyReasonZh(issue);
  if (issue.status === 'done') return t('issueDone');
  if (issue.status === 'resolved') return t('issueResolvedExternallyShort');
  return '';
}

// 一张 issue 小卡 = 整卡是一个跳转链接（用户要求只留链接）。
function buildIssueCard(g, issue) {
  const card = document.createElement('a');
  card.className = `issue-card issue-card--${issue.status}`;
  card.href = issue.url || '#';
  card.rel = 'noreferrer';
  card.onclick = (ev) => {
    ev.preventDefault();
    ev.stopPropagation();
    openExternalUrl(issue.url);
  };
  const num = document.createElement('span');
  num.className = 'issue-card__num';
  num.textContent = `#${issue.number}`;
  card.appendChild(num);
  const title = document.createElement('span');
  title.className = 'issue-card__title';
  const titleText = issueTitleClean(issue.title);
  title.textContent = titleText;
  card.appendChild(title);
  const concl = issueCardConclusion(issue);
  card.title = `#${issue.number}${titleText ? ` ${titleText}` : ''}${concl ? ` · ${concl}` : ''}`;
  if (concl) {
    const c = document.createElement('span');
    c.className = 'issue-card__concl';
    c.textContent = concl;
    card.appendChild(c);
  }
  return card;
}

// 整张 issue 看板：顶部状态总览条 + 按状态分组的小卡。
// 点总览条某段：折叠组先展开，再滚动到对应分组。
function buildIssueBoard(g) {
  const groups = issueBoardGroups(g);
  if (!groups.length) return null;
  const board = document.createElement('div');
  board.className = 'issue-board';
  const strip = document.createElement('div');
  strip.className = 'issue-overview';
  const anchorsByKey = {};
  for (const grp of groups) {
    const seg = document.createElement('button');
    seg.type = 'button';
    seg.className = `issue-overview__seg issue-overview__seg--${grp.key}`;
    seg.textContent = `${grp.label} ${grp.items.length}`;
    seg.title = grp.label;
    seg.onclick = (ev) => {
      ev.stopPropagation();
      const a = anchorsByKey[grp.key];
      if (!a) return;
      if (a.details) a.details.open = true; // 折叠组先展开
      if (a.el && a.el.scrollIntoView) a.el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    };
    strip.appendChild(seg);
  }
  board.appendChild(strip);
  for (const grp of groups) {
    if (grp.expanded) {
      const head = document.createElement('div');
      head.className = 'issue-board__group';
      head.textContent = grp.label;
      anchorsByKey[grp.key] = { el: head, details: null };
      board.appendChild(head);
      const body = document.createElement('div');
      body.className = 'issue-board__items';
      for (const it of grp.items) body.appendChild(buildIssueCard(g, it));
      board.appendChild(body);
    } else {
      const details = document.createElement('details');
      details.className = 'issue-board__details';
      anchorsByKey[grp.key] = { el: details, details };
      const summary = document.createElement('summary');
      summary.textContent = `${grp.label} (${grp.items.length})`;
      details.appendChild(summary);
      const body = document.createElement('div');
      body.className = 'issue-board__items';
      for (const it of grp.items) body.appendChild(buildIssueCard(g, it));
      details.appendChild(body);
      board.appendChild(details);
    }
  }
  return board;
}

// loopx 写的 reason/note 是英文，直接显示会让用户一头雾水。这里按 resume_when /
// reason 的关键词给一个中文归类；完整英文原文放悬停 title。
function notifyReasonZh(issue) {
  if (issue.status === 'resolved') return '已被维护者解决，无需再修复';
  const resumeWhen = String(issue.resumeWhen || '');
  const text = `${issue.reason || ''}\n${issue.note || ''}`.toLowerCase();
  // 优先用 note/reason 里的语义（owner 方向、被认领等），再退到 resume_when 的
  // 结构化条件，避免「等待其他事项完成」这种空泛归类盖过真正原因。
  if (/(owner|maintainer|direction|design|rfc|research|question|proposal|please (do not|pause|hold)|stop)/.test(text)) return '等待 owner 方向/决定';
  if (/(claim|owns|owned|duplicate|already|merged|pull request|\bpr\b|pr #)/.test(text)) return '已被他人认领/实现，不重复做';
  if (resumeWhen.startsWith('capacity_available:')) {
    const cap = resumeWhen.slice('capacity_available:'.length).trim();
    return cap ? `缺少依赖/能力：${cap}` : '缺少依赖/能力';
  }
  if (resumeWhen.startsWith('todo_done:')) return '等待关联事项完成后再继续';
  if (/(unavailable|not (on|installed|published)|missing|blocked)/.test(text)) return '依赖/条件不可用';
  return '暂不处理';
}

// The board mirrors an issue tracker, but attention comes first: ONLY the two
// things that deserve a column exist — work that needs the human (blocking)
// and work that is running. Queued auto-run goals between turns are
// intentionally invisible (they surface the moment they run or need
// approval); paused/stopped/error goals stay visible as dimmed rail cards
// (so a restart can never make a task disappear), and terminal/other-host
// goals collapse into the quiet "more" chips footer.
// Canonical board groups in tab order. "decisions" = blocking gates that need
// the human to approve/reject; "notices" = blocked/deferred issues that are
// FYI-only. Both were previously collapsed into one "review" column.
const BOARD_GROUPS = ['decisions', 'notices', 'active', 'backlog', 'paused', 'error', 'done', 'archived'];
const GROUP_I18N_KEY = {
  backlog: 'groupBacklog', active: 'groupActive',
  decisions: 'groupDecisions', notices: 'groupNotices',
  done: 'groupDone', paused: 'groupPaused', error: 'groupError',
  archived: 'groupArchived',
};
const GROUP_SUB_KEY = {
  decisions: 'colSubDecisions', notices: 'colSubNotices', active: 'colSubActive',
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
  // A human gate outranks EVERYTHING below, including a stopped tick loop:
  // "decision pending for so long that polling backed off and stopped" is the
  // classic long-unattended gate, and it must stay visible in 需决策 instead
  // of drifting into 暂停 where nobody looks for an approval.
  if (isGated(g)) return 'decisions';
  if (g.userStopped) return 'paused';
  if (g.errorCount > 0) return 'error';
  if (g.stopped) return 'paused';
  // Unread blocked/deferred issue notifications land in 知会 — FYI, not a
  // decision — so the human still sees them without an approve button. A
  // goal actively running stays in 运行中: the notice is reachable from the
  // status bar count without hiding the live run.
  if (hasAttentionIssues(g) && !g.running) return 'notices';
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

function fmtRunDuration(ms) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return m > 0 ? t('durationMinutes', m, String(s).padStart(2, '0')) : t('durationSeconds', s);
}

// Compact human size helpers for the "等待模型响应" context hint.
function formatKNumber(n) {
  const v = Number(n) || 0;
  if (v < 1000) return String(v);
  if (v < 10000) return `${(v / 1000).toFixed(1)}k`;
  return `${Math.round(v / 1000)}k`;
}
function formatKChars(n) {
  return formatKNumber(n) + (app.locale === 'en-US' ? ' chars' : ' 字符');
}
// Authoritative context label: prefer the host-reported inputTokens (which
// counts prompts + prior model output + tool results), fall back to the prompt
// char count only when no token usage has been reported yet (first turn).
function contextSizeText(g, promptChars) {
  const tokens = Number(S.config.agentInputTokensByGoal[g.goalId]) || 0;
  const max = Number(S.config.agentMaxContextTokensByGoal[g.goalId]) || 0;
  if (tokens > 0) {
    const t = `${formatKNumber(tokens)} tokens`;
    return max > 0 ? `${t} / ${formatKNumber(max)}` : t;
  }
  return formatKChars(promptChars);
}

// One-glance answer to "is this task running normally?": a colored status
// chip per card replaces the global header heartbeat readout.
function goalStatus(g) {
  if (g.archived) return { cls: 'goal-status--muted', text: t('statusArchived') };
  if (g.running) return { cls: 'goal-status--live', text: t('statusRunning') };
  if (isGated(g)) return { cls: 'goal-status--review', text: t('statusGated') };
  if (g.userStopped || g.stopped) return { cls: 'goal-status--muted', text: t('statusPaused') };
  if (g.errorCount > 0) return { cls: 'goal-status--err', text: t('statusErroring', g.errorCount) };
  // 失败后自动重试的冷却窗口：明确告诉用户「不是卡住，是在等重试」。
  if (g.retryAfter && Date.now() < g.retryAfter) {
    return { cls: 'goal-status--muted', text: t('statusCooldown', Math.max(1, Math.ceil((g.retryAfter - Date.now()) / 60000))) };
  }
  // 外部已解决：issue 已被维护者关闭/合并，不再需要本任务修复。
  if (Array.isArray(g.externalResolved) && g.externalResolved.length > 0) {
    return { cls: 'goal-status--ok', text: t('statusExternalResolved', g.externalResolved.length) };
  }
  if (!g.monitoring) return { cls: 'goal-status--muted', text: t('statusUnmonitored') };
  if (!g.autoRun) {
    // Breaker tripped (5 consecutive failures): say WHY it parked instead of
    // a bare 自动已关 — the failure reason is the recovery hint.
    if (g.lastFailReason) {
      return { cls: 'goal-status--err', text: t('statusAutoTripped'), title: g.lastFailReason };
    }
    return { cls: 'goal-status--muted', text: t('statusManual') };
  }
  return { cls: 'goal-status--ok', text: t('statusAuto') };
}

function goalStatusChip(g) {
  const s = goalStatus(g);
  const chip = document.createElement('span');
  chip.className = `goal-status ${s.cls}`;
  chip.textContent = s.text;
  if (s.title) chip.title = s.title;
  return chip;
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
  const issue = text.match(/github\.com\/([^/\s]+)\/([^/\s]+)\/(?:issues|pull)\/(\d+)/i);
  if (issue) return `${issue[1]}/${issue[2].replace(/\.git$/i, '')}#${issue[3]}`;
  const repo = text.match(/github\.com\/([^/\s]+)\/([^/\s?#]+)/i);
  if (repo) return `${repo[1]}/${repo[2].replace(/\.git$/i, '')}`;
  const dir = String(goalProjectDir(g.goalId) || '');
  const base = dir.split(/[\\/]/).filter(Boolean).pop() || '';
  if (base) return base;
  return String(g.goalId || '').replace(/^bfx-/, '');
}

// waiting_on values are loopx identifiers ('user', 'controller', …); translate
// the one that means the user instead of leaking raw ids into the UI. 'codex'
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
// Session-level "already computed" cache so a re-rendered (or re-created) goal
// never regenerates a summary the model already produced this session. Keyed by
// `${goalId}\u0000${todoId}`; survives goal-object churn, resets only on reload.
const summaryDoneSession = new Map(); // `${goalId}\u0000${todoId}` -> text

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
  const sessionKey = `${g.goalId}\u0000${todo.todo_id}`;
  if (!g.gateSummaries) g.gateSummaries = new Map();
  // Already computed this session? Re-attach to this goal object and stop — a
  // re-render or a re-created goal must not trigger another model call.
  const doneText = summaryDoneSession.get(sessionKey);
  if (doneText != null) {
    if (!g.gateSummaries.has(todo.todo_id)) g.gateSummaries.set(todo.todo_id, { status: 'done', text: doneText });
    return;
  }
  // Done summaries persist across sessions (loaded at boot); loading ones are
  // in flight. Anything else (missing / failed / empty) retries so the card
  // always converges on a ready summary without user interaction.
  const existing = g.gateSummaries.get(todo.todo_id);
  if (existing && existing.status === 'done') {
    summaryDoneSession.set(sessionKey, existing.text || '');
    return;
  }
  if (existing && existing.status === 'loading') return;
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
  // loopx's raw gate wording (e.g. "Owner approval gate: approve implementing
  // ...") is internal noise full of implementation detail — it does NOT belong
  // inline on the decision card. The card shows the localized type label + the
  // concise 背景/已完成/需要你 summary; the exact original stays available in
  // the 批准/拒绝 confirm dialog and via the 复制 button.
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
  // The raw loopx wording is not shown inline (internal noise); the 复制 button
  // embeds it for paste-out and the confirm dialog shows it at decision time.
  return card;
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
  // Defensive trim: a model chunk can open/close with a stray newline; never
  // render a leading/trailing blank line in the stream (internal newlines in
  // reasoning/multi-line output are preserved).
  const s = String(text || '').trim();
  if (s.length <= cap) return s;
  return `…${t('streamTrimmed', s.length - cap)}…\n${s.slice(-cap)}`;
}

function activityDisplayText(entry) {
  return entry.count > 1 ? `${entry.line} ×${entry.count}` : entry.line;
}

// #C think 实时预览：取最后一行，作为折叠行里不断变化的一行内容。
function latestLineOf(text) {
  const visible = String(text || '').trimEnd();
  const nl = visible.lastIndexOf('\n');
  return nl === -1 ? visible : visible.slice(nl + 1);
}

// 模型流式文本里带明显错误开头 → 视为错误行标红（agent 自述错误时更醒目）。
function isErrorText(text) {
  return /^(错误|❌|⚠|失败|error\b|failed\b|✗)/i.test(String(text || '').trim());
}

function activityLineElement(entry) {
  const row = document.createElement('div');
  row.className = 'activity-stream__line'
    + (entry.isErr ? ' activity-stream__line--err' : '')
    + (entry.kind === 'agent' && entry.stream && isErrorText(entry.line) ? ' activity-stream__line--err' : '')
    + (entry.kind ? ` activity-stream__line--${entry.kind}` : '')
    + (entry.key ? ' activity-stream__line--key' : '');
  const time = document.createElement('span');
  time.className = 'activity-stream__time';
  time.textContent = entry.time;
  row.appendChild(time);
  if (entry.kind === 'think' && entry.stream) {
    // Reasoning renders as ONE block, streamed in place and collapsed by
    // default. The summary carries a LIVE one-line preview of the latest
    // thinking line (like DSH ReasoningRow) so the model's progress is
    // visible at a glance without expanding.
    const details = document.createElement('details');
    details.className = 'activity-prompt activity-prompt--think';
    details.open = false;
    const summary = document.createElement('summary');
    const label = document.createElement('span');
    label.className = 'activity-prompt__think-label';
    label.textContent = t('thinkBlockTitle');
    const sep = document.createElement('span');
    sep.className = 'activity-prompt__think-sep';
    sep.textContent = ' · ';
    const preview = document.createElement('span');
    preview.className = 'activity-prompt__think-preview';
    preview.textContent = latestLineOf(cappedStreamText(entry.line, STREAM_DOM_CAPS.think));
    summary.append(label, sep, preview);
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
  } else if (entry.kind === 'waiting') {
    // 等待阶段：转圈；完成后变为 ✓（失败/取消收尾则为 ✗）。
    const span = document.createElement('span');
    span.className = 'activity-stream__text activity-stream__text--waiting';
    const icon = document.createElement('span');
    icon.className = entry.done
      ? (entry.failed ? 'status-icon status-icon--fail' : 'status-icon status-icon--done')
      : 'spinner';
    if (entry.done) icon.textContent = entry.failed ? '✗' : '✓';
    const label = document.createElement('span');
    label.textContent = activityDisplayText(entry);
    span.append(icon, label);
    row.appendChild(span);
  } else {
    const text = document.createElement('span');
    text.className = 'activity-stream__text';
    if (entry.key) {
      const mark = document.createElement('span');
      mark.className = 'activity-stream__keymark';
      mark.textContent = '▸ ';
      text.appendChild(mark);
    }
    // Streamed model output stays capped too — the raw view keeps the whole
    // text, the DOM only needs a readable tail window.
    text.appendChild(document.createTextNode(
      entry.stream
        ? cappedStreamText(entry.line, STREAM_DOM_CAPS.agent)
        : activityDisplayText(entry),
    ));
    row.appendChild(text);
  }
  return row;
}

// ── #4 turn 分组 ────────────────────────────────────────────
// A "turn" is one agent run (executeRunOnce). Entries carry entry.turn; turn
// 0 covers everything recorded outside a run (guidance, poll errors, …).
function activityTurnGroups(g) {
  const groups = [];
  for (const entry of g.activityLines || []) {
    const turn = entry.turn || 0;
    const grp = groups[groups.length - 1];
    if (!grp || grp.turn !== turn) groups.push({ turn, entries: [entry] });
    else grp.entries.push(entry);
  }
  return groups;
}

function activityTurnElement(group, { latest = true } = {}) {
  const details = document.createElement('details');
  details.className = 'activity-turn';
  details.dataset.turn = String(group.turn);
  // 只有最新一轮默认展开:历史轮折叠成一行摘要,点开即看(Buildkite 模式)。
  details.open = latest;
  const summary = document.createElement('summary');
  const caret = document.createElement('span');
  caret.className = 'activity-turn__caret';
  caret.textContent = '▶';
  const label = document.createElement('span');
  const first = group.entries[0];
  label.textContent = group.turn > 0
    ? t('turnLabel', group.turn, first ? first.time : '')
    : (first ? first.time : '');
  const count = document.createElement('span');
  count.className = 'activity-turn__count';
  count.textContent = t('turnLines', group.entries.length);
  summary.append(caret, label, count);
  const body = document.createElement('div');
  body.className = 'activity-turn__body';
  for (const entry of group.entries) body.appendChild(activityLineElement(entry));
  details.append(summary, body);
  return details;
}

function lastActivityRow(stream) {
  const sections = stream.querySelectorAll(':scope > .activity-turn');
  if (!sections.length) return null;
  const body = sections[sections.length - 1].querySelector('.activity-turn__body');
  return body ? body.lastElementChild : null;
}

// 单 turn 内 DOM 行数上限：一个长 turn（半小时、几十条工具行）如果无界累积，
// 会把 iframe 主线程拖垮（这正是日志卡在中间某行、后续停更的根因）。每个
// turn body 只保留最近 MAX_TURN_DOM_ROWS 行，旧行滚出 DOM；完整历史仍在
// activityLines（已按 240 行封顶）+ 持久化日志里，可重新渲染。
const MAX_TURN_DOM_ROWS = 200;
function appendActivityEntry(stream, entry) {
  const sections = stream.querySelectorAll(':scope > .activity-turn');
  const lastSection = sections.length ? sections[sections.length - 1] : null;
  const entryTurn = entry.turn || 0;
  if (lastSection && Number(lastSection.dataset.turn || '0') === entryTurn) {
    const body = lastSection.querySelector('.activity-turn__body');
    body.appendChild(activityLineElement(entry));
    while (body.children.length > MAX_TURN_DOM_ROWS) body.removeChild(body.firstChild);
  } else {
    // 新轮次开始：把上一轮折叠成一行摘要（Buildkite「自动展开最后一组」模式），
    // 让日志区聚焦当前轮，历史轮点开即看。
    if (lastSection) lastSection.open = false;
    stream.appendChild(activityTurnElement({ turn: entryTurn, entries: [entry] }));
  }
  while (stream.children.length > 240) stream.removeChild(stream.firstChild);
}

function recordGoalActivity(g, line, isErr = false, kind = null, raw = null, key = false) {
  const summary = kind === 'agent' ? String(line).trim() : activityText(line);
  if (!summary) return;
  if (!Array.isArray(g.activityLines)) g.activityLines = [];
  if (typeof g.currentActivity !== 'string') g.currentActivity = '';
  const now = new Date().toTimeString().slice(0, 8);
  // Collapse back-to-back repeats into one line with a multiplier. Covers the
  // default (kind-less) status lines AND tool lines — the agent legitimately
  // re-runs the same state-check command (e.g. loopx quota should-run) across
  // turns, and those must read as "×N", not a wall of identical rows.
  const last = g.activityLines[g.activityLines.length - 1];
  const sameKind = last && ((!last.kind && !kind) || (last.kind === 'tool' && kind === 'tool'));
  if (last && !last.isErr && !isErr && !last.isTick && sameKind
      && last.line === summary) {
    last.count = (last.count || 1) + 1;
    last.time = now;
    const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
    if (stream) {
      const row = lastActivityRow(stream);
      const textEl = row ? row.querySelector('.activity-stream__text') : null;
      if (textEl) {
        // Preserve the key marker; only the label text changes.
        const mark = textEl.querySelector('.activity-stream__keymark');
        textEl.replaceChildren();
        if (mark) textEl.appendChild(mark.cloneNode(true));
        textEl.appendChild(document.createTextNode(activityDisplayText(last)));
      }
      const timeEl = row ? row.querySelector('.activity-stream__time') : null;
      if (timeEl) timeEl.textContent = now;
    }
    g.currentActivity = summary;
    scheduleLogSave();
    return;
  }
  const entry = { time: now, line: summary, isErr, count: 1, kind, raw, key, turn: g.turnNumber || 0 };
  g.activityLines.push(entry);
  if (g.activityLines.length > 240) g.activityLines.splice(0, g.activityLines.length - 240);
  g.currentActivity = summary;

  const cardText = document.querySelector(`.goal__activity-text[data-goal="${CSS.escape(g.goalId)}"]`);
  if (cardText) cardText.textContent = summary;
  const actionEl = document.getElementById('goal-detail-action');
  if (actionEl && S.activeGoalId === g.goalId) {
    actionEl.textContent = summary;
    actionEl.hidden = false;
  }

  const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
  if (stream) {
    const follow = streamAtTail(stream);
    const emptyEl = stream.querySelector('.activity-empty');
    if (emptyEl) emptyEl.remove();
    appendActivityEntry(stream, entry);
    if (follow) streamFollowTail(stream);
  } else {
    const panel = document.getElementById('goal-detail-panel');
    if (!panel.hidden && S.activeGoalId === g.goalId) renderGoalDetails(g);
  }
  scheduleLogSave();
}

// Mark the last unresolved "waiting" line as done (spinner → ✓ / ✗) and patch
// the matching DOM row in place. Used when a startup/wait phase completes —
// and by finishRun for failed/cancelled turns, so no spinner spins forever in
// an old turn group.
function resolveWaiting(g, doneText, failed = false) {
  if (!Array.isArray(g.activityLines)) return;
  let idx = -1;
  for (let i = g.activityLines.length - 1; i >= 0; i -= 1) {
    if (g.activityLines[i].kind === 'waiting' && !g.activityLines[i].done) { idx = i; break; }
  }
  if (idx < 0) return;
  const entry = g.activityLines[idx];
  entry.done = true;
  if (failed) entry.failed = true;
  if (doneText) entry.line = doneText;
  const stream = document.querySelector(`.activity-stream[data-goal="${CSS.escape(g.goalId)}"]`);
  if (stream) {
    const rows = stream.querySelectorAll('.activity-stream__line--waiting');
    for (let i = rows.length - 1; i >= 0; i -= 1) {
      const row = rows[i];
      if (row.classList.contains('is-done')) continue;
      row.classList.add('is-done');
      const sp = row.querySelector('.spinner');
      if (sp) {
        sp.className = failed ? 'status-icon status-icon--fail' : 'status-icon status-icon--done';
        sp.textContent = failed ? '✗' : '✓';
      }
      const label = row.querySelector('.activity-stream__text--waiting span:last-child');
      if (label && doneText) label.textContent = doneText;
      break;
    }
  }
  scheduleLogSave();
}

// Resolve the "等待模型响应" spinner only when the model actually produces
// output (text stream or a tool call) — turn-start/synthetic events must not
// flip it to ✓ prematurely and make the user think the model already answered.
function markModelResponded(g) {
  if (g._modelResponded) return;
  g._modelResponded = true;
  // 模型一旦真的开始吐字，上一张「模型未响应」行动卡即失效。
  g._modelHang = null;
  // Measure the time-to-first-response and remember it: the next turn's
  // "等待模型响应" hint reuses it as a real, observed ETA (context-driven).
  const ttft = g._waitStartedAt ? Date.now() - g._waitStartedAt : 0;
  if (ttft >= 500) {
    S.config.agentTtftMsByGoal[g.goalId] = ttft;
    saveConfig();
  }
  resolveWaiting(g, ttft >= 500 ? t('activityModelRespondedEta', fmtRunDuration(ttft)) : t('activityModelResponded'));
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
  // 继续任务 → 立即打开日志面板，让用户立刻看到该任务的日志。
  openGoalDetails(g);
  if (g.userStopped) { resumeGoalTask(g); return; }
  if (g.stopped) { pollNow(g); return; }
  setAutoRun(g, true);
  pollNow(g, { force: true });
  recordGoalActivity(g, t('taskResumed', g.goalId));
  // 明确反馈：继续后若没有可推进的 issue（全部已完成/已被维护者解决），提示用户。
  if (goalHasActionableIssues(g) === false) {
    setTaskFeedback(t('resumeNothingToDo'), 'ok');
  }
}

// 该目标是否还有可推进的 issue。null = issue 数据尚未加载，无法判断。
function goalHasActionableIssues(g) {
  const board = issueBoard(g);
  if (!board.length) return null;
  return board.some((i) => i.status !== 'resolved' && i.status !== 'done');
}

function buildGoalActions(g, column) {
  const box = document.createElement('span');
  box.className = 'goal-actions' + (column ? ' goal-actions--column' : '');
  // A gated goal's decision lives inline in the 需要你决定 section (approve /
  // reject buttons on the gate card). The card-level buttons are only the
  // lifecycle controls — 继续 / 中止 / 删除 — so there is no separate 去审批
  // button: the review column IS the decision queue.
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

function buildRunItem(g, parked = false, queued = false) {
  const el = document.createElement('div');
  el.className = 'run-item' + (parked ? ' run-item--parked' : '')
    + (queued ? ' run-item--queued' : '')
    + (S.activeGoalId === g.goalId ? ' is-selected' : '');
  el.setAttribute('aria-label', g.goalId);
  el.dataset.goal = g.goalId;
  el.onclick = () => openGoalDetails(g);
  const dot = document.createElement('span');
  dot.className = queued
    ? 'dot dot--backlog'
    : (parked ? `dot dot--${g.errorCount > 0 ? 'error' : 'paused'}` : 'dot dot--active');
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
  if (status === 'resolved') return t('issueResolvedExternallyShort');
  if (status === 'open') return t('issueOpen');
  return t('issuePending');
}

function isExternallyResolved(g, url) {
  if (!url || !Array.isArray(g.externalResolved) || !g.externalResolved.length) return false;
  return g.externalResolved.some((r) => r.url === url);
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
    // 编码诊断：把每个 issue 的 title 原样打进 debug-ui.log，用于定位中文乱码发生在哪一层。
    try {
      dbgUi('goalIssues:titles', JSON.stringify((g.issues.issues || []).map((i) => ({ n: i.number, t: i.title }))));
    } catch (_) {}
  } catch (err) {
    if (!g.issues) g.issues = { issues: [], total: 0, done: 0, open: 0 };
    dbgUi('goalIssues:error', `${g.goalId} ${String(err && (err.message || err)).slice(0, 120)}`);
  } finally {
    g.issuesLoading = false;
    renderGoal(g);
    refreshDetailIssues(g); // 详情面板正在展示该 goal 时同步刷新 issue 列表
  }
}

// 外部已解决复核：轮询时向 GitHub 查一次「还有哪些未完成的 issue 其实已被
// 维护者关闭/合并」，避免 Agent 对着已经不需要修的问题白干。worker 侧已有
// 10min 结果缓存 + 全局限流退避，这里只再挡一道 RPC 往返。
// 识别后的收尾动作（不是只改 UI）：把对应的 open todo 标记 supersede，让
// 下一轮 prompt 的续跑对账不再把已解决的 issue 派给 agent；每个 issue 记一条
// 幂等的活动日志知会用户。
function maybeLiveIssueCheck(g) {
  if (Date.now() - (g.liveIssueCheckAt || 0) < 10 * 60 * 1000) return;
  g.liveIssueCheckAt = Date.now();
  app.call('loopx.liveIssueCheck', {
    argvPrefix: S.config.argvPrefix,
    projectDir: goalProjectDir(g.goalId),
    goalId: g.goalId,
  }).then((res) => {
    if (!isLiveGoal(g)) return;
    const resolved = (res && res.ok && Array.isArray(res.resolved)) ? res.resolved : [];
    if (!resolved.length) return;
    const known = new Set((g.externalResolved || []).map((r) => r.url));
    const fresh = resolved.filter((r) => !known.has(r.url));
    if (!fresh.length) return;
    g.externalResolved = (g.externalResolved || []).concat(fresh);
    // 知会 + 止损：活动日志一条（含关闭原因），并 supersede 掉仍 open 的
    // 对应 todo——否则 agent 下一轮还会照着注册表继续修这个已关闭的 issue。
    const byUrl = new Map(((g.issues && g.issues.issues) || []).map((i) => [i.url, i]));
    for (const r of fresh) {
      recordGoalActivity(g, t('issueResolvedStopped', r.number ?? '?', r.stateReason || r.state || 'closed'));
      const row = byUrl.get(r.url);
      if (row && row.todoId && row.status !== 'done') {
        app.call('loopx.supersedeTodo', {
          argvPrefix: S.config.argvPrefix,
          projectDir: goalProjectDir(g.goalId),
          goalId: g.goalId,
          todoId: row.todoId,
          reason: `resolved upstream (${r.stateReason || r.state || 'closed'})`,
        }).then((done) => {
          if (done && done.ok && isLiveGoal(g)) {
            refreshGoalIssues(g, true);
            try { app.call('loopx.invalidateResumeCache', {}).catch(() => {}); } catch (_) {}
          }
        }).catch(() => {});
      }
    }
    renderGoal(g);
  }).catch(() => {});
}

// 仓库记忆索引进度：启动/轮询时查一次 OpenViking 是否在线 + 该仓库 scope 是否
// 已落库。把「同步静默失败（超时导致 scope 为空）」暴露成一条知会，而不是假装
// 记忆可用。只跑一次（per-goal 时间戳），worker 侧不缓存（跨 goal 无共享状态）。
function maybeMemoryStatus(g) {
  const repo = goalRepoFromObjective(g);
  if (!repo) return;
  if (Date.now() - (g.memoryStatusAt || 0) < 30 * 60 * 1000) return;
  g.memoryStatusAt = Date.now();
  app.call('loopx.memoryStatus', { repoLabel: repo }).then((res) => {
    if (!isLiveGoal(g)) return;
    // 状态去重：同一状态只记一次活动行，避免每 30 分钟重复刷屏。
    const state = !res || res.serverOk === false ? 'down'
      : (res.resourceExists === false ? 'missing' : 'ok');
    if (g._memoryStatusLogged === state) return;
    g._memoryStatusLogged = state;
    if (state === 'down') {
      recordGoalActivity(g, t('memoryServerDown'), true);
    } else if (state === 'missing') {
      recordGoalActivity(g, t('memoryIndexedMissing'), true);
    } else {
      recordGoalActivity(g, t('memoryIndexedOk'));
    }
  }).catch(() => {});
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
// 去掉 loopx/GitHub 里的类型前缀（"[Feature]: "、"[Question]: "、"【疑问】" 等），
// 给一个可读的短标题，让 issue 胶囊从「一串 #编号」变成「#编号 + 一句话说清是什么」。
function issueTitleClean(title) {
  if (!title) return '';
  return String(title)
    .replace(/^(?:\[[^\]]+\]|【[^】]+】)\s*[:：]?\s*/i, '')
    .replace(/\s+/g, ' ')
    .trim();
}
function issueTitleShort(title) {
  const cleaned = issueTitleClean(title);
  return cleaned.length > 30 ? `${cleaned.slice(0, 30)}…` : cleaned;
}

// 沙箱 iframe 里 `target="_blank"` 会被拦（无 allow-popups），跳不动。
// 统一走宿主 `app.system.openExternal` 用系统浏览器打开，失败再退 window.open。
function openExternalUrl(url) {
  if (!url) return;
  try {
    if (app.system && app.system.openExternal) app.system.openExternal(url);
    else window.open(url, '_blank', 'noopener');
  } catch (_) {
    try { window.open(url, '_blank', 'noopener'); } catch (_) {}
  }
}

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
  // brief（goal 卡片 ①）：批量 goal 只给一行进度计数，不再平铺一串胶囊——
  // 截断后的英文短标题对用户是噪音，还占满整卡。单 issue 仍显示唯一胶囊。
  const briefBatch = opts.brief && !single;
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
  } else if (!briefBatch) {
    rows.slice(0, chipLimit).forEach((row) => {
      const external = isExternallyResolved(g, row.url);
      const status = external
        ? 'resolved'
        : single
          ? (goalGroup(g) === 'done' ? 'done' : 'open')
          : (row.info ? row.info.status : 'pending');
      const chip = document.createElement('a');
      chip.className = `issue-chip issue-chip--${status}`;
      chip.href = row.url;
      chip.rel = 'noreferrer';
      const short = issueTitleShort(row.info && row.info.title);
      chip.textContent = short ? `#${row.number} ${short}` : `#${row.number}`;
      const label = row.info && row.info.title ? `#${row.number} ${row.info.title}` : `#${row.number}`;
      chip.title = `${label} · ${issueStatusLabel(status)}`;
      // The chip is a link, not a card/row activation.
      chip.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); openExternalUrl(row.url); };
      strip.appendChild(chip);
    });
    if (rows.length > chipLimit) {
      const more = document.createElement('button');
      more.type = 'button';
      more.className = 'issue-chip issue-chip--more';
      const rest = rows.length - chipLimit;
      more.textContent = t('moreIssues', rest);
      more.title = t('moreIssuesHint', rest);
      more.onclick = (ev) => { ev.stopPropagation(); openGoalDetails(g); };
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

// ── 紧凑看板行（左列）─────────────────────────────────────────
// The left column is a scannable queue, not an encyclopedia: one row per goal
// with the group-specific "what do I do next" in the first lines. Full context
// (issue board, gate background, log) lives in the right panel.
function buildDecisionSummary(g, blockingTodos) {
  const wrap = document.createElement('div');
  wrap.className = 'board-row__summary';
  const td = blockingTodos[0];
  if (!td) {
    wrap.textContent = t('groupDecisions');
    return wrap;
  }
  const info = gateTodoInfo(td);
  const title = document.createElement('span');
  title.className = 'board-row__summary-title';
  title.textContent = info.title;
  wrap.appendChild(title);
  const sum = g.gateSummaries && g.gateSummaries.get(td.todo_id);
  if (sum && sum.status === 'done' && sum.text) {
    const line = cleanGateSummary(String(sum.text)).split('\n').filter((s) => s.trim())[0];
    if (line) {
      const preview = document.createElement('span');
      preview.className = 'board-row__summary-preview';
      preview.textContent = line;
      wrap.appendChild(preview);
    }
  }
  if (blockingTodos.length > 1) {
    const more = document.createElement('span');
    more.className = 'board-row__summary-more';
    more.textContent = t('moreIssues', blockingTodos.length - 1);
    wrap.appendChild(more);
  }
  return wrap;
}

function buildNoticeSummary(g, attentionIssues) {
  const wrap = document.createElement('div');
  wrap.className = 'board-row__summary board-row__summary--notice';
  for (const it of attentionIssues.slice(0, 4)) {
    const chip = document.createElement('a');
    chip.className = `issue-chip issue-chip--${it.status}`;
    chip.href = it.url || '#';
    chip.rel = 'noreferrer';
    chip.textContent = `#${it.number}`;
    chip.title = it.title ? `#${it.number} ${it.title}` : `#${it.number}`;
    chip.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); openExternalUrl(it.url); };
    wrap.appendChild(chip);
  }
  if (attentionIssues.length > 4) {
    const more = document.createElement('span');
    more.className = 'issue-chip issue-chip--more';
    more.textContent = t('moreIssues', attentionIssues.length - 4);
    wrap.appendChild(more);
  }
  return wrap;
}

function buildBoardRow(g) {
  const group = goalGroup(g);
  const el = document.createElement('div');
  el.className = 'board-row'
    + ` board-row--${group}`
    + (S.activeGoalId === g.goalId ? ' is-selected' : '');
  el.setAttribute('role', 'button');
  el.dataset.goal = g.goalId;
  if (!g.archived) {
    el.tabIndex = 0;
    el.onclick = () => openGoalDetails(g);
  }

  const dot = document.createElement('span');
  dot.className = `dot dot--${group}`;
  el.appendChild(dot);

  const main = document.createElement('div');
  main.className = 'board-row__main';

  // Title line: name + badge + status chip + lifecycle actions.
  const title = document.createElement('div');
  title.className = 'board-row__title';
  const name = document.createElement('span');
  name.className = 'board-row__name';
  name.textContent = goalDisplayName(g);
  name.title = g.goalId;
  title.appendChild(name);
  const todos = Array.isArray(g.userTodos) ? g.userTodos : [];
  const blockingTodos = todos.filter((td) => gateTodoInfo(td).isBlocking);
  const attentionIssues = noticeIssues(g);
  if (group === 'decisions' && blockingTodos.length > 0) {
    const badge = document.createElement('span');
    badge.className = 'board-row__badge board-row__badge--decision';
    badge.textContent = t('decisionCount', blockingTodos.length);
    title.appendChild(badge);
  } else if (group === 'notices' && attentionIssues.length > 0) {
    const badge = document.createElement('span');
    badge.className = 'board-row__badge board-row__badge--notice';
    badge.textContent = t('noticeCountN', attentionIssues.length);
    title.appendChild(badge);
  }
  title.appendChild(goalStatusChip(g));
  const actions = document.createElement('span');
  actions.className = 'board-row__actions';
  if (!g.archived) {
    actions.appendChild(buildGoalActions(g, false));
  } else {
    const restore = document.createElement('button');
    restore.type = 'button';
    restore.className = 'btn btn--tiny btn--primary';
    restore.textContent = t('restoreTask');
    restore.onclick = (ev) => { ev.stopPropagation(); restoreArchivedGoal(g, restore); };
    actions.appendChild(restore);
  }
  title.appendChild(actions);
  main.appendChild(title);

  // Sub line: the scannable "what is happening / what needs you".
  const sub = document.createElement('div');
  sub.className = 'board-row__sub';
  if (group === 'decisions') {
    sub.appendChild(buildDecisionSummary(g, blockingTodos));
  } else if (group === 'notices') {
    sub.appendChild(buildNoticeSummary(g, attentionIssues));
  } else if (group === 'done') {
    sub.textContent = goalConclusion(g);
  } else if (group === 'archived') {
    sub.textContent = t('archivedHint');
  } else {
    const narration = goalNarration(g);
    if (narration) {
      const n = document.createElement('div');
      n.className = 'board-row__narration';
      n.textContent = narration;
      sub.appendChild(n);
    }
    const strip = buildIssueStrip(g, { rail: true });
    if (strip) sub.appendChild(strip);
    if (g.currentActivity) {
      const act = document.createElement('div');
      act.className = 'board-row__activity';
      act.textContent = g.currentActivity;
      sub.appendChild(act);
    }
    if (group === 'error' && g.lastError) {
      const err = document.createElement('div');
      err.className = 'board-row__error';
      err.textContent = g.lastError;
      err.title = g.lastError;
      sub.appendChild(err);
    }
  }
  main.appendChild(sub);

  // Decision line (需决策 only): inline approve button(s) bound to the first
  // blocking gate(s); full diff/context opens in the right panel.
  if (group === 'decisions') {
    const decision = document.createElement('div');
    decision.className = 'board-row__decision';
    for (const td of blockingTodos.slice(0, 2)) {
      const info = gateTodoInfo(td);
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn btn--tiny btn--approve';
      btn.textContent = info.isPublish ? t('approveAndPr') : t('approveGate');
      btn.onclick = (ev) => { ev.stopPropagation(); openApproveDialog(g, td); };
      decision.appendChild(btn);
    }
    if (blockingTodos.length > 2) {
      const more = document.createElement('span');
      more.className = 'board-row__decision-more';
      more.textContent = `+${blockingTodos.length - 2}`;
      decision.appendChild(more);
    }
    main.appendChild(decision);
  } else if (group === 'notices') {
    const ack = document.createElement('button');
    ack.type = 'button';
    ack.className = 'btn btn--tiny';
    ack.textContent = t('noticeAck');
    ack.onclick = (ev) => { ev.stopPropagation(); ackNotices(g); };
    main.appendChild(ack);
  }

  el.appendChild(main);
  return el;
}

// ── #2 阶段流水线 stepper（横向内联，放进标题行右侧空白区） ──
function stepperInlineStep(label, state) {
  const s = document.createElement('span');
  s.className = `stepper-inline__step stepper-inline__step--${state}`;
  s.textContent = label;
  return s;
}

function buildGoalStepperContent(g) {
  if (!objectiveHasIssueSignal(g.objective || '')) return null;
  const terminal = isTerminal(g);
  const issues = (g.issues && g.issues.issues) || [];
  const doneN = issues.filter((i) => i.done).length;
  const totalN = issues.length;
  const publishGated = isGated(g) && Array.isArray(g.userTodos)
    && g.userTodos.some((td) => isPublishTodo(td));
  const started = terminal || g.running || g.lastRun || doneN > 0;

  const wrap = document.createElement('span');
  wrap.className = 'stepper-inline';
  const sep = () => {
    const s = document.createElement('span');
    s.className = 'stepper-inline__sep';
    s.textContent = '›';
    return s;
  };
  wrap.appendChild(stepperInlineStep((terminal || started ? '✓ ' : '') + t('stepperPlan'), terminal || started ? 'done' : 'active'));
  wrap.appendChild(sep());
  const fixLabel = totalN > 0 ? `${t('stepperFix')} ${doneN}/${totalN}` : t('stepperFix');
  const fixState = terminal ? 'done' : (publishGated ? 'done' : (started ? 'active' : 'pending'));
  wrap.appendChild(stepperInlineStep(fixLabel, fixState));
  wrap.appendChild(sep());
  const pubState = terminal ? 'done' : (publishGated ? 'active' : 'pending');
  wrap.appendChild(stepperInlineStep(t('stepperPublish'), pubState));
  return wrap;
}

function renderGoalStepperInline(g) {
  const el = document.getElementById('goal-detail-stepper');
  if (!el) return;
  const content = buildGoalStepperContent(g);
  if (!content) {
    el.hidden = true;
    el.replaceChildren();
    return;
  }
  el.hidden = false;
  el.replaceChildren(content);
  const single = parseIssueUrls(g.objective || '').length === 1;
  if (!single && !g.issues && !g.issuesLoading) refreshGoalIssues(g);
}

// ── #8 错误恢复横幅 ────────────────────────────────────────
// 失败不是只给一个红色 chip：给出最后错误 + 可执行动作（重试 / 清状态）。
function buildGoalErrorBanner(g) {
  if (g.errorCount <= 0 && !g.lastError) return null;
  const banner = document.createElement('div');
  banner.className = 'goal__error-banner';
  const title = document.createElement('div');
  title.className = 'goal__section-label';
  title.textContent = `${t('errorBannerTitle')} ×${g.errorCount || 1}`;
  banner.appendChild(title);
  const text = document.createElement('div');
  text.className = 'goal__error-text';
  text.textContent = g.lastError || t('activityFailed');
  text.title = g.lastError || '';
  banner.appendChild(text);
  const actions = document.createElement('div');
  actions.className = 'goal__error-actions';
  const retry = document.createElement('button');
  retry.type = 'button';
  retry.className = 'btn btn--tiny btn--primary';
  retry.textContent = t('errorRetry');
  retry.onclick = () => { g.errorCount = 0; g.lastError = null; pollNow(g, { force: true }); };
  actions.appendChild(retry);
  const clear = document.createElement('button');
  clear.type = 'button';
  clear.className = 'btn btn--tiny';
  clear.textContent = t('errorClearState');
  clear.onclick = () => openResetConfirm();
  actions.appendChild(clear);
  banner.appendChild(actions);
  return banner;
}

function renderGoalErrorBanner(g, body) {
  let banner = body.querySelector('.goal__error-banner');
  const fresh = buildGoalErrorBanner(g);
  if (!fresh) {
    if (banner) banner.remove();
    return;
  }
  if (!banner) {
    banner = document.createElement('div');
    banner.className = 'goal__error-banner';
    body.insertBefore(banner, body.firstChild);
  }
  banner.replaceChildren();
  while (fresh.firstChild) banner.appendChild(fresh.firstChild);
}

// ── 模型未响应行动卡 ────────────────────────────────────────
// 模型长时间不返回第一个字时，不是只把 run 取消 + 自动重试：弹一张非阻塞行动卡，
// 给用户「切换模型立即重试」或「继续等待自动重试」两个选择。卡是提示、不拦截轮询，
// 所以无论用户是否点，自动重试照常进行；点了切换模型则立刻换模型重跑。
function modelHangSig(g) {
  return g._modelHang ? `${g.goalId}|${g._modelHang.idleMin}|${g._modelHang.model}` : '';
}

// 把某目标已选模型写入 per-goal 覆盖并立即重跑（不碰全局默认，只救当前卡住的任务）。
async function applyModelHangSwitch(g, newModel) {
  if (newModel && newModel !== 'auto') {
    S.config.modelByGoal[g.goalId] = newModel;
  } else if (newModel === 'auto') {
    // 「自动」也作为显式覆盖写入，表示该任务明确跟随 BitFun 策略。
    S.config.modelByGoal[g.goalId] = 'auto';
  }
  await saveConfig();
  // 清除失败冷却，让本轮立刻重跑而不是等 autoFailCount 分钟的退避。
  g.errorCount = 0;
  g.lastError = null;
  g.autoFailCount = 0;
  g.retryAfter = 0;
  g._modelHang = null;
  const name = newModel || 'auto';
  log(`[${g.goalId}] ${t('modelHangSwitched', name)}`);
  recordGoalActivity(g, t('modelHangSwitched', name), false, 'agent');
  renderGoalDetails(g);
  requestRender();
  pollNow(g, { force: true });
}

function buildModelHangCard(g) {
  if (!g._modelHang) return null;
  const card = document.createElement('div');
  card.className = 'goal__model-hang';
  const title = document.createElement('div');
  title.className = 'goal__section-label';
  title.textContent = t('modelHangTitle');
  card.appendChild(title);
  const text = document.createElement('div');
  text.className = 'goal__model-hang-text';
  text.textContent = t('modelHangText', g._modelHang.idleMin);
  card.appendChild(text);
  const actions = document.createElement('div');
  actions.className = 'goal__model-hang-actions';
  const select = document.createElement('select');
  select.className = 'goal__model-hang-select';
  fillModelSelect(select, modelForGoal(g.goalId), false);
  actions.appendChild(select);
  const apply = document.createElement('button');
  apply.type = 'button';
  apply.className = 'btn btn--tiny btn--primary';
  apply.textContent = t('modelHangSwitchApply');
  apply.onclick = () => applyModelHangSwitch(g, select.value);
  actions.appendChild(apply);
  const wait = document.createElement('button');
  wait.type = 'button';
  wait.className = 'btn btn--tiny';
  wait.textContent = t('modelHangKeepWaiting');
  wait.onclick = () => {
    g._modelHang = null;
    log(`[${g.goalId}] ${t('modelHangKeptWaiting')}`);
    recordGoalActivity(g, t('modelHangKeptWaiting'), false, 'agent');
    renderGoalDetails(g);
    requestRender();
  };
  actions.appendChild(wait);
  card.appendChild(actions);
  return card;
}

function renderModelHangCard(g, body) {
  let card = body.querySelector('.goal__model-hang');
  const fresh = buildModelHangCard(g);
  if (!fresh) {
    if (card) card.remove();
    return;
  }
  // 只要没换目标、没换卡内容，就不重建（select 正在被用户操作时不能丢焦点）。
  if (card && card.dataset.sig === modelHangSig(g)) return;
  if (!card) {
    card = document.createElement('div');
    card.className = 'goal__model-hang';
    const errBanner = body.querySelector('.goal__error-banner');
    body.insertBefore(card, errBanner ? errBanner.nextSibling : body.firstChild);
  }
  card.dataset.sig = modelHangSig(g);
  card.replaceChildren();
  while (fresh.firstChild) card.appendChild(fresh.firstChild);
}

// ── 决策工作台 ──────────────────────────────────────────────
// The right panel's content for 需决策/知会 goals: full gate context (and
// diff-first approval via the gate card's 批准 button) above the live log.
function decisionFingerprint(g) {
  const blocking = (g.userTodos || []).filter((td) => gateTodoInfo(td).isBlocking).map((td) => td.todo_id).join(',');
  const notices = noticeIssues(g).map((i) => i.url).join(',');
  const summaries = [...(g.gateSummaries || new Map())].map(([k, v]) => `${k}:${v && v.status}`).join(',');
  return `${blocking}|${notices}|${summaries}`;
}

function renderDecisionWorkspace(g) {
  const body = document.getElementById('goal-detail-body');
  if (!body) return;
  const sig = decisionFingerprint(g);
  let ws = body.querySelector(':scope > .decision-workspace');
  if (ws && ws.dataset.sig === sig) return; // unchanged: keep scroll/focus
  if (!ws) {
    ws = document.createElement('div');
    ws.className = 'decision-workspace';
    body.insertBefore(ws, body.firstChild);
  }
  ws.dataset.sig = sig;
  ws.replaceChildren();

  const group = goalGroup(g);
  const head = document.createElement('div');
  head.className = 'decision-workspace__head';
  head.textContent = group === 'decisions' ? t('sectionDecision') : t('groupNotices');
  ws.appendChild(head);

  if (group === 'decisions') {
    const blocking = (g.userTodos || []).filter((td) => gateTodoInfo(td).isBlocking);
    if (blocking.length) {
      const list = document.createElement('div');
      list.className = 'gate-items';
      for (const td of blocking) list.appendChild(buildGateItemCard(g, td));
      ws.appendChild(list);
    } else {
      const none = document.createElement('div');
      none.className = 'goal__gate-none';
      none.textContent = t('gateEmptyHint');
      ws.appendChild(none);
    }
  }
  const board = buildIssueBoard(g);
  if (board) ws.appendChild(board);
  if (group === 'notices') {
    const ack = document.createElement('button');
    ack.type = 'button';
    ack.className = 'btn btn--small';
    ack.textContent = t('noticeAck');
    ack.onclick = () => ackNotices(g);
    ws.appendChild(ack);
  }
}

function removeDecisionWorkspace(body) {
  const ws = body && body.querySelector(':scope > .decision-workspace');
  if (ws) ws.remove();
}

// ── Issues 管理 ──────────────────────────────────────────────
// 进行中/排队/暂停/异常 目标的右侧面板在日志上方渲染 issue 管理（勾选增删）。
// 旧「进行中下拉框」的二级菜单内容移到这里，功能保留。
function issueManagementSig(g) {
  const issues = (g.issues && g.issues.issues) || [];
  return issues.map((i) => `${i.url}:${i.status}:${i.todoId || ''}`).join(',');
}

function renderIssueManagement(g) {
  const body = document.getElementById('goal-detail-body');
  if (!body) return;
  const group = goalGroup(g);
  const show = group === 'active' || group === 'backlog' || group === 'paused' || group === 'error';
  let sec = body.querySelector(':scope > .detail-issues');
  if (!show) {
    if (sec) sec.remove();
    return;
  }
  const sig = issueManagementSig(g);
  if (!sec) {
    sec = document.createElement('div');
    sec.className = 'detail-issues';
    sec.dataset.goalId = g.goalId;
    const stream = body.querySelector(':scope > .activity-stream');
    body.insertBefore(sec, stream || body.firstChild);
  } else if (sec.dataset.goalId !== g.goalId) {
    sec.dataset.goalId = g.goalId;
    sec.dataset.sig = '';
  }
  if (sec.dataset.sig === sig) return;
  sec.dataset.sig = sig;
  resetGoalPickerSelection(g);
  sec.replaceChildren();
  // 总览：所有 issue 按状态分组（受阻/暂不修复/修复中/已解决），一眼看到全貌。
  const board = buildIssueBoard(g);
  if (board) sec.appendChild(board);
  sec.appendChild(buildGoalPickerDetailContent(g));
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
  // #2 阶段流水线改为横向内联，放进标题行右侧的空白区（不再占 body 高度）。
  renderGoalStepperInline(g);
  // #3 当前动作标题行 + 内联停止任务。
  const actionEl = document.getElementById('goal-detail-action');
  if (actionEl) {
    actionEl.textContent = g.currentActivity || '';
    actionEl.hidden = !g.currentActivity;
  }
  const body = document.getElementById('goal-detail-body');

  // #8 错误恢复横幅在日志上方。
  renderGoalErrorBanner(g, body);
  // 模型未响应行动卡紧随其后（有错误横幅时放在其下方，否则置顶）。
  renderModelHangCard(g, body);

  // 决策工作台：需决策/知会 目标在日志上方渲染完整审批上下文；其它组别的右侧
  // 面板保持纯日志 + issue 管理（进行中/排队/暂停/异常）。
  const decisionMode = group === 'decisions' || group === 'notices';
  if (decisionMode) {
    renderDecisionWorkspace(g);
  } else {
    removeDecisionWorkspace(body);
    renderIssueManagement(g);
  }

  // The panel's log groups by turn (one collapsible section per run). Gate
  // items live on the review cards; the log stays the live activity surface.
  let stream = body.querySelector('.activity-stream');
  const groups = activityTurnGroups(g);
  const sectionCount = stream ? stream.querySelectorAll(':scope > .activity-turn').length : 0;
  const domRows = stream ? stream.querySelectorAll(':scope > .activity-turn .activity-stream__line').length : 0;
  const fresh = !stream || stream.dataset.goal !== g.goalId
    || sectionCount > groups.length
    || domRows > g.activityLines.length;

  if (fresh) {
    if (stream) stream.remove();
    stream = document.createElement('div');
    stream.className = 'activity-stream activity-stream--panel';
    stream.dataset.goal = g.goalId;
    if (groups.length > 0) {
      for (let i = 0; i < groups.length; i += 1) {
        stream.appendChild(activityTurnElement(groups[i], { latest: i === groups.length - 1 }));
      }
    } else {
      const empty = document.createElement('div');
      empty.className = 'activity-empty';
      empty.textContent = g.running ? t('activityStarting') : t('activityEmpty');
      stream.appendChild(empty);
    }
    body.appendChild(stream);
    // Land on the latest SYNCHRONOUSLY so no frame paints at the top, then
    // settle once more after the next frame for any late layout shifts. In
    // decision mode the workspace (not the log tail) is the primary surface,
    // so stay at the top instead.
    body.scrollTop = decisionMode ? 0 : body.scrollHeight;
    updateLogBottomBtn();
    requestAnimationFrame(() => {
      for (const pre of stream.querySelectorAll('.activity-prompt--think pre')) {
        pre.scrollTop = pre.scrollHeight;
      }
      body.scrollTop = decisionMode ? 0 : body.scrollHeight;
      updateLogBottomBtn();
    });
    return;
  }

  // In-sync stream: append only the missing turn sections / entries.
  const follow = streamAtTail(stream);
  const sections = stream.querySelectorAll(':scope > .activity-turn');
  if (sections.length < groups.length) {
    // 新轮次的分组补进来时,折叠此前的最后一组,保持只展开当前轮。
    if (sections.length > 0) sections[sections.length - 1].open = false;
    for (let i = sections.length; i < groups.length; i += 1) {
      stream.appendChild(activityTurnElement(groups[i], { latest: i === groups.length - 1 }));
    }
  } else if (sections.length === groups.length && groups.length > 0) {
    const lastGroup = groups[groups.length - 1];
    const lastBody = sections[sections.length - 1].querySelector('.activity-turn__body');
    while (lastBody.children.length < lastGroup.entries.length) {
      const row = activityLineElement(lastGroup.entries[lastBody.children.length]);
      lastBody.appendChild(row);
      const pre = row.querySelector('.activity-prompt--think pre');
      if (pre) pre.scrollTop = pre.scrollHeight;
    }
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
// ── #1 diff-first 审批 ─────────────────────────────────────
// 发布门禁在批准前加载分支改动（文件 + 每文件 +/- 行数 + 可展开的 unified
// diff），复用 worker 的 loopx.gitDiff（numstat + hunks），把"看懂了再批准"
// 变成默认流程，而不是盲批一份三行 AI 摘要。
async function loadApproveDiff(g, branch = null) {
  const container = document.getElementById('approve-diff');
  if (!container) return;
  const countEl = document.getElementById('approve-diff-count');
  const statEl = document.getElementById('approve-diff-stat');
  const listEl = document.getElementById('approve-diff-list');
  const hunkEl = document.getElementById('approve-diff-hunk');
  const hunkSummary = document.getElementById('approve-diff-hunk-summary');
  const hunkBody = document.getElementById('approve-diff-hunk-body');
  container.hidden = false;
  countEl.textContent = t('diffLoading');
  statEl.textContent = '';
  listEl.replaceChildren();
  hunkEl.hidden = true;
  try {
    // Publish gates name a per-issue branch; the human must review THAT
    // branch's changes, not whatever HEAD happens to be checked out (another
    // issue's branch, or the pristine default branch showing "no changes").
    const res = await app.call('loopx.gitDiff', {
      projectDir: goalProjectDir(g.goalId),
      branch: branch || null,
    });
    if (!res || !res.ok) throw new Error((res && res.error) || 'diff failed');
    const files = (Array.isArray(res.numstat) && res.numstat.length)
      ? res.numstat
      : (res.files || []).map((p) => ({ path: p, added: 0, deleted: 0 }));
    countEl.textContent = t('diffFilesCount', files.length);
    // Show which branch is being compared so the user can confirm the diff
    // matches the branch the PR will publish.
    statEl.textContent = [branch ? t('diffBranchLabel', branch) : '', res.stat || '']
      .filter(Boolean).join(' · ');
    if (!files.length) {
      const empty = document.createElement('div');
      empty.className = 'approve-diff__loading';
      empty.textContent = t('diffEmpty');
      listEl.appendChild(empty);
    } else {
      for (const f of files.slice(0, 60)) {
        const row = document.createElement('div');
        row.className = 'approve-diff__file';
        const path = document.createElement('span');
        path.className = 'approve-diff__file-path';
        path.textContent = f.path;
        path.title = f.path;
        const add = document.createElement('span');
        add.className = 'approve-diff__file-num approve-diff__file-num--add';
        add.textContent = `+${f.added}`;
        const del = document.createElement('span');
        del.className = 'approve-diff__file-num approve-diff__file-num--del';
        del.textContent = `−${f.deleted}`;
        row.append(path, add, del);
        listEl.appendChild(row);
      }
      if (files.length > 60) {
        const more = document.createElement('div');
        more.className = 'approve-diff__loading';
        more.textContent = t('moreIssues', files.length - 60);
        listEl.appendChild(more);
      }
    }
    if (res.hunks) {
      hunkSummary.textContent = t('diffViewHunk');
      hunkBody.textContent = res.hunks;
      hunkEl.hidden = false;
    }
  } catch (err) {
    countEl.textContent = t('diffEmpty');
    statEl.textContent = '';
    const errEl = document.createElement('div');
    errEl.className = 'approve-diff__loading';
    errEl.textContent = String(err.message || err);
    listEl.appendChild(errEl);
  }
}

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
  // The loopx original wording is the authoritative decision subject, but it is
  // verbose internal detail — collapse it behind 查看原文 so the dialog leads
  // with the concise summary and shows the full text only on demand.
  if (raw) {
    const details = document.createElement('details');
    details.className = 'gate-card__raw-details';
    const summary = document.createElement('summary');
    summary.textContent = t('viewOriginal');
    const rawEl = document.createElement('div');
    rawEl.className = 'gate-card__raw';
    rawEl.textContent = raw;
    details.append(summary, rawEl);
    approveText.appendChild(details);
  }
  dlg.querySelector('h2').textContent = isPublish
    ? t('approvePrTitle')
    : (isGate ? t('approveGateTitle') : t('todoDoneTitle'));
  dlg.querySelector('button[value="approve"]').textContent = isPublish
    ? t('approveAndPr')
    : (isGate ? t('approveConfirm') : t('todoDoneConfirm'));
  // Publish gates let the human CHOOSE: submit the PR (default), approve
  // without a PR, or reject the change outright — the choice is part of the
  // guided flow, not a setting.
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
  // Blocking decisions (user_gate / publish) support an explicit 拒绝, so
  // "同意或拒绝" is a real choice — not approve-or-cancel.
  const rejectBtn = dlg.querySelector('#btn-approve-reject');
  if (rejectBtn) {
    const blocking = isGate || isPublish;
    rejectBtn.hidden = !blocking;
    if (blocking) {
      rejectBtn.textContent = t('rejectGate');
      rejectBtn.onclick = () => {
        dlg.returnValue = 'reject';
        dlg.close('reject');
      };
    }
  }
  const noteInput = document.getElementById('approve-note');
  noteInput.value = '';
  // #1 diff-first 审批：发布门禁先加载并展示改动文件，看懂了再批准。
  // Diff 的对象必须是 gate 指定的待发布分支（publishPr 用的同一个解析），
  // 而不是工作区恰好 checkout 的 HEAD。
  const diffContainer = document.getElementById('approve-diff');
  if (isPublish) loadApproveDiff(g, branchHintFromText(todo.text || todo.title || ''));
  else if (diffContainer) diffContainer.hidden = true;
  dlg.returnValue = 'cancel';
  dlg.onclose = () => {
    if (dlg.returnValue === 'approve') approveTodo(g, todo, noteInput.value.trim(), null, { publish: true });
    else if (dlg.returnValue === 'approve-only') approveTodo(g, todo, noteInput.value.trim(), null, { publish: false });
    else if (dlg.returnValue === 'reject') approveTodo(g, todo, noteInput.value.trim(), null, { outcome: 'reject' });
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
  const goalId = g.goalId;
  // #B 乐观移除：先从看板立即消失，后台再执行删除；失败时 refreshGoals 会把
  // 目标重新拉回来（config 映射只在成功后才清理），避免「卡很久 + 卡片闪一下」。
  S.goals.delete(goalId);
  if (S.activeGoalId === goalId) {
    S.activeGoalId = null;
    document.getElementById('goal-detail-panel').hidden = true;
    document.getElementById('detail-empty').hidden = false;
  }
  renderAllGoals(true);
  setTaskFeedback(t('taskDeleted', name), 'ok');
  try {
    const res = await app.call('loopx.deleteGoal', {
      argvPrefix: S.config.argvPrefix,
      srcDir: S.config.srcDir || null,
      projectDir: goalProjectDir(goalId),
      goalId,
    });
    if (!res.ok) throw new Error(res.error || 'delete failed');
    log(`[${goalId}] ${t('taskDeleted', name)}`);
    if (res.warning) log(`[${goalId}] ${res.warning}`, true);
    for (const map of [
      S.config.ownedGoals, S.config.monitorByGoal, S.config.agentByGoal,
      S.config.autoRunByGoal, S.config.modelByGoal, S.config.projectByGoal,
      S.config.stoppedByGoal, S.config.autoRunBeforeStop,
      S.config.agentSessionByGoal,
    ]) {
      if (map) delete map[goalId];
    }
    await saveConfig();
    await refreshGoals();
    saveLogs(); // the removed goal's persisted log drops out of the snapshot
    renderAllGoals(true);
  } catch (err) {
    const message = String(err && err.message || err);
    log(`[${goalId}] delete failed: ${message}`, true);
    // Restore the goal so the board reflects the still-on-disk registry entry.
    try { await refreshGoals(); } catch (_) {}
    renderAllGoals(true);
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

// The run/stop toggle now lives next to the "Bitfun努力解bug中…" status bar
// (see startCountdownLoop); the panel footer button dispatches on the selected
// goal's current state — stop while running, resume while paused.
document.getElementById('btn-detail-toggle-run').addEventListener('click', () => {
  const g = S.activeGoalId ? S.goals.get(S.activeGoalId) : null;
  if (!g) return;
  if (g.running) openStopConfirm(g);
  else if (g.userStopped || g.stopped || !g.autoRun) resumeCardTask(g);
});

document.getElementById('btn-close-goal').addEventListener('click', () => {
  S.activeGoalId = null;
  document.getElementById('goal-detail-panel').hidden = true;
  renderAllGoals(true);
});

// #7 点击某个任务的卡片即视为已读该任务的门禁——不再一次清空所有任务的未读
// 标记（点 A 不该把 B、C 的未读也清掉）。
document.getElementById('review-zone').addEventListener('click', (e) => {
  const row = e.target && e.target.closest ? e.target.closest('[data-goal]') : null;
  const goalId = row ? row.dataset.goal : null;
  const g = goalId ? S.goals.get(goalId) : null;
  if (g && g.gateUnread) {
    g.gateUnread = false;
    requestRender(true);
  }
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

// ── #10 全局状态条 ────────────────────────────────────────
function reviewUnreadCount() {
  let n = 0;
  for (const g of S.goals.values()) if (g.gateUnread) n += 1;
  return n;
}

function onStatusBarClick(group) {
  // The status bar is the board's tab switcher: clicking a segment sets which
  // group the left column renders.
  if (!BOARD_GROUPS.includes(group)) return;
  if (S.activeBoardTab === group) return;
  S.activeBoardTab = group;
  renderAllGoals(true);
}

// ── #D 进行中目标下拉框（取代旧的「进行中」列） ───────────────
// 列出 running / queued / parked / error 目标；点击目标选中并打开日志，
// 每个目标右侧的 › 弹二级菜单显示正在修复的 issues。
// 详情面板正在展示某 goal 时，issue 投影加载完要同步刷新「Issues」区块。
function refreshDetailIssues(g) {
  const body = document.getElementById('goal-detail-body');
  if (!body || !g || S.activeGoalId !== g.goalId) return;
  const sec = body.querySelector(':scope > .detail-issues');
  if (!sec) return;
  sec.dataset.sig = '';
  renderIssueManagement(g);
}

// 收集 goal 的 issue 行（url/number/info），null = 投影尚未加载。
function goalIssueRows(g) {
  const projection = (g.issues && g.issues.issues) || [];
  const urls = parseIssueUrls(g.objective || '');
  const byUrl = new Map();
  for (const issue of projection) if (issue.url) byUrl.set(issue.url, issue);
  if (urls.length) {
    const rows = urls.map((u) => ({ url: u.url, number: u.number, info: byUrl.get(u.url) || null }));
    for (const issue of projection) {
      if (!urls.some((u) => u.url === issue.url)) rows.push({ url: issue.url, number: issue.number, info: issue });
    }
    return rows;
  }
  if (g.issues) return projection.map((issue) => ({ url: issue.url, number: issue.number, info: issue }));
  return null;
}

function issueRowStatus(g, row) {
  const urls = parseIssueUrls(g.objective || '');
  if (urls.length === 1) return goalGroup(g) === 'done' ? 'done' : 'open';
  return row.info ? row.info.status : 'pending';
}

function goalRepoFromObjective(g) {
  const m = String(g.objective || '').match(/github\.com\/([^/\s]+\/[^/\s?#]+)/i);
  return m ? m[1].replace(/\.git$/i, '') : null;
}

// ── issue 清单：勾选 + 批量应用（取代旧的行内操作和「新增 issue」输入框） ──
const goalPickerRepoIssues = new Map(); // repo -> { issues, truncated, at, error? }
let goalPickerSelection = { goalId: null, add: new Set(), remove: new Set() };

function resetGoalPickerSelection(g) {
  if (goalPickerSelection.goalId !== g.goalId) {
    goalPickerSelection = { goalId: g.goalId, add: new Set(), remove: new Set() };
  }
}

async function fetchRepoIssues(repo) {
  const cached = goalPickerRepoIssues.get(repo);
  if (cached && Date.now() - cached.at < 60000) return cached;
  try {
    const res = await app.call('loopx.listRepoIssues', { repo });
    if (!res || !res.ok) throw new Error((res && res.error) || 'list repo issues failed');
    const entry = { issues: res.issues || [], truncated: !!res.truncated, at: Date.now() };
    goalPickerRepoIssues.set(repo, entry);
    return entry;
  } catch (err) {
    return { issues: [], truncated: false, at: Date.now(), error: String(err.message || err) };
  }
}

function goalPickerSectionHead(text) {
  const h = document.createElement('div');
  h.className = 'goal-picker__section-head';
  h.textContent = text;
  return h;
}

// 加载中：转圈 spinner + 文字，替代「…」。
function loadingEl(text) {
  const el = document.createElement('div');
  el.className = 'loading';
  const sp = document.createElement('span');
  sp.className = 'spinner';
  const label = document.createElement('span');
  label.textContent = text;
  el.append(sp, label);
  return el;
}

function buildCheckRow(g, row, wasSelected) {
  const item = document.createElement('label');
  item.className = 'goal-picker__checkrow';
  const cb = document.createElement('input');
  cb.type = 'checkbox';
  cb.checked = wasSelected ? !goalPickerSelection.remove.has(row.url) : goalPickerSelection.add.has(row.url);
  cb.onchange = () => {
    if (wasSelected) {
      if (goalPickerSelection.remove.has(row.url)) goalPickerSelection.remove.delete(row.url);
      else goalPickerSelection.remove.add(row.url);
    } else {
      if (goalPickerSelection.add.has(row.url)) goalPickerSelection.add.delete(row.url);
      else goalPickerSelection.add.add(row.url);
    }
    refreshGoalPickerApplyButton();
  };
  const mark = document.createElement('span');
  const status = wasSelected ? (row.status || 'open') : 'available';
  mark.className = `goal-picker__issue-mark goal-picker__issue-mark--${status}`;
  mark.textContent = status === 'done' ? '✓' : (status === 'blocked' ? '✗' : (status === 'deferred' ? '◌' : ''));
  const num = document.createElement('span');
  num.className = 'goal-picker__issue-num';
  num.textContent = `#${row.number}`;
  const title = document.createElement('span');
  title.className = 'goal-picker__issue-title';
  title.textContent = row.title || '';
  title.title = row.title || '';
  item.append(cb, mark, num, title);
  // 已选中的 issue 带上文字状态（受阻/已修复/已搁置），符号单独看不明确；受阻/已搁置
  // 再附上原因，说明「受阻 ≠ 需要审批」，而是 agent 记录的具体卡点。
  if (status !== 'available') {
    const statusText = document.createElement('span');
    statusText.className = `goal-picker__issue-status goal-picker__issue-status--${status}`;
    statusText.textContent = issueStatusLabel(status);
    const reason = row.reason;
    if ((status === 'blocked' || status === 'deferred') && reason) {
      const compact = String(reason).replace(/\s+/g, ' ').trim();
      statusText.textContent += ` · ${compact.length > 44 ? `${compact.slice(0, 44)}…` : compact}`;
      const full = (row.note && String(row.note).trim()) || compact;
      statusText.title = full;
    }
    item.append(statusText);
  }
  return item;
}

function refreshGoalPickerApplyButton() {
  const btn = document.querySelector('.goal-picker__apply');
  if (!btn) return;
  const n = goalPickerSelection.add.size + goalPickerSelection.remove.size;
  btn.disabled = n === 0;
  btn.textContent = n > 0 ? t('issueApplyCount', n) : t('issueApply');
}

function fillIssueChecklist(g, container) {
  container.replaceChildren();
  // 触发加载 todo 投影（已选 issue 来源），加载完会经 refreshGoalPickerDetail 重渲染。
  if (!g.issues && !g.issuesLoading) refreshGoalIssues(g);
  const current = (g.issues && g.issues.issues) || [];
  const selectedUrls = new Set(current.map((i) => i.url).filter(Boolean));

  if (current.length) {
    container.appendChild(goalPickerSectionHead(t('issueSelected')));
    for (const issue of current) {
      container.appendChild(buildCheckRow(g, { url: issue.url, number: issue.number, title: issue.title || '', status: issue.status, reason: issue.reason, note: issue.note }, true));
    }
  }

  container.appendChild(goalPickerSectionHead(t('issueAvailable')));
  const availWrap = document.createElement('div');
  availWrap.className = 'goal-picker__available';
  container.appendChild(availWrap);
  availWrap.appendChild(loadingEl(t('issueLoading')));

  const repo = goalRepoFromObjective(g);
  if (!repo) {
    availWrap.replaceChildren();
    const empty = document.createElement('div');
    empty.className = 'goal-picker__empty';
    empty.textContent = t('goalPickerNoIssues');
    availWrap.appendChild(empty);
    return;
  }
  fetchRepoIssues(repo).then((entry) => {
    if (!isLiveGoal(g)) return;
    availWrap.replaceChildren();
    if (entry.error) {
      const err = document.createElement('div');
      err.className = 'goal-picker__empty';
      err.textContent = entry.error;
      availWrap.appendChild(err);
      return;
    }
    const avail = entry.issues.filter((ri) => !selectedUrls.has(ri.url));
    if (!avail.length) {
      const empty = document.createElement('div');
      empty.className = 'goal-picker__empty';
      empty.textContent = t('issueNoMore');
      availWrap.appendChild(empty);
      return;
    }
    for (const ri of avail) {
      availWrap.appendChild(buildCheckRow(g, { url: ri.url, number: ri.number, title: ri.title, status: 'available' }, false));
    }
  });
}

async function applyIssueSelection(g, btn) {
  const adds = [...goalPickerSelection.add];
  const removes = [...goalPickerSelection.remove];
  goalPickerSelection = { goalId: g.goalId, add: new Set(), remove: new Set() };
  if (btn) btn.disabled = true;
  const repo = goalRepoFromObjective(g);
  const current = (g.issues && g.issues.issues) || [];
  for (const url of removes) {
    const issue = current.find((i) => i.url === url);
    if (!issue || !issue.todoId) continue;
    try {
      await app.call('loopx.supersedeTodo', {
        argvPrefix: S.config.argvPrefix, srcDir: S.config.srcDir || null,
        projectDir: goalProjectDir(g.goalId), goalId: g.goalId,
        todoId: issue.todoId, reason: '用户取消修复该 issue',
      });
    } catch (_) {}
  }
  for (const url of adds) {
    const m = String(url).match(/\/(?:issues|pull)\/(\d+)/);
    if (!m) continue;
    try {
      await app.call('loopx.addIssueTodo', {
        argvPrefix: S.config.argvPrefix, srcDir: S.config.srcDir || null,
        projectDir: goalProjectDir(g.goalId), goalId: g.goalId,
        text: `Fix GitHub issue #${Number(m[1])} (${url})`,
        repo, agentId: g.agentId || null,
      });
    } catch (_) {}
  }
  await refreshGoalIssues(g, true);
}

function buildGoalPickerDetailContent(g) {
  const wrap = document.createElement('div');

  const head = document.createElement('div');
  head.className = 'goal-picker__detail-head';
  const name = document.createElement('span');
  name.className = 'goal-picker__detail-name';
  name.textContent = goalDisplayName(g);
  name.title = g.goalId;
  head.append(name, goalStatusChip(g));
  wrap.appendChild(head);

  // 「应用更改」放最上面，勾选后一眼可见。
  const apply = document.createElement('button');
  apply.type = 'button';
  apply.className = 'btn btn--small btn--primary goal-picker__apply';
  apply.textContent = t('issueApply');
  apply.disabled = true;
  apply.onclick = () => applyIssueSelection(g, apply);
  wrap.appendChild(apply);

  const rows = goalIssueRows(g);
  const summary = document.createElement('div');
  summary.className = 'goal-picker__detail-summary';
  if (rows) {
    const done = rows.filter((r) => issueRowStatus(g, r) === 'done').length;
    summary.textContent = t('issueSummary', rows.length, done);
  } else {
    summary.replaceChildren(loadingEl(t('issueLoading')));
  }
  wrap.appendChild(summary);

  const checklist = document.createElement('div');
  checklist.className = 'goal-picker__checklist';
  wrap.appendChild(checklist);
  fillIssueChecklist(g, checklist);

  return wrap;
}

function renderStatusBar() {
  const bar = document.getElementById('status-bar');
  if (!bar) return;
  const counts = {};
  for (const k of BOARD_GROUPS) counts[k] = 0;
  for (const g of S.goals.values()) {
    if (!isOwnedGoal(g.goalId)) continue;
    const group = goalGroup(g);
    if (counts[group] != null) counts[group] += 1;
  }
  const total = BOARD_GROUPS.reduce((sum, k) => sum + (counts[k] || 0), 0);
  if (total === 0) {
    bar.hidden = true;
    bar.replaceChildren();
    return;
  }
  bar.hidden = false;
  // The status bar doubles as the board's tab switcher: every group with a
  // count is a clickable tab, and the active tab is highlighted.
  const frag = document.createDocumentFragment();
  for (const group of BOARD_GROUPS) {
    const n = counts[group] || 0;
    if (n === 0) continue;
    const seg = document.createElement('button');
    seg.type = 'button';
    seg.className = 'status-bar__seg'
      + ` status-bar__seg--${group}`
      + (S.activeBoardTab === group ? ' is-active' : '');
    if (group === 'decisions' && reviewUnreadCount() > 0) seg.classList.add('status-bar__seg--pulse');
    const label = t(GROUP_I18N_KEY[group] || group);
    const b = document.createElement('b');
    b.textContent = String(n);
    seg.append(`${label} `, b);
    seg.onclick = () => onStatusBarClick(group);
    frag.appendChild(seg);
  }
  bar.replaceChildren(frag);
}

// 启动后自动选中第一个「需要处理」的目标：review（要你决定）→ running → backlog
// → error → paused。跳过 terminal（done/archived），只做一次，不打扰用户主动关闭。
const AUTO_SELECT_ORDER = ['decisions', 'notices', 'active', 'backlog', 'error', 'paused'];
function autoSelectGoal(owned) {
  for (const group of AUTO_SELECT_ORDER) {
    const g = owned.find((x) => goalGroup(x) === group);
    if (g) return g;
  }
  return null;
}

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
  // Every key goalGroup() can return must exist in BOARD_GROUPS: a missing key
  // made buckets.get() return undefined and crash the parked spread.
  const buckets = new Map(BOARD_GROUPS.map((k) => [k, []]));
  for (const g of owned) buckets.get(goalGroup(g)).push(g);
  // Keep the active board tab valid before the status bar renders, so the
  // highlighted tab and the board content can never disagree. An in-flight
  // intake counts as content for the 运行中 tab (its pending row lives there).
  const activeTabHasContent = (key) => buckets.get(key).length > 0
    || (key === 'active' && !!S.intakeDraft);
  if (!BOARD_GROUPS.includes(S.activeBoardTab) || !activeTabHasContent(S.activeBoardTab)) {
    const first = BOARD_GROUPS.find((k) => activeTabHasContent(k));
    S.activeBoardTab = first || 'decisions';
  }
  renderStatusBar();

  // ── The left column is a generic board: the active status tab picks which
  //    group renders here (需决策 / 知会 / 运行中 / 排队中 / …). ──
  const boardGoals = buckets.get(S.activeBoardTab);
  document.getElementById('review-zone-title').textContent = t(GROUP_I18N_KEY[S.activeBoardTab] || S.activeBoardTab);
  document.getElementById('review-zone-count').textContent = String(boardGoals.length);
  document.getElementById('review-zone-sub').textContent = t(GROUP_SUB_KEY[S.activeBoardTab] || '');
  // #7 未读角标 + 脉冲：只在「需决策」Tab 上体现新出现的审批门禁。
  const reviewUnread = S.activeBoardTab === 'decisions' ? boardGoals.filter((rg) => rg.gateUnread).length : 0;
  const reviewZone = document.getElementById('review-zone');
  const reviewBadge = document.getElementById('review-zone-badge');
  if (reviewZone) reviewZone.classList.toggle('review-zone--unread', reviewUnread > 0);
  if (reviewBadge) {
    reviewBadge.hidden = reviewUnread === 0;
    reviewBadge.textContent = String(reviewUnread);
  }
  const reviewList = document.getElementById('review-list');
  reviewList.title = t('kbHint');
  reviewList.replaceChildren();
  // Intake in flight: show the pending "创建中" row at the top of 运行中 so
  // the clone/bootstrap stages are visible on the board, not only in the
  // composer feedback line. (Its stage text is patched in place by the
  // taskIntake progress events.)
  if (S.intakeDraft && S.activeBoardTab === 'active') {
    reviewList.appendChild(buildIntakeRow(S.intakeDraft));
  }
  if (boardGoals.length === 0 && !(S.intakeDraft && S.activeBoardTab === 'active')) {
    const none = document.createElement('div');
    none.className = 'zone-empty' + (S.bootLoading ? ' zone-empty--loading' : '');
    none.textContent = S.bootLoading ? t('loadingGoals') : t('colEmpty');
    reviewList.appendChild(none);
  }
  for (const g of boardGoals) reviewList.appendChild(buildBoardRow(g));

  // Other-host goals (unowned, unmonitored until adopted) stay behind a quiet
  // chip at the board's foot. Terminal groups are now first-class tabs.
  const moreArea = document.getElementById('more-area');
  const moreSig = other.map((goal) => goal.goalId).join(',');
  if (force || moreSig !== lastMoreFingerprint) {
    moreArea.replaceChildren();
    if (other.length > 0) moreArea.appendChild(buildMoreFooter([{ key: 'other', goals: other }]));
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
    if (!S.didAutoSelect && owned.length > 0) {
      S.didAutoSelect = true;
      const first = autoSelectGoal(owned);
      if (first) {
        S.activeGoalId = first.goalId;
        panel.hidden = false;
        emptyHint.hidden = true;
        renderGoalDetails(first);
      } else {
        panel.hidden = true;
        emptyHint.hidden = false;
      }
    } else {
      panel.hidden = true;
      emptyHint.hidden = false;
    }
  }
  updateEmptyState();
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
    const running = !!(activeGoal && activeGoal.running);
    // A stopped/paused goal (not gated) also keeps the footer bar visible so
    // the same slot can host the ▶运行 resume button — no hunt for it elsewhere.
    const paused = !!activeGoal && !running
      && (activeGoal.userStopped || activeGoal.stopped || !activeGoal.autoRun)
      && !isGated(activeGoal);
    // A "running" turn that has produced no agent event for a while is almost
    // certainly stuck (a hanging tool call, a lost event pipe) — surface it
    // instead of pretending "努力解bug中" while the clock quietly climbs.
    // EXCEPT while a tool is legitimately executing: tools emit no
    // intermediate events, so narrate "工具运行中 · <name> · 已 N 分钟"
    // instead of a false "可能卡住了".
    const run = running ? agentRuns.get(activeGoal.goalId) : null;
    const idleMs = run && run.lastEventAt ? Date.now() - run.lastEventAt : 0;
    const inFlight = running ? toolsInFlight(activeGoal) : null;
    const runningTool = inFlight && inFlight.size > 0
      ? [...inFlight.values()].reduce((a, b) => (a.startedAt <= b.startedAt ? a : b))
      : null;
    const idle = running && idleMs > IDLE_WARN_MS && !runningTool;
    const bar = document.getElementById('goal-detail-status');
    const clock = document.getElementById('goal-detail-status-clock');
    const text = bar ? bar.querySelector('.detail-panel__status-text') : null;
    const toggle = document.getElementById('btn-detail-toggle-run');
    if (bar) {
      bar.hidden = !(running || paused);
      bar.classList.toggle('detail-panel__status--paused', !running && paused);
      bar.classList.toggle('detail-panel__status--idle', idle);
    }
    if (text) {
      let label = '';
      if (running) {
        if (idle) {
          label = activeGoal._modelResponded === false ? t('activityModelHang') : t('activityStalled');
        } else if (runningTool && idleMs > 15000) {
          // Only switch to the tool narration once the silence is noticeable;
          // rapid tool chatter keeps the friendlier default label.
          const variant = TOOL_VARIANTS[String(runningTool.name || '').toLowerCase()] || 'others';
          const toolLabel = (TOOL_TITLE[variant] || TOOL_TITLE.others)();
          label = t('activityToolRunning', toolLabel, fmtRunDuration(Date.now() - runningTool.startedAt));
        } else {
          label = t('activityDiving');
        }
      } else if (paused) {
        label = t('statusPaused');
      }
      text.textContent = label;
    }
    if (clock) {
      clock.textContent = running
        ? (idle
          ? `${t('idleFor', fmtRunDuration(idleMs))} · ${t('elapsedLabel', fmtRunDuration(Date.now() - activeGoal.runStartedAt))}`
          : fmtRunDuration(Date.now() - activeGoal.runStartedAt))
        : '';
    }
    if (toggle) {
      if (running) {
        toggle.hidden = false;
        toggle.textContent = `⏹ ${t('stopShort')}`;
      } else if (paused) {
        toggle.hidden = false;
        toggle.textContent = `▶ ${t('runShort')}`;
      } else {
        toggle.hidden = true;
      }
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
// watchdog cancels it and lets the poll loop relaunch on a FRESH session
// (stateless resume re-derives the next step from the loopx registry, so a
// hard-cancelled turn loses nothing). 3 minutes keeps silent stalls from
// blocking the queue while still tolerating a genuinely slow first token.
const STALL_TURN_MS = 3 * 60 * 1000;
// While a tool call is in flight, "no events" is EXPECTED: the model wrote
// the call and the tool (pytest, pip install, git clone…) is executing with
// no intermediate events. Killing at 3 minutes was strangling every slow
// tool run and tripping the auto-run breaker on test-heavy repos. Tools have
// their own host-side timeout; this is only the lost-event-pipe failsafe.
const STALL_TOOL_MS = 30 * 60 * 1000;
// A (soft or hard) gate can legitimately keep a turn waiting on a human, but
// the event pipe can also die while gated — a very long failsafe instead of
// skipping the watchdog entirely.
const STALL_GATED_MS = 60 * 60 * 1000;
// Surface a "possibly stalled" warning in the status bar well before the hard
// cancel above: a hanging tool call / lost event pipe must read as "no output
// for N minutes", not a silently climbing elapsed clock.
const IDLE_WARN_MS = 2 * 60 * 1000;

// In-flight tool calls per goal (toolId -> { name, startedAt }): drives both
// the watchdog exemption above and the "工具运行中" status-bar narration.
function toolsInFlight(g) {
  return g._toolsInFlight instanceof Map ? g._toolsInFlight : null;
}
function trackToolEvent(g, te, done) {
  const toolId = te && te.tool_id;
  if (!toolId) return;
  if (!(g._toolsInFlight instanceof Map)) g._toolsInFlight = new Map();
  if (done) {
    g._toolsInFlight.delete(toolId);
  } else if (!g._toolsInFlight.has(toolId)) {
    const name = te.effectiveToolName || te.effective_tool_name
      || te.toolName || te.tool_name || '';
    g._toolsInFlight.set(toolId, { name: String(name), startedAt: Date.now() });
    // Bound the map: a lost Completed event must not pin the watchdog
    // exemption forever — entries older than the tool stall cap fall off.
    for (const [id, info] of g._toolsInFlight) {
      if (Date.now() - info.startedAt > STALL_TOOL_MS) g._toolsInFlight.delete(id);
    }
  }
}
function stallThresholdMs(g) {
  if (isGated(g)) return STALL_GATED_MS;
  const tools = toolsInFlight(g);
  if (tools && tools.size > 0) return STALL_TOOL_MS;
  return STALL_TURN_MS;
}

async function stallRecover(g, run) {
  // 先确认是不是「已完成但完成事件丢了」：事件桥偶发丢事件时，turn 其实早已跑完，
  // 盲目 cancel 会误杀一个正常完成的 turn，还让 autoFailCount 白白 +1。
  try {
    const res = await app.agent.turnText(run.sessionId, run.turnId);
    const text = res && typeof res.text === 'string' ? res.text.trim() : '';
    if (text) {
      log(`[${g.goalId}] stall recovered: turn actually completed (${text.length} chars)`, true);
      finishRun(g, { ok: true });
      return;
    }
  } catch (_) {
    // turnText 失败 = turn 可能仍在跑或已失效；继续走 cancel 分支。
  }
  const idleMin = Math.round((Date.now() - run.lastEventAt) / 60000);
  // 区分「模型一直没吐第一个字」和「模型已响应但工具/事件管道卡住」：前者几乎
  // 肯定是模型 API 超时/过载，明确写出来比笼统的「运行僵死」更直观。
  const message = g._modelResponded === false
    ? t('turnModelHang', idleMin)
    : t('turnStalled', idleMin);
  log(`[${g.goalId}] ${message}`, true);
  // 模型未响应 → 弹非阻塞行动卡 + 系统通知，让用户能一键换模型或继续等待。
  if (g._modelResponded === false) {
    g._modelHang = { idleMin, model: modelForGoal(g.goalId) };
    try {
      if (app.notifications?.system) {
        app.notifications.system(t('notifModelHangTitle'), t('notifModelHangBody', goalDisplayName(g), idleMin));
      }
    } catch (_) {}
  }
  // finishRun(error) already records the message through its error branch —
  // recording it here too made the "运行僵死" line appear twice.
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
  g._modelResponded = false;
  // #4 turn 分组：每次运行开一个新轮次，后续活动日志都打上本轮编号。
  g.turnNumber = (g.turnNumber || 0) + 1;
  // The activity stream accumulates across runs (and restarts via the
  // persisted log): the run boundary is the 正在启动 line below. Only the
  // per-turn streaming buffers reset.
  g.agentTextBuffer = '';
  g.thinkBuffer = '';
  if (g._toolsInFlight instanceof Map) g._toolsInFlight.clear();
  // Stateless resume (loopx philosophy): every turn starts a FRESH host session
  // and re-derives its next step from the loopx registry/todos — the session is
  // never reused, so context cannot grow unbounded and stall first-token latency.
  recordGoalActivity(g, t('activityStarting'), false, 'waiting');
  // Auto-focus: a task that starts running becomes the selected task UNLESS the
  // user is already watching another running task — with multiple repos running
  // concurrently, yanking the log panel on every turn start would make the view
  // jump around. Only take over when the current view is idle (or empty).
  const currentView = S.activeGoalId ? S.goals.get(S.activeGoalId) : null;
  if (S.activeGoalId !== g.goalId && (!currentView || !currentView.running)) {
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
    // Stall watchdog: the host can cancel a turn WITHOUT the console ever
    // seeing the completion event (webview hiccup / stale-run cleanup breaks
    // the event pipe). Without this the card stays "running" forever and
    // auto-run freezes. The death threshold adapts to what the turn is doing:
    // 3m for pure model silence, 30m while a tool call is executing (tools
    // emit no intermediate events — pytest/pip/clone runs are silent by
    // design), 60m while legitimately gated on a human. Cancel, report, and
    // let the poll loop re-decide — auto-run fires the next turn fresh.
    if (Date.now() - run.lastEventAt > stallThresholdMs(g)) {
      clearInterval(tick);
      stallRecover(g, run);
    }
  }, 10000);
  g._runTick = tick;
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
    // 用户在 prompt 组装期间中止了任务：直接返回（stopGoalTask 已收尾）。
    if (!g.running) return;
    // 启动阶段完成：spinner → ✓。
    resolveWaiting(g, t('activityStartingDone'));
    dbgUi('turn:promptReady', `chars=${composed.prompt.length}`);
    // 续跑对账：把「已完成（跳过）/ 未完成（继续）」写进日志，让用户一眼看清
    // 现在从哪个 issue 继续。数据来自 loopx 注册表的 todo 状态，非会话记忆。
    if (composed.resumeNote) {
      recordGoalActivity(g, `${t('resumeReconcileTitle')}：${composed.resumeNote}`);
    }
    // 上一轮失败原因注入：新会话不记得上一轮，但失败原因（stall/超时/报错）值得
    // 原样带回去，让模型据此调整方法而不是盲目重试同样的路。
    if (g.lastFailReason) {
      composed.prompt += `\n\n上一轮尝试失败的记录（仅作参考，按需调整方法，不要复述给用户）：${g.lastFailReason}\n`;
    }
    // 软门禁下继续推进：某个 issue 的发布审批挂起时，其余 issue 照常处理。把
    // 等待人工审批的 issue 列表注入本轮 prompt，防止 agent 重复触碰待审分支。
    const pendingApprovalUrls = [...new Set((g.userTodos || [])
      .filter((td) => gateTodoInfo(td).isBlocking)
      .flatMap((td) => parseIssueUrls(String(td.text || td.title || '')).map((r) => r.url)))];
    if (pendingApprovalUrls.length) {
      composed.prompt += `\n\n以下 issue 正在等待人工审批，本轮跳过它们（不要改动其分支、不要尝试发布）：\n${pendingApprovalUrls.map((u) => `- ${u}`).join('\n')}\n`;
    }
    // The log shows what was sent to the agent — collapsed, expandable.
    recordGoalActivity(g, t('activitySentPrompt', composed.prompt.length), false, 'prompt', composed.prompt);
    // 会话上下文很大时，agent.run 到首个事件之间可能长达几分钟（恢复上下文 + 发请求），
    // 加一行「等待模型响应」避免误以为卡住。把累计上下文规模和上次实测的首字响应
    // 时长一并写进去，给用户一个「大概要等多久」的心理预期。
    const prevCtx = Number(S.config.agentInputCharsByGoal[g.goalId]) || 0;
    const ctxChars = prevCtx + composed.prompt.length;
    // Prefer the host-reported inputTokens (real context incl. prior output and
    // tool results); the char sum is only a first-turn fallback.
    const ctxLabel = contextSizeText(g, ctxChars);
    const prevTtft = Number(S.config.agentTtftMsByGoal[g.goalId]) || 0;
    const waitLabel = prevTtft > 0
      ? t('activityWaitingModelCtxEta', ctxLabel, fmtRunDuration(prevTtft))
      : t('activityWaitingModelCtx', ctxLabel);
    recordGoalActivity(g, waitLabel, false, 'waiting');
    g._waitStartedAt = Date.now();
    g._waitCtxChars = ctxChars;
    // Roll the cumulative session-context size forward so the NEXT turn's hint
    // reflects the growing context (the dominant factor in first-token latency).
    S.config.agentInputCharsByGoal[g.goalId] = ctxChars;
    saveConfig();
    // Fresh session every turn: loopx's heartbeat-prompt already carries the
    // full next-step (todos/state), so the agent resumes from the registry, not
    // from a ballooning chat transcript. No sessionId = a brand-new context.
    requestedSessionId = null;
    const run = await app.agent.run(composed.prompt, {
      sessionName: `bitfun-loopx · ${g.goalId}`,
      enableTools: true,
      model: modelForGoal(g.goalId),
    });
    // 用户在 agent.run 返回句柄前中止了任务：此刻才拿到 session/turn id，
    // 立即取消刚启动的 host turn（stopGoalTask 已收尾本地状态）。
    if (!g.running) {
      try { await app.agent.cancel(run.sessionId, run.turnId); } catch (_) {}
      return;
    }
    dbgUi('turn:agentStarted', `session=${run.sessionId} turn=${run.turnId} fresh=true`);
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

// Terminal handling shared by completion/failure/cancel events.
// byUser marks an explicit human stop (stopGoalTask); a cancel arriving via
// the event stream is host-initiated (host restart, session pruned) — the
// user never asked for it, so auto-run must survive and retry with backoff
// instead of silently parking the goal forever.
function finishRun(g, { ok, cancelled = false, error = null, byUser = false }) {
  if (g._runTick) { clearInterval(g._runTick); g._runTick = null; }
  agentRuns.delete(g.goalId);
  // Turn over → nothing is in flight any more (a lost Completed event must
  // not extend the next turn's watchdog exemption).
  if (g._toolsInFlight instanceof Map) g._toolsInFlight.clear();
  // A failed/cancelled turn leaves its "等待模型响应" spinner unresolved —
  // land it as ✗ so old turn groups never keep a forever-spinning loader.
  if (!ok) resolveWaiting(g, null, true);
  // 本轮可能改了 todo 状态：作废 worker 侧的续跑对账缓存，下一轮 prompt 用最新状态。
  try { app.call('loopx.invalidateResumeCache', {}).catch(() => {}); } catch (_) {}
  g.running = false;
  g.lastRun = {
    exitCode: ok ? 0 : 1,
    durationMs: g.runStartedAt ? Date.now() - g.runStartedAt : 0,
    status: cancelled ? 'cancelled' : (ok ? 'completed' : 'failed'),
    ok,
    cancelled,
  };
  if (cancelled && byUser) {
    // A manual cancel is an explicit "stop": disable the continuous loop
    // instead of immediately relaunching what the user just killed.
    if (g.autoRun) setAutoRun(g, false);
    log(`[${g.goalId}] ${t('runCancelled')}`);
    recordGoalActivity(g, t('runCancelled'));
  } else if (cancelled) {
    // Host-initiated cancel (restart, session pruned, stall recovery): the
    // user did not ask to stop — treat as a failed turn so auto-run retries
    // with the normal backoff instead of parking the goal forever.
    g.lastFailReason = error || 'host cancelled the turn';
    if (g.autoRun) {
      g.autoFailCount += 1;
      g.retryAfter = Date.now() + g.autoFailCount * 60 * 1000;
      if (g.autoFailCount >= AUTO_RUN_FAIL_LIMIT) {
        g.autoRun = false;
        g.retryAfter = 0;
        S.config.autoRunByGoal[g.goalId] = false;
        saveConfig();
        log(t('autoRunDisabled', g.goalId), true);
      }
    }
    log(`[${g.goalId}] ${t('runCancelledByHost')}`, true);
    recordGoalActivity(g, t('runCancelledByHost'), true);
  } else if (ok) {
    g.autoFailCount = 0;
    g.retryAfter = 0;
    g.lastFailReason = '';
    g._modelHang = null;
    log(`[${g.goalId}] turn completed`);
    recordGoalActivity(g, t('activityCompleted'));
  } else {
    g.lastFailReason = error || '';
    if (g.autoRun) {
      g.autoFailCount += 1;
      // 渐进冷却：第 N 次失败后等 N 分钟再重试，避免对瞬时抖动狂打。
      g.retryAfter = Date.now() + g.autoFailCount * 60 * 1000;
      if (g.autoFailCount >= AUTO_RUN_FAIL_LIMIT) {
        // Trip the breaker VISIBLY: flip the toggle off so the drawer shows
        // reality and re-enabling it is the documented recovery path.
        g.autoRun = false;
        g.retryAfter = 0;
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
  // A finished turn frees an auto-run slot: goals that were capped out by
  // MAX_CONCURRENT_AUTO_RUNS get their shot now instead of waiting a full
  // poll interval.
  for (const other of S.goals.values()) {
    if (other !== g) maybeAutoRun(other);
  }
}

function goalForAgentSession(sessionId) {
  for (const [goalId, run] of agentRuns) {
    if (run.sessionId === sessionId) return S.goals.get(goalId);
  }
  return null;
}

// Reverse sessionId → goalId lookup (persisted + in-memory), used by the
// token-usage handler which may fire after the run entry is already cleared.
function goalIdForSessionId(sessionId) {
  for (const [goalId, sid] of S.agentSessionByGoal) {
    if (sid === sessionId) return goalId;
  }
  for (const [goalId, sid] of Object.entries(S.config.agentSessionByGoal || {})) {
    if (sid === sessionId) return goalId;
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
  // A status/tool line ends the current stream block (upsertGoalStream walks
  // back only past ticks). If the last non-tick entry is no longer our stream
  // block, the next upsert CREATES a new block — it must contain only the new
  // text, not the whole turn's accumulated buffer (which would repaint
  // everything already shown as a duplicate).
  const kind = think ? 'think' : 'agent';
  let interrupted = false;
  if (g[key] && Array.isArray(g.activityLines)) {
    for (let i = g.activityLines.length - 1; i >= 0; i -= 1) {
      const e = g.activityLines[i];
      if (e.isTick) continue;
      interrupted = !(e.kind === kind && e.stream);
      break;
    }
  }
  if (interrupted) g[key] = '';
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
  const agentVal = pend && typeof pend.agent === 'string' ? pend.agent : g.agentTextBuffer;
  const thinkVal = pend && typeof pend.think === 'string' ? pend.think : g.thinkBuffer;
  const agentTrim = String(agentVal || '').trim();
  const thinkTrim = String(thinkVal || '').trim();
  // Reasoning models (deepseek etc.) put the real analysis in reasoning_content
  // and emit a near-empty visible reply (a few newlines/punctuation). When the
  // visible reply is that thin, promote a bounded tail of the reasoning as the
  // visible conclusion so the log still shows WHAT the agent decided — instead
  // of burying it in a collapsed "思考过程" block.
  let promoted = null;
  const meaningful = agentTrim.replace(/[\s\p{P}\p{S}]/gu, '');
  if (meaningful.length < 8 && thinkTrim) {
    promoted = latestLineOf(thinkTrim);
    if (promoted.length > 360) promoted = promoted.slice(-360);
  }
  // Flush think first (mirrors the streaming order), so the agent upsert below
  // can still find and extend the newest think block instead of duplicating it.
  if (thinkTrim) upsertGoalStream(g, 'think', thinkTrim);
  if (agentTrim) upsertGoalStream(g, 'agent', agentTrim);
  else if (promoted) upsertGoalStream(g, 'agent', promoted);
  g.agentTextBuffer = '';
  g.thinkBuffer = '';
  if (pend) { pend.think = null; pend.agent = null; }
}

// 本轮结论：从最终可见文本（reasoning 模型几乎不吐正文时退回推理尾行）里
// 取一段单行收尾。只做展示，不参与后续决策。
function turnConclusion(g) {
  let src = String(g.agentTextBuffer || '').trim();
  const meaningful = src.replace(/[\s\p{P}\p{S}]/gu, '');
  if (meaningful.length < 8) {
    const think = String(g.thinkBuffer || '').trim();
    if (think) src = latestLineOf(think);
  }
  src = String(src || '').replace(/\s+/g, ' ').trim();
  if (!src) return '';
  if (src.length > 240) src = src.slice(-240);
  return src;
}

// Create or update the single streaming block for a kind. Status/tool lines
// between chunks end the block: walk backwards past ticks and continue only
// the newest stream block of that kind; anything else starts a fresh one.
function upsertGoalStream(g, kind, text) {
  // Trim BOTH ends: a model that opens its reply with a newline (very common)
  // would otherwise push the first visible line down one row under the
  // timestamp (pre-wrap renders the leading blank line), misaligning it.
  const summary = String(text || '').trim();
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
      const preview = row.querySelector('.activity-prompt__think-preview');
      if (preview) preview.textContent = latestLineOf(cappedStreamText(summary, STREAM_DOM_CAPS.think));
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
    if (stream) {
      const row = lastActivityRow(stream);
      const follow = streamAtTail(stream);
      if (row) patchRow(row);
      if (follow) streamFollowTail(stream);
    }
  } else {
    const entry = { time: now, line: summary, isErr: false, count: 1, kind, stream: true, turn: g.turnNumber || 0 };
    g.activityLines.push(entry);
    if (g.activityLines.length > 240) g.activityLines.splice(0, g.activityLines.length - 240);
    if (stream) {
      const follow = streamAtTail(stream);
      const emptyEl = stream.querySelector('.activity-empty');
      if (emptyEl) emptyEl.remove();
      appendActivityEntry(stream, entry);
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
  if (!te.tool_id) return rawText;
  const buf = (toolParamsBuf.get(te.tool_id) || '') + rawText;
  if (buf.length > 6000) toolParamsBuf.set(te.tool_id, buf.slice(-3000));
  else toolParamsBuf.set(te.tool_id, buf);
  return buf;
}

function pruneToolMaps() {
  if (toolLinesRecorded.size <= 300) return;
  const keys = [...toolLinesRecorded.keys()].slice(0, 150);
  for (const key of keys) {
    toolLinesRecorded.delete(key);
    toolParamsBuf.delete(key);
  }
}

// ── 工具行模型（参考 DSH 的 toolRowModel）──────────────────
// 把宿主工具按类别归一，显示「类别标题 · 有意义的参数」，而不是原始命令前缀
// （"python -u -c" 这种）。shell 工具优先用模型自己写的 description，其次才提炼命令。
const TOOL_VARIANTS = {
  execcommand: 'shell', bash: 'shell', sh: 'shell', pwsh: 'shell', powershell: 'shell', cmd: 'shell',
  read: 'read', ls: 'read', list: 'read', glob: 'read', cat: 'read', web_fetch: 'read', webfetch: 'read',
  grep: 'search', search: 'search', web_search: 'search', websearch: 'search', find: 'search',
  write: 'write', edit: 'edit', patch: 'edit',
  run_code: 'code', runcode: 'code',
};
const TOOL_TITLE = {
  shell: () => t('toolShell'), read: () => t('toolRead'), search: () => t('toolSearch'),
  write: () => t('toolWrite'), edit: () => t('toolEdit'), code: () => t('toolCode'), others: () => t('toolCall'),
};

// 命令主体提炼：loopx / git / 包管理器子命令优先；python -u -c 提取 import/调用片段。
function smartCommand(cmd) {
  let c = String(cmd || '').trim().replace(/\s+/g, ' ');
  if (!c) return '';
  // 剥掉前置的环境变量赋值（"$env:PYTHONIOENCODING=utf-8;"、"set X=y"、"export X=y"）
  // 和 cd/pushd 这类纯导航噪声 —— 它们本身没有信息量，后面的真实命令才是主体。
  c = c
    .replace(/^\s*(?:(?:\$env:|set\s+|export\s+)[A-Za-z_][A-Za-z0-9_]*=[^;]*;?\s*)+/i, '')
    .replace(/^\s*(?:cd|pushd|set-location)\s+[^;&|]+[;&|]?\s*/i, '')
    .trim();
  if (!c) return '';
  let m = c.match(/loopx(?:\.cli)?\s+(?:--format\s+\S+\s+)?([a-z][a-z-]*)/i);
  if (m) return `loopx ${m[1]}`;
  m = c.match(/\bgit\s+([a-z][a-z-]*)\b/i);
  if (m) return `git ${m[1]}`;
  // python -m <module> [args]：模块名才是信息主体；"-m" 单独匹配会丢掉它。
  // "python -m pip install x" → "pip install x"；"python -m pytest" → "pytest"。
  m = c.match(/\bpython3?\s+-m\s+([A-Za-z0-9_.-]+)(?:\s+([^;&|]{0,60}))?/i);
  if (m) {
    const mod = m[1];
    const rest = (m[2] || '').trim();
    return rest ? `${mod} ${rest}` : mod;
  }
  // python -u -c one-liner first: the code fragment is the informative part.
  m = c.match(/\bpython\s+(?:-u\s+)?-c\s+["']?([^"']{0,70})/i);
  if (m) return `python -c ${m[1].trim().replace(/\s+/g, ' ')}`;
  // Package managers / interpreters: capture the next token (subcommand or flag).
  m = c.match(/\b(pip3?|python3?|npm|pnpm|yarn|cargo|node|npx)\s+([^\s"']+)/i);
  if (m) return `${m[1]} ${m[2]}`;
  // here-doc / here-string 灌给解释器：内容对人是噪音，只报目标解释器。
  m = c.match(/(@'|@"|<<-?['"]?[A-Za-z_]*['"]?)\s*\|\s*(python3?|node|pwsh|powershell)/i);
  if (m && m[2]) return `${m[2]} (heredoc)`;
  const first = c.split(/[;&|]|\r?\n/)[0].trim().replace(/"/g, '');
  return first.length > 90 ? `${first.slice(0, 87)}…` : first;
}

// 文件路径缩短：优先相对于项目根，其次退回「…/末两段」。绝对路径（尤其
// .codex/goals/.../ACTIVE_GOAL_STATE.md 这种）整段上屏只会换行且无信息量。
function shortPath(p, projectDir) {
  const raw = String(p || '').trim();
  if (!raw) return raw;
  const norm = raw.replace(/\\/g, '/');
  const dir = projectDir ? String(projectDir).replace(/\\/g, '/').replace(/\/+$/, '') : '';
  if (dir) {
    const dirLower = dir.toLowerCase();
    const idx = norm.toLowerCase().indexOf(dirLower);
    if (idx === 0) {
      const rel = norm.slice(dir.length).replace(/^\/+/, '');
      if (rel) return rel;
    }
  }
  const parts = norm.split('/');
  return parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : norm;
}

// 关键动作：克隆/提交/推送、装依赖、跑测试、提 PR、写证据 —— 这些要高亮。
const KEY_COMMAND_RE = /(git\s+(clone|commit|push|checkout)|pip(3)?\s+install|npm\s+(install|ci)|cargo\s+(build|test)|pytest|loopx\s+(evidence-log|todo\s+(add|complete)|publish)|pr\s+create|create\s+pr)/i;
function isKeyCommand(cmd) {
  return KEY_COMMAND_RE.test(String(cmd || ''));
}

function toolRowModel(name, brief, projectDir) {
  const variant = TOOL_VARIANTS[String(name || '').toLowerCase()] || 'others';
  const title = (TOOL_TITLE[variant] || TOOL_TITLE.others)();
  const rawText = String(brief || '').trim();
  let args = null;
  let argv = null;
  try {
    const p = JSON.parse(rawText);
    if (Array.isArray(p)) argv = p.map(String).join(' ');
    else if (p && typeof p === 'object') args = p;
  } catch (_) {}
  let summary = '';
  let key = false;
  if (args) {
    if (variant === 'shell') {
      const desc = String(args.description || '').trim();
      const cmd = String(args.command || args.cmd || '');
      summary = desc || smartCommand(cmd);
      key = isKeyCommand(cmd);
    } else if (variant === 'read' || variant === 'write' || variant === 'edit') {
      summary = shortPath(String(args.file_path || args.filePath || args.path || '').trim(), projectDir);
      key = variant !== 'read';
    } else if (variant === 'search') {
      const q = args.query || args.pattern || args.queries;
      summary = typeof q === 'string' ? q : String(q || '');
    } else if (variant === 'code') {
      summary = String(args.description || '').trim() || String(args.code || '').trim();
      key = true;
    } else {
      for (const v of Object.values(args)) {
        if (typeof v === 'string' && v) { summary = v; break; }
      }
    }
  } else if (argv) {
    summary = smartCommand(argv);
  } else if (variant === 'shell') {
    // Partial JSON (mid-stream): best-effort extract the description/command.
    let m = rawText.match(/"(?:description)"\s*:\s*"([^"]*)/);
    if (m && m[1]) summary = m[1].replace(/\\(.)/g, '$1');
    else {
      m = rawText.match(/"(?:command|cmd)"\s*:\s*"([^"]*)/);
      if (m && m[1]) summary = smartCommand(m[1].replace(/\\(.)/g, '$1'));
    }
  }
  if (!summary) summary = rawText.split(/[\r\n]/)[0].slice(0, 90);
  return { variant, title, summary, key };
}

function toolLine(name, brief, projectDir) {
  const row = toolRowModel(name, brief, projectDir);
  return { text: row.summary ? `${row.title} · ${row.summary}` : row.title, key: row.key };
}

app.agent.onEvent((e) => {
  // Authoritative context size: the host reports inputTokens/maxContextTokens
  // after a turn. Remember it per goal so the NEXT "等待模型响应" hint shows
  // the real session context (prompts + prior output + tool results) instead
  // of only the prompt characters we sent. Subagent usage is ignored.
  if (String(e.sourceEvent || '').endsWith('token-usage-updated')) {
    let g = goalForAgentSession(e.sessionId);
    if (!g) {
      const gid = goalIdForSessionId(e.sessionId);
      g = gid ? S.goals.get(gid) : null;
    }
    if (g && !e.isSubagent) {
      const it = Number(e.inputTokens) || 0;
      const max = Number(e.maxContextTokens) || 0;
      if (it > 0) {
        S.config.agentInputTokensByGoal[g.goalId] = it;
        if (max > 0) S.config.agentMaxContextTokensByGoal[g.goalId] = max;
        saveConfig();
      }
    }
    // Token usage IS liveness: the session produced measurable work. Feed the
    // stall watchdog so a turn mid-generation is never judged dead.
    if (g) {
      const liveRun = agentRuns.get(g.goalId);
      if (liveRun) liveRun.lastEventAt = Date.now();
    }
    return;
  }
  // Diagnostic: the activity stream was missing the model's visible text and
  // reasoning (0 agent/think lines). Log every text-chunk here so the next
  // run shows exactly which events reach the MiniApp iframe and where the
  // pipeline drops them.
  if (e.sourceEvent === 'text-chunk') {
    dbgUi('agent:text', `len=${typeof e.text === 'string' ? e.text.length : 0} think=${e.contentType === 'thinking'}`);
  }
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
      const summaryText = cleanGateSummary(String(summaryRun.buffer || '')) || '';
      // Remember for the whole session regardless of goal-object churn, so a
      // re-render never re-runs this model call.
      summaryDoneSession.set(`${summaryRun.goalId}\u0000${summaryRun.todoId}`, summaryText);
      if (sg && isLiveGoal(sg)) {
        sg.gateSummaries.set(summaryRun.todoId, {
          status: 'done',
          // Parse the labeled three-line answer out of whatever the model
          // emitted (reasoning walls included) — the card shows ONLY those
          // three lines.
          text: summaryText,
        });
        scheduleGateSummarySave();
        requestRender(true);
      }
      summaryRuns.delete(e.sessionId);
    } else if (e.sourceEvent === 'dialog-turn-failed' || e.sourceEvent === 'dialog-turn-cancelled') {
      const sg = S.goals.get(summaryRun.goalId);
      // Failed run: mark done-with-empty this session too, so it does not
      // re-trigger on every render; the card falls back to the pattern hints.
      summaryDoneSession.set(`${summaryRun.goalId}\u0000${summaryRun.todoId}`, '');
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
  // 只有真正的模型输出（文字流 / 工具调用）才算「已响应」。
  if (e.sourceEvent === 'tool-event') {
    markModelResponded(g);
    // New bridge nests the tool payload under `toolEvent` (event_type +
    // tool_name/tool_id fields); the legacy flat shape stays as a fallback.
    const te = e.toolEvent || {};
    const name = te.effectiveToolName || te.effective_tool_name
      || te.toolName || te.tool_name || e.toolName || e.tool_name || e.name;
    const phase = te.event_type || te.phase || e.phase;
    const done = phase === 'Completed' || phase === 'completed';
    // In-flight tracking feeds the stall watchdog (long tool runs are silent
    // by design) and the status-bar "工具运行中" narration.
    trackToolEvent(g, te, done);
    if (name && !QUIET_AGENT_TOOLS.has(String(name).toLowerCase())) {
      const brief = toolBrief(e, te, done);
      if (te.tool_id) {
        // One line per tool call. If params have not streamed enough yet to
        // name the command, wait — a bare name with garbage fragments is
        // worse than a line that arrives one event later. A completed call
        // with nothing parseable falls back to the bare name.
        if (toolLinesRecorded.has(te.tool_id)) return;
        // brief is the raw params JSON; toolBriefFromText returns '' while the
        // buffer is still a partial object, so wait for a parseable payload.
        if (!toolBriefFromText(brief, done) && !done) return;
        toolLinesRecorded.set(te.tool_id, true);
        pruneToolMaps();
        const line = toolLine(name, brief, goalProjectDir(g.goalId));
        recordGoalActivity(g, line.text, false, 'tool', null, line.key);
      } else {
        const line = toolLine(name, brief, goalProjectDir(g.goalId));
        recordGoalActivity(g, line.text, false, 'tool', null, line.key);
      }
    }
  } else if (e.sourceEvent === 'text-chunk') {
    markModelResponded(g);
    // Stream the model's visible output AND its reasoning (dimmed) — the
    // user wants to see what the model produces, not just its tools.
    if (typeof e.text === 'string') {
      streamAgentText(g, e.text, e.contentType === 'thinking');
    }
  } else if (e.sourceEvent === 'dialog-turn-completed') {
    const conclusion = turnConclusion(g);
    flushAgentText(g);
    if (conclusion) recordGoalActivity(g, `${t('turnConclusionLabel')}${conclusion}`, false, 'conclusion');
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
document.getElementById('btn-github').addEventListener('click', openTokenDialog);
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

// Restart-recovery banner: "上次有 N 个任务在运行 — [全部继续]". Rendered into
// the composer feedback slot with an action button; dismissed on resume or ✕.
function showBootResumeBanner(goalIds) {
  const feedback = document.getElementById('task-feedback');
  if (!feedback) return;
  feedback.replaceChildren();
  feedback.hidden = false;
  feedback.className = 'composer__feedback composer__feedback--ok';
  const label = document.createElement('span');
  label.textContent = t('bootPausedBanner', goalIds.length);
  const resumeBtn = document.createElement('button');
  resumeBtn.type = 'button';
  resumeBtn.className = 'btn btn--tiny btn--primary';
  resumeBtn.textContent = t('bootPausedResumeAll');
  resumeBtn.onclick = () => {
    let resumed = 0;
    for (const goalId of goalIds) {
      const g = S.goals.get(goalId);
      if (!g || g.archived || isTerminal(g) || g.userStopped) continue;
      setAutoRun(g, true);
      pollNow(g, { force: true });
      resumed += 1;
    }
    S.bootPausedGoalIds = null;
    setTaskFeedback(t('bootPausedResumed', resumed), 'ok');
    renderAllGoals(true);
  };
  const dismissBtn = document.createElement('button');
  dismissBtn.type = 'button';
  dismissBtn.className = 'btn btn--tiny';
  dismissBtn.textContent = t('bootPausedDismiss');
  dismissBtn.onclick = () => {
    S.bootPausedGoalIds = null;
    setTaskFeedback('');
  };
  feedback.append(label, resumeBtn, dismissBtn);
}

// ── #5 空状态 / 上手引导 ───────────────────────────────────
function updateEmptyState() {
  const onboard = document.getElementById('onboard');
  const hint = document.getElementById('detail-empty-hint');
  const icon = document.querySelector('#detail-empty > svg');
  const empty = !S.bootLoading && S.goals.size === 0;
  if (onboard) {
    onboard.hidden = !empty;
    if (empty) {
      document.getElementById('onboard-title').textContent = t('emptyBoardTitle');
      document.getElementById('onboard-hint').textContent = t('emptyBoardHint');
      document.getElementById('onboard-sample-issue').textContent = t('emptySampleIssue');
      document.getElementById('onboard-sample-repo').textContent = t('emptySampleRepo');
    }
  }
  if (hint) hint.hidden = empty;
  if (icon) icon.style.display = empty ? 'none' : '';
}

function updateTaskKind() {
  const badge = document.getElementById('task-kind');
  if (S.composerMode === 'guide') {
    badge.hidden = true;
    updateVisionHint();
    return;
  }
  const input = document.getElementById('composer-link-input');
  const kind = input ? taskInputKind(input.value) : '';
  badge.textContent = kind;
  badge.hidden = !kind;
  updateVisionHint();
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
  const link = document.getElementById('composer-link-input');
  if (link) link.disabled = busy;
  document.getElementById('btn-create-task').disabled = busy;
  setTaskFeedback(message, busy ? '' : undefined);
}

// Compact relative age for an issue's updated_at so the intake list can show
// recency without a full locale-aware date formatter.
function issueAgeLabel(iso) {
  if (!iso) return '';
  const then = new Date(iso).getTime();
  if (!Number.isFinite(then)) return '';
  const minutes = Math.floor((Date.now() - then) / 60000);
  if (minutes < 1) return t('issueAgeNow');
  if (minutes < 60) return t('issueAgeMin', minutes);
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t('issueAgeHour', hours);
  const days = Math.floor(hours / 24);
  if (days < 30) return t('issueAgeDay', days);
  return t('issueAgeMonth', Math.floor(days / 30));
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
      const body = document.createElement('span');
      body.className = 'intake-issue__body';
      const title = document.createElement('span');
      title.className = 'intake-issue__title';
      title.textContent = issue.title === `#${issue.number}` ? issue.url : issue.title;
      title.title = issue.url;
      body.appendChild(title);
      // Metadata line: type labels + comment count + recency, so the user can
      // triage issues at a glance instead of reading every title in full.
      const meta = document.createElement('span');
      meta.className = 'intake-issue__meta';
      if (Array.isArray(issue.labels) && issue.labels.length) {
        for (const label of issue.labels.slice(0, 3)) {
          const chip = document.createElement('span');
          chip.className = 'issue-label';
          chip.textContent = String(label);
          meta.appendChild(chip);
        }
      }
      if (typeof issue.comments === 'number' && issue.comments > 0) {
        const comments = document.createElement('span');
        comments.className = 'intake-issue__comments';
        comments.textContent = t('issueComments', issue.comments);
        meta.appendChild(comments);
      }
      if (issue.updatedAt) {
        const age = document.createElement('span');
        age.className = 'intake-issue__age';
        age.textContent = issueAgeLabel(issue.updatedAt);
        meta.appendChild(age);
      }
      // Image-bearing issues get a marker so the user can spot them at a glance.
      if (issue.hasImages) {
        const badge = document.createElement('span');
        badge.className = 'intake-issue__img';
        badge.textContent = '🖼';
        badge.title = t('issueHasImages');
        meta.appendChild(badge);
      }
      // Already-resolved upstream: flagged here so the user sees it BEFORE
      // confirming — but it stays a notice, not a gate (unchecking is still
      // allowed, though re-checking it would be skipped again at intake).
      if (issue.closed) {
        const badge = document.createElement('span');
        badge.className = 'intake-issue__resolved';
        badge.textContent = t('issueResolvedBadge');
        badge.title = t('issueResolvedBadgeTitle', issue.stateReason || issue.state || 'closed');
        meta.appendChild(badge);
        row.classList.add('intake-issue--resolved');
      }
      if (meta.childElementCount) body.appendChild(meta);
      row.append(cb, num, body);
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
  // Non-blocking notice: issues already closed upstream will be skipped at
  // intake — inform, don't gate. (A user may still uncheck them; the intake
  // loop re-checks live status and skips again regardless.)
  const resolvedCount = resolved.issues.filter((issue) => issue.closed).length;
  const resolvedWarn = document.getElementById('intake-resolved-warn');
  if (resolvedCount > 0) {
    resolvedWarn.hidden = false;
    resolvedWarn.textContent = t('intakeResolvedWarn', resolvedCount);
  } else {
    resolvedWarn.hidden = true;
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
  // 待创建行渲染在「运行中」Tab 顶部：切过去让用户看到 clone/创建进度。
  S.activeBoardTab = 'active';
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
      hideIntakeStepper();
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
    hideIntakeStepper();
    setTaskFeedback(String(err.message || err), 'error');
    log(`task intake error: ${err.message || err}`, true);
  });
}

// ── intake 阶段流水线（横向 stepper）────────────────────────
// 任务创建是固定流程：获取列表 → 克隆 → 创建任务 → 注册 Agent → 写 todos →
// 刷新。全列出来让用户看到总体进度；plan 与 todos 互斥，共用一个位置。
const INTAKE_STAGE_SEQ = [
  { key: 'expand', label: () => t('stageExpand') },
  { key: 'clone', label: () => t('stageClone') },
  { key: 'bootstrap', label: () => t('stageBootstrap') },
  { key: 'register', label: () => t('stageRegister') },
  { key: 'todos', label: () => t('stageWriteTodos') },
  { key: 'refresh', label: () => t('stageRefresh') },
];
const INTAKE_STAGE_IDX = { expand: 0, clone: 1, bootstrap: 2, register: 3, plan: 4, todos: 4, refresh: 5 };

function renderIntakeStepper(currentKey) {
  const el = document.getElementById('intake-stepper');
  if (!el) return;
  const idx = INTAKE_STAGE_IDX[currentKey];
  if (idx == null) { el.hidden = true; return; }
  el.hidden = false;
  el.replaceChildren();
  INTAKE_STAGE_SEQ.forEach((s, i) => {
    if (i > 0) {
      const sep = document.createElement('span');
      sep.className = 'intake-stepper__sep';
      sep.textContent = '›';
      el.appendChild(sep);
    }
    const seg = document.createElement('span');
    seg.className = 'intake-stepper__seg'
      + (i < idx ? ' is-done' : '')
      + (i === idx ? ' is-active' : '');
    const mark = document.createElement('span');
    mark.className = 'intake-stepper__mark';
    mark.textContent = i < idx ? '✓' : String(i + 1);
    const label = document.createElement('span');
    label.className = 'intake-stepper__label';
    label.textContent = s.label();
    seg.append(mark, label);
    el.appendChild(seg);
  });
}
function hideIntakeStepper() {
  const el = document.getElementById('intake-stepper');
  if (el) el.hidden = true;
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
    resolved: t('stageResolved'),
  };
  S.intakeDraft.stage = stages[d.stage] || t('taskCreating');
  setTaskFeedback(S.intakeDraft.stage);
  renderIntakeStepper(d.stage);
  // Patch the pending rail row's stage line in place — a full board rebuild
  // per progress event (clone percent streams) is exactly the flicker we
  // remove.
  const live = document.querySelector('.run-item--pending .goal__activity-text');
  if (live) live.textContent = S.intakeDraft.stage;
});

app.on('worker:taskIntake:done', async (result) => {
  hideIntakeStepper();
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
  // Issues already resolved upstream are NOT errors: skip silently re-fixing
  // them and surface a non-blocking notice (composer note + activity log),
  // never a blocking gate.
  const resolvedItems = Array.isArray(result.skippedResolvedIssues) ? result.skippedResolvedIssues : [];
  const resolvedNote = result.skippedResolved > 0 ? ` ${t('skippedResolved', result.skippedResolved)}` : '';
  const resolvedReason = (it) => it.stateReason || it.state || 'closed';
  for (const it of resolvedItems) {
    log(t('issueResolvedLog', it.number, resolvedReason(it)));
  }
  if (!result.ok) {
    // Goal exists but some todos failed — adopt it, say so honestly.
    setTaskFeedback(t('taskPartial', resultName, result.writtenOk ?? 0, result.error || '') + skipNote + resolvedNote, 'error');
    log(`[${result.goalId}] task intake partial: ${result.error}`, true);
  } else if (result.mode === 'guide') {
    setTaskFeedback(t('guideStarted', resultName) + skipNote + resolvedNote, 'ok');
    log(`[${result.goalId}] guidance written (${result.written.length} todos)`);
  } else {
    setTaskFeedback(t('taskStarted', resultName) + skipNote + resolvedNote, 'ok');
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
    // Non-blocking notice: an issue already closed upstream was skipped at
    // intake, so the agent never re-fixes it. Surface it in the goal's own
    // activity log — informational, never a review-column decision.
    for (const it of resolvedItems) {
      recordGoalActivity(goal, t('issueResolvedLog', it.number, resolvedReason(it)));
    }
    // Memory wiring outcome, also informational (never a gate): whether the
    // OpenViking repository/reward memory actually took effect for this goal.
    if (result.rewardMemory && result.rewardMemory.ok) {
      recordGoalActivity(goal, t('rewardMemoryOn'));
    } else if (result.rewardMemory) {
      recordGoalActivity(goal, t('rewardMemoryOff', result.rewardMemory.error || result.rewardMemory.reason || 'unknown'), true);
    }
    if (result.repoMemorySync && result.repoMemorySync.pending) {
      recordGoalActivity(goal, t('repoMemorySyncStarted'));
    } else if (result.repoMemorySync && result.repoMemorySync.ok) {
      recordGoalActivity(goal, t('repoMemorySyncOn', result.repoMemorySync.scopeRef || '?'));
    } else if (result.repoMemorySync) {
      recordGoalActivity(goal, t('repoMemorySyncOff', result.repoMemorySync.error || result.repoMemorySync.reason || 'unknown'), true);
    }
    // 联动：新建/并入任务后，日志面板立即切到这个目标（不等待首轮启动）。
    openGoalDetails(goal);
    // refreshGoals already polled the goal; a should_run=true decision fires
    // the first turn through maybeAutoRun — one launch path, no races.
    pollNow(goal, { force: true });
  } else {
    requestRender(true);
  }
});

// 后台仓库记忆索引完成/失败：把结果补记进对应任务的活动日志（intake 已不再
// 阻塞等它）。失败是非致命的——记忆缺席只是降级，不影响修复流程。
app.on('worker:repoMemorySync:done', (d) => {
  if (!d || !d.goalId) return;
  const goal = S.goals.get(d.goalId);
  if (!goal || !isLiveGoal(goal)) return;
  if (d.ok) recordGoalActivity(goal, t('repoMemorySyncOn', d.scopeRef || '?'));
  else recordGoalActivity(goal, t('repoMemorySyncOff', d.error || d.reason || 'unknown'), true);
});

async function createTaskFromInput() {
  dbgUi('createTask:start', `mode=${S.composerMode}`);

  // 引导 mode: the textarea holds the guidance message; route to startGuidance.
  if (S.composerMode === 'guide') {
    const input = document.getElementById('task-input');
    const text = (input.value || '').trim();
    if (!text) { input.focus(); return; }
    const target = guidanceTargetGoal();
    if (!target) {
      const runningCount = [...S.goals.values()].filter((g) => g.running).length;
      dbgUi('createTask:guidanceRejected', `running=${runningCount}`);
      setTaskFeedback(runningCount > 1 ? t('guidancePickOne') : t('guidanceNoRunning'), 'error');
      return;
    }
    startGuidance(target, text);
    return;
  }

  // 新建 mode: link + optional note → issue intake (always a new task).
  const objective = composerObjective();
  if (!objective) {
    const link = document.getElementById('composer-link-input');
    if (link) link.focus();
    return;
  }
  if (!taskInputKind(objective)) {
    const bad = firstUnsupportedGithubUrl(objective);
    if (bad) {
      dbgUi('createTask:unsupported', bad);
      setTaskFeedback(t('taskUnsupportedPath', bad), 'error');
      return;
    }
    setTaskFeedback(t('taskNeedLink'), 'error');
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
  // Single-issue submissions go through the SAME confirmation sheet as
  // batches: the sheet is the user's documented consent to fix (and edit)
  // this repository — refreshUserTodos later cites it to auto-approve the
  // write-access gate, so skipping it would auto-approve on consent that was
  // never actually shown.
  if (resolved.issues.length === 0) {
    startTaskIntake({ resolved, objective, selected: new Set() }, null);
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
  // 联动：把日志面板切到被纠正的目标，让用户立刻看到指令落在哪里。
  if (S.activeGoalId !== g.goalId) {
    S.activeGoalId = g.goalId;
    document.getElementById('goal-detail-panel').hidden = false;
    document.getElementById('detail-empty').hidden = true;
    renderGoalDetails(g);
  }
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

// 双态 composer：模式 tab 切换 + 链接输入实时 unfurl。
document.getElementById('mode-new').addEventListener('click', () => setComposerMode('new'));
document.getElementById('mode-guide').addEventListener('click', () => setComposerMode('guide'));
const linkInputEl = document.getElementById('composer-link-input');
if (linkInputEl) {
  linkInputEl.addEventListener('input', () => {
    renderLinkUnfurl();
    updateTaskKind();
    setTaskFeedback('');
  });
  linkInputEl.addEventListener('keydown', (event) => {
    if (event.key !== 'Enter' || event.shiftKey) return;
    event.preventDefault();
    createTaskFromInput();
  });
}

// #5 空状态：样例模板。占位符 <owner>/<repo> 无法直接提交(会解析失败),
// 用 placeholder 展示格式、把光标留在空输入框里让用户直接粘贴真实链接。
const sampleFill = (placeholder) => {
  setComposerMode('new');
  const input = document.getElementById('composer-link-input');
  if (input) {
    input.value = '';
    input.placeholder = placeholder;
    input.focus();
  }
  renderLinkUnfurl();
  updateTaskKind();
};
document.getElementById('onboard-sample-issue').addEventListener('click', () => {
  sampleFill('https://github.com/owner/repo/issues/123');
});
document.getElementById('onboard-sample-repo').addEventListener('click', () => {
  sampleFill('https://github.com/owner/repo');
});

// ── #6 键盘筛选/审批 ──────────────────────────────────────
// review 列是决策队列：j/k 移动、Enter/x 打开详情、a 打开首个审批。
function moveReviewFocus(cards, delta) {
  const idx = cards.findIndex((c) => c === document.activeElement);
  const next = idx < 0 ? 0 : (idx + delta + cards.length) % cards.length;
  cards[next].focus();
  cards[next].scrollIntoView({ block: 'nearest' });
}
document.addEventListener('keydown', (e) => {
  if (document.querySelector('dialog[open]')) return;
  const tag = (document.activeElement && document.activeElement.tagName) || '';
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  const reviewCards = [...document.querySelectorAll('#review-list .board-row[tabindex="0"]')];
  if (!reviewCards.length) return;
  const key = e.key.toLowerCase();
  if (key === 'j' || key === 'arrowdown') {
    e.preventDefault();
    moveReviewFocus(reviewCards, 1);
  } else if (key === 'k' || key === 'arrowup') {
    e.preventDefault();
    moveReviewFocus(reviewCards, -1);
  } else if (key === 'enter' || key === 'x') {
    const target = reviewCards.find((c) => c === document.activeElement) || reviewCards[0];
    if (target) { e.preventDefault(); target.click(); }
  } else if (key === 'a') {
    const target = reviewCards.find((c) => c === document.activeElement) || reviewCards[0];
    const approveBtn = target && target.querySelector('.btn--approve');
    if (approveBtn) { e.preventDefault(); approveBtn.click(); }
  }
});
document.getElementById('composer-model').addEventListener('change', async () => {
  S.config.defaultModel = document.getElementById('composer-model').value || 'auto';
  await saveConfig();
  log(t('modelChanged', S.config.defaultModel));
  updateVisionHint();
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
// The old #D in-progress goal dropdown was folded into the board + composer
// tabs; its DOM element is gone, so no listeners are wired here.


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
  if (review && S.config.reviewZoneWidth > 0) {
    review.style.flex = `0 0 ${S.config.reviewZoneWidth}px`;
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

// ── 环境引导页（Setup Gate）───────────────────────────────
// 首次使用 / 环境缺依赖时，用引导页替代任务输入框：一键装好并配置，再放行到
// 主界面。检测通过则不打扰（直接进 composer）。
const ENV_ITEM_LABEL = {
  python: () => t('envItemPython'),
  git: () => t('envItemGit'),
  loopx: () => t('envItemLoopx'),
  openviking: () => t('envItemOpenViking'),
  openvikingRunning: () => t('envItemOpenVikingServer'),
  openvikingCli: () => t('envItemOvCli'),
  gh: () => t('envItemGh'),
};
const ENV_ITEM_WHY = {
  python: { zh: 'loopx / OpenViking 的运行时', en: 'Runtime for loopx / OpenViking' },
  git: { zh: '克隆目标仓库', en: 'Clone target repos' },
  loopx: { zh: '修 issue 引擎', en: 'The issue-fix engine' },
  openviking: { zh: '仓库记忆 / 语义检索', en: 'Repo memory / semantic search' },
  openvikingRunning: { zh: '上下文数据库服务', en: 'Context database service' },
  openvikingCli: { zh: '记忆读写通道（ov CLI 连接）', en: 'Memory read/write channel (ov CLI)' },
  gh: { zh: '发布 PR / 解除 API 限流', en: 'Publish PRs / lift API rate limit' },
};
// Manual install pointers for deps the one-click flow cannot install itself
// (python/git are ITS prerequisites). Opened via the system browser.
const ENV_ITEM_INSTALL = {
  python: {
    url: 'https://www.python.org/downloads/',
    cmd: () => (navigator.platform || '').startsWith('Win') ? 'winget install Python.Python.3.12' : 'brew install python',
  },
  git: {
    url: 'https://git-scm.com/downloads',
    cmd: () => (navigator.platform || '').startsWith('Win') ? 'winget install Git.Git' : 'brew install git',
  },
};
function envWhy(key) {
  const locale = app.locale === 'en-US' ? 'en' : 'zh';
  return (ENV_ITEM_WHY[key] || {})[locale] || '';
}

function envOnboardShowState(name) {
  for (const key of ['loading', 'guide', 'progress', 'done']) {
    const el = document.getElementById(`env-state-${key}`);
    if (el) el.hidden = key !== name;
  }
}

function envOnboardSetVisible(visible) {
  const overlay = document.getElementById('env-onboard');
  if (overlay) overlay.hidden = !visible;
}

function renderEnvChecklist(items) {
  const list = document.getElementById('env-onboard-list');
  if (!list) return;
  list.replaceChildren();
  const rows = [
    ['python', items.python],
    ['git', items.git],
    ['loopx', items.loopx],
    ['openviking', items.openviking],
    ['openvikingRunning', items.openvikingRunning],
    ['openvikingCli', items.openvikingCli],
    ['gh', items.gh],
  ];
  for (const [key, item] of rows) {
    // "OpenViking 服务" 需要二进制存在且进程正在监听端口，二者都就绪才算通过。
    const ok = key === 'openvikingRunning'
      ? !!(item && item.ok && items.openvikingServer && items.openvikingServer.ok)
      : !!(item && item.ok);
    const li = document.createElement('li');
    li.className = `env-onboard__item ${ok ? 'env-onboard__item--ok' : 'env-onboard__item--miss'}`;
    const dot = document.createElement('span');
    dot.className = 'env-onboard__item-dot';
    const label = document.createElement('span');
    label.className = 'env-onboard__item-label';
    const name = document.createElement('span');
    name.textContent = (ENV_ITEM_LABEL[key] && ENV_ITEM_LABEL[key]()) || key;
    const why = document.createElement('span');
    why.className = 'env-onboard__item-why';
    why.textContent = ok ? (item.version || envWhy(key)) : `缺失 · ${envWhy(key)}`;
    label.append(name, document.createElement('br'), why);
    li.append(dot, label);
    if (key === 'gh' && !ok) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn btn--tiny btn--primary';
      btn.textContent = t('ghLoginBtn');
      btn.onclick = () => runGhLoginFromOnboard(btn);
      li.appendChild(btn);
    }
    // python/git 是「一键安装」自身的前置：装不了它们自己，必须给出可执行的
    // 出口（官网下载 + 平台安装命令），而不是一行「缺失」死文案。
    if (ENV_ITEM_INSTALL[key] && !ok) {
      const info = ENV_ITEM_INSTALL[key];
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn btn--tiny btn--primary';
      btn.textContent = t('envInstallLinkBtn');
      btn.onclick = () => openExternalUrl(info.url);
      li.appendChild(btn);
      const cmdHint = document.createElement('code');
      cmdHint.className = 'env-onboard__item-cmd';
      cmdHint.textContent = info.cmd();
      cmdHint.title = t('envInstallCmdHint');
      li.appendChild(cmdHint);
    }
    // ov CLI 连接断开可自愈：给一个「重连」按钮直接调 ensureOvCli。
    if (key === 'openvikingCli' && !ok && items.openviking && items.openviking.ok) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn btn--tiny';
      btn.textContent = t('envOvCliReconnect');
      btn.onclick = async () => {
        btn.disabled = true;
        try {
          const res = await app.call('loopx.ensureOvCli');
          if (res && res.ok) {
            const check = await app.call('loopx.envCheck');
            if (check && check.items) renderEnvChecklist(check.items);
            return;
          }
        } catch (_) {}
        btn.disabled = false;
      };
      li.appendChild(btn);
    }
    list.appendChild(li);
  }
}

// Inline gh login from the env checklist: the same one-click `gh auth login
// --web` flow as the token dialog, but self-contained so the onboarding works
// without opening that dialog.
async function runGhLoginFromOnboard(btn) {
  btn.disabled = true;
  btn.textContent = t('ghLoginWorking');
  try {
    const res = await app.call('loopx.ghLogin', {});
    if (!res || !res.ok) throw new Error((res && res.error) || 'gh login failed');
    S.ghAvailable = true;
    btn.textContent = t('ghLoginDoneShort');
    log(`[gh] login complete (${res.login || '?'})`);
    const check = await app.call('loopx.envCheck');
    if (check && check.items) renderEnvChecklist(check.items);
  } catch (err) {
    const message = String(err && err.message || err);
    setTaskFeedback(`${t('ghLoginFailed')}：${message}`, 'error');
    btn.disabled = false;
    btn.textContent = t('ghLoginBtn');
  }
}

async function envOnboardFillDefaults() {
  try {
    const res = await app.call('loopx.defaultVlmConfig');
    const vlm = res && res.vlm;
    if (vlm) {
      const model = document.getElementById('env-cfg-vlm-model');
      const base = document.getElementById('env-cfg-vlm-base');
      const key = document.getElementById('env-cfg-vlm-key');
      if (model) model.value = vlm.model || 'deepseek-v4-flash';
      if (base) base.value = vlm.api_base || 'https://api.openbitfun.com/v1';
      if (key) key.value = vlm.api_key || '';
    }
  } catch (_) {
    // Defaults stay empty; the user can type credentials in the advanced block.
  }
}

// Synchronous readiness cache so boot() can decide to gate with the overlay
// BEFORE its first await — otherwise the composer paints ahead of the gate on
// a first launch. localStorage is a best-effort fast path; the durable source
// of truth is S.config.envReadyAt (app.storage). Both are written together.
const ENV_READY_CACHE_KEY = 'bitfun-loopx.envReadyAt';
function envReadyCacheGet() {
  try {
    if (typeof localStorage === 'undefined') return 0;
    return Number(localStorage.getItem(ENV_READY_CACHE_KEY)) || 0;
  } catch (_) { return 0; }
}
function envReadyCacheSet(ts) {
  try {
    if (typeof localStorage === 'undefined') return;
    if (ts) localStorage.setItem(ENV_READY_CACHE_KEY, String(ts));
    else localStorage.removeItem(ENV_READY_CACHE_KEY);
  } catch (_) {}
}
function envMarkReady(ts = Date.now()) {
  S.config.envReadyAt = ts;
  envReadyCacheSet(ts);
  saveConfig();
}
function envMarkNotReady() {
  S.config.envReadyAt = 0;
  envReadyCacheSet(0);
  saveConfig();
}
// "Ready" gates on INSTALLED deps, not on whether the OpenViking server is
// currently listening: the server is a runtime state the console can (re)start
// on its own, so a reboot that leaves the server down must NOT re-open the
// setup guide every launch.
function envInstalledReady(items) {
  return !!(items
    && items.python && items.python.ok
    && items.git && items.git.ok
    && items.loopx && items.loopx.ok
    && items.openviking && items.openviking.ok
    && items.openvikingServer && items.openvikingServer.ok);
}

// silent=true means we already believe the env is ready (confirmed on a prior
// launch): run the check in the background with NO overlay, and only surface
// the guide if something actually broke. silent=false is the first-launch path:
// the overlay (loading) is already visible and we flip to guide or pass.
async function runEnvOnboarding({ silent = false } = {}) {
  const overlay = document.getElementById('env-onboard');
  if (!overlay) return;
  if (!silent) {
    envOnboardShowState('loading');
    envOnboardSetVisible(true);
  }
  let check = null;
  try {
    check = await app.call('loopx.envCheck');
  } catch (_) {
    check = null;
  }
  if (!check) {
    // Could not probe (worker error etc.) — never block the user on it.
    if (!silent) envOnboardSetVisible(false);
    return;
  }
  if (envInstalledReady(check.items)) {
    // Deps are installed. If the OpenViking server is merely not listening
    // (e.g. after a reboot), (re)start it silently instead of gating the user.
    const running = check.items.openvikingRunning && check.items.openvikingRunning.ok;
    if (!running) {
      try { await app.call('loopx.startOvServer'); } catch (_) {}
    }
    // The `ov` CLI needs a saved config pointing at the server before loopx's
    // repository/reward memory can reach it. Installed binaries are not the
    // same as a connected CLI — self-heal (re-register + verify) when missing,
    // so a prior partial setup can never leave memory silently dead.
    const cliConnected = check.items.openvikingCli && check.items.openvikingCli.ok;
    if (!cliConnected) {
      try { await app.call('loopx.ensureOvCli'); } catch (_) {}
    }
    envMarkReady();
    if (!silent) envOnboardSetVisible(false);
    return;
  }
  // Deps actually missing: this covers BOTH first launch and a later breakage
  // detected by the silent background re-check.
  renderEnvChecklist(check.items || {});
  await envOnboardFillDefaults();
  envOnboardShowState('guide');
  envOnboardSetVisible(true);
  envMarkNotReady();
}

function envOnboardAppendLog(line) {
  const logEl = document.getElementById('env-onboard-log');
  if (!logEl) return;
  logEl.textContent += String(line || '') + '\n';
  logEl.scrollTop = logEl.scrollHeight;
}

async function runEnvInstall() {
  envOnboardShowState('progress');
  const logEl = document.getElementById('env-onboard-log');
  if (logEl) logEl.textContent = '';
  const vlm = {
    provider: 'openai',
    model: (document.getElementById('env-cfg-vlm-model') || {}).value || 'deepseek-v4-flash',
    api_base: (document.getElementById('env-cfg-vlm-base') || {}).value || 'https://api.openbitfun.com/v1',
    api_key: (document.getElementById('env-cfg-vlm-key') || {}).value || '',
  };
  try {
    // 0) python/git 是本流程装不了的硬前置：先探测，缺了直接给出安装指引并
    //    终止，而不是让后面的 pip 以 spawn ENOENT 失败打转。
    let check0 = null;
    try { check0 = await app.call('loopx.envCheck'); } catch (_) { check0 = null; }
    const items0 = (check0 && check0.items) || {};
    if (items0.python && !items0.python.ok) {
      envOnboardAppendLog(t('prereqNeedPython'));
      envOnboardShowState('guide');
      renderEnvChecklist(items0);
      return;
    }
    if (items0.git && !items0.git.ok) {
      envOnboardAppendLog(t('prereqNeedGit'));
      envOnboardShowState('guide');
      renderEnvChecklist(items0);
      return;
    }
    // 1) 缺 loopx 引擎先装(vendor 拉源码,免 pip):没有它整个产品无法工作,
    //    只装 OpenViking 却宣布「环境就绪」是自相矛盾。
    if (items0.loopx && !items0.loopx.ok) {
      envOnboardAppendLog('$ 拉取 loopx 源码（vendor，无需 pip）');
      const vend = await app.call('loopx.ensureVendor');
      if (!vend || !vend.ok) {
        envOnboardAppendLog(`loopx 安装失败：${(vend && vend.error) || 'unknown'}`);
        envOnboardShowState('guide');
        return;
      }
      envOnboardAppendLog('✓ loopx 就绪');
    }
    // 2) 缺 OpenViking 才装
    let needInstall = !(items0.openviking && items0.openviking.ok);
    if (needInstall) {
      envOnboardAppendLog('$ pip install openviking[local-embed]');
      const inst = await app.call('loopx.installOpenViking');
      if (!inst || !inst.ok) {
        envOnboardAppendLog(`安装失败：${(inst && inst.error) || 'unknown'}`);
        envOnboardShowState('guide');
        return;
      }
      envOnboardAppendLog('✓ OpenViking 安装完成');
    }
    // 3) 写 ov.conf
    envOnboardAppendLog('$ 写入 ~/.openviking/ov.conf');
    const conf = await app.call('loopx.writeOvConf', { vlm });
    if (!conf || !conf.ok) {
      envOnboardAppendLog(`配置写入失败：${(conf && conf.error) || 'unknown'}`);
      envOnboardShowState('guide');
      return;
    }
    envOnboardAppendLog('✓ ov.conf 写入完成');
    if (conf.vlmKeyEmpty) envOnboardAppendLog(`⚠ ${t('envVlmKeyEmpty')}`);
    // 4) 起 server
    envOnboardAppendLog('$ openviking-server');
    const svc = await app.call('loopx.startOvServer');
    if (!svc || !svc.ok) {
      envOnboardAppendLog(`启动失败：${(svc && svc.error) || 'unknown'}`);
      envOnboardShowState('guide');
      return;
    }
    envOnboardAppendLog('✓ OpenViking server 已启动（后台）');
    // 5) 连 CLI
    envOnboardAppendLog('$ ov config add custom --url http://127.0.0.1:1933 --activate');
    const cli = await app.call('loopx.ovConfigCli');
    envOnboardAppendLog(cli && cli.ok ? '✓ ov CLI 已连接' : `⚠ ${(cli && cli.error) || 'ov 连接失败'}`);
    // 6) 完成前真实复检：spawn 成功 ≠ server 活着。轮询 envCheck（最多 ~15s）,
    //    全绿才进 done——避免「✓ 环境就绪」和下次启动被门禁拦回自相矛盾。
    envOnboardAppendLog(t('envVerifying'));
    let finalCheck = null;
    for (let i = 0; i < 15; i += 1) {
      try { finalCheck = await app.call('loopx.envCheck'); } catch (_) { finalCheck = null; }
      if (finalCheck && envInstalledReady(finalCheck.items)
        && finalCheck.items.openvikingRunning && finalCheck.items.openvikingRunning.ok) break;
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    if (finalCheck && envInstalledReady(finalCheck.items)) {
      envOnboardAppendLog(`✓ ${t('envVerifyOk')}`);
      envOnboardShowState('done');
    } else {
      envOnboardAppendLog(`⚠ ${t('envVerifyFailed')}`);
      renderEnvChecklist((finalCheck && finalCheck.items) || {});
      envOnboardShowState('guide');
    }
  } catch (err) {
    envOnboardAppendLog(`出错：${String(err && err.message || err)}`);
    envOnboardShowState('guide');
  }
}

app.on('worker:installOpenViking:progress', (d) => {
  if (d && d.line != null) envOnboardAppendLog(d.line);
});
// loopx vendor progress also streams into the env install log when the
// onboarding flow is the one driving it (progress state visible).
app.on('worker:vendorLoopx:progress', (d) => {
  const progressEl = document.getElementById('env-state-progress');
  if (progressEl && !progressEl.hidden && d && d.line != null) envOnboardAppendLog(d.line);
});

const envBtnInstall = document.getElementById('btn-env-install');
const envBtnRetry = document.getElementById('btn-env-retry');
const envBtnDone = document.getElementById('btn-env-done');
if (envBtnInstall) envBtnInstall.addEventListener('click', runEnvInstall);
if (envBtnRetry) envBtnRetry.addEventListener('click', () => runEnvOnboarding());
if (envBtnDone) envBtnDone.addEventListener('click', () => {
  // Setup completed: mark the env ready so later launches skip the gate
  // (they still re-check silently in the background).
  envMarkReady();
  envOnboardSetVisible(false);
  requestRender(true);
});

// ── boot ──────────────────────────────────────────────────
(async function boot() {
  dbgUi('boot:start', `t=${bootMs()}ms readyState=${document.readyState} theme=${themeProbe()}`);
  // Env gate: if we have never confirmed the environment ready, cover the
  // composer with the loading overlay SYNCHRONOUSLY (before any await) so it
  // never paints ahead of the setup gate on a first launch. Confirmed installs
  // skip this entirely and re-check silently in the background.
  const envKnownReady = envReadyCacheGet() > 0;
  if (!envKnownReady) {
    envOnboardShowState('loading');
    envOnboardSetVisible(true);
  }
  await loadConfig();
  // Reconcile the durable stamp with the sync cache (localStorage may be
  // unavailable or cleared): if the durable flag says ready, skip the gate.
  if (!envKnownReady && S.config.envReadyAt > 0) {
    envReadyCacheSet(S.config.envReadyAt);
    envOnboardSetVisible(false);
  }
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
  runEnvOnboarding({ silent: envKnownReady || S.config.envReadyAt > 0 });
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
  // BUT the "was running before" fact must survive: snapshot it and offer a
  // one-click 全部继续, otherwise a 10-issue unattended batch dies silently
  // at every app update/restart with only per-card 继续 to recover.
  let pausedAny = false;
  const runningBefore = [];
  for (const g of S.goals.values()) {
    if (g.autoRun) {
      g.autoRun = false;
      S.config.autoRunByGoal[g.goalId] = false;
      if (!g.archived && !isTerminal(g)) runningBefore.push(g.goalId);
      pausedAny = true;
    }
  }
  if (pausedAny) saveConfig();
  if (runningBefore.length) {
    S.bootPausedGoalIds = runningBefore;
    showBootResumeBanner(runningBefore);
  }
  // Fill the composer target dropdown even before the first board paint.
  refillComposerTarget();
  const paints = performance.getEntriesByType('paint')
    .map((p) => `${p.name}@${Math.round(p.startTime)}ms`).join(' ') || '(no paint entries)';
  dbgUi('boot:done', `t=${bootMs()}ms goals=${S.goals.size} theme=${themeProbe()} paint=${paints}`);
  requestRender(true);
})();
