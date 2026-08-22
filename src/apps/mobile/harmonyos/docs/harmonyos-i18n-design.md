# HarmonyOS 手机端 i18n 设计

Date: 2026-08-18

Status: Implementation

Scope: `src/apps/mobile/harmonyos`

## 问题

鸿蒙手机端已有 `RemoteI18n.t/f/f2`，但只有一份写死的简体中文词表，没有语言状态、没有持久化、设置页也没有入口。因此界面无法切换中英文。

## 目标

- 产品内支持规范 locale：`zh-CN`、`en-US`。
- 设置页可显式切换，切换后当前界面立即刷新。
- 用户选择写入本机 preferences，冷启动恢复；首次启动按系统语言解析，解析不到则用 `zh-CN`。
- 系统权限文案、语音识别语言、相对/绝对时间、错误分类都跟随当前语言。
- 不引入 Web UI / mobile-web 的词库。locale id 与别名规则对齐共享合同。

## 结构

| 单元 | 职责 |
|---|---|
| `i18n/AppLocale` | 规范 id、别名解析、fallback、展示名 |
| `i18n/ZhCnMessages` / `EnUsMessages` | 各语言完整 key 词表，key 必须一对一 |
| `i18n/AppLocaleState` | `@ObservedV2` 语言身份，供 ArkUI V2 订阅 |
| `i18n/RemoteI18n` | 查找、插值、跨语言匹配、日期格式；`t()` 读取 `@Trace language` |
| `services/LocalePreferenceStore` | preferences 持久化、系统语言探测、`setAppPreferredLanguage` |
| `pages/viewmodel/LocaleController` | 初始化与切换；切换后刷新已落地的连接/服务状态文案 |
| `pages/components/LanguageSettingsPanel` | 设置里的中/英选择面板 |

## 数据流

1. `AppRootRuntime.aboutToAppear` 先初始化 `LocaleController`。
2. 已保存的语言优先生效；否则解析系统语言并写入。
3. `RemoteI18n.setLanguage` 更新 `AppLocaleState.language/revision`，UI 通过 `t()` 的 `@Trace` 读取重建。
4. 设置页调用 `onSetLanguage` → `LocaleController.setLanguage` → 持久化 + 应用首选语言 + 刷新派生状态文案。

## 错误匹配

`ConnectionErrorPolicy` / `GeneralChatServiceStatus` 不得只对比当前语言译文。用 `RemoteI18n.textEquals` / `textContains` 对 `zh-CN` 与 `en-US` 同时匹配，避免切换语言后把旧错误文案认丢。

## 远程场景

本能力是控制器本机 UI 语言，不代理到桌面、不随 Peer/Dispatch 同步。远程工作区路径、协议字段保持不变。语音识别语言跟随本机 UI locale。

## 验证

- 词表 key 对等、别名解析、切换/回退、跨语言错误匹配、时间格式的本地单测。
- `pnpm run harmony:architecture`
- HarmonyOS LocalTest + `assembleHap`
