# bitfun-page-function-runtime

**描述**: 嵌入式 JS Page Function 运行时（基于 rquickjs）。

**包名**: `bitfun-page-function-runtime` | lib: `bitfun_page_function_runtime`

## 核心类型

| 类型 | 说明 |
|------|------|
| `PageFunctionError` | 运行时错误枚举（Init/Eval/MissingFetch/Handler/Timeout） |
| `FetchRequest` | 入站请求结构体（method/url/path/headers/body） |
| `FetchResponse` | 出站响应结构体（status/headers/body） |
| `PageMeta` | 页面元数据（username/slug/version_id/visibility） |
| `PageHost` (trait) | 宿主能力注入接口（KV/DB/BLOBS/ASSETS/PAGE） |
| `MemoryPageHost` | 内存宿主实现（测试用） |

## 关键功能

- `run_fetch()` — 执行 worker fetch 处理函数到完成或超时
- 内存限制 16MB，栈大小 256KB
- 超时中断（通过 rquickjs interrupt handler）
- 支持同步和 async/await fetch 处理器
- 宿主绑定通过 JSON 字符串桥接调用

## 宿主绑定

| 绑定 | 方法 |
|------|------|
| `env.KV` | get/put/delete/list |
| `env.DB` | execute/query |
| `env.BLOBS` | put/get/delete |
| `env.ASSETS` | fetch |
| `env.PAGE` | 页面元数据 |

## 一句话总结

基于 rquickjs 的 Cloudflare Workers 风格嵌入式 JS 运行时，为 BitFun Pages 提供服务端 fetch 处理。
