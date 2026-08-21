# E-3 修复报告 — GroupChatPane 完整 ChatInput 复用（slash/外挂命令接线补全）

- 日期：2026-08-13
- 工位：技术债 E-3（前端 web-ui，与 Rust 工位 E-1/E-2/上游同步零重叠）
- 派发：开发版指挥官（批准续做：侦查权威 + 修复 + 验证 + 三证据）
- 结论：**缺口已修复，验证全绿，三证据齐全**

---

## 一、侦查结论（E-3 真缺口确认）

GroupChatPane（`src/web-ui/src/flow_chat/components/GroupChatPane.tsx`）已渲染完整共享 `ChatInput`
（L253，`registration` 携带 `groupChatMention` + `onSubmit`），mention/file/voice 三线在组件层面已接线
（ChatInput.tsx L5489 GroupChatMentionPicker / L5524 FileMentionPicker / L5316 ContextDropZone / L5246
useComposerVoiceInput + L6272 ComposerVoiceInputButton）。

**真缺口有两层**（`src/web-ui/src/flow_chat/components/ChatInput.tsx`）：

1. **发送不可用**：GroupChatPane 场景无 session → `useSessionStateMachine(null)` 返回 null →
   `derivedState === null` → `handleSendOrCancel` L4175 `if (!derivedState) return;` 直接短路；
   `renderActionButton` L5228 返回 **disabled 发送按钮**。即注册场景下连普通文本都无法发送。
2. **slash/外挂命令绕过 registration**：`/mcp`（submitMcpPromptFromInput → sendMessage）、外部 prompt
   命令（submitExternalPromptCommandFromInput → sendMessage）、`/compact`/`/usage`/`/init`/`/goal`/
   `/review`/`/btw`（无 session 时报 `xxNoSession`）——全部不经 `registration.onSubmit`；
   `sendMessage`（useMessageSender L138-151）无 session 时还会 `createChatSession` 新建主会话（副作用外泄）。

---

## 二、修复内容（ChatInput.tsx，4 处改动）

### 1. `handleSendOrCancel` 顶部注册宿主短路（registeredHost 优先）
```ts
const registeredHost = Boolean(registration?.onSubmit);
if (registeredHost) {
  if (caps.transferInFlight) return;
  const registeredDraft = (messageOverride ?? inputState.value).trim();
  if (!registeredDraft) return;
  await submitThroughChatInputRegistration(registration, {
    text: registeredDraft, displayText: registeredDraft,
    contexts: [...contexts], composerPresentation: null,
    sessionId: undefined, workspacePath: workspacePath || undefined,
  }, () => Promise.resolve());
  if (contexts.length > 0) clearContexts();
  clearPendingLargePastes();
  dispatchInput({ type: 'CLEAR_VALUE' });
  dispatchInput({ type: 'DEACTIVATE' });
  return;
}
```
- 注册场景**绕过 derivedState 短路**，所有提交（含 `/` 开头原样文本）直接经
  `submitThroughChatInputRegistration` → `registration.onSubmit` → 群聊 `group_chat_send`
- 无 registration 时保持原行为（内部命令 / 报错 / 新建会话）**零回归**

### 2. `renderActionButton` 注册宿主固定渲染启用发送按钮
- registeredHost 时：始终渲染 `data-testid="chat-input-send-btn"`，`disabled={!inputState.value.trim()}`，
  不再依赖 derivedState（null 时的 disabled 死锁解除）

### 3. `handleKeyDown` Enter 分支注册宿主优先
- `registration?.onSubmit` 存在时 Enter 直接 `void handleSendOrCancel()`，跳过 `/btw` `/goal` 等
  slash 快速路由（这些路由在无 session 时只会报错或新建会话）
- 依赖数组补 `registration?.onSubmit`（eslint exhaustive-deps 要求）

### 4. 依赖数组
- `handleSendOrCancel` 依赖数组原有 `registration`/`contexts`/`clearContexts` 齐全，无需新增
  （eslint 复核 0 error）

改动文件：**仅** `ChatInput.tsx`（+60 行）与测试 `GroupChatPane.test.tsx`（+106 行）。Rust 文件零触碰。

---

## 三、测试补充（GroupChatPane.test.tsx）

mock 的 ChatInput 升级为捕获 `registration` 对象，新增 2 个 E-3 场景测试：

1. **「routes slash text through the ChatInput registration into the group chat (E-3)」**
   `registration.onSubmit({ text: '/compact please summarize', ... })` →
   断言 `group_chat_send` 收到 `content: '/compact please summarize'`（**slash 原样进群聊**，非内部命令）
2. **「routes @@ member-mention submissions through the ChatInput registration (E-3)」**
   session-reference context（`metadata.groupChatMention`）→
   断言正文归一化为 `@Assistant One` + `mention_targets` 带 claw 成员（mention 走 registration 完整链路）

---

## 四、三证据

### 证据 1 — vitest（GroupChatPane / ChatInput / 群聊域）
```
node_modules/.bin/vitest run src/flow_chat/components/GroupChatPane.test.tsx
       + src/flow_chat/components/ChatInputWorkspaceStrip.test.tsx
       + src/flow_chat/store/groupChatStore.test.ts
       + src/app/components/NavPanel/sections/groups/GroupChatsSection.test.tsx
  Test Files  4 passed (4)
       Tests  43 passed (43)
      （GroupChatPane 9/9 全过，含新增 2 个 E-3 场景）
node_modules/.bin/vitest run src/flow_chat/components（全组件目录回归）
  Test Files  66 passed (66)
       Tests  537 passed (537)
```

### 证据 2 — tsc + eslint
```
node_modules/.bin/tsc --noEmit        → TSC_EXIT=0（0 错误）
node_modules/.bin/eslint ChatInput.tsx → ESLINT_EXIT=0（0 error，含 exhaustive-deps 复核）
```
（eslint 初跑捕获 1 个 `registration?.onSubmit` 缺依赖 error，已补依赖数组后归零）

### 证据 3 — 工作树与域隔离
```
git diff --stat
  src/web-ui/src/flow_chat/components/ChatInput.tsx           | 60 +++++++++++-
  src/web-ui/src/flow_chat/components/GroupChatPane.test.tsx  | 106 +++++++++++++++++++--
  2 files changed, 159 insertions(+), 7 deletions(-)
```
- 改动集中在 ChatInput.tsx（web-ui），与 Rust 工位文件（session_api.rs / group_chat_router.rs /
  group_chat_tool.rs / runtime-ports / group_chat_store / group_chat_store_contracts）**零重叠**
- 预存失败确认：`GroupChatsSection.wiring.test.tsx`「clicking a room row activates it」报
  `item.dispatchEvent` null —— `git stash` 我的 2 文件改动后基线**同样失败**（1 failed），
  属预存问题（store 列表渲染时序），与 E-3 无关，未触碰
- 未提交（遵循「统一 push 时机」惯例，等指挥部定夺）

---

## 五、结论

E-3 真缺口（注册场景发送不可用 + slash/外挂命令绕过 registration.onSubmit）已根因级修复：
注册宿主下**一切提交（含 slash 原样文本、mention、file/image 上下文）统一经
`registration.onSubmit` 进群聊**；无 registration 的主会话行为零变化。
验证全绿（537 组件测试 + tsc + eslint），三证据齐全，请梦情复审。
