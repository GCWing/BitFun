# bitfun-static-hook-support

**描述**: 共享的有界读取和脱敏支持库，供静态外部源码适配器使用。

**包名**: `bitfun-static-hook-support` | lib: `bitfun_static_hook_support`

## 核心类型/功能

| 类型 | 说明 |
|------|------|
| `BoundedFileRead` / `BoundedTextRead` | 有界文件读取（防止大文件 OOM） |
| `BoundedDirectoryWalkLimits` | 目录遍历限制（深度/条目/目录/文件数） |
| `BoundedDirectoryWalkError` | 遍历错误（IO / 超限） |
| `StaticHookParseResult` | Hook 文档解析结果 |
| `StaticHookHandlerFact` | Hook 处理器事实（去敏后） |
| `StaticHookHandlerRule` | Handler 解析规则定义 |
| `StaticHookParseIssue` | 解析问题枚举 |
| `StaticHookDocumentFormat` | 文档格式（Json / Toml） |

## 关键函数

- `read_bounded_file()` / `read_bounded_text()` — 有界文件读取
- `collect_bounded_regular_files()` — 有界目录遍历收集
- `regular_file_exists()` — 安全文件存在性检查
- `resolve_bounded_regular_file()` — 安全路径解析（防穿越）
- `bounded_project_ancestors()` — 有界项目祖先路径链
- `parse_hook_document()` — Hook 文档解析（JSON/TOML）
- `redacted_parse_content_version()` — 内容版本指纹（去敏）
- `redacted_executable_preview()` — 安全可执行文件预览名

## 一句话总结

为外部编程助手适配器提供安全的有限文件/目录访问、Hook 文档解析和敏感信息脱敏工具。
