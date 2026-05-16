const STORAGE_KEY = 'builtin-pr-review-state-v2';
const DEFAULT_POLL_MINUTES = 5;
const MAX_WORKSPACE_SCAN_DEPTH = 3;
const MAX_WORKSPACE_SCAN_DIRS = 180;
const SKIP_WORKSPACE_DIRS = new Set([
  '.git',
  '.bitfun',
  '.svn',
  '.hg',
  'node_modules',
  'target',
  'dist',
  'build',
  '.next',
  '.turbo',
  '.cache',
  'vendor',
]);

const I18N = {
  'en-US': {
    title: 'PR Review Inbox',
    subtitle: 'Watch repositories, open PRs, review diffs, compose feedback, and publish with confirmation.',
    queueModeAll: 'Repository PRs',
    queueModeMine: 'Needs my review',
    queueModeAllHint: 'Sync open PRs from watched repositories.',
    queueModeMineHint: 'Requires a session token because providers need your identity.',
    directUrl: 'Open PR URL',
    directPlaceholder: 'Paste a GitHub, GitCode, or private PR link',
    repoRef: 'Repository',
    repoRefPlaceholder: 'owner/repo or repository URL',
    openPr: 'Open PR',
    openExternal: 'Open in browser',
    syncQueue: 'Sync queue',
    syncMine: 'Sync assigned',
    syncAllTitle: 'Repository PRs',
    syncAllDesc: 'Fetch open PRs from watched repositories.',
    syncMineTitle: 'Needs my review',
    syncMineDesc: 'Use your provider identity to find review requests.',
    startReview: 'Start AI review',
    refreshNow: 'Refresh now',
    autoSync: 'Auto-sync',
    every: 'every',
    minutes: 'min',
    queue: 'Review queue',
    queueEmpty: 'Add a watched repository, sync assigned PRs, or paste a PR link above.',
    sources: 'Sources and access',
    watchedRepos: 'Watched repositories',
    noWatchedRepos: 'No watched repositories yet.',
    addWatchedRepo: 'Add watched repository',
    providers: 'Providers',
    addProvider: 'Add provider',
    delete: 'Delete',
    removeRepo: 'Remove',
    listenEnabled: 'Listening',
    listenDisabled: 'Paused',
    pauseListening: 'Pause listening',
    resumeListening: 'Resume listening',
    save: 'Save',
    provider: 'Provider',
    token: 'Session token',
    tokenReady: 'Token ready',
    tokenMissing: 'No token',
    tokenHelp: 'Public PRs can be read without a token. Assigned-review sync, private repositories, and publishing require one. Tokens stay in memory only.',
    authorizeGitHubCli: 'Use GitHub CLI',
    authorizingGitHubCli: 'Reading GitHub CLI token...',
    tokenFromGh: 'Loaded token from GitHub CLI',
    authUnavailable: 'Could not read GitHub CLI token. Run `gh auth login` first or paste a token.',
    authAutoHint: 'GitHub token is read from local gh when needed and kept in memory only.',
    manualToken: 'Manual token',
    repoAddedSyncing: 'Repository added. Syncing queue...',
    advancedProviders: 'Advanced provider settings',
    errorRepoNotFound: 'Repository not found or not accessible. Check the repository path or authorize GitHub CLI.',
    workspaceDiscovering: 'Detecting repositories from the current workspace...',
    workspaceDiscovered: 'Added {count} repository from the current workspace.',
    workspaceDiscoveredMany: 'Added {count} repositories from the current workspace.',
    workspaceNone: 'No Git remotes were detected in the current workspace.',
    workspaceRepo: 'Workspace',
    rediscoverWorkspace: 'Detect workspace repos',
    repositoryFirst: 'Watch a repository',
    repositoryFirstHint: 'Add the repo you care about, then sync its review queue.',
    singlePrFallback: 'Inspect one PR',
    singlePrFallbackHint: 'Use this when the repository is not watched yet.',
    owner: 'Owner',
    repo: 'Repository',
    providerName: 'Display name',
    kind: 'Kind',
    webBase: 'Web base URL',
    apiBase: 'API base URL',
    credentialLabel: 'Token label',
    github: 'GitHub',
    gitcode: 'GitCode',
    custom: 'Custom',
    selectedPr: 'Selected PR',
    noPr: 'Select a PR to start review.',
    author: 'Author',
    state: 'State',
    branch: 'Branch',
    created: 'Created',
    updated: 'Updated',
    files: 'Changed files',
    changedLines: 'Changed lines',
    overview: 'Overview',
    ciDetails: 'CI details',
    existingReview: 'Existing discussion',
    ciFolded: 'CI is folded by default. Open it when status needs attention.',
    noCi: 'No CI status returned.',
    noBody: 'No description.',
    noFiles: 'No changed files returned by the provider.',
    noReviews: 'No review comments returned by the provider.',
    manualComment: 'Manual comment',
    manualCommentPlaceholder: 'Write a PR-level comment or paste a finding here.',
    addManualComment: 'Add to review',
    composer: 'Review composer',
    composerHint: 'Draft, edit, select, then publish. Nothing is sent without confirmation.',
    modeFast: 'Fast scan',
    modeFocused: 'Focused',
    modeDeep: 'Deep',
    summaryComment: 'Summary comment',
    inlineComment: 'Inline comment',
    reviewDecision: 'Review decision',
    decisionComment: 'Comment',
    decisionApprove: 'Approve',
    decisionRequestChanges: 'Request changes',
    selectedOps: 'selected',
    publishSelected: 'Publish selected',
    publishConfirmTitle: 'Publish selected review items?',
    publishConfirmBody: 'These comments will be posted to the provider. This action cannot be undone from BitFun.',
    publishStaleTitle: 'PR head changed',
    publishStaleBody: 'The draft was created for an older head. Refresh or confirm that you want to publish against the latest head.',
    staleConfirm: 'I understand the PR head changed',
    publishNow: 'Publish now',
    cancel: 'Cancel',
    markReviewed: 'Mark current head reviewed',
    audit: 'Publish audit',
    statusReady: 'Ready',
    statusRefreshing: 'Syncing queue...',
    statusAssignedNeedsToken: 'Assigned-review sync needs a session token for the selected provider.',
    statusNoSubscriptions: 'Add a watched repository or paste a PR URL first.',
    statusNoActiveSubscriptions: 'All watched repositories are paused. Re-enable one or paste a PR URL.',
    statusLoading: 'Loading PR...',
    statusOpeningPr: 'Opening PR...',
    statusGenerating: 'Generating review draft...',
    reviewProgress: 'Review progress',
    reviewStageRead: 'Reading PR metadata and diffs',
    reviewStageAi: 'Asking AI to draft review comments',
    reviewStageBuild: 'Preparing editable review items',
    reviewDetailOpeningPr: 'Fetching PR metadata, changed files, reviews, and status.',
    reviewDetailRead: 'Reading metadata and changed files.',
    reviewDetailAi: 'AI is analyzing the diff and existing discussion.',
    reviewDetailAiWait: 'Still analyzing; large diffs can take a little while.',
    reviewDetailBuild: 'Building an editable review draft.',
    cancelReview: 'Cancel review',
    reviewCancelled: 'Review cancelled',
    statusPublishing: 'Publishing review...',
    statusSaved: 'Saved',
    statusPublished: 'Review published',
    statusReviewed: 'Current head marked reviewed',
    errorParse: 'Could not identify a PR from this URL.',
    errorNetwork: 'Provider request failed',
    newPrTitle: 'New reviewable PR',
    newHeadTitle: 'New commits on reviewed PR',
    publicRead: 'Public read',
    privateAction: 'Private and write actions',
    draftStatus: 'Draft',
    readyStatus: 'Ready',
    overviewHint: 'Expand for full description.',
    noActionableFindings: 'No actionable findings were generated. Add a manual comment or edit this review decision before publishing.',
    binary: 'binary',
    large: 'large',
    stale: 'stale',
    published: 'published',
    skipped: 'skipped',
    failed: 'failed',
    success: 'success',
  },
  'zh-CN': {
    title: 'PR 审核台',
    subtitle: '监听仓库、打开 PR、查看变更、组织意见，并在二次确认后发布 Review。',
    queueModeAll: '仓库全部 PR',
    queueModeMine: '待我审核',
    queueModeAllHint: '从已监听仓库同步打开状态的 PR。',
    queueModeMineHint: '需要会话 Token，因为代码平台要识别你的身份。',
    directUrl: '打开 PR 链接',
    directPlaceholder: '粘贴 GitHub、GitCode 或内网 PR 链接',
    repoRef: '仓库',
    repoRefPlaceholder: 'owner/repo 或仓库链接',
    openPr: '打开 PR',
    openExternal: '在浏览器打开',
    syncQueue: '同步队列',
    syncMine: '同步待我审核',
    syncAllTitle: '仓库全部 PR',
    syncAllDesc: '从已监听仓库拉取打开状态的 PR。',
    syncMineTitle: '待我审核',
    syncMineDesc: '使用你的平台身份查找需要你审核的 PR。',
    startReview: '开始 AI 审核',
    refreshNow: '立即刷新',
    autoSync: '自动刷新',
    every: '每',
    minutes: '分钟',
    queue: '审核队列',
    queueEmpty: '先添加监听仓库、同步待我审核，或在上方粘贴 PR 链接。',
    sources: '来源与权限',
    watchedRepos: '监听仓库',
    noWatchedRepos: '还没有监听仓库。',
    addWatchedRepo: '添加监听仓库',
    providers: '代码平台',
    addProvider: '添加平台',
    delete: '删除',
    removeRepo: '移除',
    listenEnabled: '监听中',
    listenDisabled: '已暂停',
    pauseListening: '暂停监听',
    resumeListening: '重新开启监听',
    save: '保存',
    provider: '代码平台',
    token: '会话 Token',
    tokenReady: 'Token 已填写',
    tokenMissing: '未填写 Token',
    tokenHelp: '公开 PR 可不填 Token 读取。同步待我审核、私有仓库和发布评论需要 Token。Token 只保存在当前内存中。',
    authorizeGitHubCli: '使用 GitHub CLI',
    authorizingGitHubCli: '正在读取 GitHub CLI Token...',
    tokenFromGh: '已从 GitHub CLI 读取 Token',
    authUnavailable: '无法读取 GitHub CLI Token。请先运行 `gh auth login`，或手动粘贴 Token。',
    authAutoHint: '需要时会从本地 gh 自动读取 GitHub Token，仅保存在当前内存。',
    manualToken: '手动 Token',
    repoAddedSyncing: '已添加监听仓库，正在同步队列...',
    advancedProviders: '高级平台设置',
    errorRepoNotFound: '仓库不存在或当前无权访问，请检查仓库路径或授权 GitHub CLI。',
    workspaceDiscovering: '正在从当前工作区识别仓库...',
    workspaceDiscovered: '已从当前工作区添加 {count} 个仓库。',
    workspaceDiscoveredMany: '已从当前工作区添加 {count} 个仓库。',
    workspaceNone: '当前工作区没有识别到 Git remote。',
    workspaceRepo: '工作区',
    rediscoverWorkspace: '识别工作区仓库',
    repositoryFirst: '监听仓库',
    repositoryFirstHint: '先添加要关注的仓库，再同步它的审核队列。',
    singlePrFallback: '单独检视一个 PR',
    singlePrFallbackHint: '当这个仓库暂时不需要监听时使用。',
    owner: 'Owner',
    repo: '仓库',
    providerName: '显示名称',
    kind: '类型',
    webBase: 'Web 地址',
    apiBase: 'API 地址',
    credentialLabel: 'Token 标签',
    github: 'GitHub',
    gitcode: 'GitCode',
    custom: '自定义',
    selectedPr: '当前 PR',
    noPr: '选择一个 PR 后开始审核。',
    author: '提出人',
    state: '状态',
    branch: '分支',
    created: '创建于',
    updated: '更新于',
    files: '变更文件',
    changedLines: '变更行',
    overview: '概览',
    ciDetails: 'CI 详情',
    existingReview: '已有讨论',
    ciFolded: 'CI 默认折叠，只有需要定位状态时再展开。',
    noCi: '代码平台没有返回 CI 状态。',
    noBody: '没有描述。',
    noFiles: '代码平台没有返回变更文件。',
    noReviews: '代码平台没有返回 Review 评论。',
    manualComment: '手写评论',
    manualCommentPlaceholder: '在这里写 PR 级评论，或粘贴你已经发现的问题。',
    addManualComment: '加入 Review',
    composer: 'Review 编辑器',
    composerHint: '生成、编辑、选择，再发布。未经确认不会提交到代码平台。',
    modeFast: '快速扫读',
    modeFocused: '重点审核',
    modeDeep: '深度审核',
    summaryComment: '总结评论',
    inlineComment: '行内评论',
    reviewDecision: 'Review 结论',
    decisionComment: '评论',
    decisionApprove: '通过',
    decisionRequestChanges: '要求修改',
    selectedOps: '已选',
    publishSelected: '发布选中项',
    publishConfirmTitle: '确认发布选中的 Review 内容？',
    publishConfirmBody: '这些评论会提交到代码平台。BitFun 无法替你撤回这个操作。',
    publishStaleTitle: 'PR head 已变化',
    publishStaleBody: '草稿基于旧 head 生成。请刷新，或明确确认要基于最新 head 继续发布。',
    staleConfirm: '我确认 PR head 已变化',
    publishNow: '立即发布',
    cancel: '取消',
    markReviewed: '标记当前 head 已审',
    audit: '发布审计',
    statusReady: '就绪',
    statusRefreshing: '正在同步队列...',
    statusAssignedNeedsToken: '同步待我审核需要当前代码平台的会话 Token。',
    statusNoSubscriptions: '请先添加监听仓库，或粘贴一个 PR 链接。',
    statusNoActiveSubscriptions: '已添加的监听仓库都处于暂停状态，请重新开启一个仓库或粘贴 PR 链接。',
    statusLoading: '正在加载 PR...',
    statusOpeningPr: '正在打开 PR...',
    statusGenerating: '正在生成审核草稿...',
    reviewProgress: '审核进展',
    reviewStageRead: '读取 PR 元信息和变更',
    reviewStageAi: '调用 AI 生成审核意见',
    reviewStageBuild: '整理为可编辑的 Review 项',
    reviewDetailOpeningPr: '正在获取 PR 元信息、变更文件、已有评论和状态。',
    reviewDetailRead: '正在阅读 PR 元信息和变更文件。',
    reviewDetailAi: 'AI 正在分析 diff 和已有讨论。',
    reviewDetailAiWait: '仍在分析中，大型 diff 可能需要稍等。',
    reviewDetailBuild: '正在整理可编辑的审核草稿。',
    cancelReview: '中止审核',
    reviewCancelled: '审核已中止',
    statusPublishing: '正在发布 Review...',
    statusSaved: '已保存',
    statusPublished: 'Review 已发布',
    statusReviewed: '已标记当前 head 已审',
    errorParse: '无法从这个链接识别 PR。',
    errorNetwork: '代码平台请求失败',
    newPrTitle: '新的可审核 PR',
    newHeadTitle: '已审 PR 有新提交',
    publicRead: '公开读取',
    privateAction: '私有与写入操作',
    draftStatus: '草稿',
    readyStatus: '可审',
    overviewHint: '展开查看完整描述。',
    noActionableFindings: '没有生成可操作问题。发布前可以添加手写评论，或编辑这条 Review 结论。',
    binary: '二进制',
    large: '过大',
    stale: '已过期',
    published: '已发布',
    skipped: '跳过',
    failed: '失败',
    success: '成功',
  },
  'zh-TW': {
    title: 'PR 審核台',
    subtitle: '監聽倉庫、開啟 PR、檢視變更、組織意見，並在二次確認後發布 Review。',
    queueModeAll: '倉庫全部 PR',
    queueModeMine: '待我審核',
    queueModeAllHint: '從已監聽倉庫同步開啟狀態的 PR。',
    queueModeMineHint: '需要工作階段 Token，因為程式碼平台要識別你的身分。',
    directUrl: '開啟 PR 連結',
    directPlaceholder: '貼上 GitHub、GitCode 或內網 PR 連結',
    repoRef: '倉庫',
    repoRefPlaceholder: 'owner/repo 或倉庫連結',
    openPr: '開啟 PR',
    openExternal: '在瀏覽器開啟',
    syncQueue: '同步佇列',
    syncMine: '同步待我審核',
    syncAllTitle: '倉庫全部 PR',
    syncAllDesc: '從已監聽倉庫拉取開啟狀態的 PR。',
    syncMineTitle: '待我審核',
    syncMineDesc: '使用你的平台身分查找需要你審核的 PR。',
    startReview: '開始 AI 審核',
    refreshNow: '立即重新整理',
    autoSync: '自動重新整理',
    every: '每',
    minutes: '分鐘',
    queue: '審核佇列',
    queueEmpty: '先新增監聽倉庫、同步待我審核，或在上方貼上 PR 連結。',
    sources: '來源與權限',
    watchedRepos: '監聽倉庫',
    noWatchedRepos: '還沒有監聽倉庫。',
    addWatchedRepo: '新增監聽倉庫',
    providers: '程式碼平台',
    addProvider: '新增平台',
    delete: '刪除',
    removeRepo: '移除',
    listenEnabled: '監聽中',
    listenDisabled: '已暫停',
    pauseListening: '暫停監聽',
    resumeListening: '重新開啟監聽',
    save: '儲存',
    provider: '程式碼平台',
    token: '工作階段 Token',
    tokenReady: 'Token 已填寫',
    tokenMissing: '未填寫 Token',
    tokenHelp: '公開 PR 可不填 Token 讀取。同步待我審核、私有倉庫和發布評論需要 Token。Token 只保存在目前記憶體中。',
    authorizeGitHubCli: '使用 GitHub CLI',
    authorizingGitHubCli: '正在讀取 GitHub CLI Token...',
    tokenFromGh: '已從 GitHub CLI 讀取 Token',
    authUnavailable: '無法讀取 GitHub CLI Token。請先執行 `gh auth login`，或手動貼上 Token。',
    authAutoHint: '需要時會從本機 gh 自動讀取 GitHub Token，僅保存在目前記憶體。',
    manualToken: '手動 Token',
    repoAddedSyncing: '已新增監聽倉庫，正在同步佇列...',
    advancedProviders: '進階平台設定',
    errorRepoNotFound: '倉庫不存在或目前無權存取，請檢查倉庫路徑或授權 GitHub CLI。',
    workspaceDiscovering: '正在從目前工作區識別倉庫...',
    workspaceDiscovered: '已從目前工作區新增 {count} 個倉庫。',
    workspaceDiscoveredMany: '已從目前工作區新增 {count} 個倉庫。',
    workspaceNone: '目前工作區沒有識別到 Git remote。',
    workspaceRepo: '工作區',
    rediscoverWorkspace: '識別工作區倉庫',
    repositoryFirst: '監聽倉庫',
    repositoryFirstHint: '先新增要關注的倉庫，再同步它的審核佇列。',
    singlePrFallback: '單獨檢視一個 PR',
    singlePrFallbackHint: '當這個倉庫暫時不需要監聽時使用。',
    owner: 'Owner',
    repo: '倉庫',
    providerName: '顯示名稱',
    kind: '類型',
    webBase: 'Web 位址',
    apiBase: 'API 位址',
    credentialLabel: 'Token 標籤',
    github: 'GitHub',
    gitcode: 'GitCode',
    custom: '自訂',
    selectedPr: '目前 PR',
    noPr: '選擇一個 PR 後開始審核。',
    author: '提出人',
    state: '狀態',
    branch: '分支',
    created: '建立於',
    updated: '更新於',
    files: '變更檔案',
    changedLines: '變更行',
    overview: '概覽',
    ciDetails: 'CI 詳情',
    existingReview: '既有討論',
    ciFolded: 'CI 預設摺疊，只有需要定位狀態時再展開。',
    noCi: '程式碼平台沒有回傳 CI 狀態。',
    noBody: '沒有描述。',
    noFiles: '程式碼平台沒有回傳變更檔案。',
    noReviews: '程式碼平台沒有回傳 Review 評論。',
    manualComment: '手寫評論',
    manualCommentPlaceholder: '在這裡寫 PR 級評論，或貼上你已經發現的問題。',
    addManualComment: '加入 Review',
    composer: 'Review 編輯器',
    composerHint: '產生、編輯、選擇，再發布。未經確認不會提交到程式碼平台。',
    modeFast: '快速掃讀',
    modeFocused: '重點審核',
    modeDeep: '深度審核',
    summaryComment: '總結評論',
    inlineComment: '行內評論',
    reviewDecision: 'Review 結論',
    decisionComment: '評論',
    decisionApprove: '通過',
    decisionRequestChanges: '要求修改',
    selectedOps: '已選',
    publishSelected: '發布選取項',
    publishConfirmTitle: '確認發布選取的 Review 內容？',
    publishConfirmBody: '這些評論會提交到程式碼平台。BitFun 無法替你撤回這個操作。',
    publishStaleTitle: 'PR head 已變更',
    publishStaleBody: '草稿基於舊 head 產生。請重新整理，或明確確認要基於最新 head 繼續發布。',
    staleConfirm: '我確認 PR head 已變更',
    publishNow: '立即發布',
    cancel: '取消',
    markReviewed: '標記目前 head 已審',
    audit: '發布審計',
    statusReady: '就緒',
    statusRefreshing: '正在同步佇列...',
    statusAssignedNeedsToken: '同步待我審核需要目前程式碼平台的工作階段 Token。',
    statusNoSubscriptions: '請先新增監聽倉庫，或貼上一個 PR 連結。',
    statusNoActiveSubscriptions: '已新增的監聽倉庫都處於暫停狀態，請重新開啟一個倉庫或貼上 PR 連結。',
    statusLoading: '正在載入 PR...',
    statusOpeningPr: '正在開啟 PR...',
    statusGenerating: '正在產生審核草稿...',
    reviewProgress: '審核進展',
    reviewStageRead: '讀取 PR 中繼資料和變更',
    reviewStageAi: '呼叫 AI 產生審核意見',
    reviewStageBuild: '整理為可編輯的 Review 項',
    reviewDetailOpeningPr: '正在取得 PR 中繼資料、變更檔案、既有評論和狀態。',
    reviewDetailRead: '正在閱讀 PR 中繼資料和變更檔案。',
    reviewDetailAi: 'AI 正在分析 diff 和既有討論。',
    reviewDetailAiWait: '仍在分析中，大型 diff 可能需要稍等。',
    reviewDetailBuild: '正在整理可編輯的審核草稿。',
    cancelReview: '中止審核',
    reviewCancelled: '審核已中止',
    statusPublishing: '正在發布 Review...',
    statusSaved: '已儲存',
    statusPublished: 'Review 已發布',
    statusReviewed: '已標記目前 head 已審',
    errorParse: '無法從這個連結識別 PR。',
    errorNetwork: '程式碼平台請求失敗',
    newPrTitle: '新的可審核 PR',
    newHeadTitle: '已審 PR 有新提交',
    publicRead: '公開讀取',
    privateAction: '私有與寫入操作',
    draftStatus: '草稿',
    readyStatus: '可審',
    overviewHint: '展開查看完整描述。',
    noActionableFindings: '沒有生成可操作問題。發布前可以新增手寫評論，或編輯這條 Review 結論。',
    binary: '二進位',
    large: '過大',
    stale: '已過期',
    published: '已發布',
    skipped: '略過',
    failed: '失敗',
    success: '成功',
  },
};

const DEFAULT_PROFILES = [
  {
    id: 'github',
    kind: 'github',
    displayName: 'GitHub',
    webBaseUrl: 'https://github.com',
    apiBaseUrl: 'https://api.github.com',
    credentialLabel: 'GitHub token',
    enabled: true,
  },
  {
    id: 'gitcode',
    kind: 'gitcode',
    displayName: 'GitCode',
    webBaseUrl: 'https://gitcode.com',
    apiBaseUrl: 'https://api.gitcode.com/api/v5',
    credentialLabel: 'GitCode token',
    enabled: true,
  },
];

const state = {
  locale: 'en-US',
  data: {
    profiles: DEFAULT_PROFILES,
    subscriptions: [],
    items: [],
    selectedKey: null,
    selectedFilePath: null,
    directUrl: '',
    mode: 'focused_review',
    queueMode: 'all',
    drafts: {},
    audit: [],
    lastReviewedHeads: {},
    notifiedKeys: [],
    dismissedWorkspaceRepos: [],
    workspaceAutoListenDoneFor: '',
    notifyNewItems: true,
    pollMinutes: DEFAULT_POLL_MINUTES,
  },
  ui: {
    busy: null,
    status: null,
    error: null,
    reviewProgress: null,
    cancelReviewRequested: false,
    activeProviderId: 'github',
    confirm: null,
  },
  volatile: {
    sessionTokens: {},
    pollTimer: null,
  },
};

const root = document.getElementById('app');

function readReviewWorkspaceScroll() {
  const workspace = document.querySelector('.pr-review-workspace');
  if (!(workspace instanceof HTMLElement)) return null;
  return {
    top: workspace.scrollTop,
    left: workspace.scrollLeft,
  };
}

function restoreReviewWorkspaceScroll(position) {
  if (!position) return;
  window.requestAnimationFrame(() => {
    const workspace = document.querySelector('.pr-review-workspace');
    if (!(workspace instanceof HTMLElement)) return;
    workspace.scrollTop = position.top;
    workspace.scrollLeft = position.left;
  });
}

function t(key, params = {}) {
  const table = I18N[state.locale] || I18N['en-US'];
  const fallback = I18N['en-US'][key] || key;
  return String(table[key] || fallback).replace(/\{(\w+)\}/g, (_, name) => params[name] ?? '');
}

function esc(value) {
  return String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function renderHighlightedDiff(patch, targetPosition = null) {
  const text = String(patch || '');
  if (!text) return '';
  let position = 0;
  const requestedPosition = Number(targetPosition || 0);
  return text.split('\n').map((line) => {
    let kind = 'context';
    if (line.startsWith('@@')) kind = 'hunk';
    else if (line.startsWith('diff --git') || line.startsWith('index ')) kind = 'meta';
    else if (line.startsWith('+') && !line.startsWith('+++')) kind = 'add';
    else if (line.startsWith('-') && !line.startsWith('---')) kind = 'remove';
    const isPositioned = kind !== 'hunk';
    const linePosition = isPositioned ? ++position : 0;
    const isTarget = requestedPosition > 0 && linePosition === requestedPosition;
    const attrs = linePosition ? ` data-position="${linePosition}"` : '';
    return `<span class="pr-diff-line pr-diff-line--${kind}${isTarget ? ' is-target' : ''}"${attrs}>${esc(line || ' ')}</span>`;
  }).join('');
}

function normalizeBaseUrl(value) {
  return String(value || '').trim().replace(/\/+$/, '');
}

function profileById(id) {
  return state.data.profiles.find((profile) => profile.id === id) || state.data.profiles[0];
}

function activeProfile() {
  return profileById(state.ui.activeProviderId);
}

function hasToken(profile) {
  return Boolean(profile && state.volatile.sessionTokens[profile.id]);
}

function setReviewProgress(stageKey, detail = '', progressPct = 8) {
  state.ui.reviewProgress = {
    stage: t(stageKey),
    detail,
    progressPct: Math.max(0, Math.min(100, Number(progressPct) || 0)),
    cancelled: state.ui.cancelReviewRequested,
  };
  render();
}

function modeLabel(mode) {
  if (mode === 'deep_review') return t('modeDeep');
  if (mode === 'fast_check') return t('modeFast');
  return t('modeFocused');
}

async function withReviewProgressTicker(stageKey, details, startPct, endPct, task) {
  const detailList = details.filter(Boolean);
  let tick = 0;
  setReviewProgress(stageKey, detailList[0] || '', startPct);
  const timer = window.setInterval(() => {
    tick += 1;
    const pct = Math.min(endPct - 4, startPct + tick * 6);
    const detail = detailList[Math.min(tick, detailList.length - 1)] || detailList[0] || '';
    setReviewProgress(stageKey, detail, pct);
  }, 4800);
  try {
    return await task();
  } finally {
    window.clearInterval(timer);
  }
}

async function readGitHubCliToken(profile) {
  if (!profile || profile.kind !== 'github') return false;
  const result = await app.shell.exec(['gh', 'auth', 'token'], { timeout: 8000 });
  const token = String(result?.stdout || '').trim();
  if (!token) throw new Error('empty gh token');
  state.volatile.sessionTokens[profile.id] = token;
  return true;
}

async function ensureProfileToken(profile) {
  if (hasToken(profile)) return true;
  if (!profile || profile.kind !== 'github') return false;
  state.ui.status = t('authorizingGitHubCli');
  state.ui.error = null;
  render();
  try {
    await readGitHubCliToken(profile);
    state.ui.status = t('tokenFromGh');
    render();
    return true;
  } catch {
    return false;
  }
}

async function authorizeGitHubCli(profile = activeProfile()) {
  if (!profile || profile.kind !== 'github') {
    setError(t('authUnavailable'));
    return;
  }
  setBusy('auth', 'authorizingGitHubCli');
  try {
    await readGitHubCliToken(profile);
    await finish('tokenFromGh');
  } catch {
    setError(t('authUnavailable'));
  }
}

function snapshotKey(snapshot) {
  if (!snapshot) return '';
  const id = snapshot.identity;
  return `${id.providerId}:${id.owner}/${id.repo}#${id.number}`;
}

function itemKey(item) {
  return snapshotKey(item);
}

function providerHeaders(profile, jsonBody = false) {
  const headers = {
    Accept: 'application/json',
    'User-Agent': 'BitFun-PR-Review-MiniApp',
  };
  const token = state.volatile.sessionTokens[profile.id];
  if (token) headers.Authorization = `Bearer ${token}`;
  if (profile.kind === 'github') {
    headers.Accept = 'application/vnd.github+json';
    headers['X-GitHub-Api-Version'] = '2022-11-28';
  }
  if (jsonBody) headers['Content-Type'] = 'application/json';
  return headers;
}

async function netJson(url, options = {}) {
  const response = await app.net.fetch(url, {
    method: options.method || 'GET',
    headers: options.headers || {},
    body: options.body,
  });
  const status = Number(response.status || 0);
  const text = response.body || '';
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = text;
  }
  if (status < 200 || status >= 300) {
    const message = typeof body === 'object' && body
      ? (body.message || body.error || JSON.stringify(body).slice(0, 240))
      : String(body || `${status}`);
    const error = new Error(`${t('errorNetwork')}: ${message}`);
    error.status = status;
    error.body = body;
    throw error;
  }
  return body;
}

async function requestWithAuthRetry(profile, runRequest) {
  try {
    return await runRequest();
  } catch (error) {
    const canRetryWithGh = profile?.kind === 'github'
      && !hasToken(profile)
      && [401, 403, 404].includes(Number(error?.status || 0));
    if (!canRetryWithGh) throw error;
    const tokenReady = await ensureProfileToken(profile);
    if (!tokenReady) throw error;
    return runRequest();
  }
}

async function loadStorage() {
  try {
    const saved = await app.storage.get(STORAGE_KEY);
    if (!saved) return;
    const parsed = typeof saved === 'string' ? JSON.parse(saved) : saved;
    const profiles = Array.isArray(parsed.profiles) && parsed.profiles.length ? parsed.profiles : DEFAULT_PROFILES;
    const subscriptions = Array.isArray(parsed.subscriptions)
      ? parsed.subscriptions
        .map((subscription) => normalizeSubscription(
          subscription,
          profiles.find((profile) => profile.id === subscription?.providerId) || profiles[0]
        ))
        .filter(Boolean)
      : [];
    state.data = {
      ...state.data,
      ...parsed,
      profiles,
      subscriptions,
      items: Array.isArray(parsed.items) ? parsed.items : [],
      drafts: parsed.drafts && typeof parsed.drafts === 'object' ? parsed.drafts : {},
      audit: Array.isArray(parsed.audit) ? parsed.audit : [],
      lastReviewedHeads: parsed.lastReviewedHeads || {},
      notifiedKeys: Array.isArray(parsed.notifiedKeys) ? parsed.notifiedKeys : [],
      dismissedWorkspaceRepos: Array.isArray(parsed.dismissedWorkspaceRepos) ? parsed.dismissedWorkspaceRepos : [],
      workspaceAutoListenDoneFor: parsed.workspaceAutoListenDoneFor || '',
      queueMode: parsed.queueMode || 'all',
    };
    state.ui.activeProviderId = state.data.profiles[0]?.id || 'github';
  } catch (error) {
    state.ui.error = String(error?.message || error);
  }
}

async function saveStorage() {
  await app.storage.set(STORAGE_KEY, persistableState());
}

function persistableState() {
  const cloned = { ...state.data };
  delete cloned.sessionTokens;
  delete cloned.sessionToken;
  delete cloned.token;
  delete cloned.accessToken;
  delete cloned.refreshToken;
  delete cloned.authorization;
  delete cloned.password;
  delete cloned.secret;
  return cloned;
}

function setBusy(key, statusKey) {
  state.ui.busy = key;
  state.ui.status = statusKey ? t(statusKey) : null;
  state.ui.error = null;
  if (key !== 'draft') {
    state.ui.reviewProgress = null;
    state.ui.cancelReviewRequested = false;
  }
  render();
}

async function finish(statusKey) {
  state.ui.busy = null;
  state.ui.status = statusKey ? t(statusKey) : null;
  state.ui.reviewProgress = null;
  state.ui.cancelReviewRequested = false;
  await saveStorage();
  render();
}

function setError(error) {
  state.ui.busy = null;
  state.ui.error = typeof error === 'string' ? error : String(error?.message || error);
  state.ui.reviewProgress = null;
  state.ui.cancelReviewRequested = false;
  render();
}

function parsePrUrl(rawUrl) {
  let url;
  try {
    url = new URL(rawUrl);
  } catch {
    return null;
  }
  const path = url.pathname.split('/').filter(Boolean);
  const profiles = [...state.data.profiles].sort((a, b) => normalizeBaseUrl(b.webBaseUrl).length - normalizeBaseUrl(a.webBaseUrl).length);
  const profile = profiles.find((item) => {
    try {
      return new URL(normalizeBaseUrl(item.webBaseUrl)).host === url.host;
    } catch {
      return false;
    }
  });
  if (!profile || path.length < 4) return null;

  const pullIndex = path.findIndex((part) => ['pull', 'pulls', 'merge_requests'].includes(part));
  if (pullIndex < 2 || !path[pullIndex + 1]) return null;
  const number = Number(path[pullIndex + 1]);
  if (!Number.isFinite(number)) return null;
  return {
    providerId: profile.id,
    providerKind: profile.kind,
    owner: path[0],
    repo: path.slice(1, pullIndex).join('/'),
    number,
    url: rawUrl,
  };
}

function parseRepositoryRef(rawValue, provider = activeProfile()) {
  const value = String(rawValue || '').trim();
  if (!value) return null;
  try {
    const url = new URL(value);
    const matchedProfile = state.data.profiles.find((item) => {
      try {
        return new URL(normalizeBaseUrl(item.webBaseUrl)).host === url.host;
      } catch {
        return false;
      }
    }) || provider;
    const path = url.pathname.split('/').filter(Boolean);
    const pullIndex = path.findIndex((part) => ['pull', 'pulls', 'merge_requests'].includes(part));
    const repoPath = pullIndex > 0 ? path.slice(0, pullIndex) : path;
    if (repoPath.length >= 2) {
      return {
        providerId: matchedProfile?.id || activeProfile()?.id || 'github',
        owner: repoPath[0],
        repo: repoPath.slice(1).join('/').replace(/\.git$/, ''),
      };
    }
  } catch {
    // Fall through to owner/repo parsing.
  }

  const sshMatch = value.match(/^git@[^:]+:(.+)$/);
  const pathValue = (sshMatch ? sshMatch[1] : value)
    .replace(/^\/+|\/+$/g, '')
    .replace(/\.git$/, '');
  const parts = pathValue.split('/').filter(Boolean);
  if (parts.length < 2) return null;
  return {
    providerId: provider?.id || activeProfile()?.id || 'github',
    owner: parts[0],
    repo: parts.slice(1).join('/'),
  };
}

function looksLikeUrlOrSshRef(value) {
  const text = String(value || '').trim();
  return /^[a-z][a-z0-9+.-]*:\/\//i.test(text) || /^[\w.-]+@[^:]+:/.test(text);
}

function cleanRepositoryPart(value) {
  return String(value || '')
    .trim()
    .replace(/^\/+|\/+$/g, '')
    .replace(/\.git$/i, '');
}

function normalizeRepositoryParts(raw, fallbackProfile = activeProfile()) {
  const providerId = String(raw?.providerId || fallbackProfile?.id || activeProfile()?.id || 'github');
  const profile = profileById(providerId) || fallbackProfile || activeProfile();
  const ownerValue = String(raw?.owner || '').trim();
  const repoValue = String(raw?.repo || raw?.repoRef || '').trim();
  const directValue = String(raw?.url || '').trim();
  const candidates = [];

  if (looksLikeUrlOrSshRef(repoValue)) candidates.push(repoValue);
  if (looksLikeUrlOrSshRef(ownerValue)) candidates.push(ownerValue);
  if (looksLikeUrlOrSshRef(directValue)) candidates.push(directValue);
  if (!ownerValue && repoValue.includes('/')) candidates.push(repoValue);

  for (const candidate of candidates) {
    const parsed = parseRepositoryRef(candidate, profile);
    if (parsed?.owner && parsed?.repo) return parsed;
  }

  const owner = cleanRepositoryPart(ownerValue);
  const repo = cleanRepositoryPart(repoValue);
  if (!owner || !repo || looksLikeUrlOrSshRef(owner) || looksLikeUrlOrSshRef(repo)) return null;
  return {
    providerId,
    owner,
    repo,
  };
}

function normalizeSubscription(raw, fallbackProfile = activeProfile()) {
  const identity = normalizeRepositoryParts(raw, fallbackProfile);
  if (!identity) return null;
  return {
    ...raw,
    providerId: identity.providerId,
    owner: identity.owner,
    repo: identity.repo,
    enabled: raw?.enabled !== false,
    notify: raw?.notify !== false,
  };
}

function subscriptionKey(subscription) {
  return `${subscription.providerId}:${subscription.owner}/${subscription.repo}`.toLowerCase();
}

function activeSubscriptions() {
  return state.data.subscriptions.filter((subscription) => subscription.enabled !== false);
}

function providerForRemoteUrl(remoteUrl) {
  const normalized = String(remoteUrl || '');
  return [...state.data.profiles]
    .sort((a, b) => normalizeBaseUrl(b.webBaseUrl).length - normalizeBaseUrl(a.webBaseUrl).length)
    .find((profile) => {
      try {
        const host = new URL(normalizeBaseUrl(profile.webBaseUrl)).host.toLowerCase();
        return normalized.toLowerCase().includes(host);
      } catch {
        return false;
      }
    }) || activeProfile();
}

function repositoryFromRemoteUrl(remoteUrl) {
  const value = String(remoteUrl || '').trim();
  if (!value) return null;
  const profile = providerForRemoteUrl(value);
  let pathValue = '';
  try {
    const url = new URL(value);
    pathValue = url.pathname;
  } catch {
    const sshMatch = value.match(/^[\w.-]+@[^:]+:(.+)$/);
    if (sshMatch) pathValue = sshMatch[1];
  }
  if (!pathValue) return null;
  const parts = pathValue
    .replace(/^\/+|\/+$/g, '')
    .replace(/\.git$/, '')
    .split('/')
    .filter(Boolean);
  if (parts.length < 2) return null;
  return {
    providerId: profile?.id || 'github',
    owner: parts[0],
    repo: parts.slice(1).join('/'),
  };
}

function joinPath(base, name) {
  const separator = String(base).includes('\\') ? '\\' : '/';
  return `${String(base).replace(/[\\/]+$/, '')}${separator}${name}`;
}

async function collectWorkspaceGitRoots(rootDir) {
  if (!rootDir) return [];
  const roots = [];
  const queue = [{ path: rootDir, depth: 0 }];
  let scanned = 0;
  const seen = new Set();

  while (queue.length && scanned < MAX_WORKSPACE_SCAN_DIRS) {
    const current = queue.shift();
    if (!current?.path || seen.has(current.path)) continue;
    seen.add(current.path);
    scanned += 1;

    let entries = [];
    try {
      entries = await app.fs.readdir(current.path);
    } catch {
      continue;
    }
    if ((entries || []).some((entry) => entry?.name === '.git')) {
      roots.push(current.path);
      continue;
    }
    if (current.depth >= MAX_WORKSPACE_SCAN_DEPTH) continue;

    for (const entry of entries || []) {
      if (!entry?.isDirectory || SKIP_WORKSPACE_DIRS.has(entry.name)) continue;
      queue.push({ path: entry.path || joinPath(current.path, entry.name), depth: current.depth + 1 });
    }
  }
  return roots;
}

async function gitRemoteUrlForDir(dir) {
  try {
    const result = await app.shell.exec(['git', 'remote', 'get-url', 'origin'], {
      cwd: dir,
      timeout: 5000,
    });
    return String(result?.stdout || '').trim();
  } catch {
    return '';
  }
}

async function discoverWorkspaceRepositories() {
  const workspaceDir = app.workspaceDir;
  if (!workspaceDir) return [];
  const gitRoots = await collectWorkspaceGitRoots(workspaceDir);
  const discovered = [];
  for (const dir of gitRoots) {
    const remoteUrl = await gitRemoteUrlForDir(dir);
    const repo = repositoryFromRemoteUrl(remoteUrl);
    if (!repo) continue;
    discovered.push({
      ...repo,
      path: dir,
      remoteUrl,
      pollIntervalMinutes: state.data.pollMinutes,
      notify: true,
      enabled: true,
      source: 'workspace',
    });
  }
  const unique = new Map(discovered.map((item) => [subscriptionKey(item), item]));
  return Array.from(unique.values());
}

async function applyWorkspaceDiscoveredRepositories({ force = false, sync = true } = {}) {
  const workspaceDir = app.workspaceDir || '';
  if (!workspaceDir) return;

  state.ui.status = t('workspaceDiscovering');
  state.ui.error = null;
  render();

  try {
    const ignored = new Set(state.data.dismissedWorkspaceRepos || []);
    const existing = new Set(state.data.subscriptions.map(subscriptionKey));
    const nextRepos = (await discoverWorkspaceRepositories())
      .filter((repo) => !existing.has(subscriptionKey(repo)))
      .filter((repo) => !ignored.has(subscriptionKey(repo)));
    if (nextRepos.length) {
      state.data.subscriptions.push(...nextRepos);
      state.data.queueMode = 'all';
      state.ui.activeProviderId = nextRepos[0].providerId;
      state.ui.status = t(nextRepos.length === 1 ? 'workspaceDiscovered' : 'workspaceDiscoveredMany', { count: nextRepos.length });
    } else if (force) {
      state.ui.status = t('workspaceNone');
    } else {
      state.ui.status = null;
    }
    state.data.workspaceAutoListenDoneFor = workspaceDir;
    await saveStorage();
    render();
    if (sync && nextRepos.length) void syncQueue('all');
  } catch (error) {
    state.ui.status = null;
    state.ui.error = String(error?.message || error);
    render();
  }
}

async function refreshQueueOnOpen() {
  await applyWorkspaceDiscoveredRepositories({ sync: false });
  if (activeSubscriptions().length) {
    void syncQueue('all');
  }
}

function normalizeFile(file) {
  const path = file.filename || file.path || file.new_path || file.name || '';
  const patch = file.patch || file.diff || file.content || file.changes || '';
  return {
    path,
    oldPath: file.previous_filename || file.old_path || null,
    status: file.status || file.change_type || 'modified',
    additions: Number(file.additions || file.added_lines || 0),
    deletions: Number(file.deletions || file.removed_lines || 0),
    patch: typeof patch === 'string' ? patch : '',
    isBinary: Boolean(file.binary || file.is_binary),
    isTooLarge: Boolean(file.too_large || file.is_too_large),
  };
}

function normalizeReview(review, kind = 'review') {
  return {
    id: String(review.id || review.node_id || review.noteable_id || `${kind}-${Math.random()}`),
    kind,
    author: review.user?.login || review.author?.username || review.author?.name || review.user?.name || review.author || '',
    state: review.state || review.status || null,
    body: review.body || review.note || review.comment || '',
    path: review.path || null,
    position: review.position || review.line || null,
    submittedAt: review.submitted_at || review.created_at || review.updated_at || null,
    url: review.html_url || review.url || null,
  };
}

function summarizeReviews(reviews) {
  return {
    approvals: reviews.filter((review) => String(review.state || '').toLowerCase() === 'approved').length,
    changesRequested: reviews.filter((review) => String(review.state || '').toLowerCase() === 'changes_requested').length,
    comments: reviews.length,
    unresolvedThreads: reviews.filter((review) => review.resolved === false).length,
  };
}

function normalizeChecks(statusBody, checksBody) {
  const statusChecks = Array.isArray(statusBody?.statuses) ? statusBody.statuses.map((status) => ({
    name: status.context || status.name || 'status',
    status: status.state || 'completed',
    conclusion: status.state || null,
    url: status.target_url || status.html_url || null,
  })) : [];
  const checkRuns = Array.isArray(checksBody?.check_runs) ? checksBody.check_runs.map((run) => ({
    name: run.name || 'check',
    status: run.status || 'completed',
    conclusion: run.conclusion || null,
    url: run.html_url || null,
  })) : [];
  return [...statusChecks, ...checkRuns];
}

async function fetchGithubSnapshot(identity) {
  const profile = profileById(identity.providerId);
  const base = normalizeBaseUrl(profile.apiBaseUrl);
  const ownerRepo = `${encodeURIComponent(identity.owner)}/${encodeURIComponent(identity.repo)}`;
  const pr = await requestWithAuthRetry(profile, () => netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}`, {
    headers: providerHeaders(profile),
  }));
  const headers = providerHeaders(profile);
  const [filesResult, reviewsResult, commentsResult, statusResult, checksResult] = await Promise.allSettled([
    netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}/files?per_page=100`, { headers }),
    netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}/reviews?per_page=100`, { headers }),
    netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}/comments?per_page=100`, { headers }),
    netJson(`${base}/repos/${ownerRepo}/commits/${pr.head?.sha}/status`, { headers }),
    netJson(`${base}/repos/${ownerRepo}/commits/${pr.head?.sha}/check-runs`, { headers }),
  ]);
  const files = filesResult.status === 'fulfilled' && Array.isArray(filesResult.value)
    ? filesResult.value.map(normalizeFile)
    : [];
  const reviews = [
    ...(reviewsResult.status === 'fulfilled' && Array.isArray(reviewsResult.value) ? reviewsResult.value.map((item) => normalizeReview(item, 'review')) : []),
    ...(commentsResult.status === 'fulfilled' && Array.isArray(commentsResult.value) ? commentsResult.value.map((item) => normalizeReview(item, 'inline')) : []),
  ];
  const checks = normalizeChecks(
    statusResult.status === 'fulfilled' ? statusResult.value : null,
    checksResult.status === 'fulfilled' ? checksResult.value : null,
  );
  return {
    identity: {
      providerId: profile.id,
      providerKind: profile.kind,
      owner: identity.owner,
      repo: identity.repo,
      number: identity.number,
    },
    url: pr.html_url || identity.url || `${normalizeBaseUrl(profile.webBaseUrl)}/${identity.owner}/${identity.repo}/pull/${identity.number}`,
    title: pr.title || `#${identity.number}`,
    body: pr.body || '',
    author: pr.user?.login || '',
    state: pr.state || '',
    isDraft: Boolean(pr.draft),
    baseBranch: pr.base?.ref || '',
    headBranch: pr.head?.ref || '',
    headSha: pr.head?.sha || '',
    createdAt: pr.created_at || '',
    updatedAt: pr.updated_at || '',
    files,
    checks,
    reviews,
    reviewSummary: summarizeReviews(reviews),
    providerCapabilities: {
      publishSummaryComment: true,
      publishInlineComment: true,
      publishReviewDecision: true,
    },
  };
}

async function fetchCompatibleSnapshot(identity) {
  const profile = profileById(identity.providerId);
  const base = normalizeBaseUrl(profile.apiBaseUrl);
  const ownerRepo = `${encodeURIComponent(identity.owner)}/${encodeURIComponent(identity.repo)}`;
  let pr;
  try {
    pr = await requestWithAuthRetry(profile, () => netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}`, {
      headers: providerHeaders(profile),
    }));
  } catch {
    pr = await requestWithAuthRetry(profile, () => netJson(`${base}/projects/${encodeURIComponent(`${identity.owner}/${identity.repo}`)}/merge_requests/${identity.number}`, {
      headers: providerHeaders(profile),
    }));
  }

  const headers = providerHeaders(profile);
  const [filesResult, reviewsResult] = await Promise.allSettled([
    netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}/files`, { headers }).catch(() =>
      netJson(`${base}/projects/${encodeURIComponent(`${identity.owner}/${identity.repo}`)}/merge_requests/${identity.number}/changes`, { headers })
    ),
    netJson(`${base}/repos/${ownerRepo}/pulls/${identity.number}/reviews`, { headers }).catch(() =>
      netJson(`${base}/projects/${encodeURIComponent(`${identity.owner}/${identity.repo}`)}/merge_requests/${identity.number}/notes`, { headers })
    ),
  ]);

  const rawFiles = filesResult.status === 'fulfilled'
    ? (Array.isArray(filesResult.value) ? filesResult.value : filesResult.value?.changes || filesResult.value?.files || [])
    : [];
  const rawReviews = reviewsResult.status === 'fulfilled'
    ? (Array.isArray(reviewsResult.value) ? reviewsResult.value : reviewsResult.value?.reviews || reviewsResult.value?.notes || [])
    : [];

  const files = rawFiles.map(normalizeFile);
  const reviews = rawReviews.map((item) => normalizeReview(item));
  const headSha = pr.head?.sha || pr.head_sha || pr.sha || pr.diff_refs?.head_sha || '';
  return {
    identity: {
      providerId: profile.id,
      providerKind: profile.kind,
      owner: identity.owner,
      repo: identity.repo,
      number: identity.number,
    },
    url: pr.html_url || pr.web_url || identity.url || `${normalizeBaseUrl(profile.webBaseUrl)}/${identity.owner}/${identity.repo}/pull/${identity.number}`,
    title: pr.title || `#${identity.number}`,
    body: pr.body || pr.description || '',
    author: pr.user?.login || pr.author?.username || pr.author?.name || '',
    state: pr.state || pr.status || '',
    isDraft: Boolean(pr.draft || pr.work_in_progress),
    baseBranch: pr.base?.ref || pr.target_branch || '',
    headBranch: pr.head?.ref || pr.source_branch || '',
    headSha,
    createdAt: pr.created_at || '',
    updatedAt: pr.updated_at || '',
    files,
    checks: [],
    reviews,
    reviewSummary: summarizeReviews(reviews),
    providerCapabilities: {
      publishSummaryComment: true,
      publishInlineComment: true,
      publishReviewDecision: false,
    },
  };
}

async function fetchSnapshot(identity) {
  const profile = profileById(identity.providerId);
  return profile.kind === 'github'
    ? fetchGithubSnapshot(identity)
    : fetchCompatibleSnapshot(identity);
}

async function listRepositoryPullRequests(subscription) {
  const profile = profileById(subscription.providerId);
  subscription = normalizeSubscription(subscription, profile) || subscription;
  const base = normalizeBaseUrl(profile.apiBaseUrl);
  const ownerRepo = `${encodeURIComponent(subscription.owner)}/${encodeURIComponent(subscription.repo)}`;
  let raw = [];
  if (profile.kind === 'github') {
    try {
      raw = await requestWithAuthRetry(profile, () => netJson(`${base}/repos/${ownerRepo}/pulls?state=open&per_page=20`, {
        headers: providerHeaders(profile),
      }));
    } catch (error) {
      if (Number(error?.status || 0) === 404) {
        throw new Error(`${t('errorRepoNotFound')} (${subscription.owner}/${subscription.repo})`);
      }
      throw error;
    }
    return (Array.isArray(raw) ? raw : raw?.values || raw?.items || []).slice(0, 20).map((row) => ({
      providerId: profile.id,
      providerKind: profile.kind,
      owner: subscription.owner,
      repo: subscription.repo,
      number: Number(row.number || row.iid || row.id),
      url: row.html_url || row.web_url || '',
    })).filter((row) => Number.isFinite(row.number));
  }
  try {
    raw = await requestWithAuthRetry(profile, () => netJson(`${base}/repos/${ownerRepo}/pulls?state=open&per_page=20`, {
      headers: providerHeaders(profile),
    }));
  } catch {
    raw = await requestWithAuthRetry(profile, () => netJson(`${base}/projects/${encodeURIComponent(`${subscription.owner}/${subscription.repo}`)}/merge_requests?state=opened&per_page=20`, {
      headers: providerHeaders(profile),
    }));
  }
  const rows = Array.isArray(raw) ? raw : raw?.values || raw?.items || [];
  return rows.slice(0, 20).map((row) => ({
    providerId: profile.id,
    providerKind: profile.kind,
    owner: subscription.owner,
    repo: subscription.repo,
    number: Number(row.number || row.iid || row.id),
    url: row.html_url || row.web_url || '',
  })).filter((row) => Number.isFinite(row.number));
}

async function listReviewRequested(profile) {
  if (!await ensureProfileToken(profile)) return [];
  if (profile.kind === 'github') {
    const base = normalizeBaseUrl(profile.apiBaseUrl);
    const headers = providerHeaders(profile);
    const query = encodeURIComponent('is:pr is:open review-requested:@me archived:false');
    const result = await netJson(`${base}/search/issues?q=${query}&per_page=20`, { headers });
    return (result.items || []).map((item) => parsePrUrl(item.html_url)).filter(Boolean);
  }
  try {
    const result = await netJson(`${normalizeBaseUrl(profile.apiBaseUrl)}/user/review_requests`, {
      headers: providerHeaders(profile),
    });
    return (Array.isArray(result) ? result : result?.items || []).map((item) => parsePrUrl(item.html_url || item.web_url || item.url)).filter(Boolean);
  } catch {
    return [];
  }
}

function mergeItems(nextItems) {
  const previous = new Map(state.data.items.map((item) => [itemKey(item), item]));
  const merged = new Map();
  for (const item of nextItems) {
    merged.set(itemKey(item), { ...previous.get(itemKey(item)), ...item, stale: false });
  }
  for (const item of state.data.items) {
    if (!merged.has(itemKey(item))) merged.set(itemKey(item), { ...item, stale: true });
  }
  state.data.items = Array.from(merged.values()).sort((a, b) => String(b.updatedAt || '').localeCompare(String(a.updatedAt || '')));
  if (!state.data.selectedKey && state.data.items[0]) {
    state.data.selectedKey = itemKey(state.data.items[0]);
    state.data.selectedFilePath = state.data.items[0].files?.[0]?.path || null;
  }
}

async function notifyNewWork(previousItems, nextItems) {
  if (!state.data.notifyNewItems || !app.notifications?.system) return;
  const notified = new Set(state.data.notifiedKeys || []);
  const previousByKey = new Map(previousItems.map((item) => [itemKey(item), item]));
  const notifications = [];
  for (const item of nextItems) {
    const key = itemKey(item);
    const previous = previousByKey.get(key);
    const headKey = `${key}:${item.headSha || 'unknown'}`;
    if (!previous && !notified.has(`new:${headKey}`)) {
      notifications.push({
        key: `new:${headKey}`,
        title: t('newPrTitle'),
        body: `${item.identity.owner}/${item.identity.repo}#${item.identity.number} ${item.title || ''}`,
      });
      continue;
    }
    const reviewedHead = state.data.lastReviewedHeads[key];
    if (previous && reviewedHead && previous.headSha === reviewedHead && item.headSha && item.headSha !== reviewedHead) {
      const notifyKey = `head:${headKey}`;
      if (!notified.has(notifyKey)) {
        notifications.push({
          key: notifyKey,
          title: t('newHeadTitle'),
          body: `${item.identity.owner}/${item.identity.repo}#${item.identity.number} ${item.title || ''}`,
        });
      }
    }
  }
  for (const notice of notifications.slice(0, 4)) {
    try {
      await app.notifications.system(notice.title, notice.body);
      notified.add(notice.key);
    } catch {
      break;
    }
  }
  state.data.notifiedKeys = Array.from(notified).slice(-200);
}

async function syncQueue(mode = state.data.queueMode) {
  state.data.queueMode = mode;
  const profile = activeProfile();
  const subscriptions = activeSubscriptions();
  if (mode === 'mine' && !await ensureProfileToken(profile)) {
    state.ui.error = t('statusAssignedNeedsToken');
    state.ui.status = null;
    render();
    return;
  }
  if (mode === 'all' && subscriptions.length === 0) {
    state.ui.error = state.data.subscriptions.length ? t('statusNoActiveSubscriptions') : t('statusNoSubscriptions');
    state.ui.status = null;
    render();
    return;
  }

  setBusy('refresh', 'statusRefreshing');
  try {
    const previousItems = [...state.data.items];
    const identities = [];
    if (mode === 'all') {
      for (const subscription of subscriptions) {
        try {
          identities.push(...await listRepositoryPullRequests(subscription));
        } catch (error) {
          state.ui.error = String(error?.message || error);
        }
      }
    } else {
      for (const item of state.data.profiles.filter((profileItem) => profileItem.enabled && hasToken(profileItem))) {
        try {
          identities.push(...await listReviewRequested(item));
        } catch (error) {
          state.ui.error = String(error?.message || error);
        }
      }
    }
    const unique = new Map(identities.map((identity) => [`${identity.providerId}:${identity.owner}/${identity.repo}#${identity.number}`, identity]));
    const snapshots = [];
    for (const identity of Array.from(unique.values()).slice(0, 30)) {
      try {
        snapshots.push(await fetchSnapshot(identity));
      } catch (error) {
        state.ui.error = String(error?.message || error);
      }
    }
    mergeItems(snapshots);
    await notifyNewWork(previousItems, snapshots);
    await finish('statusReady');
    resetPollTimer();
  } catch (error) {
    setError(error);
  }
}

async function openDirectUrl() {
  const input = document.getElementById('direct-url');
  const url = input?.value?.trim() || state.data.directUrl;
  state.data.directUrl = url;
  const identity = parsePrUrl(url);
  if (!identity) {
    setError(t('errorParse'));
    return;
  }
  setBusy('direct', 'statusOpeningPr');
  try {
    const snapshot = await fetchSnapshot(identity);
    const byKey = new Map(state.data.items.map((item) => [itemKey(item), item]));
    byKey.set(itemKey(snapshot), snapshot);
    state.data.items = Array.from(byKey.values());
    state.data.selectedKey = itemKey(snapshot);
    state.data.selectedFilePath = snapshot.files?.[0]?.path || null;
    state.ui.activeProviderId = identity.providerId;
    await finish('statusReady');
  } catch (error) {
    setError(error);
  }
}

function selectedSnapshot() {
  return state.data.items.find((item) => itemKey(item) === state.data.selectedKey) || null;
}

function selectedDraft() {
  const snapshot = selectedSnapshot();
  return snapshot ? state.data.drafts[snapshotKey(snapshot)] || null : null;
}

function recommendMode(snapshot) {
  const lines = snapshot.files.reduce((sum, file) => sum + file.additions + file.deletions, 0);
  const security = snapshot.files.some((file) => /auth|permission|crypto|secret|token|security/i.test(file.path));
  const failedCi = snapshot.checks.some((check) => ['failure', 'failed', 'error', 'timed_out'].includes(String(check.conclusion || check.status).toLowerCase()));
  if (security || snapshot.files.length > 30 || lines > 1200) return 'deep_review';
  if (failedCi || snapshot.files.length > 8 || lines > 300) return 'focused_review';
  return 'fast_check';
}

function localSummary(snapshot) {
  const lines = snapshot.files.reduce((sum, file) => sum + file.additions + file.deletions, 0);
  const topFiles = snapshot.files.slice(0, 8).map((file) => `- ${file.path} (+${file.additions}/-${file.deletions})`).join('\n');
  return [
    `Review draft for "${snapshot.title}".`,
    '',
    `Changed files: ${snapshot.files.length}. Changed lines: ${lines}.`,
    snapshot.checks.length ? `CI: ${snapshot.checks.map((check) => `${check.name}:${check.conclusion || check.status}`).join(', ')}` : 'CI: no status returned.',
    '',
    'Suggested focus:',
    topFiles || '- No reviewable files returned by the provider.',
    '',
    'Please edit before publishing.',
  ].join('\n');
}

function reviewPrompt(snapshot, mode) {
  const files = snapshot.files.slice(0, mode === 'deep_review' ? 24 : 12).map((file) => ({
    path: file.path,
    status: file.status,
    additions: file.additions,
    deletions: file.deletions,
    patch: (file.patch || '').slice(0, mode === 'deep_review' ? 12000 : 5000),
  }));
  return [
    'You are reviewing a pull request. Return JSON only with actionable review items.',
    'Do not create a general summary comment. The reviewed author does not need a recap unless there is a principle-level concern about the PR direction.',
    'Use summaryComment only when the issue is a principle-level PR direction concern that cannot be tied to a specific file or diff line.',
    `Depth: ${modeLabel(mode)}. Prefer concrete functionality direction, implementation risks, and missing tests.`,
    'Schema: {"findings":[{"path":"src/file.ts","position":12,"body":"specific issue"}],"summaryComment":"","decision":"comment","decisionBody":""}.',
    'Use a 1-based diff position only when you can identify it from the patch. Omit findings that are not supported by the diff.',
    '',
    JSON.stringify({
      title: snapshot.title,
      author: snapshot.author,
      body: snapshot.body,
      base: snapshot.baseBranch,
      head: snapshot.headBranch,
      ci: snapshot.checks,
      existingReviews: snapshot.reviews.slice(0, 12),
      files,
    }, null, 2),
  ].join('\n');
}

function extractJsonObject(value) {
  const text = String(value || '').trim();
  if (!text) return null;
  const start = text.indexOf('{');
  const end = text.lastIndexOf('}');
  if (start < 0 || end <= start) return null;
  try {
    return JSON.parse(text.slice(start, end + 1));
  } catch {
    return null;
  }
}

function normalizeDecision(value) {
  return ['approve', 'request_changes', 'comment'].includes(value) ? value : 'comment';
}

function buildReviewOperations(snapshot, aiText, mode) {
  const parsed = extractJsonObject(aiText);
  const timestamp = snapshot.headSha || Date.now();
  const validPaths = new Set(snapshot.files.map((file) => file.path));
  const operations = [];

  if (parsed && typeof parsed === 'object') {
    const summaryComment = String(parsed.summaryComment || '').trim();
    if (summaryComment) {
      operations.push({
        id: `principle-${timestamp}`,
        kind: 'summary_comment',
        body: summaryComment,
        selected: true,
        stale: false,
        published: false,
      });
    }

    const findings = Array.isArray(parsed.findings) ? parsed.findings : [];
    for (const finding of findings.slice(0, 12)) {
      const path = String(finding?.path || '').trim();
      const body = String(finding?.body || '').trim();
      const position = Number(finding?.position || 0);
      if (!path || !body || !validPaths.has(path) || !Number.isFinite(position) || position <= 0) {
        continue;
      }
      operations.push({
        id: `inline-${timestamp}-${operations.length}`,
        kind: 'inline_comment',
        path,
        position,
        body,
        selected: true,
        stale: false,
        published: false,
      });
    }

    const decisionBody = String(parsed.decisionBody || '').trim();
    if (decisionBody) {
      operations.push({
        id: `decision-${timestamp}`,
        kind: 'review_decision',
        body: decisionBody,
        decision: normalizeDecision(parsed.decision),
        selected: false,
        stale: false,
        published: false,
      });
    }
  }

  if (!operations.length) {
    operations.push({
      id: `decision-${timestamp}`,
      kind: 'review_decision',
      body: String(aiText || t('noActionableFindings')).trim() || t('noActionableFindings'),
      decision: 'comment',
      selected: false,
      stale: false,
      published: false,
      mode,
    });
  }

  return operations;
}

async function generateDraft() {
  const snapshot = selectedSnapshot();
  if (!snapshot) return;
  state.ui.cancelReviewRequested = false;
  setBusy('draft', 'statusGenerating');
  try {
    const mode = state.data.mode || recommendMode(snapshot);
    setReviewProgress('reviewStageRead', `${snapshot.files.length} ${t('files')} · ${t('reviewDetailRead')}`, 18);
    let reviewText = localSummary(snapshot);
    try {
      const result = await withReviewProgressTicker(
        'reviewStageAi',
        [
          `${modeLabel(mode)} · ${t('reviewDetailAi')}`,
          t('reviewDetailAiWait'),
        ],
        32,
        86,
        () => app.ai.complete(reviewPrompt(snapshot, mode), {
          maxTokens: mode === 'deep_review' ? 2200 : 1200,
          temperature: 0.2,
        }),
      );
      if (state.ui.cancelReviewRequested) {
        await finish('reviewCancelled');
        return;
      }
      if (result?.text) reviewText = result.text.trim();
    } catch (error) {
      if (state.ui.cancelReviewRequested) {
        await finish('reviewCancelled');
        return;
      }
      reviewText = `${reviewText}\n\nAI generation was unavailable: ${String(error?.message || error)}`;
    }
    setReviewProgress('reviewStageBuild', t('reviewDetailBuild'), 92);
    const draft = {
      id: `draft-${snapshot.headSha || Date.now()}`,
      headSha: snapshot.headSha,
      mode,
      createdAt: new Date().toISOString(),
      operations: buildReviewOperations(snapshot, reviewText, mode),
    };
    state.data.drafts[snapshotKey(snapshot)] = draft;
    await finish('statusReady');
  } catch (error) {
    setError(error);
  }
}

async function addManualComment() {
  const snapshot = selectedSnapshot();
  if (!snapshot) return;
  const input = document.getElementById('manual-comment');
  const body = input?.value?.trim();
  if (!body) return;
  const key = snapshotKey(snapshot);
  const draft = state.data.drafts[key] || {
    id: `manual-${snapshot.headSha || Date.now()}`,
    headSha: snapshot.headSha,
    mode: state.data.mode,
    createdAt: new Date().toISOString(),
    operations: [],
  };
  draft.operations.unshift({
    id: `manual-${Date.now()}`,
    kind: 'summary_comment',
    body,
    selected: true,
    stale: false,
    published: false,
  });
  state.data.drafts[key] = draft;
  input.value = '';
  await finish('statusSaved');
}

async function deleteDraftOperation(operationId) {
  const draft = selectedDraft();
  if (!draft || !operationId) return;
  draft.operations = draft.operations.filter((operation) => operation.id !== operationId);
  await finish('statusSaved');
}

function selectedOperations(draft) {
  return (draft?.operations || []).filter((op) => op.selected && !op.published);
}

async function requestPublish() {
  const snapshot = selectedSnapshot();
  const draft = selectedDraft();
  const ops = selectedOperations(draft);
  if (!snapshot || !draft || !ops.length) return;
  const profile = profileById(snapshot.identity.providerId);
  if (!hasToken(profile)) {
    setError(t('tokenHelp'));
    return;
  }
  setBusy('stale-check', 'statusLoading');
  try {
    const fresh = await fetchSnapshot(snapshot.identity);
    const stale = draft.headSha && fresh.headSha && draft.headSha !== fresh.headSha;
    if (stale) {
      draft.operations.forEach((op) => { op.stale = true; });
      const byKey = new Map(state.data.items.map((item) => [itemKey(item), item]));
      byKey.set(snapshotKey(fresh), fresh);
      state.data.items = Array.from(byKey.values());
      state.data.selectedKey = snapshotKey(fresh);
    }
    state.ui.busy = null;
    state.ui.confirm = { stale, operationIds: ops.map((op) => op.id), headSha: fresh.headSha };
    render();
  } catch (error) {
    setError(error);
  }
}

async function publishGithubOperation(profile, snapshot, operation) {
  const base = normalizeBaseUrl(profile.apiBaseUrl);
  const ownerRepo = `${encodeURIComponent(snapshot.identity.owner)}/${encodeURIComponent(snapshot.identity.repo)}`;
  const headers = providerHeaders(profile, true);
  if (operation.kind === 'summary_comment') {
    const result = await netJson(`${base}/repos/${ownerRepo}/issues/${snapshot.identity.number}/comments`, {
      method: 'POST',
      headers,
      body: JSON.stringify({ body: operation.body }),
    });
    return result.id || result.node_id || result.url || null;
  }
  if (operation.kind === 'inline_comment') {
    const result = await netJson(`${base}/repos/${ownerRepo}/pulls/${snapshot.identity.number}/comments`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        body: operation.body,
        commit_id: snapshot.headSha,
        path: operation.path,
        position: operation.position,
      }),
    });
    return result.id || result.node_id || result.url || null;
  }
  if (operation.kind === 'review_decision') {
    const event = operation.decision === 'approve'
      ? 'APPROVE'
      : operation.decision === 'request_changes'
        ? 'REQUEST_CHANGES'
        : 'COMMENT';
    const result = await netJson(`${base}/repos/${ownerRepo}/pulls/${snapshot.identity.number}/reviews`, {
      method: 'POST',
      headers,
      body: JSON.stringify({ body: operation.body, event }),
    });
    return result.id || result.node_id || result.url || null;
  }
  return null;
}

async function publishCompatibleOperation(profile, snapshot, operation) {
  if (operation.kind === 'review_decision') return 'skipped';
  const base = normalizeBaseUrl(profile.apiBaseUrl);
  const ownerRepo = `${encodeURIComponent(snapshot.identity.owner)}/${encodeURIComponent(snapshot.identity.repo)}`;
  const headers = providerHeaders(profile, true);
  const payload = {
    body: operation.body,
    path: operation.path,
    position: operation.position,
  };
  try {
    const result = await netJson(`${base}/repos/${ownerRepo}/pulls/${snapshot.identity.number}/comments`, {
      method: 'POST',
      headers,
      body: JSON.stringify(payload),
    });
    return result.id || result.url || null;
  } catch {
    const result = await netJson(`${base}/projects/${encodeURIComponent(`${snapshot.identity.owner}/${snapshot.identity.repo}`)}/merge_requests/${snapshot.identity.number}/notes`, {
      method: 'POST',
      headers,
      body: JSON.stringify({ body: operation.body }),
    });
    return result.id || result.web_url || null;
  }
}

async function confirmPublish() {
  const snapshot = selectedSnapshot();
  const draft = selectedDraft();
  const confirm = state.ui.confirm;
  if (!snapshot || !draft || !confirm) return;
  if (confirm.stale && !document.getElementById('confirm-stale')?.checked) return;
  const operations = selectedOperations(draft).filter((op) => confirm.operationIds.includes(op.id));
  const profile = profileById(snapshot.identity.providerId);
  setBusy('publish', 'statusPublishing');
  const results = [];
  for (const operation of operations) {
    try {
      const providerOperationId = profile.kind === 'github'
        ? await publishGithubOperation(profile, snapshot, operation)
        : await publishCompatibleOperation(profile, snapshot, operation);
      operation.published = providerOperationId !== 'skipped';
      results.push({
        operationId: operation.id,
        status: providerOperationId === 'skipped' ? 'skipped' : 'success',
        providerOperationId,
      });
    } catch (error) {
      results.push({
        operationId: operation.id,
        status: 'failed',
        message: String(error?.message || error),
      });
    }
  }
  const auditEntries = results.map((result) => ({
    id: `${snapshotKey(snapshot)}:${draft.id}:${result.operationId}`,
    providerId: profile.id,
    owner: snapshot.identity.owner,
    repo: snapshot.identity.repo,
    number: snapshot.identity.number,
    draftId: draft.id,
    operationId: result.operationId,
    status: result.status,
    providerOperationId: result.providerOperationId || null,
    message: result.message || null,
    timestamp: new Date().toISOString(),
  }));
  state.data.audit = [...auditEntries, ...state.data.audit].slice(0, 50);
  state.data.lastReviewedHeads[snapshotKey(snapshot)] = snapshot.headSha;
  state.ui.confirm = null;
  await finish('statusPublished');
}

async function markReviewed() {
  const snapshot = selectedSnapshot();
  if (!snapshot) return;
  state.data.lastReviewedHeads[snapshotKey(snapshot)] = snapshot.headSha;
  await finish('statusReviewed');
}

function resetPollTimer() {
  if (state.volatile.pollTimer) clearInterval(state.volatile.pollTimer);
  const minutes = Math.max(1, Number(state.data.pollMinutes || DEFAULT_POLL_MINUTES));
  state.volatile.pollTimer = setInterval(() => {
    if (!state.ui.busy && (activeSubscriptions().length || state.data.queueMode === 'mine')) {
      void syncQueue(state.data.queueMode);
    }
  }, minutes * 60 * 1000);
}

function renderStatus() {
  if (state.ui.error) return `<div class="pr-status pr-status--error">${esc(state.ui.error)}</div>`;
  if (state.ui.status) return `<div class="pr-status">${esc(state.ui.status)}</div>`;
  return '';
}

function renderCommandBar() {
  const profile = activeProfile();
  return `
    <header class="pr-command-bar">
      <div class="pr-brand">
        <div class="pr-brand-mark">PR</div>
        <div>
          <h1>${esc(t('title'))}</h1>
          <p>${esc(t('subtitle'))}</p>
        </div>
      </div>
      <div class="pr-repo-first pr-url-card">
        <div class="pr-mini-title">
          <strong>${esc(t('repositoryFirst'))}</strong>
          <span>${esc(t('repositoryFirstHint'))}</span>
        </div>
        <form class="pr-top-watch-form" id="quick-subscription-form">
          <label class="pr-field">
            <span>${esc(t('provider'))}</span>
            <select class="pr-select" name="providerId" aria-label="${esc(t('provider'))}">
              ${state.data.profiles.map((item) => `<option value="${esc(item.id)}" ${item.id === state.ui.activeProviderId ? 'selected' : ''}>${esc(item.displayName)}</option>`).join('')}
            </select>
          </label>
          <label class="pr-field pr-field--repo-ref">
            <span>${esc(t('repoRef'))}</span>
            <input class="pr-input" name="repoRef" placeholder="${esc(t('repoRefPlaceholder'))}" required />
          </label>
          <button class="pr-btn pr-btn--primary pr-btn--compact" type="submit">${esc(t('addWatchedRepo'))}</button>
        </form>
      </div>
      <div class="pr-url-card pr-url-fallback">
        <div class="pr-mini-title">
          <strong>${esc(t('singlePrFallback'))}</strong>
          <span>${esc(t('singlePrFallbackHint'))}</span>
        </div>
        <div class="pr-open-strip">
          <label class="pr-field pr-field--url">
            <span>${esc(t('directUrl'))}</span>
            <input class="pr-input pr-url-input" id="direct-url" value="${esc(state.data.directUrl)}" placeholder="${esc(t('directPlaceholder'))}" />
          </label>
          <button class="pr-btn pr-btn--primary pr-btn--compact" data-action="open-direct" ${state.ui.busy ? 'disabled' : ''}>${esc(t('openPr'))}</button>
        </div>
      </div>
      <div class="pr-access">
        <div class="pr-mini-title">
          <strong>${esc(t('privateAction'))}</strong>
          <span>${esc(t('authAutoHint'))}</span>
        </div>
        <div class="pr-access-row">
          <label class="pr-field">
            <span>${esc(t('provider'))}</span>
            <select class="pr-select" id="active-provider">
              ${state.data.profiles.map((item) => `<option value="${esc(item.id)}" ${item.id === state.ui.activeProviderId ? 'selected' : ''}>${esc(item.displayName)}</option>`).join('')}
            </select>
          </label>
          ${profile?.kind === 'github' ? `<button class="pr-btn pr-btn--compact" data-action="authorize-gh" ${state.ui.busy ? 'disabled' : ''}>${esc(t('authorizeGitHubCli'))}</button>` : ''}
          <span class="pr-token-badge ${hasToken(profile) ? 'is-ready' : ''}">${esc(hasToken(profile) ? t('tokenReady') : t('tokenMissing'))}</span>
        </div>
        <details class="pr-token-details">
          <summary>${esc(t('manualToken'))}</summary>
          <label class="pr-field pr-field--token">
            <span>${esc(t('token'))}</span>
            <input class="pr-input" id="session-token" type="password" placeholder="${esc(profile?.credentialLabel || t('token'))}" autocomplete="off" />
          </label>
        </details>
      </div>
    </header>
  `;
}

function renderQueuePanel() {
  const mode = state.data.queueMode;
  const activeRepoCount = activeSubscriptions().length;
  return `
    <aside class="pr-sidebar">
      ${renderStatus()}
      ${renderSourcesPanel()}
      <section class="pr-card pr-card--queue">
        <div class="pr-card-head">
          <div>
            <h2>${esc(t('queue'))}</h2>
            <p>${esc(activeRepoCount ? t(mode === 'mine' ? 'queueModeMineHint' : 'queueModeAllHint') : t(state.data.subscriptions.length ? 'statusNoActiveSubscriptions' : 'queueEmpty'))}</p>
          </div>
          <span class="pr-count">${state.data.items.length}</span>
        </div>
        <div class="pr-sync-panel">
          <button class="pr-sync-tile ${mode === 'all' ? 'is-active' : ''}" data-action="queue-mode" data-mode="all" ${state.ui.busy ? 'disabled' : ''}>
            <strong>${esc(t('syncAllTitle'))}</strong>
            <span>${esc(t('syncAllDesc'))}</span>
          </button>
          <button class="pr-sync-tile ${mode === 'mine' ? 'is-active' : ''}" data-action="queue-mode" data-mode="mine" ${state.ui.busy ? 'disabled' : ''}>
            <strong>${esc(t('syncMineTitle'))}</strong>
            <span>${esc(t('syncMineDesc'))}</span>
          </button>
        </div>
        <div class="pr-queue-actions">
          <label class="pr-mini-control">
            <span>${esc(t('autoSync'))}</span>
            <input class="pr-input" id="poll-minutes" type="number" min="1" max="120" value="${esc(state.data.pollMinutes)}" />
            <span>${esc(t('minutes'))}</span>
          </label>
          <button class="pr-btn pr-btn--compact pr-refresh-now" data-action="sync-current" ${state.ui.busy ? 'disabled' : ''}>${esc(t('refreshNow'))}</button>
        </div>
        <div class="pr-list">
          ${state.data.items.length ? state.data.items.map(renderInboxItem).join('') : `<div class="pr-empty">${esc(t('queueEmpty'))}</div>`}
        </div>
      </section>
    </aside>
  `;
}

function renderSourcesPanel() {
  return `
    <section class="pr-card pr-card--sources">
      <div class="pr-card-head">
        <div>
          <h2>${esc(t('watchedRepos'))}</h2>
          <p>${esc(t('repositoryFirstHint'))}</p>
        </div>
        <button class="pr-btn pr-btn--compact" data-action="discover-workspace" ${state.ui.busy ? 'disabled' : ''}>${esc(t('rediscoverWorkspace'))}</button>
      </div>
      <div class="pr-source-list pr-source-list--open">
        ${state.data.subscriptions.length ? state.data.subscriptions.map(renderSubscriptionRow).join('') : `<div class="pr-muted-box">${esc(t('noWatchedRepos'))}</div>`}
      </div>
      <details class="pr-config-group">
        <summary>${esc(t('advancedProviders'))}</summary>
        <div class="pr-source-list">
          ${state.data.profiles.map(renderProviderRow).join('')}
        </div>
        ${renderProviderForm()}
      </details>
    </section>
  `;
}

function renderSubscriptionRow(subscription, index) {
  const profile = profileById(subscription.providerId);
  const isEnabled = subscription.enabled !== false;
  return `
    <div class="pr-source-row ${isEnabled ? '' : 'is-paused'}">
      <div class="pr-source-main">
        <strong>${esc(subscription.owner)}/${esc(subscription.repo)}</strong>
        <span>${esc(profile?.displayName || subscription.providerId)} · ${esc(subscription.source === 'workspace' ? t('workspaceRepo') : t('autoSync'))}</span>
      </div>
      <div class="pr-source-actions">
        <label class="pr-listen-switch" title="${esc(t(isEnabled ? 'pauseListening' : 'resumeListening'))}">
          <input class="subscription-enabled" type="checkbox" data-action="toggle-subscription" data-index="${index}" ${isEnabled ? 'checked' : ''} />
          <span aria-hidden="true"></span>
          <em>${esc(t(isEnabled ? 'listenEnabled' : 'listenDisabled'))}</em>
        </label>
        <button class="pr-text-btn" title="${esc(t('delete'))}" data-action="delete-subscription" data-index="${index}">${esc(t('removeRepo'))}</button>
      </div>
    </div>
  `;
}

function renderProviderRow(profile, index) {
  return `
    <div class="pr-source-row">
      <div>
        <strong>${esc(profile.displayName)}</strong>
        <span>${esc(profile.kind)} · ${esc(normalizeBaseUrl(profile.webBaseUrl))}</span>
      </div>
      <button class="pr-icon-btn" title="${esc(t('delete'))}" data-action="delete-provider" data-index="${index}" ${state.data.profiles.length <= 1 ? 'disabled' : ''}>×</button>
    </div>
  `;
}

function renderSubscriptionForm() {
  return `
    <form class="pr-form pr-form--compact" id="subscription-form">
      <label class="pr-field">
        <span>${esc(t('provider'))}</span>
        <select class="pr-select" name="providerId" aria-label="${esc(t('provider'))}">
          ${state.data.profiles.map((profile) => `<option value="${esc(profile.id)}">${esc(profile.displayName)}</option>`).join('')}
        </select>
      </label>
      <label class="pr-field">
        <span>${esc(t('owner'))}</span>
        <input class="pr-input" name="owner" placeholder="GCWing" required />
      </label>
      <label class="pr-field">
        <span>${esc(t('repo'))}</span>
        <input class="pr-input" name="repo" placeholder="BitFun" required />
      </label>
      <button class="pr-btn" type="submit">${esc(t('addWatchedRepo'))}</button>
    </form>
  `;
}

function renderProviderForm() {
  return `
    <form class="pr-form" id="provider-form">
      <div class="pr-form-grid">
        <label class="pr-field">
          <span>${esc(t('providerName'))}</span>
          <input class="pr-input" name="displayName" placeholder="GitHub Enterprise" required />
        </label>
        <label class="pr-field">
          <span>${esc(t('kind'))}</span>
          <select class="pr-select" name="kind">
            <option value="github">${esc(t('github'))}</option>
            <option value="gitcode">${esc(t('gitcode'))}</option>
            <option value="custom">${esc(t('custom'))}</option>
          </select>
        </label>
      </div>
      <label class="pr-field">
        <span>${esc(t('webBase'))}</span>
        <input class="pr-input" name="webBaseUrl" placeholder="https://git.example.com" required />
      </label>
      <label class="pr-field">
        <span>${esc(t('apiBase'))}</span>
        <input class="pr-input" name="apiBaseUrl" placeholder="https://git.example.com/api/v3" required />
      </label>
      <label class="pr-field">
        <span>${esc(t('credentialLabel'))}</span>
        <input class="pr-input" name="credentialLabel" placeholder="Private token" />
      </label>
      <button class="pr-btn" type="submit">${esc(t('addProvider'))}</button>
    </form>
  `;
}

function renderInboxItem(item) {
  const key = itemKey(item);
  const checks = item.checks || [];
  const failed = checks.some((check) => ['failure', 'failed', 'error', 'timed_out'].includes(String(check.conclusion || check.status).toLowerCase()));
  const ok = checks.length && !failed;
  const lines = item.files.reduce((sum, file) => sum + file.additions + file.deletions, 0);
  const providerName = profileById(item.identity.providerId)?.displayName || item.identity.providerId;
  const excerpt = textSnippet(item.body || item.reviewSummary || '', 132);
  return `
    <button class="pr-queue-item ${key === state.data.selectedKey ? 'is-active' : ''}" data-action="select-pr" data-key="${esc(key)}">
      <span class="pr-queue-title">${esc(item.title || `#${item.identity.number}`)}</span>
      <span class="pr-queue-meta pr-queue-meta--primary">
        <span class="pr-queue-actor">@${esc(item.author || '--')}</span>
        <span>${esc(providerName)}</span>
        <span>${esc(item.identity.owner)}/${esc(item.identity.repo)}#${esc(item.identity.number)}</span>
        <span>${esc(t('created'))}: ${esc(formatDate(item.createdAt))}</span>
        <span>${esc(t('updated'))}: ${esc(formatDate(item.updatedAt))}</span>
      </span>
      ${excerpt ? `<span class="pr-queue-excerpt">${esc(excerpt)}</span>` : ''}
      <span class="pr-queue-signals">
        ${renderDraftStateChip(item)}
        <span class="pr-chip">${esc(item.files.length)} ${esc(t('files'))}</span>
        <span class="pr-chip">${esc(lines)} ${esc(t('changedLines'))}</span>
        ${checks.length ? `<span class="pr-chip ${failed ? 'is-bad' : ok ? 'is-ok' : ''}">CI ${esc(failed ? t('failed') : t('success'))}</span>` : ''}
        ${item.stale ? `<span class="pr-chip is-warn">${esc(t('stale'))}</span>` : ''}
      </span>
    </button>
  `;
}

function renderDraftStateChip(item) {
  return `<span class="pr-chip ${item.isDraft ? 'is-draft' : 'is-ready'}">${esc(t(item.isDraft ? 'draftStatus' : 'readyStatus'))}</span>`;
}

function renderOverviewSection(snapshot) {
  const body = snapshot.body || t('noBody');
  const summary = textSnippet(body, 160) || t('overviewHint');
  return `
    <details class="pr-review-section pr-fold pr-overview-fold">
      <summary>${esc(t('overview'))}<span>${esc(summary)}</span></summary>
      <div class="pr-description">${esc(body)}</div>
    </details>
  `;
}

function renderReviewWorkspace() {
  const snapshot = selectedSnapshot();
  if (!snapshot) {
    return `
      <section class="pr-review-workspace">
        <div class="pr-empty pr-empty--large">${esc(t('noPr'))}</div>
      </section>
    `;
  }
  const summary = snapshot.reviewSummary || summarizeReviews(snapshot.reviews || []);
  const lines = snapshot.files.reduce((sum, file) => sum + file.additions + file.deletions, 0);
  return `
    <section class="pr-review-workspace">
      <div class="pr-pr-header">
        <div>
          <div class="pr-eyebrow">${esc(t('selectedPr'))} · ${esc(snapshot.identity.owner)}/${esc(snapshot.identity.repo)}#${esc(snapshot.identity.number)}</div>
          <h2>${esc(snapshot.title)}</h2>
          <div class="pr-meta-row">
            <span>${esc(t('author'))}: ${esc(snapshot.author)}</span>
            <span>${esc(t('state'))}: ${esc(snapshot.state)}</span>
            <span>${esc(t('branch'))}: ${esc(snapshot.baseBranch)} &larr; ${esc(snapshot.headBranch)}</span>
            <span>${esc(t('created'))}: ${esc(formatDate(snapshot.createdAt))}</span>
            <span>${esc(t('updated'))}: ${esc(formatDate(snapshot.updatedAt))}</span>
          </div>
        </div>
        <div class="pr-pr-actions">
          <button class="pr-btn" data-action="open-external">${esc(t('openExternal'))}</button>
          <button class="pr-btn" data-action="mark-reviewed">${esc(t('markReviewed'))}</button>
          <button class="pr-btn pr-btn--primary" data-action="start-review" ${state.ui.busy ? 'disabled' : ''}>${esc(t('startReview'))}</button>
        </div>
      </div>
      <div class="pr-kpis">
        <div><strong>${snapshot.files.length}</strong><span>${esc(t('files'))}</span></div>
        <div><strong>${lines}</strong><span>${esc(t('changedLines'))}</span></div>
        <div><strong>${summary.comments}</strong><span>${esc(t('existingReview'))}</span></div>
        <div><strong>${snapshot.checks.length}</strong><span>${esc(t('ciDetails'))}</span></div>
      </div>
      ${renderOverviewSection(snapshot)}
      ${renderFilesExplorer(snapshot)}
      <details class="pr-review-section pr-fold">
        <summary>${esc(t('ciDetails'))}<span>${esc(t('ciFolded'))}</span></summary>
        ${renderChecks(snapshot.checks)}
      </details>
      <section class="pr-review-section">
        <h3>${esc(t('existingReview'))}</h3>
        ${renderReviews(snapshot.reviews)}
        ${renderManualComment()}
      </section>
    </section>
  `;
}

function renderFilesExplorer(snapshot) {
  const files = snapshot.files || [];
  const activePath = state.data.selectedFilePath || files[0]?.path || null;
  const activeFile = files.find((file) => file.path === activePath) || files[0];
  const focusedPosition = state.ui.focusedDiffPath === activeFile?.path
    ? state.ui.focusedDiffPosition
    : null;
  if (!files.length) {
    return `
      <section class="pr-review-section">
        <h3>${esc(t('files'))}</h3>
        <div class="pr-empty">${esc(t('noFiles'))}</div>
      </section>
    `;
  }
  return `
    <section class="pr-review-section pr-files-explorer">
      <div class="pr-section-head">
        <h3>${esc(t('files'))}</h3>
        <span class="pr-chip">${files.length}</span>
      </div>
      <div class="pr-files-layout">
        <nav class="pr-file-list">
          ${files.map((file) => `
            <button class="${file.path === activeFile.path ? 'is-active' : ''}" data-action="select-file" data-path="${esc(file.path)}">
              <span>${esc(file.path)}</span>
              <small>+${esc(file.additions)} -${esc(file.deletions)}</small>
            </button>
          `).join('')}
        </nav>
        <article class="pr-diff-panel" id="pr-diff-view">
          <div class="pr-diff-head">
            <strong>${esc(activeFile.path)}</strong>
            <span>
              <span class="pr-chip is-ok">+${esc(activeFile.additions)}</span>
              <span class="pr-chip is-bad">-${esc(activeFile.deletions)}</span>
              ${activeFile.isBinary ? `<span class="pr-chip">${esc(t('binary'))}</span>` : ''}
              ${activeFile.isTooLarge ? `<span class="pr-chip is-warn">${esc(t('large'))}</span>` : ''}
            </span>
          </div>
          <pre class="pr-diff">${renderHighlightedDiff((activeFile.patch || '').slice(0, 10000) || activeFile.status, focusedPosition)}</pre>
        </article>
      </div>
    </section>
  `;
}

function renderChecks(checks) {
  if (!checks.length) return `<div class="pr-empty">${esc(t('noCi'))}</div>`;
  return `<div class="pr-review-list">${checks.map((check) => `
    <div class="pr-review-row">
      <strong>${esc(check.name)}</strong>
      <span class="pr-chip">${esc(check.conclusion || check.status)}</span>
    </div>
  `).join('')}</div>`;
}

function renderReviews(reviews) {
  if (!reviews.length) return `<div class="pr-empty">${esc(t('noReviews'))}</div>`;
  return `<div class="pr-review-list">${reviews.slice(0, 24).map((review) => `
    <div class="pr-review-row">
      <div>
        <strong>${esc(review.author || review.kind)}</strong>
        ${review.path ? renderFileTargetLink(review.path, review.position) : ''}
      </div>
      <span class="pr-chip">${esc(review.state || review.kind)}</span>
      ${review.body ? `<p>${esc(review.body).slice(0, 900)}</p>` : ''}
    </div>
  `).join('')}</div>`;
}

function renderManualComment() {
  return `
    <div class="pr-manual-comment">
      <label for="manual-comment">${esc(t('manualComment'))}</label>
      <textarea id="manual-comment" class="pr-textarea" placeholder="${esc(t('manualCommentPlaceholder'))}"></textarea>
      <button class="pr-btn" data-action="add-manual-comment">${esc(t('addManualComment'))}</button>
    </div>
  `;
}

function renderComposer() {
  const snapshot = selectedSnapshot();
  const draft = selectedDraft();
  const selected = selectedOperations(draft).length;
  return `
    <aside class="pr-composer">
      <div class="pr-card-head">
        <div>
          <h2>${esc(t('composer'))}</h2>
          <p>${esc(t('composerHint'))}</p>
        </div>
      </div>
      ${renderComposerStatus()}
      <div class="pr-segmented pr-segmented--modes">
        ${renderModeTab('fast_check', t('modeFast'))}
        ${renderModeTab('focused_review', t('modeFocused'))}
        ${renderModeTab('deep_review', t('modeDeep'))}
      </div>
      <div class="pr-compose-actions">
        <button class="pr-btn pr-btn--primary" data-action="start-review" ${!snapshot || state.ui.busy ? 'disabled' : ''}>${esc(t('startReview'))}</button>
        ${state.ui.busy === 'draft' ? `<button class="pr-btn pr-btn--compact" data-action="cancel-review">${esc(t('cancelReview'))}</button>` : ''}
        <button class="pr-btn" data-action="request-publish" ${!draft || !selected || state.ui.busy ? 'disabled' : ''}>${esc(t('publishSelected'))}</button>
      </div>
      ${renderReviewProgress()}
      <div class="pr-muted-box">${esc(t('selectedOps'))}: ${selected}</div>
      ${draft ? `<div class="pr-draft-list">${draft.operations.map(renderOperation).join('')}</div>` : `<div class="pr-empty">${esc(t('noPr'))}</div>`}
      ${renderAudit()}
    </aside>
  `;
}

function renderComposerStatus() {
  if (state.ui.busy !== 'draft' && !state.ui.reviewProgress) return '';
  const progress = state.ui.reviewProgress;
  const detail = [
    progress?.stage,
    progress?.detail,
  ].filter(Boolean).join(' · ');
  return `
    <div class="pr-live-status">
      <span class="pr-live-status-dot"></span>
      <div>
        <strong>${esc(state.ui.status || t('reviewProgress'))}</strong>
        ${detail ? `<span>${esc(detail)}</span>` : ''}
      </div>
    </div>
  `;
}

function renderReviewProgress() {
  if (!state.ui.reviewProgress) return '';
  const progressPct = Math.max(4, Math.min(100, Number(state.ui.reviewProgress.progressPct || 0)));
  return `
    <div class="pr-progress">
      <div class="pr-progress-head">
        <strong>${esc(t('reviewProgress'))}</strong>
        <span>${esc(state.ui.reviewProgress.stage)}</span>
      </div>
      <div class="pr-progress-bar"><span style="width: ${progressPct}%"></span></div>
      ${state.ui.reviewProgress.detail ? `<p>${esc(state.ui.reviewProgress.detail)}</p>` : ''}
    </div>
  `;
}

function renderModeTab(mode, label) {
  return `<button class="${state.data.mode === mode ? 'is-active' : ''}" data-action="set-mode" data-mode="${esc(mode)}">${esc(label)}</button>`;
}

function renderOperation(operation) {
  const kindLabel = operation.kind === 'summary_comment'
    ? t('summaryComment')
    : operation.kind === 'inline_comment'
      ? t('inlineComment')
      : t('reviewDecision');
  return `
    <article class="pr-draft-op" data-op-id="${esc(operation.id)}">
      <div class="pr-draft-head">
        <label>
          <input type="checkbox" class="op-selected" data-op-id="${esc(operation.id)}" ${operation.selected ? 'checked' : ''} ${operation.published ? 'disabled' : ''} />
          <strong>${esc(kindLabel)}</strong>
        </label>
        <span class="pr-draft-head-actions">
          ${operation.path ? renderFileTargetLink(operation.path, operation.position) : ''}
          ${operation.stale ? `<span class="pr-chip is-warn">${esc(t('stale'))}</span>` : ''}
          ${operation.published ? `<span class="pr-chip is-ok">${esc(t('published'))}</span>` : ''}
          <button class="pr-text-btn pr-op-delete" data-action="delete-operation" data-op-id="${esc(operation.id)}" ${operation.published || state.ui.busy ? 'disabled' : ''}>${esc(t('delete'))}</button>
        </span>
      </div>
      ${operation.kind === 'review_decision' ? renderDecisionSelect(operation) : ''}
      <textarea class="pr-textarea op-body" data-op-id="${esc(operation.id)}">${esc(operation.body)}</textarea>
    </article>
  `;
}

function renderDecisionSelect(operation) {
  return `
    <select class="pr-select op-decision" data-op-id="${esc(operation.id)}">
      <option value="comment" ${operation.decision === 'comment' ? 'selected' : ''}>${esc(t('decisionComment'))}</option>
      <option value="approve" ${operation.decision === 'approve' ? 'selected' : ''}>${esc(t('decisionApprove'))}</option>
      <option value="request_changes" ${operation.decision === 'request_changes' ? 'selected' : ''}>${esc(t('decisionRequestChanges'))}</option>
    </select>
  `;
}

function renderAudit() {
  if (!state.data.audit.length) return '';
  return `
    <details class="pr-audit">
      <summary>${esc(t('audit'))}</summary>
      ${state.data.audit.slice(0, 8).map((entry) => `
        <div class="pr-audit-row">
          <span>${esc(entry.owner)}/${esc(entry.repo)}#${esc(entry.number)}</span>
          <span>${esc(t(entry.status) || entry.status)} · ${esc(formatDate(entry.timestamp))}</span>
        </div>
      `).join('')}
    </details>
  `;
}

function renderConfirm() {
  const confirm = state.ui.confirm;
  const draft = selectedDraft();
  if (!confirm || !draft) return '';
  const count = selectedOperations(draft).length;
  return `
    <div class="pr-modal-backdrop">
      <section class="pr-modal" role="dialog" aria-modal="true">
        <div class="pr-modal-head">
          <h2>${esc(confirm.stale ? t('publishStaleTitle') : t('publishConfirmTitle'))}</h2>
        </div>
        <div class="pr-modal-body">
          <p>${esc(confirm.stale ? t('publishStaleBody') : t('publishConfirmBody'))}</p>
          <div class="pr-muted-box">${esc(t('selectedOps'))}: ${count}</div>
          ${confirm.stale ? `<label class="pr-check"><input id="confirm-stale" type="checkbox" /><span>${esc(t('staleConfirm'))}</span></label>` : ''}
        </div>
        <div class="pr-modal-foot">
          <button class="pr-btn" data-action="cancel-confirm">${esc(t('cancel'))}</button>
          <button class="pr-btn pr-btn--primary" data-action="confirm-publish">${esc(t('publishNow'))}</button>
        </div>
      </section>
    </div>
  `;
}

function render(options = {}) {
  const reviewWorkspaceScroll = options.preserveReviewWorkspaceScroll
    ? readReviewWorkspaceScroll()
    : null;
  root.innerHTML = `
    <main class="pr-shell">
      ${renderCommandBar()}
      <div class="pr-main-layout">
        ${renderQueuePanel()}
        ${renderReviewWorkspace()}
        ${renderComposer()}
      </div>
      ${renderConfirm()}
    </main>
  `;
  if (options.preserveReviewWorkspaceScroll) {
    restoreReviewWorkspaceScroll(reviewWorkspaceScroll);
  }
}

function formatDate(value) {
  if (!value) return '--';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(state.locale, {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

function textSnippet(value, limit = 120) {
  const normalized = String(value || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/[#>*_`[\]()]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  if (normalized.length <= limit) return normalized;
  return `${normalized.slice(0, Math.max(0, limit - 1)).trim()}...`;
}

function compactPath(path, maxLength = 38) {
  const value = String(path || '');
  if (value.length <= maxLength) return value;
  const parts = value.split('/').filter(Boolean);
  const fileName = parts.pop() || value;
  const parent = parts.pop();
  const tail = parent ? `${parent}/${fileName}` : fileName;
  if (tail.length <= maxLength - 4) return `.../${tail}`;
  return `.../${tail.slice(Math.max(0, tail.length - maxLength + 4))}`;
}

function renderFileTargetLink(path, position) {
  if (!path) return '';
  const positionText = position ? `:${position}` : '';
  return `
    <a class="pr-file-link" href="#pr-diff-view" data-action="jump-file-target" data-path="${esc(path)}" data-position="${esc(position || '')}" title="${esc(path)}${esc(positionText)}">
      <span>${esc(compactPath(path))}${esc(positionText)}</span>
    </a>
  `;
}

function getDraftOperation(id) {
  const draft = selectedDraft();
  return draft?.operations.find((operation) => operation.id === id) || null;
}

async function jumpToFileTarget(path, position) {
  if (!path) return;
  state.data.selectedFilePath = path;
  state.ui.focusedDiffPath = path;
  state.ui.focusedDiffPosition = Number(position || 0) || null;
  await saveStorage();
  render();
  window.requestAnimationFrame(() => {
    const target = document.querySelector('.pr-diff-line.is-target') || document.getElementById('pr-diff-view');
    target?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  });
}

async function openSelectedPrExternal() {
  const snapshot = selectedSnapshot();
  if (!snapshot?.url) return;
  state.ui.status = t('statusOpeningPr');
  state.ui.error = null;
  render();
  try {
    if (app.system?.openExternal) {
      await app.system.openExternal(snapshot.url);
    } else {
      window.open(snapshot.url, '_blank', 'noopener,noreferrer');
    }
    state.ui.status = t('statusReady');
    render();
  } catch (error) {
    try {
      window.open(snapshot.url, '_blank', 'noopener,noreferrer');
      state.ui.status = t('statusReady');
      render();
    } catch {
      setError(error);
    }
  }
}

document.addEventListener('click', (event) => {
  const target = event.target instanceof Element ? event.target.closest('[data-action]') : null;
  if (!target) return;
  if (target instanceof HTMLAnchorElement) event.preventDefault();
  const action = target.dataset.action;
  if (action === 'open-direct') void openDirectUrl();
  if (action === 'authorize-gh') void authorizeGitHubCli();
  if (action === 'discover-workspace') void applyWorkspaceDiscoveredRepositories({ force: true });
  if (action === 'sync-current') void syncQueue(state.data.queueMode);
  if (action === 'queue-mode') {
    const nextMode = target.dataset.mode || 'all';
    state.data.queueMode = nextMode;
    void saveStorage();
    render();
    void syncQueue(nextMode);
  }
  if (action === 'select-pr') {
    state.data.selectedKey = target.dataset.key;
    const snapshot = selectedSnapshot();
    if (snapshot) {
      state.ui.activeProviderId = snapshot.identity.providerId;
      state.data.selectedFilePath = snapshot.files?.[0]?.path || null;
      state.data.mode = state.data.mode || recommendMode(snapshot);
    }
    void saveStorage();
    render();
  }
  if (action === 'select-file') {
    state.data.selectedFilePath = target.dataset.path;
    state.ui.focusedDiffPath = null;
    state.ui.focusedDiffPosition = null;
    void saveStorage();
    render({ preserveReviewWorkspaceScroll: true });
  }
  if (action === 'jump-file-target') void jumpToFileTarget(target.dataset.path, target.dataset.position);
  if (action === 'set-mode') {
    state.data.mode = target.dataset.mode;
    void saveStorage();
    render();
  }
  if (action === 'start-review') void generateDraft();
  if (action === 'cancel-review') {
    state.ui.cancelReviewRequested = true;
    state.ui.reviewProgress = {
      stage: t('reviewCancelled'),
      detail: '',
      cancelled: true,
    };
    render();
  }
  if (action === 'add-manual-comment') void addManualComment();
  if (action === 'delete-operation') void deleteDraftOperation(target.dataset.opId);
  if (action === 'request-publish') void requestPublish();
  if (action === 'confirm-publish') void confirmPublish();
  if (action === 'cancel-confirm') {
    state.ui.confirm = null;
    render();
  }
  if (action === 'mark-reviewed') void markReviewed();
  if (action === 'open-external') {
    void openSelectedPrExternal();
  }
  if (action === 'delete-subscription') {
    const [removed] = state.data.subscriptions.splice(Number(target.dataset.index), 1);
    if (removed?.source === 'workspace') {
      const dismissed = new Set(state.data.dismissedWorkspaceRepos || []);
      dismissed.add(subscriptionKey(removed));
      state.data.dismissedWorkspaceRepos = Array.from(dismissed);
    }
    void finish('statusSaved');
  }
  if (action === 'delete-provider') {
    const index = Number(target.dataset.index);
    if (state.data.profiles.length > 1) {
      const [removed] = state.data.profiles.splice(index, 1);
      delete state.volatile.sessionTokens[removed.id];
      state.data.subscriptions = state.data.subscriptions.filter((subscription) => subscription.providerId !== removed.id);
      state.ui.activeProviderId = state.data.profiles[0]?.id || 'github';
      void finish('statusSaved');
    }
  }
});

document.addEventListener('input', (event) => {
  const target = event.target;
  if (!(target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement)) return;
  if (target.id === 'direct-url') state.data.directUrl = target.value;
  if (target.id === 'session-token') {
    const profile = activeProfile();
    if (profile) state.volatile.sessionTokens[profile.id] = target.value;
  }
  if (target.id === 'poll-minutes') {
    state.data.pollMinutes = Math.max(1, Number(target.value || DEFAULT_POLL_MINUTES));
    void saveStorage();
    resetPollTimer();
  }
  if (target.classList.contains('op-body')) {
    const op = getDraftOperation(target.dataset.opId);
    if (op) {
      op.body = target.value;
      void saveStorage();
    }
  }
});

document.addEventListener('change', (event) => {
  const target = event.target;
  if (!(target instanceof HTMLInputElement || target instanceof HTMLSelectElement)) return;
  if (target.id === 'active-provider') {
    state.ui.activeProviderId = target.value;
    render();
  }
  if (target.classList.contains('subscription-enabled')) {
    const subscription = state.data.subscriptions[Number(target.dataset.index)];
    if (subscription) {
      subscription.enabled = target.checked;
      state.ui.status = t('statusSaved');
      state.ui.error = null;
      void saveStorage();
      resetPollTimer();
      render();
    }
  }
  if (target.classList.contains('op-selected')) {
    const op = getDraftOperation(target.dataset.opId);
    if (op) {
      op.selected = target.checked;
      void saveStorage();
      render();
    }
  }
  if (target.classList.contains('op-decision')) {
    const op = getDraftOperation(target.dataset.opId);
    if (op) {
      op.decision = target.value;
      void saveStorage();
    }
  }
});

document.addEventListener('submit', (event) => {
  event.preventDefault();
  const form = event.target;
  const values = Object.fromEntries(new FormData(form).entries());
  if (form.id === 'subscription-form' || form.id === 'quick-subscription-form') {
    const profile = profileById(String(values.providerId));
    const parsedRepo = values.repoRef
      ? parseRepositoryRef(values.repoRef, profile)
      : normalizeRepositoryParts({
        providerId: String(values.providerId),
        owner: String(values.owner || '').trim(),
        repo: String(values.repo || '').trim(),
      }, profile);
    if (!parsedRepo?.owner || !parsedRepo?.repo) {
      setError(t('statusNoSubscriptions'));
      return;
    }
    const nextSubscription = {
      providerId: String(parsedRepo.providerId || values.providerId),
      owner: parsedRepo.owner,
      repo: parsedRepo.repo,
      pollIntervalMinutes: Number(values.pollIntervalMinutes || state.data.pollMinutes),
      notify: true,
      enabled: true,
    };
    const existingIndex = state.data.subscriptions.findIndex((subscription) =>
      subscriptionKey(subscription) === subscriptionKey(nextSubscription)
    );
    state.data.dismissedWorkspaceRepos = (state.data.dismissedWorkspaceRepos || [])
      .filter((key) => key !== subscriptionKey(nextSubscription));
    if (existingIndex >= 0) {
      state.data.subscriptions[existingIndex] = {
        ...state.data.subscriptions[existingIndex],
        ...nextSubscription,
        enabled: true,
      };
    } else {
      state.data.subscriptions.push(nextSubscription);
    }
    state.ui.activeProviderId = nextSubscription.providerId;
    state.data.queueMode = 'all';
    state.ui.status = t('repoAddedSyncing');
    state.ui.error = null;
    void saveStorage();
    render();
    resetPollTimer();
    void syncQueue('all');
  }
  if (form.id === 'provider-form') {
    const id = String(values.displayName || values.webBaseUrl)
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '') || `provider-${Date.now()}`;
    state.data.profiles.push({
      id,
      kind: String(values.kind),
      displayName: String(values.displayName).trim(),
      webBaseUrl: normalizeBaseUrl(values.webBaseUrl),
      apiBaseUrl: normalizeBaseUrl(values.apiBaseUrl),
      credentialLabel: String(values.credentialLabel || ''),
      enabled: true,
    });
    state.ui.activeProviderId = id;
    void finish('statusSaved');
  }
});

async function init() {
  state.locale = app.locale || 'en-US';
  if (app.onLocaleChange) {
    app.onLocaleChange((locale) => {
      state.locale = locale || 'en-US';
      render();
    });
  }
  await loadStorage();
  render();
  resetPollTimer();
  void refreshQueueOnOpen();
}

void init();
