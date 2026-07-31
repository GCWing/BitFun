# BitFun MiniApp Market Web

这里是 MiniApp 市场独立网页的源码目录。生产地址是
`https://market.openbitfun.com/miniapp/`。

> 最短结论：修改这个目录里的网页，完成检查并提交 Git commit 后，按照
> [生产部署手册](../../deploy/miniapp-market/README.md)重建并重启
> `bitfun-miniapp-market` 容器。网页和 Rust 后端在同一个 Docker 镜像中，
> 不要单独把 `dist/` 上传到服务器。

## 给 AI Agent 的执行约束

1. 先阅读仓库根目录的 `AGENTS.md`、本文件和
   [生产部署手册](../../deploy/miniapp-market/README.md)。
2. 只修改任务要求涉及的文件；保留工作区里不属于本任务的改动。
3. `/miniapp/` 是固定的 Vite base，`/miniapp/api/v1` 是固定的 API 前缀。
   修改它们会同时影响 Nginx、OAuth callback、桌面客户端和生产链接，不能
   当作普通重命名处理。
4. 网页必须通过 `src/api.ts` 访问同源 API，并保留 Cookie、CSRF 和统一错误
   envelope 的行为。组件里不要直接拼另一个服务器地址。
5. 这个站点是独立的小型产品界面，不得导入 `src/web-ui` 的完整 locale
   目录。用户可见文本要同时维护 `zh-CN`、`zh-TW`、`en-US` fallback。
6. 不要把 GitHub client secret、session secret、Cookie、token 或生产
   `.env` 写进源码、日志、截图或提交记录。
7. 未完成本文件中的最小验证、没有明确 Git commit，或生产健康检查未通过
   时，不得声称已经发布。
8. `webSubmissionsEnabled=false` 时网页投稿必须是只读模式：隐藏新建、上传、
   提交新版本和撤回操作，只保留“我的投稿”历史。不要仅靠隐藏按钮保护接口；
   后端还会按认证来源拒绝 Web Cookie 投稿写请求。

## 源码地图

| 位置 | 作用 |
| --- | --- |
| `src/App.tsx` | 目录、详情、受开关控制的投稿、只读“我的投稿”和管理员审核页面 |
| `src/api.ts` | `/miniapp/api/v1` 客户端、CSRF、登录和下载 URL |
| `src/types.ts` | 网页使用的 API DTO |
| `src/MiniAppIcon.tsx` | 将 MiniApp 元数据中的 Lucide 图标名安全解析为图标组件 |
| `src/i18n.ts` | `zh-CN`、`zh-TW`、`en-US` 文案与 fallback |
| `src/format.ts` | 市场页面的日期和数字格式化 |
| `src/styles.css` | 响应式布局与视觉样式 |
| `src/api.test.ts` | API 客户端契约测试 |
| `vite.config.ts` | `/miniapp/` base、本地端口和 API proxy |
| `public/` | 网页静态资源 |
| `dist/` | 本地构建产物；由构建生成，不手工修改、不单独部署 |

BitFun 桌面端内嵌的原生市场 Scene 不在这里。它位于
`src/web-ui/src/app/scenes/miniapps/`，通过
`src/apps/desktop/src/api/miniapp_market_api.rs` 访问市场。

## 本地开发

首次开发先在仓库根目录安装依赖：

```bash
pnpm install
```

终端一启动本地 Rust 后端。默认数据会写到
`var/miniapp-market/`，默认配置只适合开发：

```bash
cargo run -p bitfun-miniapp-market-server
```

终端二启动 Vite：

```bash
pnpm run dev:miniapp-market
```

打开 `http://127.0.0.1:1431/miniapp/`。Vite 会把
`/miniapp/api/*` 代理到 `http://127.0.0.1:9710`。后端不在默认地址时：

```bash
MARKET_DEV_API=http://127.0.0.1:19710 pnpm run dev:miniapp-market
```

本地未配置 GitHub OAuth 时，浏览和无登录页面仍可开发，登录按钮会处于不可
用状态。不要为了本地调试复制生产 secret。

网页投稿默认关闭。只有在本地专门验证网页投稿旧流程时，才给本地 Rust 服务设置
`MARKET_WEB_SUBMISSIONS_ENABLED=true`；生产保持 `false`。桌面客户端使用
Bearer token 投稿，不受这个网页开关影响。

`src/api.ts` 和 `SubmitPage` 暂时保留未来可能重新启用的 Web 投稿实现；它们存在
不代表生产能力已开放。所有入口必须只根据后端 `/config` 返回的
`webSubmissionsEnabled` 显示，配置加载失败时按关闭处理。不要增加仅由前端常量、
URL 参数或本地存储绕过的开关。

## 修改后的最小验证

在仓库根目录运行：

```bash
pnpm run type-check:miniapp-market
pnpm run test:miniapp-market
pnpm run build:miniapp-market
```

如果修改了本地化文案或 fallback，再运行：

```bash
pnpm run i18n:contract:test
pnpm run i18n:audit
```

如果新增或修改颜色，再运行：

```bash
pnpm run theme:color-audit:all
```

行为改动至少人工检查：

- `/miniapp/` 目录可加载、搜索、排序和分页；
- `/miniapp/apps/<slug>` 详情可打开；
- 三种语言可切换，窄屏和宽屏没有明显溢出；
- API 失败会显示可理解的错误，不在控制台泄露凭据；
- 网页投稿关闭时看不到投稿/更新/撤回按钮，直接访问 `/miniapp/submit` 会提示
  改用 BitFun Desktop，“我的投稿”仍可查看；
- 登录、桌面投稿或审核相关改动使用测试账号走完对应流程。

## API 类型变更

网页 `src/types.ts` 不是独立的协议所有者。API DTO 或状态机变化需要同步检查：

- `src/crates/contracts/product-domains/src/miniapp/market.rs`
- `src/crates/services/miniapp-market-service/`
- `src/miniapp-market-web/src/api.test.ts`
- 桌面市场客户端

列表响应必须保持 `{items,nextCursor}`，错误必须保持
`{error:{code,message,requestId,details?}}`。兼容性不明确时先停止部署，
补齐契约测试后再继续。

## 发布

前端构建发生在
`src/apps/miniapp-market-server/Dockerfile` 的 `web-builder` 阶段，生成的
`dist/` 会复制进后端运行镜像的 `/app/web/`。所以：

- 只改前端：仍然重建并重启同一个市场容器；
- 只改后端：仍然重建并重启同一个市场容器；
- 前后端一起改：只发布一个包含同一 Git commit 的镜像。

完整的备份、精确 commit 发布、健康检查和回滚命令见
[生产部署手册](../../deploy/miniapp-market/README.md)。
