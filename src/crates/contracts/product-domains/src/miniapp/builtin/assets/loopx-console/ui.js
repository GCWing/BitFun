const CACHE_KEY = 'loopx-console-cache-v1';
const CACHE_VERSION = 1;
const COMMAND_TIMEOUT_MS = 60_000;

const copy = {
  'zh-CN': {
    title: 'LoopX 控制台',
    updated: '更新于 {time}',
    neverUpdated: '尚未更新',
    ready: '可用',
    refreshing: '正在刷新',
    cached: '正在显示缓存',
    unavailable: '不可用',
    retry: '重试',
    refresh: '刷新',
    goals: '目标',
    progress: '最近进展',
    gates: '用户 Gate',
    runnable: '可执行',
    waiting: '等待通道',
    currentWorkspace: '当前工作区',
    noWorkspace: '未打开工作区',
    localWorkspace: '本地工作区',
    remoteWorkspace: '远程工作区',
    matchedGoal: '已匹配 LoopX 目标',
    noMatchedGoal: '当前项目尚未接入 LoopX',
    remoteReadOnly: '远程工作区首版仅支持查看',
    globalReadOnly: '打开本地工作区后可执行',
    newGoal: '新建目标',
    all: '全部',
    action: '行动项',
    waitingFilter: '等待中',
    noGoals: '没有可显示的目标',
    selectGoal: '选择一个目标',
    selectGoalCopy: '查看它的当前通道和下一步安全动作。',
    current: '当前',
    continue: '继续执行',
    running: 'Agent 执行中',
    status: '状态',
    waitingOn: '等待对象',
    adapter: '适配器',
    nextAction: '下一步安全动作',
    noNextAction: 'LoopX 暂未提供下一步动作。',
    userGate: '用户 Gate',
    visibleWork: '可见待办',
    handoff: 'Agent 交接包',
    reload: '重新加载',
    detailLoading: '正在读取只读交接包...',
    detailUnavailable: '交接包暂不可用，可重试加载。',
    recentProgress: '最近进展',
    startGoal: '新建 LoopX 目标',
    goal: '目标描述',
    goalPlaceholder: '描述你希望在当前项目持续推进的目标',
    cancel: '取消',
    startInAgent: '交给 Agent',
    close: '关闭',
    goalRequired: '请输入目标描述。',
    loopxMissing: '未找到 LoopX。请先安装或修复 PATH，然后重试。',
    incompatible: 'LoopX JSON 契约版本不兼容。已保留上一次成功数据。',
    invalidData: 'LoopX 返回了无效数据。已保留上一次成功数据。',
    refreshFailed: '刷新失败。已保留上一次成功数据。',
    noCacheSuffix: '当前没有可用缓存。',
    agentStarted: '已切换到 Agent 会话',
    agentFailed: 'Agent 启动失败，请重试。',
    currentOnly: '请先切换到该目标对应的本地工作区。',
    cacheRestored: '已恢复上次成功数据，正在后台刷新。',
    composerPlaceholder: '让 Agent 按 LoopX 控制面推进当前目标',
  },
  'en-US': {
    title: 'LoopX Console',
    updated: 'Updated {time}',
    neverUpdated: 'Not updated yet',
    ready: 'Ready',
    refreshing: 'Refreshing',
    cached: 'Showing cache',
    unavailable: 'Unavailable',
    retry: 'Retry',
    refresh: 'Refresh',
    goals: 'Goals',
    progress: 'Recent progress',
    gates: 'User gates',
    runnable: 'Runnable',
    waiting: 'Waiting lanes',
    currentWorkspace: 'Current workspace',
    noWorkspace: 'No workspace open',
    localWorkspace: 'Local workspace',
    remoteWorkspace: 'Remote workspace',
    matchedGoal: 'Matched LoopX goal',
    noMatchedGoal: 'This project is not connected to LoopX',
    remoteReadOnly: 'Remote workspaces are read-only in this release',
    globalReadOnly: 'Open a local workspace to run actions',
    newGoal: 'New goal',
    all: 'All',
    action: 'Action',
    waitingFilter: 'Waiting',
    noGoals: 'No goals to show',
    selectGoal: 'Select a goal',
    selectGoalCopy: 'Inspect its current lane and next safe action.',
    current: 'Current',
    continue: 'Continue',
    running: 'Agent is running',
    status: 'Status',
    waitingOn: 'Waiting on',
    adapter: 'Adapter',
    nextAction: 'Next safe action',
    noNextAction: 'LoopX has not provided a next action.',
    userGate: 'User gate',
    visibleWork: 'Visible work',
    handoff: 'Agent handoff',
    reload: 'Reload',
    detailLoading: 'Loading the read-only handoff packet...',
    detailUnavailable: 'The handoff packet is unavailable. Retry when ready.',
    recentProgress: 'Recent progress',
    startGoal: 'Start a LoopX goal',
    goal: 'Goal',
    goalPlaceholder: 'Describe the durable goal for this project',
    cancel: 'Cancel',
    startInAgent: 'Start in Agent',
    close: 'Close',
    goalRequired: 'Enter a goal.',
    loopxMissing: 'LoopX was not found. Install it or repair PATH, then retry.',
    incompatible: 'The LoopX JSON contract is incompatible. The last successful data is preserved.',
    invalidData: 'LoopX returned invalid data. The last successful data is preserved.',
    refreshFailed: 'Refresh failed. The last successful data is preserved.',
    noCacheSuffix: 'No cached data is available.',
    agentStarted: 'Focused the Agent session',
    agentFailed: 'Could not start the Agent. Retry when ready.',
    currentOnly: 'Switch to this goal\'s local workspace before continuing.',
    cacheRestored: 'Restored the last successful data and refreshing in the background.',
    composerPlaceholder: 'Ask the Agent to advance the current goal through LoopX',
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
  filter: 'all',
  packets: new Map(),
  refreshing: false,
  running: null,
  notice: null,
};

const dom = {};

function $(id) {
  return document.getElementById(id);
}

function t(key, values = {}) {
  const dictionary = copy[state.locale] || copy['en-US'];
  let value = dictionary[key] || copy['en-US'][key] || key;
  for (const [name, replacement] of Object.entries(values)) {
    value = value.replace(`{${name}}`, String(replacement));
  }
  return value;
}

function resolveLocale(locale) {
  const normalized = String(locale || '').toLowerCase();
  if (normalized.startsWith('zh')) return 'zh-CN';
  return 'en-US';
}

function createElement(tagName, className, text) {
  const element = document.createElement(tagName);
  if (className) element.className = className;
  if (text !== undefined && text !== null) element.textContent = String(text);
  return element;
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function numberOrZero(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function formatTimestamp(value) {
  if (!value) return '--';
  return String(value)
    .replace('T', ' ')
    .replace(/:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/, '');
}

function normalizePath(value) {
  let path = String(value || '').trim().replace(/\//g, '\\');
  while (path.length > 3 && path.endsWith('\\')) path = path.slice(0, -1);
  if (/^[A-Za-z]:\\/.test(path)) path = path.toLowerCase();
  return path;
}

function selectedGoal() {
  return asArray(state.registry?.goals).find((goal) => goal.id === state.selectedGoalId) || null;
}

function currentGoal() {
  if (!state.workspace.available || state.workspace.isRemote) return null;
  const workspacePath = normalizePath(state.workspace.path);
  if (!workspacePath) return null;
  return asArray(state.registry?.goals).find((goal) => normalizePath(goal.repo) === workspacePath) || null;
}

function goalFacts(goalId) {
  const lanes = asArray(state.summary?.lanes);
  const groups = state.summary?.groups || {};
  return {
    lane: lanes.find((item) => item.goal_id === goalId) || null,
    gates: asArray(state.summary?.gates || groups.user_gates).filter((item) => item.goal_id === goalId),
    todos: asArray(state.summary?.todos || groups.runnable_agent_work).filter((item) => item.goal_id === goalId),
    progress: asArray(state.summary?.recent_progress || groups.recent_progress).filter(
      (item) => item.goal_id === goalId,
    ),
  };
}

function goalCategory(goal) {
  const facts = goalFacts(goal.id);
  const status = String(facts.lane?.status || '').toLowerCase();
  const waitingOn = String(facts.lane?.waiting_on || goal.waiting_on || '').toLowerCase();
  if (facts.gates.length || status === 'operator_gate' || (waitingOn && waitingOn !== 'codex')) {
    return 'waiting';
  }
  if (facts.todos.length || status === 'eligible' || waitingOn === 'codex') return 'action';
  return 'neutral';
}

function isLoopxMissing(error) {
  const message = String(error?.message || error || '').toLowerCase();
  return (
    message.includes('not found') ||
    message.includes('not recognized') ||
    message.includes('cannot find') ||
    message.includes('enoent')
  );
}

function schemaError() {
  const error = new Error('LOOPX_SCHEMA_INCOMPATIBLE');
  error.code = 'schema';
  return error;
}

function invalidDataError() {
  const error = new Error('LOOPX_INVALID_DATA');
  error.code = 'invalid';
  return error;
}

async function runLoopx(args, timeout = COMMAND_TIMEOUT_MS) {
  const result = await window.app.shell.exec(['loopx', ...args], {
    cwd: window.app.appDataDir,
    timeout,
  });
  return String(result?.stdout || '').trim();
}

async function runLoopxJson(args) {
  const stdout = await runLoopx(args);
  try {
    return JSON.parse(stdout.replace(/^\uFEFF/, ''));
  } catch {
    throw invalidDataError();
  }
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
  if (
    value?.ok !== true ||
    value?.goal_id !== goalId ||
    value?.handoff_only !== true ||
    (typeof value.handoff_text !== 'string' && typeof value.project_agent_handoff !== 'string')
  ) {
    throw schemaError();
  }
  return value;
}

async function readCache() {
  try {
    let cached = await window.app.storage.get(CACHE_KEY);
    if (typeof cached === 'string') cached = JSON.parse(cached);
    if (
      cached?.cacheVersion === CACHE_VERSION &&
      cached.registry?.schema_version === '0.1' &&
      cached.summary?.schema_version === 'global_manager_command_response_v0'
    ) {
      state.version = String(cached.version || '');
      state.registry = cached.registry;
      state.summary = cached.summary;
      state.fetchedAt = String(cached.fetchedAt || '');
      state.selectedGoalId = cached.selectedGoalId || null;
      state.notice = { kind: 'cache', message: t('cacheRestored') };
      return true;
    }
  } catch {
    // A broken cache is ignored; the live read below remains authoritative.
  }
  return false;
}

async function writeCache() {
  await window.app.storage.set(CACHE_KEY, {
    cacheVersion: CACHE_VERSION,
    version: state.version,
    registry: state.registry,
    summary: state.summary,
    fetchedAt: state.fetchedAt,
    selectedGoalId: state.selectedGoalId,
  });
}

function setNotice(kind, message) {
  state.notice = message ? { kind, message } : null;
  renderNotice();
}

function renderNotice() {
  if (!state.notice) {
    dom.notice.hidden = true;
    return;
  }
  dom.notice.hidden = false;
  dom.notice.dataset.kind = state.notice.kind;
  dom.noticeText.textContent = state.notice.message;
  dom.noticeRetry.textContent = t('retry');
  dom.noticeRetry.hidden = state.notice.kind === 'cache';
}

function renderRuntime() {
  dom.runtimeVersion.textContent = state.version || 'LoopX';
  dom.updatedAt.textContent = state.fetchedAt
    ? t('updated', { time: formatTimestamp(state.fetchedAt) })
    : t('neverUpdated');
  dom.refreshButton.disabled = state.refreshing;
  dom.refreshButton.classList.toggle('loading', state.refreshing);
  dom.refreshButton.title = t('refresh');
  dom.refreshButton.setAttribute('aria-label', t('refresh'));
  dom.runtimeDot.className = 'status-dot';
  if (state.refreshing) {
    dom.runtimeStatus.textContent = t('refreshing');
  } else if (state.registry && state.summary) {
    dom.runtimeStatus.textContent = state.notice?.kind === 'error' ? t('cached') : t('ready');
    dom.runtimeDot.classList.add('online');
  } else {
    dom.runtimeStatus.textContent = t('unavailable');
    dom.runtimeDot.classList.add('error');
  }
}

function renderMetrics() {
  const metrics = state.summary?.summary || {};
  dom.metricGoals.textContent = state.registry ? numberOrZero(state.registry.goal_count) : '--';
  dom.metricProgress.textContent = state.summary ? numberOrZero(metrics.progress_count) : '--';
  dom.metricGates.textContent = state.summary ? numberOrZero(metrics.open_gate_count) : '--';
  dom.metricRunnable.textContent = state.summary ? numberOrZero(metrics.runnable_todo_count) : '--';
  dom.metricWaiting.textContent = state.summary ? numberOrZero(metrics.waiting_lane_count) : '--';
}

function renderWorkspace() {
  const workspace = state.workspace;
  const matched = currentGoal();
  dom.workspaceKicker.textContent = t('currentWorkspace');
  dom.workspaceName.textContent = workspace.available ? workspace.name || t('localWorkspace') : t('noWorkspace');
  dom.workspacePath.textContent = workspace.available ? workspace.path || '--' : '--';
  dom.newGoalButton.hidden = true;
  if (!workspace.available) {
    dom.workspaceState.textContent = t('globalReadOnly');
  } else if (workspace.isRemote) {
    dom.workspaceState.textContent = t('remoteReadOnly');
  } else if (matched) {
    dom.workspaceState.textContent = `${t('matchedGoal')}: ${matched.id}`;
  } else {
    dom.workspaceState.textContent = t('noMatchedGoal');
    dom.newGoalButton.hidden = false;
    dom.newGoalButton.disabled = Boolean(state.running);
  }
}

function renderGoals() {
  const goals = asArray(state.registry?.goals);
  const filtered = goals.filter((goal) => state.filter === 'all' || goalCategory(goal) === state.filter);
  const matched = currentGoal();
  dom.goalCount.textContent = String(filtered.length);
  dom.goalsList.replaceChildren();

  for (const button of dom.goalFilters.querySelectorAll('button')) {
    button.classList.toggle('active', button.dataset.filter === state.filter);
  }

  if (!filtered.length) {
    dom.goalsList.append(createElement('div', 'empty-state', t('noGoals')));
    return;
  }

  for (const goal of filtered) {
    const facts = goalFacts(goal.id);
    const category = goalCategory(goal);
    const item = createElement('button', 'goal-item');
    item.type = 'button';
    item.classList.toggle('selected', goal.id === state.selectedGoalId);
    item.classList.toggle('current', goal.id === matched?.id);
    item.dataset.goalId = goal.id;

    const primary = createElement('span', 'goal-primary');
    primary.append(createElement('span', 'goal-title', goal.id));
    const status = createElement(
      'span',
      `goal-status ${category}`,
      facts.lane?.status || goal.status || '--',
    );
    primary.append(status);
    item.append(primary);
    item.append(createElement('span', 'goal-path', goal.repo || '--'));
    const meta = createElement('span', 'goal-meta');
    meta.append(createElement('span', '', facts.lane?.waiting_on || goal.waiting_on || goal.domain || '--'));
    if (facts.gates.length) meta.append(createElement('span', '', `${facts.gates.length} gate`));
    if (facts.todos.length) meta.append(createElement('span', '', `${facts.todos.length} todo`));
    item.append(meta);
    dom.goalsList.append(item);
  }
}

function renderList(container, items, className, formatter) {
  container.replaceChildren();
  for (const item of items) {
    container.append(createElement('div', `detail-list-item ${className}`, formatter(item)));
  }
}

function renderDetail() {
  const goal = selectedGoal();
  dom.detailEmpty.hidden = Boolean(goal);
  dom.detailContent.hidden = !goal;
  if (!goal) return;

  const facts = goalFacts(goal.id);
  const matched = currentGoal();
  const isCurrent = matched?.id === goal.id;
  const packetState = state.packets.get(goal.id);
  dom.detailHeading.textContent = goal.id;
  dom.detailRepo.textContent = goal.repo || '--';
  dom.detailCurrent.hidden = !isCurrent;
  dom.detailCurrent.textContent = t('current');
  dom.detailStatus.textContent = facts.lane?.status || goal.status || '--';
  dom.detailWaiting.textContent = facts.lane?.waiting_on || goal.waiting_on || '--';
  dom.detailAdapter.textContent = goal.adapter_kind || '--';
  dom.nextActionText.textContent =
    facts.lane?.next_safe_action || goal.recommended_action || goal.next_probe || t('noNextAction');

  dom.gateSection.hidden = facts.gates.length === 0;
  dom.gateCount.textContent = String(facts.gates.length);
  renderList(dom.gateList, facts.gates, 'gate', (item) => item.question || item.next_safe_action || item.gate_id);

  dom.todoSection.hidden = facts.todos.length === 0;
  dom.todoCount.textContent = String(facts.todos.length);
  renderList(
    dom.todoList,
    facts.todos,
    'todo',
    (item) => [item.priority, item.title || item.next_safe_action || item.top_todo_id || item.todo_id]
      .filter(Boolean)
      .join(' '),
  );

  if (packetState?.status === 'ready') {
    dom.handoffContent.textContent =
      packetState.packet.handoff_text || packetState.packet.project_agent_handoff || '--';
  } else if (packetState?.status === 'error') {
    dom.handoffContent.textContent = t('detailUnavailable');
  } else {
    dom.handoffContent.textContent = t('detailLoading');
  }

  dom.progressSection.hidden = facts.progress.length === 0;
  dom.progressList.replaceChildren();
  for (const progress of facts.progress) {
    const item = createElement('div', 'timeline-item');
    item.append(createElement('time', '', formatTimestamp(progress.generated_at)));
    item.append(
      createElement('div', '', progress.recommended_action || progress.classification || '--'),
    );
    dom.progressList.append(item);
  }

  dom.continueButton.hidden = false;
  dom.continueButton.disabled = !isCurrent || state.workspace.isRemote || Boolean(state.running);
  dom.continueButton.title = isCurrent ? '' : t('currentOnly');
  dom.continueLabel.textContent = state.running?.goalId === goal.id ? t('running') : t('continue');
}

function applyCopy() {
  document.documentElement.lang = state.locale;
  dom.appTitle.textContent = t('title');
  dom.metricGoalsLabel.textContent = t('goals');
  dom.metricProgressLabel.textContent = t('progress');
  dom.metricGatesLabel.textContent = t('gates');
  dom.metricRunnableLabel.textContent = t('runnable');
  dom.metricWaitingLabel.textContent = t('waiting');
  dom.newGoalLabel.textContent = t('newGoal');
  dom.goalsHeading.textContent = t('goals');
  const filterButtons = dom.goalFilters.querySelectorAll('button');
  filterButtons[0].textContent = t('all');
  filterButtons[1].textContent = t('action');
  filterButtons[2].textContent = t('waitingFilter');
  dom.emptyTitle.textContent = t('selectGoal');
  dom.emptyCopy.textContent = t('selectGoalCopy');
  dom.statusLabel.textContent = t('status');
  dom.waitingLabel.textContent = t('waitingOn');
  dom.adapterLabel.textContent = t('adapter');
  dom.nextActionHeading.textContent = t('nextAction');
  dom.gateHeading.textContent = t('userGate');
  dom.todoHeading.textContent = t('visibleWork');
  dom.handoffHeading.textContent = t('handoff');
  dom.reloadDetailButton.textContent = t('reload');
  dom.progressHeading.textContent = t('recentProgress');
  dom.dialogTitle.textContent = t('startGoal');
  dom.goalTextLabel.textContent = t('goal');
  dom.goalText.placeholder = t('goalPlaceholder');
  dom.dialogCancel.textContent = t('cancel');
  dom.dialogSubmitLabel.textContent = t('startInAgent');
  dom.dialogClose.title = t('close');
  dom.dialogClose.setAttribute('aria-label', t('close'));
}

function render() {
  applyCopy();
  renderNotice();
  renderRuntime();
  renderMetrics();
  renderWorkspace();
  renderGoals();
  renderDetail();
}

async function loadPacket(goalId, force = false) {
  if (!goalId || (!force && state.packets.has(goalId))) return;
  state.packets.set(goalId, { status: 'loading' });
  renderDetail();
  try {
    const packet = validatePacket(
      await runLoopxJson(['--format', 'json', 'review-packet', '--goal-id', goalId, '--handoff-only', '--limit', '5']),
      goalId,
    );
    state.packets.set(goalId, { status: 'ready', packet });
  } catch (error) {
    state.packets.set(goalId, { status: 'error', error });
  }
  renderDetail();
}

function refreshFailureMessage(error) {
  const suffix = state.registry && state.summary ? '' : ` ${t('noCacheSuffix')}`;
  if (error?.code === 'schema') return `${t('incompatible')}${suffix}`;
  if (error?.code === 'invalid') return `${t('invalidData')}${suffix}`;
  if (isLoopxMissing(error)) return `${t('loopxMissing')}${suffix}`;
  return `${t('refreshFailed')}${suffix}`;
}

async function refresh() {
  if (state.refreshing) return;
  state.refreshing = true;
  renderRuntime();
  try {
    const [version, registryValue] = await Promise.all([
      runLoopx(['--version'], 20_000),
      runLoopxJson(['--format', 'json', 'registry']),
    ]);
    const registry = validateRegistry(registryValue);
    const summary = validateSummary(
      await runLoopxJson(['--format', 'json', 'global-summary', '--time-range', '24h', '--limit', '8']),
    );

    state.version = version;
    state.registry = registry;
    state.summary = summary;
    state.fetchedAt = summary.generated_at || new Date().toISOString();
    state.notice = null;
    const goals = asArray(registry.goals);
    const matched = currentGoal();
    if (!goals.some((goal) => goal.id === state.selectedGoalId)) {
      state.selectedGoalId = matched?.id || goals[0]?.id || null;
    }
    await writeCache().catch(() => {
      // Live data remains authoritative even when the optional cache cannot be persisted.
    });
    if (state.selectedGoalId) void loadPacket(state.selectedGoalId, true);
  } catch (error) {
    state.notice = { kind: 'error', message: refreshFailureMessage(error) };
  } finally {
    state.refreshing = false;
    render();
  }
}

async function claimComposer() {
  try {
    await window.app.chat.claimComposer({
      title: t('title'),
      composer: { placeholder: t('composerPlaceholder') },
    });
  } catch {
    // The console remains useful as a read-only dashboard without composer ownership.
  }
}

async function startAgent(prompt, displayText, sessionName, goalId) {
  const result = await window.app.agent.run(prompt, {
    sessionName,
    displayText,
    enableTools: true,
  });
  if (!result?.sessionId || !result?.turnId) throw new Error('AGENT_SESSION_UNAVAILABLE');
  state.running = { sessionId: result.sessionId, turnId: result.turnId, goalId };
  await window.app.chat.focusSession(result.sessionId);
  setNotice('agent', t('agentStarted'));
  render();
}

function newGoalPrompt(goalText) {
  return `You are operating inside the current BitFun local workspace.
The host surface is BitFun's interactive chat-box, not Codex App.
LoopX is the control-plane source of truth. Do not directly edit .loopx files, goal state files, todo files, or the global registry.

Start the following durable goal by running LoopX's guided flow in the current project:
loopx start-goal --guided --project . --goal-text <the exact goal text below> --host-surface chat-box

<goal-text>
${goalText}
</goal-text>

Pass the goal text exactly as provided. Preserve every LoopX goal-selection, approval, and write gate. Do not confirm or approve any gate on the user's behalf. If LoopX asks for a decision, stop and surface that decision in this interactive chat. Only after the guided flow permits work, complete one bounded progress segment with an implementation artifact, targeted validation, and LoopX state writeback.`;
}

function continueGoalPrompt(goalId) {
  return `You are operating inside the current BitFun local workspace.
The host surface is BitFun's interactive chat-box, not Codex App.
LoopX is the control-plane source of truth for goal_id=${goalId}. Do not directly edit .loopx files, goal state files, todo files, or the global registry.

First read the current handoff with this exact read-only command:
loopx --format json review-packet --goal-id ${goalId} --handoff-only --limit 5

Confirm the packet matches goal_id=${goalId}, then follow its current gate, owner, stop condition, quota, and next-action guidance. Never infer or auto-approve a user/controller gate. Complete exactly one bounded progress segment in this workspace, including a coherent implementation artifact, targeted validation, and LoopX writeback through the LoopX CLI. If the packet blocks execution or requires user authority, stop and ask in this interactive chat.`;
}

async function submitNewGoal() {
  const goalText = dom.goalText.value.trim();
  if (!goalText) {
    dom.goalText.setCustomValidity(t('goalRequired'));
    dom.goalText.reportValidity();
    return;
  }
  dom.goalText.setCustomValidity('');
  if (!state.workspace.available || state.workspace.isRemote || state.running) return;
  dom.dialogSubmit.disabled = true;
  try {
    await startAgent(newGoalPrompt(goalText), goalText, `${t('title')}: ${state.workspace.name}`, null);
    dom.newGoalDialog.close();
    dom.goalText.value = '';
  } catch {
    setNotice('error', t('agentFailed'));
  } finally {
    dom.dialogSubmit.disabled = false;
  }
}

async function continueGoal() {
  const goal = selectedGoal();
  if (!goal || currentGoal()?.id !== goal.id || state.workspace.isRemote || state.running) return;
  try {
    await loadPacket(goal.id, true);
    const packetState = state.packets.get(goal.id);
    if (packetState?.status !== 'ready') throw new Error('HANDOFF_UNAVAILABLE');
    await startAgent(
      continueGoalPrompt(goal.id),
      goalFacts(goal.id).lane?.next_safe_action || `${t('continue')}: ${goal.id}`,
      `${t('title')}: ${goal.id}`,
      goal.id,
    );
  } catch {
    setNotice('error', t('agentFailed'));
  }
}

function onAgentEvent(event) {
  if (!state.running || !event || typeof event !== 'object') return;
  const sourceEvent = String(event.sourceEvent || event.source_event || event.type || '');
  const sessionId = event.sessionId || event.session_id;
  const turnId = event.turnId || event.turn_id;
  if (sessionId && sessionId !== state.running.sessionId) return;
  if (turnId && turnId !== state.running.turnId) return;

  if (sourceEvent.endsWith('dialog-turn-completed')) {
    state.running = null;
    setNotice(null, null);
    render();
    void refresh();
  } else if (
    sourceEvent.endsWith('dialog-turn-failed') ||
    sourceEvent.endsWith('dialog-turn-cancelled')
  ) {
    state.running = null;
    setNotice('error', t('agentFailed'));
    render();
  }
}

function bindEvents() {
  dom.refreshButton.addEventListener('click', () => void refresh());
  dom.noticeRetry.addEventListener('click', () => void refresh());
  dom.goalFilters.addEventListener('click', (event) => {
    const button = event.target.closest('button[data-filter]');
    if (!button) return;
    state.filter = button.dataset.filter;
    renderGoals();
  });
  dom.goalsList.addEventListener('click', (event) => {
    const button = event.target.closest('[data-goal-id]');
    if (!button) return;
    state.selectedGoalId = button.dataset.goalId;
    renderGoals();
    renderDetail();
    void loadPacket(state.selectedGoalId);
  });
  dom.reloadDetailButton.addEventListener('click', () => void loadPacket(state.selectedGoalId, true));
  dom.newGoalButton.addEventListener('click', () => {
    dom.dialogWorkspace.textContent = state.workspace.path;
    dom.newGoalDialog.showModal();
    dom.goalText.focus();
  });
  dom.dialogClose.addEventListener('click', () => dom.newGoalDialog.close());
  dom.dialogCancel.addEventListener('click', () => dom.newGoalDialog.close());
  dom.newGoalForm.addEventListener('submit', (event) => {
    event.preventDefault();
    void submitNewGoal();
  });
  dom.goalText.addEventListener('input', () => dom.goalText.setCustomValidity(''));
  dom.continueButton.addEventListener('click', () => void continueGoal());
  window.app.agent.onEvent(onAgentEvent);
  window.app.onLocaleChange((locale) => {
    state.locale = resolveLocale(locale);
    render();
    void claimComposer();
  });
}

function collectDom() {
  const ids = [
    'app-title', 'runtime-version', 'updated-at', 'runtime-status', 'runtime-dot', 'refresh-button',
    'notice', 'notice-text', 'notice-retry', 'metric-goals', 'metric-goals-label', 'metric-progress',
    'metric-progress-label', 'metric-gates', 'metric-gates-label', 'metric-runnable',
    'metric-runnable-label', 'metric-waiting', 'metric-waiting-label', 'workspace-kicker',
    'workspace-name', 'workspace-path', 'workspace-state', 'new-goal-button', 'new-goal-label',
    'goals-heading', 'goal-count', 'goal-filters', 'goals-list', 'detail-empty', 'detail-content',
    'empty-title', 'empty-copy', 'detail-heading', 'detail-current', 'detail-repo', 'continue-button',
    'continue-label', 'status-label', 'detail-status', 'waiting-label', 'detail-waiting', 'adapter-label',
    'detail-adapter', 'next-action-heading', 'next-action-text', 'gate-section', 'gate-heading',
    'gate-count', 'gate-list', 'todo-section', 'todo-heading', 'todo-count', 'todo-list',
    'handoff-heading', 'reload-detail-button', 'handoff-content', 'progress-section',
    'progress-heading', 'progress-list', 'new-goal-dialog', 'new-goal-form', 'dialog-title',
    'dialog-close', 'goal-text-label', 'goal-text', 'dialog-workspace', 'dialog-cancel',
    'dialog-submit', 'dialog-submit-label',
  ];
  for (const id of ids) {
    const key = id.replace(/-([a-z])/g, (_, char) => char.toUpperCase());
    dom[key] = $(id);
  }
}

async function initialize() {
  collectDom();
  state.locale = resolveLocale(window.app.locale);
  bindEvents();
  const [hadCache, workspace] = await Promise.all([
    readCache(),
    window.app.workspace.info().catch(() => state.workspace),
  ]);
  state.workspace = workspace || state.workspace;
  const goals = asArray(state.registry?.goals);
  const matched = currentGoal();
  if (!goals.some((goal) => goal.id === state.selectedGoalId)) {
    state.selectedGoalId = matched?.id || goals[0]?.id || null;
  }
  render();
  await claimComposer();
  if (!hadCache) setNotice(null, null);
  void refresh();
  if (state.selectedGoalId) void loadPacket(state.selectedGoalId);
}

void initialize();
