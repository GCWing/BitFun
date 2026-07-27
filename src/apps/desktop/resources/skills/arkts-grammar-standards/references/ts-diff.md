# TypeScript to ArkTS Syntax Differences

This file highlights TypeScript patterns that often need rewriting in ArkTS.

## Dynamic JavaScript patterns are narrower

- ArkTS is not a drop-in TypeScript superset in practice.
- Patterns that depend on dynamic object mutation, loose shape matching, or reflective syntax are often rejected.
- Patterns based on `Record<string, T>`, arbitrary indexes, or namespace objects often need explicit ArkTS models.

Reference:
- Guide: `docs/ArkTS-Language-Guide/09-Migration-Guide/01-TypeScript-to-ArkTS/typescript-to-arkts-migration-guide.md`
- Linter summary: overview section

## Prefer named types over inline shapes

TypeScript style:

```ts
function printPoint(point: { x: number; y: number }): void {}
```

ArkTS style:

```ts
interface PointLike {
  x: number
  y: number
}

function printPoint(point: PointLike): void {}
```

Reference:
- Guide: `docs/ArkTS-Language-Guide/02-Basic-Syntax/interfaces.md`

## Prefer declarations over expressions

- Rewrite function expressions to arrow functions when the function is used as a value.
- Rewrite class expressions to named class declarations.
- Move nested function declarations to top-level declarations, class methods, or arrow-function values.

Reference:
- Guide: `docs/ArkTS-Language-Guide/02-Basic-Syntax/classes.md`
- Linter summary: function and class sections

## Prefer explicit class and interface modeling

- TypeScript often treats classes as object shapes; ArkTS prefers constructor-based initialization and declared members.
- TypeScript often relies on structural typing; ArkTS narrows that style and expects explicit named contracts.
- TypeScript often uses `Record` or indexed access for flexible maps; ArkTS authoring should prefer named interfaces/classes and direct property access when property names are known.

Reference:
- Guide: `docs/ArkTS-Language-Guide/02-Basic-Syntax/classes.md`
- Guide: `docs/ArkTS-Language-Guide/02-Basic-Syntax/interfaces.md`
- Linter summary: structural typing restriction

## Rewrite template strings

TypeScript style:

```ts
const label: string = `Count: ${count}`
```

ArkTS style:

```ts
const label: string = "Count: " + count.toString()
```

Reference:
- Linter summary: syntax restriction section

## Rewrite namespace-as-value patterns

TypeScript style:

```ts
const api = common
api.doWork()
```

ArkTS style:

```ts
common.doWork()
```

If the code needs a runtime object, define that object explicitly instead of using a namespace as the value.

Reference:
- Linter summary: namespace section

## Remove unsupported or narrowed syntax

Common rewrites:

- `var` -> `let` or `const`
- destructuring declaration -> explicit local bindings
- `value as T` -> typed variable, typed function return, constructor, or explicit conversion
- `` `${value}` `` -> string concatenation with explicit conversion
- `delete obj.x` -> construct the desired value without runtime property deletion
- `catch (err: Error)` -> `catch (err)`
- `type T = typeof Foo` -> use the explicit type you need instead of a type query

Reference:
- Linter summary: declaration, operator, and statement sections
