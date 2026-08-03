---
name: deveco-create-project
description: Load this skill when creating, initializing, or scaffolding an ArkTS project, including "0-1", "from scratch", "new ArkTS project", "新建工程", "创建项目", and empty directory initialization tasks. Load this skill even if the target directory already exists — never assume an existing same-named directory is the user's intended project and skip to `devecocli build`/`run`. If the user provides a Chinese or other non-ASCII project name (e.g. 购物车, 天气预报), you MUST propose 2-3 UpperCamelCase ASCII candidates (e.g. 购物车 → ShoppingCart / ShopCart / Cart) and let the user choose via AskUserQuestion BEFORE invoking the script — never pass non-ASCII names through to the script, and never pick a single translation on the user's behalf. Use the skill's private POSIX sh script to create ArkTS projects reliably.
---

# deveco-create-project

Use the skill's private POSIX sh script to create an ArkTS project, instead of relying on the model to copy template files one by one.

## Required Parameters

Confirm the following parameters before execution. Ask the user if any required value is missing:

| Parameter | Required | Default | Example |
|------|---------|--------|------|
| `projectPath` | Required | — | `/Users/yellow/Desktop/projects` |
| `appName` | Required | — | `HelloWorld` |
| `bundleName` | Auto-derived, no need to ask | `com.example.{appName lowercase}` | `com.example.helloworld` |
| `apiLevel` | Optional | fallback to `22` | `21` |

### appName rules

`appName` must match `^[A-Za-z][A-Za-z0-9_]{0,127}$`. Chinese / non-ASCII names are NOT allowed — the script will reject them (exit code `4`, `APP_NAME_INVALID`).

When the user provides a Chinese or other non-ASCII name, you MUST:
1. Propose 2-3 UpperCamelCase ASCII candidates based on meaning (e.g. `购物车` → `ShoppingCart` / `ShopCart` / `Cart`; `天气预报` → `WeatherForecast` / `Weather` / `Forecast`). Fall back to pinyin only when meaning is unclear.
2. Let the user pick one via `AskUserQuestion` before invoking the script — do NOT pick on the user's behalf, even if one option seems obviously best.
3. Never pass the original non-ASCII name to the script.

### Target directory conflict

If `{projectPath}/{appName}` already exists and is not empty, the script will exit with code `2` and emit a `PROJECT_EXISTS` JSON payload. When you see it, ask the user via `AskUserQuestion` whether to overwrite, rename, or cancel — do NOT silently re-run or delete the directory yourself.

If the user explicitly specifies an SDK/API level, pass it through directly.
If the user does not specify one, do not let the model invent a version. Let the script use its OHOS fallback API level `22`.

The script's stdout JSON (`apiLevel`, `source`) is authoritative.

### Optional: Brief Requirement Checklist for Complex App Requests

If the current session is already executing an approved Plan Mode plan or an existing plan file is referenced, do not create another plan, do not call `plan_enter` or `plan_write`, and do not ask for plan approval again. Treat the existing plan as the source of truth.

If there is no existing approved plan and the user asks to create a new project with a complex app requirement, make a brief requirement checklist before copying or editing files.

The checklist must list:
- pages to implement
- the first screen / entry page
- navigation between pages
- key feature points for each page
- verification points for pages and navigation

Keep this checklist concise and continue automatically unless required project parameters are missing or the requirement is contradictory.
Do not expand this skill into ArkUI design guidance; load `arkui-knowledge` before implementing UI code.

## Execution Steps

> `copy-template.sh` reads the sibling skill directory `deveco-create-project/application/` as the template source by default.
> This repository targets HarmonyOS/OHOS. Use `sh` to run the private script; do not use Node.js for project creation in the agent worker.
> Default skills are extracted to a local user skill directory before execution. Keep all scripts in this skill self-contained and do not import repo-only source files.

### Step 1: Run the Private Script

Run the following with Shell:

```bash
sh "{SKILL_DIR}/scripts/copy-template.sh" --project-path "{projectPath}" --app-name "{appName}" --bundle-name "{bundleName}" --api-level "{apiLevel}"
```

If `apiLevel` is not explicitly provided by the user, omit `--api-level` and let the script use the OHOS fallback value `22`.

Execution requirements:

- Do not manually copy template files one by one.
- Let the script handle recursive copying, binary asset copying, placeholder replacement, and basic validation.
- The script is responsible for selecting the API level. Do not decide the SDK version in the prompt by guesswork.
- If the script exits with a non-zero code, report the error to the user and stop.

### Step 2: Verify the Result

At minimum, verify that the following file exists:

- `{projectPath}/{appName}/build-profile.json5`

If the file is missing, treat the creation as failed and do not proceed to later compile or page-generation steps.

If the script reports `source: "fallback"`, it used the OHOS default API level `22`.

### Step 3: Switch to Project Directory (Required)

After project creation succeeds, use Shell to `cd` to the generated project root (`{projectPath}/{appName}`) before any `devecocli` command.

Reason:

- `devecocli build` / `devecocli run` must run from the directory that contains `build-profile.json5`.
- Without `cd` to the generated project, subsequent build/run may target the wrong directory.

### Step 4: Continue Feature Work in the Generated Project

If the user's request includes app behavior, UI, pages, or business requirements in addition to project creation, continue only after `cd` to the project root.

Before implementing the feature:

- Read `entry/src/main/resources/base/profile/main_pages.json` to identify the launch page list.
- Read the launch page file, usually `entry/src/main/ets/pages/Index.ets` and `entry/src/main/ets/entryability/EntryAbility.ets`.
- Modify the actual launch page or its navigation path so the requested feature is reachable from the first screen.

> **CRITICAL: `EntryAbility.ets` and `main_pages.json` must stay in sync.**
>
> `EntryAbility.ets` calls `windowStage.loadContent('pages/SomePage', ...)` to load the first screen.
> That page path **must** appear in `main_pages.json`'s `src` array — otherwise the framework silently fails to load the page, resulting in a **white screen**.
>
> When you create custom pages and update `main_pages.json`, you **must** also update `EntryAbility.ets`:
> - If you **rename or replace** the first entry in `main_pages.json`, update `loadContent()` to match the new first page.
> - If you **prepend** a new splash/landing page to `main_pages.json`, update `loadContent()` to point to that page.
>
> Always re-read both files after editing to confirm they are consistent.

- Do not finish by only creating a new named page/component unless the launch page routes to it.

> **CRITICAL: Desktop app name — both `app_name` and `EntryAbility_label` must be updated.**
>
> The desktop icon label is controlled by `EntryAbility_label`, not `app_name`:
> - `AppScope/resources/base/element/string.json` → `app_name` — used as the **application-level** label (settings, etc.).
> - `entry/src/main/resources/base/element/string.json` → `EntryAbility_label` — used as the **Ability-level** label (**this is what appears on the desktop icon**).

- After changes, load skill `deveco-cli` if not already loaded; Bash `devecocli build`; if it succeeds, `devecocli run --skip-build`.

### Step 5: Report Back to the User

Report after all requested creation, implementation, build, run, and verification work is complete, or immediately when a blocking failure stops the flow.

Output:

- The absolute project path
- App name / bundle name / API Level
- `source` of the selected API level: `user_input` / `sdk_pkg` / `fallback`
- Whether the template integrity check passed
- Whether `cd` to project root succeeded before build/run
- Build/run/verification status when feature work was requested
