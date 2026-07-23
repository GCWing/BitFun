# PR 标题（conventional commit 格式）

> `feat:` / `fix:` / `chore:` / `docs:` / `refactor:` + 简短描述

## 概述

简要说明这个 PR 做了什么、为什么做。

## 变更类型

- [ ] 新功能 (feat)
- [ ] Bug 修复 (fix)
- [ ] 工程化 (chore)
- [ ] 文档 (docs)
- [ ] 重构 (refactor)
- [ ] 上游同步 (sync)

## 验证

- [ ] `cargo check --workspace` 通过
- [ ] 涉及前端：`pnpm run type-check:web` 通过
- [ ] 涉及 mobile-web：`pnpm --dir src/mobile-web run type-check` 通过
- [ ] 无闭源代码泄露（taiji-dvmi/taiji-magnet/taiji-thrust/taiji-risk）

## 关联 Issue

Closes #N

## 备注

任何需要评审者注意的事项。
