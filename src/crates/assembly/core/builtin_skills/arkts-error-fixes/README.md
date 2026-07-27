# HarmonyOS ArkTS 错误解决方案Skills

> 专为 AI 辅助开发 HarmonyOS 应用设计的 用于解决ArkTS 编译错误和类型不匹配问题Skills

## 📖 项目背景

在使用 AI 模型（如 Cursor、GitHub Copilot、Claude 等）开发 HarmonyOS 应用时，我们发现了一个普遍存在的问题：

- **AI 模型会重复犯同样的 ArkTS 类型错误**
- **需要开发者反复手动修复相同的错误**，严重影响开发效率

例如，AI 经常生成：
- 使用全局 `animateTo` 而不是 `this.getUIContext().animateTo()`
- 使用 `window.Rect` 而不是 `window.TitleButtonRect`
- 访问 `TitleButtonRect` 不存在的 `left` 和 `top` 属性
- 在 catch 子句中使用类型注解
- 等等...

为了解决这个问题，我们开发了这个 **ArkTS 错误解决方案库**，旨在：

✅ **提高 AI 开发效率** - 让 AI 模型能够自动识别和修复常见错误  
✅ **减少重复工作** - 避免反复修复相同的类型错误  
✅ **提供最佳实践** - 每个错误都配有详细的解决方案和代码示例  

## 🎯 项目目标

本仓库收集并整理了 **32+ 种常见的 ArkTS 编译错误和类型不匹配问题**，每个错误都包含：

- 📝 详细的错误描述和原因分析
- ✅ 正确的解决方案
- 💡 最佳实践建议
- 📚 完整的代码示例

## 📦 内容概览

### 错误分类

| 错误类型 | 数量 | 说明 |
|---------|------|------|
| **API 类型错误** | 5+ | Notification、Window、AppStorage 等 API 类型不匹配 |
| **对象类型错误** | 6+ | 对象展开、对象字面量、接口方法签名等 |
| **函数类型错误** | 4+ | 函数返回类型、箭头函数转换等 |
| **装饰器错误** | 2+ | @StorageLink 默认值、未使用变量警告等 |
| **其他类型错误** | 15+ | Catch 子句、ESObject、资源转换等 |

### 主要错误类型

- ✅ **Notification API 类型错误** - ContentType 类型不兼容
- ✅ **Window API 类型错误** - `window.getLastWindow` 类型推断问题
- ✅ **AppStorage 类型错误** - `AppStorage.get()` 类型推断错误
- ✅ **对象展开类型错误** - 对象展开时的类型推断问题
- ✅ **@StorageLink 默认值错误** - 缺少默认值
- ✅ **对象字面量接口错误** - 对象字面量缺少显式接口
- ✅ **函数返回类型错误** - 返回类型推断受限
- ✅ **箭头函数转换错误** - 使用函数表达式而非箭头函数
- ✅ **TitleButtonRect 类型错误** - 返回类型错误和访问不存在属性
- ✅ **Catch 子句类型错误** - catch 子句中的类型注解
- ✅ **ESObject 类型错误** - ESObject 类型使用受限
- ✅ **资源转换错误** - Resource 到 string/number 转换错误
- ✅ 以及更多...

## 🚀 使用方法

### 作为 AI Skill 使用

本仓库设计为 AI 开发工具的 Skill，可以直接被 AI 模型调用：

1. **配置 Skill**：将本仓库添加到你的 AI 开发工具（如 Cursor）的 Skills 目录
2. **自动识别**：AI 模型在生成代码时会自动参考这些解决方案
3. **减少错误**：AI 生成的代码将更符合 HarmonyOS API 11+ 规范

### 手动查阅

你也可以直接查阅文档和代码示例：

- 📚 **参考文档**：查看 `reference/` 目录下的详细错误说明
- 💻 **代码示例**：查看 `assets/` 目录下的完整代码示例

## 📁 目录结构

```
arkts-error-solution-skill/
├── README.md                 # 本文件
├── SKILL.md                  # Skill 配置文件
├── assets/                   # 代码示例目录
│   ├── NotificationError.ets
│   ├── WindowTypeError.ets
│   ├── AppStorageError.ets
│   └── ... (27+ 个示例文件)
└── reference/                 # 参考文档目录
    ├── notification_errors.md
    ├── window_type_errors.md
    ├── appstorage_errors.md
    └── ... (27+ 个文档文件)
```

## 💡 使用示例

### 问题场景

AI 生成代码时经常出现这样的错误：

```typescript
// ❌ AI 经常生成的错误代码
async function getTitleButtonRect(context: UIAbilityContext): Promise<window.Rect> {
  const win = await window.getLastWindow(context);
  const rect = win.getTitleButtonRect();
  return rect; // 类型错误！
}
```

### 解决方案

参考本仓库的解决方案，AI 可以生成正确的代码：

```typescript
// ✅ 正确的代码
async function getTitleButtonRect(context: UIAbilityContext): Promise<window.TitleButtonRect> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(new Error(err.message));
        return;
      }
      const titleButtonRect = win.getTitleButtonRect();
      resolve(titleButtonRect); // 正确！
    });
  });
}
```

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

如果你发现了新的常见错误类型，或者有更好的解决方案，欢迎贡献：

1. Fork 本仓库
2. 创建你的特性分支 (`git checkout -b feature/AmazingError`)
3. 提交你的更改 (`git commit -m 'Add some AmazingError'`)
4. 推送到分支 (`git push origin feature/AmazingError`)
5. 开启一个 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情

---

**让 AI 开发 HarmonyOS 应用更高效！** 🚀

