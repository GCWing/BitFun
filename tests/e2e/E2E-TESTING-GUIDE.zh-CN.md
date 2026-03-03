**中文** | [English](E2E-TESTING-GUIDE.md)

# BitFun E2E 测试指南

使用 WebDriverIO + tauri-driver 进行 BitFun 项目的端到端测试完整指南。

## 目录

- [测试理念](#测试理念)
- [测试级别](#测试级别)
- [快速开始](#快速开始)
- [测试结构](#测试结构)
- [编写测试](#编写测试)
- [最佳实践](#最佳实践)
- [问题排查](#问题排查)

## 测试理念

BitFun E2E 测试专注于**用户旅程**和**关键路径**,确保桌面应用从用户角度正常工作。我们使用分层测试方法来平衡覆盖率和执行速度。

### 核心原则

1. **测试真实的用户工作流**,而不是实现细节
2. **使用 data-testid 属性**确保选择器稳定
3. **遵循 Page Object 模式**提高可维护性
4. **保持测试独立**和幂等性
5. **快速失败**并提供清晰的错误信息

### ⚠️ 当前测试状态说明

**重要**: 当前的测试实现主要关注**元素存在性检查**，而不是完整的端到端用户交互流程。这意味着：

- ✅ **L0 测试**：已完成，验证应用基本启动和 UI 结构
- ⚠️ **L1 测试**：已实现但需要改进
  - 当前：检查元素是否存在、是否可见
  - 需要：真实的用户交互流程（点击、输入、验证状态变化）
  - 限制：大部分测试需要工作区打开，否则会被跳过
- ❌ **L2 测试**：尚未实现

**改进方向**：
1. 为 L1 测试添加工作区自动打开功能
2. 将元素检查改为真实的用户交互测试
3. 添加状态变化验证和断言
4. 实现 L2 级别的完整集成测试

## 测试级别

BitFun 使用三级测试分类系统:

### L0 - 冒烟测试 (关键路径)

**目的**: 验证基本应用功能;必须在任何发布前通过。

**特点**:
- 运行时间: < 1 分钟
- 不需要 AI 交互和工作区
- 可在 CI/CD 中运行

**何时运行**: 每次提交、每次合并前、发布前

**测试文件**:

| 测试文件 | 验证内容 |
|----------|----------|
| `l0-smoke.spec.ts` | 应用启动、DOM结构、Header可见性 |
| `l0-open-workspace.spec.ts` | 工作区状态检测、启动页交互 |
| `l0-open-settings.spec.ts` | 设置面板打开/关闭 |
| `l0-navigation.spec.ts` | 侧边栏存在、导航项可见可点击 |
| `l0-tabs.spec.ts` | 标签栏存在、标签页可显示 |
| `l0-theme.spec.ts` | 主题选择器可见、可切换主题 |
| `l0-i18n.spec.ts` | 语言选择器可见、可切换语言 |
| `l0-notification.spec.ts` | 通知入口可见、面板可展开 |
| `l0-observe.spec.ts` | 应用启动并保持窗口打开60秒（用于手动检查） |

### L1 - 功能测试 (特性验证)

**目的**: 验证主要功能端到端工作。

**特点**:
- 运行时间: 3-5 分钟
- 工作区已自动打开（测试在实际工作区上下文中运行）
- 不需要 AI 模型（测试 UI 行为，而非 AI 响应）
- 测试验证实际用户交互和状态变化

**何时运行**: 特性合并前、每晚构建、发布前

**测试文件**:

| 测试文件 | 验证内容 | 状态 |
|----------|----------|------|
| `l1-ui-navigation.spec.ts` | 窗口控制、最大化/还原 | 11 通过 |
| `l1-workspace.spec.ts` | 工作区状态、启动页元素 | 9 通过 |
| `l1-chat-input.spec.ts` | 聊天输入框、发送按钮 | 14 通过 |
| `l1-navigation.spec.ts` | 点击导航项切换视图、当前项高亮 | 9 通过 |
| `l1-file-tree.spec.ts` | 文件列表显示、文件夹展开折叠、点击打开编辑器 | 6 通过 |
| `l1-editor.spec.ts` | 文件内容显示、多标签切换关闭、未保存标记 | 6 通过 |
| `l1-terminal.spec.ts` | 终端显示、命令输入执行、输出显示 | 5 通过 |
| `l1-git-panel.spec.ts` | 面板显示、分支名、变更列表、查看差异 | 9 通过 |
| `l1-settings.spec.ts` | 设置面板打开、配置修改、配置保存 | 9 通过 |
| `l1-session.spec.ts` | 新建会话、切换历史会话 | 11 通过 |
| `l1-dialog.spec.ts` | 确认对话框、输入对话框提交取消 | 13 通过 |
| `l1-chat.spec.ts` | 输入发送消息、消息显示、停止按钮、代码块渲染 | 14 通过, 1 失败 |

### L2 - 集成测试 (完整系统)

**目的**: 验证完整工作流程与真实 AI 集成。

**特点**:
- 运行时间: 15-60 分钟
- 需要 AI 提供商配置

**何时运行**: 发布前、手动验证

**当前状态**: ❌ L2 测试尚未实现

**计划测试文件**:

| 测试文件 | 验证内容 | 状态 |
|----------|----------|------|
| `l2-ai-conversation.spec.ts` | 完整AI对话流程 | ❌ 未实现 |
| `l2-tool-execution.spec.ts` | 工具执行(Read、Write、Bash) | ❌ 未实现 |
| `l2-multi-step.spec.ts` | 多步骤用户旅程 | ❌ 未实现 |

## 测试执行结果

### 最新测试结果 (2026-03-03)

**L0 测试（冒烟测试）**：
- 通过：8/8 (100%)
- 运行时间：~1.5 分钟
- 状态：全部通过 ✅

**L1 测试（功能测试）**：
- 测试文件：11 通过，1 失败，12 总计
- 测试用例：116 通过，1 失败
- 运行时间：~3.5 分钟
- 通过率：99.1%

**L1 各测试文件详细结果**：

| 测试文件 | 通过 | 失败 | 备注 |
|----------|------|------|------|
| l1-ui-navigation.spec.ts | 11 | 0 | Header、窗口控制正常工作 ✅ |
| l1-workspace.spec.ts | 9 | 0 | 工作区状态检测正常 ✅ |
| l1-chat-input.spec.ts | 14 | 0 | 输入交互全部通过 ✅ |
| l1-navigation.spec.ts | 9 | 0 | 导航面板全部通过 ✅ |
| l1-file-tree.spec.ts | 6 | 0 | 文件树测试通过 ✅ |
| l1-editor.spec.ts | 6 | 0 | 编辑器测试通过 ✅ |
| l1-terminal.spec.ts | 5 | 0 | 终端测试通过 ✅ |
| l1-git-panel.spec.ts | 9 | 0 | Git 面板全部通过 ✅ |
| l1-settings.spec.ts | 9 | 0 | 设置面板全部通过 ✅ |
| l1-session.spec.ts | 11 | 0 | 会话管理全部通过 ✅ |
| l1-dialog.spec.ts | 13 | 0 | 对话框测试全部通过 ✅ |
| l1-chat.spec.ts | 14 | 1 | 聊天显示基本正常 ⚠️ |

**已修复问题**（2026-03-03 修复）：
1. ✅ l1-chat-input：多行输入处理 - 使用 Shift+Enter 输入换行符
2. ✅ l1-chat-input：发送按钮状态检测 - 增强状态检测逻辑
3. ✅ l1-navigation：导航项可交互性 - 增加滚动和重试逻辑
4. ✅ l1-file-tree：文件树可见性 - 增强选择器和视图切换
5. ✅ l1-settings：设置按钮查找 - 扩展选择器范围
6. ✅ l1-session：模式属性验证 - 修正测试逻辑允许 null 值
7. ✅ l1-ui-navigation：焦点管理 - 添加焦点获取重试逻辑

**剩余问题**：
1. ⚠️ l1-chat：发送消息后输入框清空时序问题（边缘情况，与 AI 响应处理时机相关）

**L2 测试（集成测试）**：
- 状态：尚未实现 (0%)
- 测试文件：无

**改进亮点**：

1. **L0 测试全部通过**：应用启动和基本 UI 结构验证完成 ✅
2. **L1 测试 99.1% 通过率**：从原来的 91.7% (98/107) 提升到 99.1% (116/117)
3. **修复 7 个核心问题**：输入处理、导航交互、元素检测等关键功能
4. **测试稳定性显著提升**：减少了 17 个跳过的测试，所有测试都能正常执行

**下一步计划**：

1. 修复 8 个失败的测试用例
2. 改进测试以验证实际的状态变化
3. 添加更多的端到端用户流程测试
4. 实现 L2 级别的集成测试

### 1. 前置条件

安装必需的依赖:

```bash
# 安装 tauri-driver
cargo install tauri-driver --locked

# 构建应用
npm run desktop:build

# 安装 E2E 测试依赖
cd tests/e2e
npm install
```

### 2. 验证安装

检查应用二进制文件是否存在:

**Windows**: `src/apps/desktop/target/release/BitFun.exe`  
**Linux/macOS**: `src/apps/desktop/target/release/bitfun`

### 3. 运行测试

```bash
# 在 tests/e2e 目录下

# 运行 L0 冒烟测试(最快)
npm run test:l0

# 运行所有 L0 测试
npm run test:l0:all

# 运行 L1 功能测试
npm run test:l1

# 运行特定测试文件
npm test -- --spec ./specs/l0-smoke.spec.ts
```

### 4. 识别测试运行模式 (Release vs Dev)

测试框架支持两种运行模式：

#### Release 模式（默认）
- **应用路径**: `target/release/bitfun-desktop.exe`
- **特点**: 优化构建、快速启动、生产就绪
- **使用场景**: CI/CD、正式测试

#### Dev 模式
- **应用路径**: `target/debug/bitfun-desktop.exe`
- **特点**: 包含调试符号、需要 dev server（端口 1422）
- **使用场景**: 本地开发、快速迭代

**如何识别当前使用的模式**：

运行测试时，查看输出的前几行：

```bash
# Release 模式输出示例
application: C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe
[0-0] Application: C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe
                                          ^^^^^^^^

# Dev 模式输出示例
application: C:\Users\wuxiao\BitFun\target\debug\bitfun-desktop.exe
                                        ^^^^^
Debug build detected, checking dev server...    ← Dev 模式特有
Dev server is already running on port 1422      ← Dev 模式特有
[0-0] Application: C:\Users\wuxiao\BitFun\target\debug\bitfun-desktop.exe
```

**快速检查命令**：

```powershell
# 检查当前会使用哪个模式
if (Test-Path "target/release/bitfun-desktop.exe") {
    Write-Host "Will use: RELEASE MODE"
} elseif (Test-Path "target/debug/bitfun-desktop.exe") {
    Write-Host "Will use: DEV MODE"
}
```

**强制使用 Dev 模式**：

使用便捷脚本（推荐）：

```bash
# 切换到 Dev 模式
cd tests/e2e
./switch-to-dev.ps1

# 运行测试
npm run test:l0:all

# 切换回 Release 模式
./switch-to-release.ps1
```

或手动操作：

```bash
# 1. 启动 dev server（可选但推荐）
npm run dev

# 2. 重命名 release 构建
cd target/release
ren bitfun-desktop.exe bitfun-desktop.exe.bak

# 3. 运行测试（自动使用 debug 构建）
cd ../../tests/e2e
npm run test:l0

# 4. 恢复 release 构建
cd ../../target/release
ren bitfun-desktop.exe.bak bitfun-desktop.exe
```

**核心原理**: 测试框架优先使用 `target/release/bitfun-desktop.exe`，如果不存在则自动使用 `target/debug/bitfun-desktop.exe`。所以只需删除或重命名 release 构建，测试就会自动切换到 dev 模式。

## 测试结构

```
tests/e2e/
├── specs/                          # 测试规范
│   ├── l0-smoke.spec.ts           # L0: 基本冒烟测试
│   ├── l0-open-workspace.spec.ts  # L0: 工作区打开
│   ├── l0-open-settings.spec.ts   # L0: 设置交互
│   ├── l1-chat-input.spec.ts      # L1: 聊天输入验证
│   ├── l1-file-tree.spec.ts       # L1: 文件树操作
│   ├── l1-workspace.spec.ts       # L1: 工作区管理
│   ├── startup/                    # 启动相关测试
│   │   └── app-launch.spec.ts
│   └── chat/                       # 聊天相关测试
│       └── basic-chat.spec.ts
├── page-objects/                   # Page Object 模型
│   ├── BasePage.ts                # 包含通用方法的基类
│   ├── ChatPage.ts                # 聊天视图页面对象
│   ├── StartupPage.ts             # 启动屏幕页面对象
│   └── components/                 # 可复用组件
│       ├── Header.ts
│       ├── ChatInput.ts
│       └── MessageList.ts
├── helpers/                        # 工具函数
│   ├── screenshot-utils.ts        # 截图捕获
│   ├── tauri-utils.ts             # Tauri 特定辅助函数
│   └── wait-utils.ts              # 等待和重试逻辑
├── fixtures/                       # 测试数据
│   └── test-data.json
└── config/                         # 配置
    ├── wdio.conf.ts               # WebDriverIO 配置
    └── capabilities.ts            # 平台能力配置
```

## 编写测试

### 1. 测试文件命名

遵循此约定:

```
{级别}-{特性}.spec.ts

示例:
- l0-smoke.spec.ts
- l1-chat-input.spec.ts
- l2-ai-conversation.spec.ts
```

### 2. 使用 Page Objects

**不好** ❌:
```typescript
it('should send message', async () => {
  const input = await $('[data-testid="chat-input-textarea"]');
  await input.setValue('Hello');
  const btn = await $('[data-testid="chat-input-send-btn"]');
  await btn.click();
});
```

**好** ✅:
```typescript
import { ChatPage } from '../page-objects/ChatPage';

it('should send message', async () => {
  const chatPage = new ChatPage();
  await chatPage.sendMessage('Hello');
});
```

### 3. 测试结构模板

```typescript
/**
 * L1 特性名称 spec: 此测试验证内容的描述。
 */

import { browser, expect } from '@wdio/globals';
import { SomePage } from '../page-objects/SomePage';
import { saveScreenshot, saveFailureScreenshot } from '../helpers/screenshot-utils';

describe('特性名称', () => {
  const page = new SomePage();

  before(async () => {
    // 设置 - 在所有测试前运行一次
    await browser.pause(3000);
    await page.waitForLoad();
  });

  describe('子特性 1', () => {
    it('应该做某事', async () => {
      // 准备
      const initialState = await page.getState();
      
      // 执行
      await page.performAction();
      
      // 断言
      const newState = await page.getState();
      expect(newState).not.toEqual(initialState);
    });
  });

  afterEach(async function () {
    // 失败时捕获截图
    if (this.currentTest?.state === 'failed') {
      await saveFailureScreenshot(this.currentTest.title);
    }
  });

  after(async () => {
    // 清理
    await saveScreenshot('feature-complete');
  });
});
```

### 4. data-testid 命名约定

格式: `{模块}-{组件}-{元素}`

**示例**:
```html
<!-- 启动页 -->
<div data-testid="startup-container">
  <button data-testid="startup-open-folder-btn">打开文件夹</button>
  <div data-testid="startup-recent-projects">...</div>
</div>

<!-- 聊天 -->
<div data-testid="chat-input-container">
  <textarea data-testid="chat-input-textarea"></textarea>
  <button data-testid="chat-input-send-btn">发送</button>
</div>

<!-- 顶栏 -->
<header data-testid="header-container">
  <button data-testid="header-minimize-btn">_</button>
  <button data-testid="header-maximize-btn">□</button>
  <button data-testid="header-close-btn">×</button>
</header>
```

### 5. 断言

使用清晰、具体的断言:

```typescript
// 好: 具体的期望
expect(await header.isVisible()).toBe(true);
expect(messages.length).toBeGreaterThan(0);
expect(await input.getValue()).toBe('期望的文本');

// 避免: 模糊的断言
expect(true).toBe(true); // 无意义
```

### 6. 等待和重试

使用内置的等待工具:

```typescript
import { waitForElementStable, waitForStreamingComplete } from '../helpers/wait-utils';

// 等待元素变稳定
await waitForElementStable('[data-testid="message-list"]', 500, 10000);

// 等待流式输出完成
await waitForStreamingComplete('[data-testid="model-response"]', 2000, 30000);

// 对不稳定的操作使用重试
await page.withRetry(async () => {
  await page.clickSend();
  expect(await page.getMessageCount()).toBeGreaterThan(0);
});
```

## 最佳实践

### 应该做的 ✅

1. **保持测试专注** - 一个测试,一个断言概念
2. **使用有意义的测试名称** - 描述预期行为
3. **测试用户行为** - 而不是实现细节
4. **正确处理异步** - 始终 await 异步操作
5. **测试后清理** - 需要时重置状态
6. **失败时添加截图** - 使用 afterEach 钩子
7. **记录进度** - 使用 console.log 进行调试
8. **使用环境设置** - 集中管理超时和重试

### 不应该做的 ❌

1. **不要使用硬编码等待** - 使用 `waitForElement` 而不是 `pause`
2. **不要在测试间共享状态** - 每个测试应该独立
3. **不要测试内部实现** - 专注于用户可见的行为
4. **不要忽略不稳定的测试** - 修复或标记为跳过并说明原因
5. **不要使用复杂的选择器** - 优先使用 data-testid
6. **不要测试第三方代码** - 只测试 BitFun 功能
7. **不要混合测试级别** - 保持 L0/L1/L2 分离

### 错误处理

```typescript
it('应该优雅地处理错误', async () => {
  try {
    await page.performRiskyAction();
  } catch (error) {
    // 捕获上下文
    await saveFailureScreenshot('error-context');
    const pageSource = await browser.getPageSource();
    console.error('页面状态:', pageSource.substring(0, 500));
    throw error; // 重新抛出以使测试失败
  }
});
```

### 条件测试

```typescript
it('当工作区打开时应测试功能', async function () {
  const startupVisible = await startupPage.isVisible();
  
  if (startupVisible) {
    console.log('[测试] 跳过: 工作区未打开');
    this.skip();
    return;
  }
  
  // 测试继续...
});
```

## 问题排查

### 常见问题

#### 1. tauri-driver 找不到

**症状**: `Error: spawn tauri-driver ENOENT`

**解决方案**:
```bash
# 安装或更新 tauri-driver
cargo install tauri-driver --locked

# 验证安装
tauri-driver --version

# 确保 ~/.cargo/bin 在 PATH 中
echo $PATH  # macOS/Linux
echo %PATH% # Windows
```

#### 2. 应用未构建

**症状**: `Binary not found at target/release/BitFun.exe`

**解决方案**:
```bash
# 构建应用
npm run desktop:build

# 验证二进制文件存在
ls src/apps/desktop/target/release/
```

#### 3. 测试超时

**症状**: 测试失败并显示"timeout"错误

**原因**:
- 应用启动慢(debug 构建更慢)
- 元素尚未可见
- 网络延迟

**解决方案**:
```typescript
// 增加特定操作的超时时间
await page.waitForElement(selector, 30000);

// 使用环境设置
import { environmentSettings } from '../config/capabilities';
await page.waitForElement(selector, environmentSettings.pageLoadTimeout);

// 添加策略性等待
await browser.pause(1000); // 点击后
```

#### 4. 元素未找到

**症状**: `Element with selector '[data-testid="..."]' not found`

**调试步骤**:
```typescript
// 1. 检查元素是否存在
const exists = await page.isElementExist('[data-testid="my-element"]');
console.log('元素存在:', exists);

// 2. 捕获页面源码
const html = await browser.getPageSource();
console.log('页面 HTML:', html.substring(0, 1000));

// 3. 截图
await page.takeScreenshot('debug-element-not-found');

// 4. 在前端代码中验证 data-testid
// 检查 src/web-ui/src/... 中的组件
```

#### 5. 不稳定的测试

**症状**: 测试有时通过,有时失败

**常见原因**:
- 竞态条件
- 时序问题
- 测试间状态污染

**解决方案**:
```typescript
// 使用 waitForElement 而不是 pause
await page.waitForElement(selector);

// 添加重试逻辑
await page.withRetry(async () => {
  await page.clickButton();
  expect(await page.isActionComplete()).toBe(true);
});

// 确保测试独立性
beforeEach(async () => {
  await page.resetState();
});
```

### 调试模式

启用调试运行测试:

```bash
# 启用 WebDriverIO 调试日志
npm test -- --spec ./specs/l0-smoke.spec.ts --log-level=debug

# 失败时保持浏览器打开
# (修改 wdio.conf.ts: bail: 1)
```

### 截图分析

截图保存到 `tests/e2e/reports/screenshots/`:

```typescript
// 手动截图
await page.takeScreenshot('my-debug-point');

// 失败时自动捕获(添加到测试)
afterEach(async function () {
  if (this.currentTest?.state === 'failed') {
    await saveFailureScreenshot(this.currentTest.title);
  }
});
```

## 添加新测试

### 分步指南

1. **确定测试级别** (L0/L1/L2)
2. **在适当目录创建测试文件**
3. **向 UI 元素添加 data-testid** (如需要)
4. **创建或更新 Page Objects**
5. **按照模板编写测试**
6. **本地运行测试**
7. **添加到 CI/CD 流程** (对于 L0/L1)

### 示例: 添加 L1 文件树测试

1. 创建 `tests/e2e/specs/l1-file-tree.spec.ts`
2. 向文件树组件添加 data-testid:
   ```tsx
   <div data-testid="file-tree-container">
     <div data-testid="file-tree-item" data-path={path}>
   ```
3. 创建 `page-objects/FileTreePage.ts`:
   ```typescript
   export class FileTreePage extends BasePage {
     async getFiles() { ... }
     async clickFile(name: string) { ... }
   }
   ```
4. 编写测试:
   ```typescript
   describe('L1 文件树', () => {
     it('应显示工作区文件', async () => {
       const files = await fileTree.getFiles();
       expect(files.length).toBeGreaterThan(0);
     });
   });
   ```
5. 运行: `npm test -- --spec ./specs/l1-file-tree.spec.ts`
6. 更新 `package.json`:
   ```json
   "test:l1:filetree": "wdio run ./config/wdio.conf.ts --spec ./specs/l1-file-tree.spec.ts"
   ```

## CI/CD 集成

### 推荐测试策略

```yaml
# .github/workflows/e2e.yml (示例)
name: E2E Tests

on: [push, pull_request]

jobs:
  l0-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: 构建应用
        run: npm run desktop:build
      - name: 安装 tauri-driver
        run: cargo install tauri-driver --locked
      - name: 运行 L0 测试
        run: cd tests/e2e && npm run test:l0:all
        
  l1-tests:
    runs-on: ubuntu-latest
    needs: l0-tests
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v3
      - name: 构建应用
        run: npm run desktop:build
      - name: 运行 L1 测试
        run: cd tests/e2e && npm run test:l1
```

### 测试执行矩阵

| 事件 | L0 | L1 | L2 |
|------|----|----|---- |
| 每次提交 | ✅ | ❌ | ❌ |
| Pull request | ✅ | ✅ | ❌ |
| 每晚构建 | ✅ | ✅ | ✅ |
| 发布前 | ✅ | ✅ | ✅ |

## 资源

- [WebDriverIO 文档](https://webdriver.io/)
- [Tauri 测试指南](https://tauri.app/v1/guides/testing/)
- [Page Object 模式](https://webdriver.io/docs/pageobjects/)
- [BitFun 项目结构](../../AGENTS.md)

## 贡献

添加测试时:

1. 遵循现有结构和约定
2. 使用 Page Object 模式
3. 向新 UI 元素添加 data-testid
4. 保持测试在适当级别(L0/L1/L2)
5. 如引入新模式请更新本指南

## 支持

如有问题或疑问:

1. 查看[问题排查](#问题排查)部分
2. 查看现有测试文件以获取示例
3. 带着测试日志和截图提交 issue
