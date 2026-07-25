# bitfun-relay-service

**描述**: 可复用的中继运行时，用于 BitFun Remote Connect。

**包名**: `bitfun-relay-service` | lib: `bitfun_relay_service`

## 核心模块

| 模块 | 说明 |
|------|------|
| `relay` | 房间管理 (`RoomManager`)、设备管理 |
| `routes` | HTTP API 路由、WebSocket、认证、页面路由 |
| `admin` | 管理接口 |
| `db` | SQLite 数据库（加密 blob、密码 hash） |
| `page_data` | Page 数据存储 |
| `page_execution` | Page 执行保护 |

## 关键类型/功能

- `RoomManager` — 房间生命周期管理
- `WebAssetStore` trait — 每个房间的静态资源抽象存储
- `MemoryAssetStore` — 内存后端（嵌入式中继用）
- `DiskAssetStore` — 磁盘后端（独立中继服务器用）
- `PageBrowserAuthConfig` — 浏览器页面认证配置
- `build_relay_router()` — 构建完整 Relay Router（5 层重载）
- `AppState` — 应用共享状态
- `relay_security_headers` — 安全响应头中间件

## 架构模式

- 桌面客户端通过 WebSocket 连接
- 移动客户端通过 HTTP POST 交互
- 中继不检查加密负载，仅转发
- 支持多种 `WebAssetStore` 实现

## 一句话总结

无状态 HTTP-to-WebSocket 加密消息中继，支持桌面/移动端远程连接和 BitFun Pages 静态资源托管。
