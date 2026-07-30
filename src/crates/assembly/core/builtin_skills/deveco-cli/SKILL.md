---
name: deveco-cli
description: >-
  **MUST** use 'devecocli' for HarmonyOS apps (scaffold, build, run, debug, devices, logs, docs, 鸿蒙开发文档, skills). **REQUIRED** and **MANDATORY** if workspace has build-profile.json5 or oh-package.json5, or user mentions HarmonyOS, 鸿蒙, DevEco, ArkTS, ArkUI, docs, 鸿蒙文档, 知识, 搜索, or skills.
---

# DevEco CLI

`devecocli` wraps DevEco Studio's `hvigor`, `ohpm`, `hdc` toolchain, and HarmonyOS-skills installer. **Prefer `devecocli` over invoking underlying tools directly.**

Available commands: `build`, `run`, `update`, `device`, `skills`, `log`, `create`, `init`, `serve`, `docs`.

**Sandbox Rule**: Commands tagged `[Outside sandbox]` must be run outside the sandbox.

## 1. Code → Build → Deploy → Run → Debug

### `devecocli create`
Scaffold a new HarmonyOS project.
- `--app-name <name>` (Req): 1–200 chars, `^[a-zA-Z][a-zA-Z0-9_]*$`
- `--project-path <path>`: Auto-created if omitted (`./<app-name>`). Must be empty if exists.
- `--bundle-name <bundle>`: Default `com.example.<appname-lowercase>`. 7–128 chars, ≥3 segments.
- `--api-level <level>`: int ≥17 (default: auto or 23).
*Ex*: `devecocli create --app-name MyApp --project-path ./CustomDir --api-level 23`

### `devecocli build` `[Outside sandbox]`
Compile and package project/modules. (Defaults: `--product default`, `--build-mode debug`)
| Goal | Command |
|---|---|
| Single-module / single-`entry` | `devecocli build` |
| Specific modules | `devecocli build --modules <m1> <m2>@<target>` |
| Whole product bundle (.app) | `devecocli build --product <name>` |
| Clean build outputs | `devecocli build clean` |

### `devecocli docs`
Search/read local HarmonyOS docs.
- `search <keywords...>`: Match any keyword. Opts: `--catalog <name>`, `--format <default|json>`, `--limit <n>`.
- `read <documentId>`: Read full content by ID (e.g. `devecocli docs read 开发指南/冷启动_Launch分析/Launch模板基本操作/ide-insight-session-launch`).
- `catalog`: List available catalogs.

### `devecocli device`
- `list`: Show active real devices.
- `view`: Detailed info. Req `-t <name|serial>` on multi-device hosts.

### `devecocli run` `[Outside sandbox]`
Build, install, and launch.
- `--module <module>`: Target module (auto-selected if only one runnable).
- `--device <name|serial>`: Target device (Req if multiple connected).
- `--product <product>` / `--build-mode <mode>`: Defaults: `default` / `debug`.
- `--ability <ability>`: Default from `module.json5`.
- `--uninstall`: Uninstall existing app first (Fixes signing key issues).
- `--skip-build`: Deploy existing artifacts.

### `devecocli log`
Fetch hilog or crash logs. Req `--device <name|serial>` on multi-device hosts.
- `--crash`: Dump crash logs.
- `--level D|I|W|E|F`: Filter by level.
- `--bundle-name` / `--keyword`: Filter output.
- `--from <start>` / `--to <end>`: Relative offsets (`30s`, `5m`).
- `--tail <num>` / `--follow`: Keep last N lines / stream real-time (no `--to`).
*Ex*: `devecocli log --crash --bundle-name com.example.app`, `devecocli log --level E --from 5m --tail 200`

## Fallback: hdc when devecocli device list finds no devices

When `start_app` or `hdc_log` reports no devices found (from `devecocli device list`), try `hdc` directly via `ExecCommand` to discover and use devices. Do NOT hardcode paths — discover them at runtime.

### Discover devices
```
hdc list targets
```
If `hdc` is not in PATH, check if the user has DevEco SDK installed and ask where `hdc` is located.

### If hdc also finds no devices
Prompt the user to connect via wireless debugging:
1. Ask the user to open **系统设置 → 系统 → 开发者选项 → 无线调试** on the HarmonyOS device.
2. Ask the user for the **IP 地址和端口号** shown on that screen (e.g. `192.168.1.100:5555`).
3. Connect using:
   ```
   hdc tconn <ip:port>
   ```
4. Verify the device appears:
   ```
   hdc list targets
   ```

### If a device is found via hdc but not via devecocli
Deploy and launch the app directly with `hdc`:

1. **Find the HAP** — search build outputs:
   ```
   find . -name "*.hap" -path "*/outputs/*"
   ```
   Pick the one matching the target module/product/build-mode. If multiple, choose the most recently built one.

2. **Read bundleName and ability** from project config:
   - bundleName: `AppScope/app.json5` → `app.bundleName`
   - ability name: `module.json5` (in the entry module) → `module.abilities[0].name`
   Use `Read` tool or `cat` via `ExecCommand` to inspect these files.

3. **Install** the HAP:
   ```
   hdc install "<hap_path_from_step_1>"
   ```
   - Signing key changed: `hdc shell bm uninstall -n <bundleName>` first.
   - Already installed: `hdc install -r "<hap_path>"`.

4. **Launch** the ability:
   ```
   hdc shell aa start -a <ability_from_step_2> -b <bundleName_from_step_2>
   ```

5. **Verify** the app is running:
   ```
   hdc shell ps -ef | grep <bundleName>
   ```

## 2. Setup

### `devecocli init`
MUTUALLY EXCLUSIVE modes for setup:
1. `--skill` (Default): Install `deveco-cli` skill to AI agents.
2. `--mcp`: Configure `deveco-mcp` server (ArkTS/C++ syntax checking).
*Options*:
- `--agent <agents>`: Comma-separated (e.g. `opencode,cursor`). Omitting targets all.
- `--project <path>`: Project-level config (Abs path for MCP).
- `--path <path>`: Direct skill install path.
- `-f, --force`: Overwrite existing config.
*MCP Rules*: Global MCP (no `--project`) only supports `opencode` and `cursor`. Others require `--project`.

### `devecocli skills`
Manage HarmonyOS skills in AI agents/projects.
- `list [-l|--long]` / `find <keyword>`: List or search skills.
- `add (--all | --skill <name>) [--agent <a,b…>] [--project <path>] [--path <path>] [-f]`: Install.
- `remove --skill <name> [...]`: Uninstall.

## 3. Maintenance

- **`devecocli update`** `[Outside sandbox]`: Update CLI to latest version.
- **`devecocli serve mcp`**: Host stdio MCP server (`check` tool for `.ets`/C/C++). Used via `init --mcp`. (Env: `PROJECT_PATH`, `DEVECO_PATH`, `NODE_MAX_OLD_SPACE_SIZE`, `DEBUG=1`).

## Recipes

- **Build and run on connected device**:
  `devecocli build` -> `devecocli run`
- **Diagnose crash**:
  `devecocli log --crash --bundle-name <bundle>`
- **Release build**:
  `devecocli build --product oversea --build-mode release`

## Troubleshooting

- **"Product / Build mode `<x>` not found"**: Check `build-profile.json5`.
- **"Multiple entry modules" / "No entry module"**: Pass `--modules` (build) or `--module` (run).
- **"No active devices" / "Multiple devices connected"**: Connect a device with debugging enabled. Pass `-t <serial>` (device view) or `--device <name|serial>` (run/log). If `devecocli device list` shows nothing, try `hdc list targets` (see Fallback section above).
- **`error:install sign info inconsistent`**: Signing key changed. Run `devecocli run --uninstall`.
- **`skills add` agent not found**: Valid: `codebuddy`, `cursor`, `opencode`, `qoder`, `trae-cn`.
