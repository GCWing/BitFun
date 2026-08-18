**中文** | [English](README.md)

# 设计令牌

该目录包含 BitFun 组件库的设计令牌，统一管理颜色、字体、间距、阴影、动效与层级。

## 文件

- `tokens.scss`: 设计令牌与组合令牌定义
- `_overlay-surfaces.scss`：临时弹层 Surface 的语义化外观契约

## 弹层 Surface

按交互语义选择契约，不按尺寸或组件名称选择：

- `floating-surface`：所有非模态临时卡片，包括跟随触发位置的菜单、选择器、Popover，以及通知、Toast 和状态提示；在统一弹出卡片外壳上按需增加毛玻璃。
- `dialog-surface`：模态或捕获焦点的 Surface，通常居中显示。它使用不透明的高层背景，确保应用内容不会透出；描边、12px 圆角和阴影仍与设备状态卡片读取同一份定义。
- 透明原生窗口在毛玻璃会触发整窗重合成时可传入 `$backdrop-blur: false`；描边、圆角、背景与阴影仍读取同一个内部定义。

公开的弹出卡片视觉契约只保留这两套。业务样式可以定义尺寸、定位、内容布局和状态，但不得重新定义外层描边、圆角、背景或阴影。内容卡片、Tooltip 和全屏 Surface 不属于弹出卡片，继续使用各自语义契约。

## 使用

### 在组件中引入

```scss
@import '../../styles/tokens.scss';

.my-component {
  background: $color-bg-primary;
  color: $color-text-primary;
  border: 1px solid $border-base;
  padding: $size-gap-4;
  border-radius: $size-radius-base;
  box-shadow: $shadow-base;
  transition: all $motion-base $easing-standard;
}
```

### 组合令牌

```scss
@import '../../styles/tokens.scss';

.card {
  background: var(--bf-appearance-token-element-bg-subtle);
  border: 1px solid var(--bf-appearance-token-border-base);
  box-shadow: var(--bf-appearance-token-shadow-sm);
}
```

### 导出为 CSS 变量（可选）

```scss
@import '../../styles/tokens.scss';

:root {
  @include apply-design-tokens;
}
```

## 命名规范

- 基础：`$color-*`、`$size-*`、`$font-*`、`$shadow-*`、`$motion-*`、`$easing-*`、`$z-*`
- 组合：`$panel-*`、`$card-*`、`$input-*`、`$modal-*`、`$nav-*`、`$button-*`

## 最佳实践

- 优先使用基础令牌
- 常见场景使用组合令牌
- 避免硬编码并保持语义化

## 扩展

1. 在 `tokens.scss` 中新增变量
2. 遵循命名规范
3. 需要时补充组合令牌
4. 更新 `DesignTokens` 预览
