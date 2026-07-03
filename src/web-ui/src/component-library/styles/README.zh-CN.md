**中文** | [English](README.md)

# 设计令牌

该目录包含 BitFun 组件库的设计令牌，统一管理颜色、字体、间距、阴影、动效与层级。

## 文件

- `tokens.scss`: 设计令牌与组合令牌定义

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
  background: $card-bg-default;
  border: 1px solid var(--border-base);
  box-shadow: var(--shadow-sm);
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
