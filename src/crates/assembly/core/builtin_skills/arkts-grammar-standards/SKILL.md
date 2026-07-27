---
name: arkts-grammar-standards
description: Load this skill when writing or modifying .ets files. Use it for ArkTS syntax rules, ArkTS-specific restrictions, TypeScript-to-ArkTS syntax differences, syntax compliance review, and ArkTS syntax questions.
---

# arkts-grammar-standards

Use this skill before authoring ArkTS code and to answer ArkTS syntax and restriction questions with grounded references.

## Core authoring checklist

Before writing or modifying `.ets` files:

- Treat the code as ArkTS, not generic TypeScript.
- Do not use `any` or `unknown` unless the user explicitly allows it.
- Do not use `as` type assertions; use explicit types, constructors, or typed helper functions.
- Do not rely on structural typing; prefer named classes, interfaces, and explicit `implements` relationships.
- Do not use dynamic property access such as `obj[key]` as a normal modeling pattern; prefer direct property access with known names.
- Give object literals explicit type context through typed variables, typed parameters, or class/interface construction.
- Do not use inline object literal types; define a named interface or class instead.
- Do not use template literals such as `` `${value}` ``; use string concatenation and explicit conversion.
- Do not use namespaces as runtime values; import or reference the concrete exported value/type that is needed.
- Avoid restricted TypeScript patterns such as destructuring declarations, destructuring parameters, function expressions, nested local function declarations, class expressions, `delete`, `in`, `for...in`, and type queries like `typeof Foo`.

Prefer the bundled reference files over model memory. Keep the answer focused on:

- whether a syntax form is allowed
- what ArkTS expects instead
- whether the rule comes from the language guide or from the linter-derived summary
- which topic best matches the user's code or question

## Reference order

Read these files as needed:

1. `references/topic-aliases.json`
2. `references/basic-syntax.md`
3. `references/restrictions.md`
4. `references/ts-diff.md`

Use `basic-syntax.md` for normal ArkTS writing patterns.
Use `restrictions.md` when the question is about forbidden syntax, restricted operators, object literal rules, `Sendable`, or review comments.
Use `ts-diff.md` when the user is porting TypeScript or asking why a familiar TypeScript pattern does not work in ArkTS.

## Source rules

- Treat `basic-syntax.md` and `ts-diff.md` as guide-oriented summaries backed by the bundled ArkTS language guide sections.
- Treat `restrictions.md` as implementation-derived guidance based on the linter summary. Say that clearly when citing it.
- Do not present linter-derived restrictions as if they were verbatim official spec text.
- If both a guide-oriented explanation and a linter restriction apply, mention both and explain the relationship in one or two sentences.

## Response shape

Use this format unless the user asks for something else:

```markdown
- Topic: <short topic>
- Source: <guide-summary | linter-summary | ts-diff-summary>
- Reference: <reference file and section>
- Why it matches: <one sentence>
- Guidance: <one or two sentences>
```

If the user shows code, add a short rewrite suggestion after the guidance.

## Working rules

- Prefer direct syntax guidance over broad language tutorials.
- Prefer named ArkTS alternatives such as class, interface, explicit field type, arrow function, or direct property access.
- Keep citations short and traceable.
- Do not expand the answer into build, run, debug, or tool workflows unless the user explicitly asks for that after the syntax answer.
