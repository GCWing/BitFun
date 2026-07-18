# 蜂群架构画布 + 进度监控

> 2026-07-18 | MiniApp SVG DAG + Agent 推状态更新

---

## 机制

```
state = { nodes: { cmd: 'idle', sec: 'idle', ..., opt: 'idle' } }

初始化: 手动计算布局 → SVG渲染 → 全灰 (idle=#3f3f46)
执行中: Agent 每过一节点 → 写 storage.json → MiniApp poll(500ms) → render() → 节点变色
完成:   全绿 DAG (done=#34d399)
```

---

## Phase 0: 调查 ✅

### 调查结论

| 候选通道 | 可行性 | 结论 |
|---|---|---|
| Canvas API state update | ❌ | Canvas 和 MiniApp 是两个独立系统，Canvas state 通过 `useCanvasState` + postMessage 管理，不适用于 MiniApp |
| `app.storage` 直写 | ✅ **采用** | MiniApp 通过 `app.storage.get/set` 读写 KV 存储，底层是 `<miniapps_dir>/<app_id>/storage.json` 文件 |
| MiniApp Bridge postMessage 反向通道 | ⚠️ 可行但需改代码 | Bridge 已支持 `bitfun:event` 任意事件转发，但需 Agent 侧新增 Tauri 事件发射机制 |

### 实际通信架构

```
Agent (指挥官)
  │  ExecCommand: python bee_colony_state_push.py set cmd running
  ▼
storage.json (磁盘文件)
  %APPDATA%/bitfun/data/miniapps/bee-colony-dag/storage.json
  │
  │  MiniApp 每 500ms 轮询 app.storage.get('bee-colony-state')
  ▼
MiniApp (iframe)
  state.nodes → render() → SVG 节点变色
```

### 关键文件

- MiniApp storage 磁盘路径: `%APPDATA%/bitfun/data/miniapps/bee-colony-dag/storage.json`
- Bridge builder: `src/crates/contracts/product-domains/src/miniapp/bridge_builder.rs`
- Bridge 前端: `src/web-ui/src/app/scenes/miniapps/hooks/useMiniAppBridge.ts`
- Storage 服务: `src/crates/services/services-integrations/src/miniapp/storage.rs`
- MiniApp 管理器: `src/crates/assembly/core/src/miniapp/manager.rs`

---

## Phase 1: 静态架构 DAG 渲染 ✅

### 产出: `MiniApp/bee-colony-dag/`

| 文件 | 行数 | 说明 |
|---|---|---|
| `meta.json` | 35 | 权限(node.enabled=false) + i18n |
| `index.html` | 24 | Shell: header(SVG badge) + SVG canvas + footer |
| `style.css` | 163 | 深色主题 + 4色节点状态 + running脉冲动画 + gate虚线边框 |
| `ui.js` | 328 | 核心逻辑: 布局计算 + SVG render + 500ms轮询 + 徽章更新 |

### 8 节点（标准链顺序）

```
cmd(指挥官) → sec(秘书B01) → pm(产品经理) → plan(规划师)
  → exec(执行者) → review(审查者·Gate) → accept(验收者·Gate) → opt(优化者)
```

### 渲染特性

- 节点: 180×52px 圆角矩形, 角色名(13px bold) + 树编号(10px dim)
- 连线: 垂直箭头, Gate边红色虚线
- Gate 节点(review/accept): 红色虚线边框叠加
- 4 色状态: idle=#3f3f46(灰), running=#60a5fa(蓝·脉冲动画), done=#34d399(绿), failed=#ef4444(红)
- 徽章: 待命/执行中/异常/完成

### 与五子棋模式对比

| 特性 | 五子棋 | 蜂群DAG |
|---|---|---|
| 状态源 | 本地 state 对象 (用户点击) | 外部 Agent 推送 (app.storage) |
| 渲染 | renderStones() 等 SVG DOM 操作 | 同模式，render() 重绘全部节点 |
| 持久化 | app.storage 存战绩统计 | app.storage 存节点状态 |
| 刷新触发 | 用户操作后手动调 render() | 500ms 轮询检测变化后调 render() |

---

## Phase 2: 状态推送 ✅

### Agent 端工具

**`%APPDATA%/bitfun/tools/bee_colony_state_push.py`** (115 行)

```bash
# 重置全部节点为 idle
python bee_colony_state_push.py reset

# 设置单个节点状态
python bee_colony_state_push.py set cmd running
python bee_colony_state_push.py set cmd done "任务完成"
python bee_colony_state_push.py set exec failed "编译错误"

# 批量推送完整状态JSON
python bee_colony_state_push.py '{"nodes":{"cmd":{"status":"done"},"sec":{"status":"running"},...}}'
```

### Agent 使用方式 (ExecCommand)

```
# 每过一个节点，执行:
ExecCommand: python C:\Users\Administrator\AppData\Roaming\bitfun\tools\bee_colony_state_push.py set <node_id> <status>

# 例如蜂群执行标准链时:
# 1. 指挥官决策完成 → set cmd done
# 2. 秘书检索完成   → set sec done
# 3. 产品经理定义完成 → set pm done
# ...以此类推
```

### MiniApp 端轮询 (ui.js 264-287行)

```javascript
async function pollState() {
  const raw = await app.storage.get("bee-colony-state");
  const hash = simpleHash(JSON.stringify(raw));
  if (hash === state.lastHash) return;  // 无变化跳过
  // 更新节点状态 + render()
}
setInterval(pollState, 500);
```

---

## Phase 3: 固定展示入口

### 当前状态

MiniApp 已创建，通过以下方式之一打开:
1. BitFun 桌面端 → MiniApp 菜单 → 从文件夹导入 `MiniApp/bee-colony-dag/`
2. 或通过 `miniapp_import_from_path` Tauri 命令导入

### 待实现

- [ ] MiniApp 固定到侧边栏/底部面板
- [ ] 参考: `MiniAppRunner.tsx` 渲染容器, `NavPanel/MiniAppEntry.tsx` 入口
- [ ] 方案A: 复用 FloatingMiniChat 的浮动面板模式，在底部固定显示
- [ ] 方案B: 在 NavPanel 添加"蜂群监控"快捷入口
- [ ] 确保不随聊天对话框滚动消失

---

## 文件清单

| 路径 | 用途 |
|---|---|
| `docs/plans/bee-colony-visual-orchestration-plan.md` | 本计划文档 |
| `MiniApp/bee-colony-dag/meta.json` | MiniApp 元数据 + 权限 |
| `MiniApp/bee-colony-dag/index.html` | HTML Shell |
| `MiniApp/bee-colony-dag/style.css` | 深色主题样式 |
| `MiniApp/bee-colony-dag/ui.js` | DAG 渲染 + 轮询逻辑 |
| `%APPDATA%/bitfun/tools/bee_colony_state_push.py` | Agent 端状态推送工具 |
| `%APPDATA%/bitfun/legions/bee-colony-standard.json` | 蜂群标准链模板 (8节点) |
| `%APPDATA%/bitfun/agents/*.md` | 8 角色 Agent 定义 |
