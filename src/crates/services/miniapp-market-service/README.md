# BitFun MiniApp Market Service

这里是 MiniApp 市场后端的主要业务实现。Axum 可执行入口位于
`../../../apps/miniapp-market-server/`，生产部署文件位于
`../../../../deploy/miniapp-market/`。

## 给 AI Agent 的最短指引

1. 先阅读仓库根 `AGENTS.md`、上级 `src/crates/services/AGENTS.md`、
   [Server README](../../../apps/miniapp-market-server/README.md) 和
   [生产部署手册](../../../../deploy/miniapp-market/README.md)。
2. 本 crate 负责具体 SQLite、文件存储、GitHub OAuth、包校验和市场 HTTP
   路由。纯 DTO、状态机和策略属于
   `../../contracts/product-domains/src/miniapp/market.rs`。
3. 不要让 Services 依赖 UI、桌面 command、Assembly 或其他上层产品入口。
4. API 前缀、DTO、分页和错误 envelope 是 Web 与桌面共享的稳定契约；变化时
   必须同步消费者和测试。
5. 安全约束不是可选功能。不得放宽 OAuth state/PKCE、CSRF、token 轮换、管理员
   数字 ID、包白名单、zip 限制、截图重编码、hash 绑定或审计规则来绕过失败。
6. 不读取、输出或提交生产 secret、Cookie、token、包内容或个人 IP。
7. 生产发布只能来自明确 commit，并按部署手册先备份、后构建、再验证 revision。

## 文件地图

| 文件 | 所有权 |
| --- | --- |
| `src/lib.rs` | 装配数据库、artifact store、认证、API 和静态网页 |
| `src/routes.rs` | `/miniapp/api/v1` 路由、请求/响应和事务流程 |
| `src/auth.rs` | GitHub Web OAuth、桌面授权事务、session 和 token |
| `src/db.rs` | SQLite WAL、查询、唯一性和审核事务 |
| `src/package.rs` | `.bfminiapp` ZIP、Node/npm/ESM、大小和截图验证 |
| `src/artifacts.rs` | SHA-256 内容寻址持久化 |
| `src/retention.rs` | 草稿、驳回和撤回 artifact 清理 |
| `src/error.rs` | 统一 API 错误 envelope |
| `src/request_id.rs` | request ID |
| `src/config.rs` | 环境变量和生产/开发配置 |
| `migrations/` | 编号 SQLite schema migration |

## 不变量

- 公开 API 固定在 `/miniapp/api/v1`。
- 列表响应保持 `{items,nextCursor}`。
- 错误保持 `{error:{code,message,requestId,details?}}`。
- 投稿状态保持 `draft → submitted → approved | rejected | withdrawn`；批准的
  release 可被永久 yank。
- Release 不可变，新版本审核期间继续提供旧的已批准版本。
- 批准必须原子绑定 package hash、截图 hash、规范化 metadata 和
  `review_bundle_hash`。
- 市场包只能包含协议白名单文件，必须拒绝 Node、npm、非空 ESM、zip-slip、
  link、重复/大小写冲突路径和超限解压。
- GitHub token 只用于读取公开 `{id,login,avatar_url}`，随后丢弃，不能下发给
  Web 或桌面客户端。
- 管理员身份每次请求按 GitHub 数字 ID 计算，不能依赖客户端声明。

## 本地验证

在仓库根目录运行：

```bash
pnpm run fmt:rs
cargo test -p bitfun-miniapp-market-service
cargo check -p bitfun-miniapp-market-server
cargo check --workspace
```

领域 DTO 或状态机变化再运行：

```bash
cargo test -p bitfun-product-domains --features miniapp
pnpm run type-check:miniapp-market
pnpm run test:miniapp-market
```

按改动补 focused tests，至少覆盖相应拒绝或回滚路径。包校验、OAuth、权限、批准、
yank、上传和 migration 变化不能只测试成功路径。

## Migration 规则

`migrations/0001_init.sql` 已进入生产，不得原地修改。新增 schema 时：

1. 新增下一个编号 migration；
2. 更新 migration runner，按编号、事务化、只执行一次；
3. 用已有旧 schema 数据库测试升级；
4. 验证重复启动幂等；
5. 在部署说明中写明旧 binary 是否仍兼容；
6. 发布前运行一致性备份和恢复演练。

不兼容 migration 不能依靠“切回旧镜像”回滚。恢复生产备份会丢失备份时间点之后
的数据，必须先取得人工确认并制定数据恢复方案。

## 发布

此 crate 被 `bitfun-miniapp-market-server` 编译进与网页相同的 Docker 镜像。
不要在生产服务器直接执行 `cargo run`，不要手工替换 binary，也不要直接编辑
SQLite/artifacts。完整流程见
[生产部署手册](../../../../deploy/miniapp-market/README.md)。
