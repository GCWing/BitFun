# ArkTS Restrictions

This file summarizes ArkTS-specific restrictions that are especially useful in code review and syntax troubleshooting.

Unless noted otherwise, the rules below are derived from the bundled linter summary rather than copied from the official guide text.

## Declarations

- Do not use `var`; use `let` or `const`.
- Do not use destructuring in variable declarations.
- Do not use destructuring in parameters.
- Do not rely on `any` or `unknown`.
- Do not use `as` type assertions as a shortcut around ArkTS type checking.
- Avoid definite assignment assertions like `!`; in `Sendable` classes they are not allowed.

Reference:
- Linter summary: declaration section
- Rules: `arkts-no-var`, `arkts-no-destruct-decls`, `arkts-no-destruct-params`, `arkts-no-any-unknown`, `arkts-no-definite-assignment`
- Project authoring rule: avoid `as` type assertions unless explicitly requested.

## Functions and classes

- Do not use nested local function declarations.
- Do not use function expressions; prefer arrow functions.
- Do not use generator functions or `yield`.
- Do not use class expressions.
- Do not use standalone `this` in free functions.

Reference:
- Linter summary: function and class sections
- Rules: `arkts-no-nested-funcs`, `arkts-no-func-expressions`, `arkts-no-generators`, `arkts-no-class-literals`, `arkts-no-standalone-this`

## Object and property access

- Do not treat object literals as free-form structural types.
- Do not declare inline object literal types in place of named interfaces or classes.
- Object literals must have explicit type context, such as a typed variable, typed parameter, or declared class/interface target.
- Do not depend on dynamic property access as a normal modeling pattern.
- Avoid `Record<string, T>` when it encourages dynamic indexing; define a named type with known properties instead.
- Prefer identifier property names and direct dot access.

Reference:
- Linter summary: object literal and property access sections
- Rules: `arkts-no-structural-typing`, `arkts-no-untyped-obj-literals`, `arkts-no-obj-literals-as-types`, `arkts-no-props-by-index`, `arkts-identifiers-as-prop-names`

## String syntax

- Template literals are not supported for ArkTS authoring in this project.
- Rewrite interpolation to string concatenation with explicit conversion.

Example rewrite:

```ts
const label: string = "Likes: " + likeCount.toString()
```

Reference:
- Linter summary: syntax restriction section

## Namespaces

- Do not use a namespace itself as a runtime value.
- Avoid namespace bodies that contain runtime statements; keep namespace-like organization to supported declarations.
- Prefer importing concrete exported symbols or defining explicit runtime classes/objects.

Reference:
- Linter summary: namespace section
- Rules: `arkts-no-ns-statements`

## Restricted operators and statements

- `delete` is not supported.
- `in` and `for...in` are restricted.
- `typeof` is allowed in expression context, but not as a type query.
- `catch` clauses must not carry an explicit exception type annotation.
- Destructuring assignment is not supported.

Reference:
- Linter summary: operators and statements sections
- Rules: `arkts-no-delete`, `arkts-no-in`, `arkts-no-types-in-catch`, `arkts-no-destruct-assignment`, `arkts-no-type-query`

## Sendable-focused restrictions

- `Sendable` classes must use explicit field types.
- `Sendable` field types must themselves be sendable.
- `Sendable` types must not be initialized directly from object literals or array literals.
- `Sendable` classes and functions have stricter capture and inheritance rules.

Reference:
- Linter summary: sendable sections
- Rules: `arkts-sendable-explicit-field-type`, `arkts-sendable-prop-types`, `arkts-sendable-obj-init`, `arkts-sendable-class-inheritance`

## How to cite this file

When using this file in an answer, say that the restriction comes from the linter-derived ArkTS summary.
Use that wording especially for:

- forbidden syntax claims
- `Sendable` restrictions
- dynamic object model limitations
