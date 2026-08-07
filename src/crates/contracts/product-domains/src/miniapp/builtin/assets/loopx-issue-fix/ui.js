/**
 * LoopX Issue-Fix MiniApp.
 *
 * Control surface over the host's native issue_fix_* bridge (window.app.issueFix).
 * The host owns scheduling (hidden heartbeat session + cron), the LoopX kernel
 * owns all repair state; this UI is a read-only projection plus typed commands,
 * mirroring the interaction rules refined in the native panel:
 *  - polls only the zero-write endpoint (issueFix.poll) on an interval;
 *  - a monotonic ticket guards every state apply so a slow response can never
 *    resurrect an answered gate;
 *  - polling pauses while a mutation is in flight.
 */
'use strict';

const POLL_INTERVAL_MS = 30_000;

const state = {
  repositoryPath: '',
  supported: null, // null=probing, false=non-GitHub/none, true=ok
  host: null,
  projectPath: null,
  readiness: null,
  issues: [],
  control: null,
  selected: new Set(),
  ticket: 0,
  appliedTicket: 0,
  mutationDepth: 0,
  starting: false,
  stopping: false,
  answering: false,
  activity: [],
};

const $ = (id) => document.getElementById(id);

// ── Bridge helpers ────────────────────────────────────────────────────────

function takeTicket() {
  return ++state.ticket;
}

function applyControl(ticket, control) {
  if (ticket < state.appliedTicket) return false;
  state.appliedTicket = ticket;
  state.control = control;
  return true;
}

async function refreshStatus() {
  const ticket = takeTicket();
  try {
    const status = await window.app.issueFix.status({});
    if (applyControl(ticket, status)) render();
  } catch (error) {
    showNotice(String(error?.message || error));
  }
}

async function pollLight() {
  if (document.hidden || state.mutationDepth > 0) return;
  const ticket = takeTicket();
  try {
    const poll = await window.app.issueFix.poll({});
    if (!poll || !state.control) return;
    if (
      applyControl(ticket, {
        ...state.control,
        actionRequired: poll.actionRequired,
        gatePrompt: poll.userQuestion ? poll.userQuestion.prompt : null,
        userQuestion: poll.userQuestion || null,
        issues: (poll.issues || []).map((issue) => ({
          ...issue,
          selected: issue.todoId === state.control.selectedTodoId,
        })),
        userTodos: poll.userTodos || [],
        hostLoop: poll.hostLoop,
      })
    ) {
      render();
    }
  } catch {
    // Polling is best-effort; the next tick retries.
  }
}

// ── Actions ───────────────────────────────────────────────────────────────

async function handleStart() {
  if (state.starting || !state.projectPath || state.selected.size === 0) return;
  state.starting = true;
  state.mutationDepth += 1;
  render();
  const ticket = takeTicket();
  try {
    const selectedIssues = state.issues
      .filter((issue) => state.selected.has(issue.issueId))
      .map((issue) => ({ issueRef: issue.issueId, issueUrl: issue.webUrl }));
    const started = await window.app.issueFix.start({
      repo: state.projectPath,
      issues: selectedIssues,
    });
    if (applyControl(ticket, started)) {
      state.selected.clear();
      pushActivity(`已启动：${started.addedIssueRefs?.length ?? 0} 个 issue 进入修复队列`);
    }
  } catch (error) {
    showNotice(String(error?.message || error));
  } finally {
    state.mutationDepth -= 1;
    state.starting = false;
    render();
  }
}

async function handleStop() {
  if (state.stopping) return;
  state.stopping = true;
  state.mutationDepth += 1;
  render();
  const ticket = takeTicket();
  try {
    const hostLoop = await window.app.issueFix.stop({});
    if (state.control && applyControl(ticket, { ...state.control, hostLoop })) {
      pushActivity('已停止心跳调度（修复进度保留，可随时重新启动）');
    }
  } catch (error) {
    showNotice(String(error?.message || error));
  } finally {
    state.mutationDepth -= 1;
    state.stopping = false;
    render();
  }
}

async function handleGateSubmit() {
  const question = state.control?.userQuestion;
  const decision = document.querySelector('input[name="gate-decision"]:checked')?.value;
  if (!question || !decision || state.answering) return;
  state.answering = true;
  state.mutationDepth += 1;
  render();
  const ticket = takeTicket();
  try {
    const answered = await window.app.issueFix.answer({
      todoId: question.todoId,
      decision,
      reason: $('gate-reason').value.trim() || null,
    });
    if (applyControl(ticket, answered)) {
      $('gate-reason').value = '';
      pushActivity(`已提交决定（${decision}）：${question.prompt.slice(0, 80)}`);
    }
  } catch (error) {
    showNotice(String(error?.message || error));
    // The gate may already be closed kernel-side; re-project truth.
    void refreshStatus();
  } finally {
    state.mutationDepth -= 1;
    state.answering = false;
    render();
  }
}

// ── Rendering ─────────────────────────────────────────────────────────────

function showNotice(text) {
  $('notice-text').textContent = text;
  $('notice').hidden = false;
}

function pushActivity(line) {
  const stamp = new Date().toLocaleTimeString();
  state.activity.unshift(`[${stamp}] ${line}`);
  state.activity = state.activity.slice(0, 200);
}

function rowState(issue, kernelTodo) {
  if (!kernelTodo) return state.selected.has(issue.issueId) ? 'queued' : 'idle';
  if (kernelTodo.status === 'done') return 'done';
  if (kernelTodo.status === 'blocked') return 'blocked';
  if (kernelTodo.selected) {
    if (state.control?.actionRequired) return 'blocked';
    if (state.control?.hostLoop?.enabled) return 'fixing';
  }
  return 'queued';
}

const ROW_LABEL = {
  idle: '',
  queued: '排队中',
  fixing: '修复中',
  done: '已处理',
  blocked: '待确认',
};

function render() {
  const control = state.control;
  const loop = control?.hostLoop;

  $('repo-label').textContent = state.projectPath || '--';
  $('kernel-label').textContent = control ? `${control.goalId} · ${control.kernelState}` : 'LoopX';

  const dot = $('loop-dot');
  const status = $('loop-status');
  if (loop?.enabled) {
    dot.className = loop.activeTurnId ? 'status-dot is-active' : 'status-dot is-idle';
    status.textContent = loop.activeTurnId ? '心跳执行中' : '持续修复运行中';
  } else {
    dot.className = 'status-dot';
    status.textContent = control ? '未运行' : '未连接';
  }

  $('stop-button').hidden = !loop?.enabled;
  $('stop-button').disabled = state.stopping || state.starting;
  $('stop-button').textContent = state.stopping ? '停止中…' : '停止';
  const startDisabled =
    state.starting || state.stopping || state.supported !== true || state.selected.size === 0
    || Boolean(control?.actionRequired);
  $('start-button').disabled = startDisabled;
  $('start-button').textContent = state.starting ? '启动中…' : '启动持续修复';

  // Gate card: reserved for genuinely blocking decisions, same as the panel.
  const question = control?.userQuestion;
  $('gate-card').hidden = !question;
  if (question) {
    $('gate-prompt').textContent = question.prompt;
    $('gate-submit').disabled =
      state.answering || !document.querySelector('input[name="gate-decision"]:checked');
    $('gate-submit').textContent = state.answering ? '提交中…' : '提交决定';
  }

  // Pending user-lane todos (merge reminders etc.) — ambient list, no actions.
  const todos = control?.userTodos || [];
  $('pending-card').hidden = todos.length === 0;
  $('pending-count').textContent = String(todos.length);
  const pendingList = $('pending-list');
  pendingList.replaceChildren(
    ...todos.map((todo) => {
      const li = document.createElement('li');
      const badge = document.createElement('span');
      badge.className = `pending-badge ${todo.taskClass === 'user_gate' ? 'is-gate' : 'is-action'}`;
      badge.textContent = todo.taskClass === 'user_gate' ? '待审' : '待办';
      const text = document.createElement('span');
      text.className = 'pending-text';
      text.textContent = todo.link ? todo.text.split(todo.link).join('').replace(/\(\s*\)/g, '').trim() : todo.text;
      li.append(badge, text);
      if (todo.link) {
        const link = document.createElement('a');
        link.href = todo.link;
        link.textContent = '打开';
        link.className = 'pending-link';
        link.addEventListener('click', (event) => {
          event.preventDefault();
          void window.app.call('host.openExternal', { url: todo.link }).catch(() => {});
        });
        li.append(link);
      }
      return li;
    }),
  );

  // Issue list.
  const kernelByRef = new Map((control?.issues || []).map((todo) => [todo.issueRef, todo]));
  const list = $('issue-list');
  const empty = $('issues-empty');
  if (state.supported === false) {
    empty.hidden = false;
    empty.textContent = state.host
      ? `当前仓库托管在 ${state.host}。持续修复目前仅支持 GitHub 仓库。`
      : '打开一个带 GitHub 远程仓库的工作区，即可持续修复其 Issue。';
    list.replaceChildren();
  } else if (state.issues.length === 0) {
    empty.hidden = false;
    empty.textContent = state.supported === null ? '正在加载…' : '没有开放的 Issue。';
    list.replaceChildren();
  } else {
    empty.hidden = true;
    list.replaceChildren(
      ...state.issues.map((issue) => {
        const kernelTodo = kernelByRef.get(issue.issueId);
        const rs = rowState(issue, kernelTodo);
        const li = document.createElement('li');
        li.className = `issue-row is-${rs}`;
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = Boolean(kernelTodo) || state.selected.has(issue.issueId);
        checkbox.disabled = Boolean(kernelTodo);
        checkbox.addEventListener('change', () => {
          if (checkbox.checked) state.selected.add(issue.issueId);
          else state.selected.delete(issue.issueId);
          render();
        });
        const num = document.createElement('span');
        num.className = 'issue-num';
        num.textContent = `#${issue.number}`;
        const title = document.createElement('span');
        title.className = 'issue-title';
        title.textContent = issue.title;
        title.title = issue.title;
        li.append(checkbox, num, title);
        if (ROW_LABEL[rs]) {
          const tag = document.createElement('span');
          tag.className = 'issue-state';
          tag.textContent = ROW_LABEL[rs];
          li.append(tag);
        }
        return li;
      }),
    );
  }
  $('selected-count').textContent = `已选 ${state.selected.size}`;
  const selectable = state.issues.filter((issue) => !kernelByRef.has(issue.issueId));
  $('select-all').checked =
    selectable.length > 0 && selectable.every((issue) => state.selected.has(issue.issueId));

  $('activity-meta').textContent = loop?.lastRunStatus
    ? `上次心跳：${loop.lastRunStatus}`
    : '--';
  if (state.activity.length > 0) {
    $('activity-log').textContent = state.activity.join('\n');
  }
  if (loop?.lastError) {
    showNotice(`心跳执行异常：${loop.lastError}`);
  }
}

// ── Boot ──────────────────────────────────────────────────────────────────

async function boot() {
  $('refresh-button').addEventListener('click', () => void reload());
  $('start-button').addEventListener('click', () => void handleStart());
  $('stop-button').addEventListener('click', () => void handleStop());
  $('gate-submit').addEventListener('click', () => void handleGateSubmit());
  $('notice-dismiss').addEventListener('click', () => { $('notice').hidden = true; });
  document.addEventListener('change', (event) => {
    if (event.target?.name === 'gate-decision') render();
  });
  $('select-all').addEventListener('change', () => {
    const kernelByRef = new Set((state.control?.issues || []).map((todo) => todo.issueRef));
    const selectable = state.issues.filter((issue) => !kernelByRef.has(issue.issueId));
    if ($('select-all').checked) selectable.forEach((issue) => state.selected.add(issue.issueId));
    else selectable.forEach((issue) => state.selected.delete(issue.issueId));
    render();
  });

  await reload();
  setInterval(() => void pollLight(), POLL_INTERVAL_MS);
}

async function reload() {
  try {
    const readiness = await window.app.issueFix.probe();
    state.readiness = readiness;
    if (!readiness.available) {
      state.supported = false;
      showNotice('LoopX 引擎不可用。重新安装 BitFun 可恢复内置引擎。');
      render();
      return;
    }
    if (readiness.ghInstalled === false) {
      showNotice('未安装 GitHub CLI（gh）。请从 https://cli.github.com 安装后重启应用。');
    } else if (readiness.ghAuthenticated === false) {
      showNotice('GitHub CLI 未登录。请在终端运行 gh auth login 后刷新。');
    }
    const listing = await window.app.issueFix.listIssues({});
    state.supported = listing.supported;
    state.host = listing.host;
    state.projectPath = listing.projectPath;
    state.issues = listing.issues || [];
    // Drop stale selections for issues that vanished from the refreshed list.
    const known = new Set(state.issues.map((issue) => issue.issueId));
    state.selected = new Set([...state.selected].filter((id) => known.has(id)));
    await refreshStatus();
  } catch (error) {
    showNotice(String(error?.message || error));
  }
  render();
}

boot();
