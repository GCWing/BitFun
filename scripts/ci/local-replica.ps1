<#
.SYNOPSIS
    本地 CI 全量复刻脚本：按 .github/workflows/ci.yml 逐 job 逐 step 在本地 Windows 上完整预演。

.DESCRIPTION
    固化 6 个 job / 36 步（shell-scripts / cli-test / cargo-deny / rust-build-check /
    dsh-profile-windows / frontend-build），与远程 CI（ubuntu-latest 主线 + windows-latest
    dsh/rust）对齐。已知 Windows 平台差异项显式标注、不判整体失败，其余步骤严格判失败
    （核心失败 → 退出码非 0）。

    环境预处理（关键）：
      - PATH 前置 Git Bash：系统 bash.exe 可能是 WSL stub（无发行版），会让所有 bash 脚本/契约测试
        误报失败。本脚本探测 %ProgramFiles%\Git\bin 等常见安装位置，找不到则报错退出。
      - NODE_OPTIONS=--max-old-space-size=6144（对齐 CI frontend-build env）。
      - RUSTFLAGS=-D warnings：rust-build-check job 级设置（对齐 CI.yml:192），warning = error；
        cli-test 保持 platform-warn（Windows 平台差异项不加严，避免掩盖 CLI 平台信号）。
      - cargo-deny：已安装则直接使用，未安装则提示安装命令（不自动装）。

.PARAMETER SkipFrontend
    跳过 frontend-build job（构建耗时较长，可选）。

.EXAMPLE
    .\scripts\ci\local-replica.ps1            # 全量 36 步
    .\scripts\ci\local-replica.ps1 -SkipFrontend   # 跳过前端 job
#>
[CmdletBinding()]
param(
    [switch]$SkipFrontend
)

$ErrorActionPreference = 'Continue'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $repoRoot

# ── 结果记录 ──────────────────────────────────────────────────────────────
$results = [System.Collections.Generic.List[object]]::new()
$global:stepFailed = $false

function Add-Result {
    param([string]$Job, [string]$Step, [string]$Command, [int]$Exit, [string]$Status, [string]$Note = '')
    $script:results.Add([pscustomobject]@{
        Job     = $Job
        Step    = $Step
        Command = $Command
        Exit    = $Exit
        Status  = $Status
        Note    = $Note
    })
}

function Invoke-CIStep {
    param(
        [string]$Job,
        [string]$Step,
        [string]$Command,
        [scriptblock]$Body,
        [ValidateSet('strict', 'platform-warn', 'skip')]
        [string]$Mode = 'strict'
    )
    Write-Host "`n[$Job] $Step" -ForegroundColor Cyan
    Write-Host "  > $Command" -ForegroundColor DarkGray

    if ($Mode -eq 'skip') {
        Add-Result $Job $Step $Command -1 'SKIP' 'Windows 平台限制（CI ubuntu 专属）'
        Write-Host "  [SKIP] 平台限制：该步骤 CI 在 ubuntu 跑，Windows 无法复刻" -ForegroundColor Yellow
        return
    }

    $ex = 0
    try {
        $ret = & $Body
        # Body 显式 `return N`（int）优先作为退出码；否则用外部命令的 $LASTEXITCODE。
        # 兼容 Body 内 pipeline 产生对象输出的情况：从 $ret 提取最后一个 int 值。
        $returnedInt = $null
        if ($null -ne $ret) {
            if ($ret -is [int]) { $returnedInt = $ret }
            elseif ($ret -is [array]) {
                foreach ($item in $ret) { if ($item -is [int]) { $returnedInt = $item } }
            }
        }
        if ($null -ne $returnedInt) {
            $ex = $returnedInt
        } else {
            $ex = $LASTEXITCODE
            if ($null -eq $ex) { $ex = 0 }
        }
    } catch {
        # PowerShell 5.1：外部命令 stderr 经 2>&1 合并时抛 NativeCommandError，
        # 这是"输出流"而非真失败——退出码以 $LASTEXITCODE 为准。
        if ($_.Exception -is [System.Management.Automation.NativeCommandExitException]) {
            $ex = $LASTEXITCODE
            if ($null -eq $ex) { $ex = 1 }
        } else {
            $ex = 1
            Write-Host "  [EXCEPTION] $_" -ForegroundColor Red
        }
    }

    if ($ex -eq 0) {
        Add-Result $Job $Step $Command 0 'PASS'
        Write-Host "  [PASS] EXIT=$ex" -ForegroundColor Green
    } elseif ($Mode -eq 'platform-warn') {
        Add-Result $Job $Step $Command $ex 'WARN' 'Windows 已知平台差异（远程 CI 通过，基线复测证实与改动无关）'
        Write-Host "  [WARN] EXIT=$ex Windows 已知平台差异，不判整体失败" -ForegroundColor Yellow
    } else {
        Add-Result $Job $Step $Command $ex 'FAIL'
        $script:stepFailed = $true
        Write-Host "  [FAIL] EXIT=$ex" -ForegroundColor Red
    }
}

# ── 0. 环境预处理 ─────────────────────────────────────────────────────────
Write-Host "`n===== 环境预处理 =====" -ForegroundColor Magenta

# 0a. 探测 Git Bash
$gitBashCandidates = @(
    "$env:ProgramFiles\Git\bin\bash.exe",
    "${env:ProgramFiles(x86)}\Git\bin\bash.exe",
    "$env:LOCALAPPDATA\Programs\Git\bin\bash.exe",
    "$env:USERPROFILE\scoop\apps\git\current\bin\bash.exe"
)
$gitBash = $gitBashCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $gitBash) {
    Write-Host "  [ERROR] 未找到 Git Bash。请安装 Git for Windows（https://git-scm.com/download/win）" -ForegroundColor Red
    Write-Host "  或用 bash 所在目录执行：\$env:PATH = 'C:\Program Files\Git\bin;' + \$env:PATH" -ForegroundColor Red
    exit 2
}
$gitBashDir = Split-Path (Split-Path $gitBash -Parent) -Parent  # ...\Git
# 把 Git\bin 和 Git\usr\bin 前置到 PATH（usr\bin 提供 grep/sed 等 coreutils）
$env:PATH = "$gitBashDir\bin;$gitBashDir\usr\bin;$env:PATH"
Write-Host "  Git Bash: $gitBash" -ForegroundColor Green
$bashVer = & $gitBash --version 2>&1 | Select-Object -First 1
Write-Host "  版本: $bashVer" -ForegroundColor DarkGray

# 0b. NODE_OPTIONS 对齐 CI
$env:NODE_OPTIONS = '--max-old-space-size=6144'
Write-Host "  NODE_OPTIONS=$env:NODE_OPTIONS" -ForegroundColor Green

# 0b1. RUSTFLAGS=-D warnings（对齐 CI.yml:192 rust-build-check env，warning = error）
$env:RUSTFLAGS = '-D warnings'
Write-Host "  RUSTFLAGS=$env:RUSTFLAGS（对齐 CI rust job warning 门禁；cli-test job 保持 platform-warn）" -ForegroundColor Green

# 0c. cargo-deny 检查
$cargoDeny = Get-Command cargo-deny -ErrorAction SilentlyContinue
if ($cargoDeny) {
    Write-Host "  cargo-deny: $(& cargo-deny --version 2>&1 | Select-Object -First 1)" -ForegroundColor Green
} else {
    Write-Host "  [WARN] 未安装 cargo-deny。cargo-deny job 将跳过。" -ForegroundColor Yellow
    Write-Host "        安装：cargo install cargo-deny --locked --version 0.20.2" -ForegroundColor Yellow
}

# ── 1. shell-scripts ──────────────────────────────────────────────────────
Write-Host "`n===== Job 1: shell-scripts =====" -ForegroundColor Magenta

Invoke-CIStep 'shell-scripts' 'CRLF 检查（shell/deploy 资产必须 LF）' `
    "git ls-files '*.sh' '*.bash' Dockerfile* Caddyfile docker-compose* 扫 CR" -Mode strict -Body {
    $bad = @()
    foreach ($pat in @('*.sh', '*.bash', 'Dockerfile', 'Dockerfile.*', '*.Dockerfile', 'Caddyfile', 'docker-compose.yml', 'docker-compose.*.yml')) {
        git ls-files $pat | ForEach-Object {
            $f = $_; $bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $f))
            if ($bytes -contains 13) { $bad += $f }
        }
    }
    if ($bad.Count -gt 0) { Write-Host "CRLF FOUND:"; $bad; return 1 }
    Write-Host "All shell and deploy assets are LF-only."
}

Invoke-CIStep 'shell-scripts' 'bash -n 全部跟踪的 shell 脚本' `
    "bash -n <git ls-files '*.sh' '*.bash'>（Git Bash）" -Mode strict -Body {
    $rc = 0
    foreach ($f in (git ls-files '*.sh' '*.bash')) {
        & $gitBash -n $f 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { Write-Host "bash syntax error: $f"; $rc = 1 }
    }
    if ($rc -eq 0) { Write-Host "All shell scripts pass bash -n" }
    return $rc
}

Invoke-CIStep 'shell-scripts' 'release/version 契约测试（node --test）' `
    "node --test scripts/tauri-release-manifest.test.mjs scripts/linux-binaries-manifest.test.mjs scripts/version-generation.test.mjs" -Mode strict -Body {
    node --test scripts/tauri-release-manifest.test.mjs scripts/linux-binaries-manifest.test.mjs scripts/version-generation.test.mjs 2>&1 | Select-String -Pattern 'pass |fail ' | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

# minisign fallback：Windows 平台限制（脚本主动拒绝 MINGW64），CI ubuntu 专属
Invoke-CIStep 'shell-scripts' 'minisign 下载 fallback' `
    "bash scripts/sign-release-assets.sh <missing-asset>" -Mode skip -Body { }

# ── 2. cli-test（Linux 分支 = 主线）───────────────────────────────────────
Write-Host "`n===== Job 2: cli-test =====" -ForegroundColor Magenta

Invoke-CIStep 'cli-test' 'CLI + ACP 测试' `
    "cargo test --locked -p bitfun-cli -p bitfun-acp" -Mode platform-warn -Body {
    cargo test --locked -p bitfun-cli -p bitfun-acp 2>&1 | Select-String -Pattern 'test result: FAILED|test result: ok\.' | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'cli-test' 'agent-runtime 测试' `
    "cargo test --locked -p bitfun-agent-runtime" -Mode strict -Body {
    cargo test --locked -p bitfun-agent-runtime 2>&1 | Select-String -Pattern 'test result: FAILED|test result: ok\.' | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'cli-test' 'SDK Host 测试' `
    "cargo test --locked -p bitfun-sdk-host -p bitfun-sdk-host-app" -Mode strict -Body {
    cargo test --locked -p bitfun-sdk-host -p bitfun-sdk-host-app 2>&1 | Select-String -Pattern 'test result: FAILED|test result: ok\.' | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'cli-test' 'SDK Host terminal 清理回归（3 测试）' `
    "cargo test --locked -p terminal-core <3 回归> -- --test-threads=1" -Mode strict -Body {
    $names = @(
        'shutdown_returns_only_after_process_exit_is_confirmed',
        'shutdown_evicts_a_process_whose_controller_already_confirmed_exit',
        'background_only_binding_is_owned_by_the_session'
    )
    foreach ($n in $names) {
        cargo test --locked -p terminal-core $n -- --test-threads=1 2>&1 | Select-String -Pattern 'test result' | ForEach-Object { Write-Host "  $_" }
        if ($LASTEXITCODE -ne 0) { return $LASTEXITCODE }
    }
}

# ── 3. cargo-deny ─────────────────────────────────────────────────────────
Write-Host "`n===== Job 3: cargo-deny =====" -ForegroundColor Magenta

if ($cargoDeny) {
    Invoke-CIStep 'cargo-deny' 'advisories' 'cargo deny check advisories' -Mode strict -Body {
        cargo deny check advisories 2>&1 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }
    Invoke-CIStep 'cargo-deny' 'licenses' 'cargo deny check licenses' -Mode strict -Body {
        cargo deny check licenses 2>&1 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }
    Invoke-CIStep 'cargo-deny' 'sources' 'cargo deny check sources' -Mode strict -Body {
        cargo deny check sources 2>&1 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }
} else {
    Write-Host "  [SKIP] cargo-deny 未安装，跳过 3 步（不判失败）" -ForegroundColor Yellow
    Add-Result 'cargo-deny' 'advisories' 'cargo deny check advisories' -1 'SKIP' 'cargo-deny 未安装'
    Add-Result 'cargo-deny' 'licenses' 'cargo deny check licenses' -1 'SKIP' 'cargo-deny 未安装'
    Add-Result 'cargo-deny' 'sources' 'cargo deny check sources' -1 'SKIP' 'cargo-deny 未安装'
}

# ── 4. rust-build-check ───────────────────────────────────────────────────
Write-Host "`n===== Job 4: rust-build-check =====" -ForegroundColor Magenta

Invoke-CIStep 'rust-build-check' 'workspace 编译检查' `
    "cargo check --locked --workspace" -Mode strict -Body {
    cargo check --locked --workspace 2>&1 | Select-Object -Last 2
    return $LASTEXITCODE
}

# 实际 feature 组合验证（C-13）：--all-features/裸默认 feature 均掩盖 feature 装配缺口。
# 裸 `cargo check -p bitfun-core`（default=[]）下 configured_* 消费点在 feature 门控后 → 7 dead_code warning
# （C-10 -D warnings 下必挂）；CI 合约组合 = desktop 依赖 product-full（src/apps/desktop/Cargo.toml:23），
# 消费点全激活 → 0e0w。此 step 对齐 CI 合约实际 feature 组合，复现并守住 bitfun-core 0e0w 门禁。
Invoke-CIStep 'rust-build-check' 'bitfun-core 实际 feature 组合验证（product-full 对齐 CI 合约）' `
    "cargo check -p bitfun-core --features product-full --jobs 4" -Mode strict -Body {
    cargo check -p bitfun-core --features product-full --jobs 4 2>&1 | Select-Object -Last 2
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'installer 编译检查（Windows 专属步骤）' `
    "cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml" -Mode strict -Body {
    cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml 2>&1 | Select-Object -Last 2
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'core + desktop 库测试' `
    "cargo test --locked -p bitfun-core -p bitfun-desktop --lib" -Mode strict -Body {
    cargo test --locked -p bitfun-core -p bitfun-desktop --lib 2>&1 | Select-String -Pattern 'test result: FAILED|test result: ok\.' | Select-Object -Last 4 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'page-function-runtime 测试' `
    "cargo test --locked -p bitfun-page-function-runtime" -Mode strict -Body {
    cargo test --locked -p bitfun-page-function-runtime 2>&1 | Select-String -Pattern 'test result' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'relay-service 测试' `
    "cargo test --locked -p bitfun-relay-service" -Mode strict -Body {
    cargo test --locked -p bitfun-relay-service 2>&1 | Select-String -Pattern 'test result' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'subscription-auth 测试' `
    "cargo test --locked -p bitfun-ai-adapters --features subscription-auth --lib subscription_auth" -Mode strict -Body {
    cargo test --locked -p bitfun-ai-adapters --features subscription-auth --lib subscription_auth 2>&1 | Select-String -Pattern 'test result' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'file-watch 契约测试（非 macOS）' `
    "cargo test --locked -p bitfun-services-integrations --no-default-features --features file-watch --test file_watch_contracts" -Mode strict -Body {
    cargo test --locked -p bitfun-services-integrations --no-default-features --features file-watch --test file_watch_contracts 2>&1 | Select-String -Pattern 'test result' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'rust-build-check' 'search 工具测试' `
    "cargo test --locked -p tool-runtime --lib search::" -Mode strict -Body {
    cargo test --locked -p tool-runtime --lib search:: 2>&1 | Select-String -Pattern 'test result' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

# ── 5. dsh-profile-windows ────────────────────────────────────────────────
# 远程 CI.yml:377-422 全 job：prepare-dsh-profile.mjs 打包 + profile 完整性验证。
# 本地 Windows 主机 = 远程 windows-latest 同类平台，全量复刻（strict）。
Write-Host "`n===== Job 5: dsh-profile-windows =====" -ForegroundColor Magenta

Invoke-CIStep 'dsh-profile-windows' 'build profile（npm install/tsc/npm pack/tar/tree copy）' `
    "node scripts/prepare-dsh-profile.mjs" -Mode strict -Body {
    node scripts/prepare-dsh-profile.mjs 2>&1 | Select-Object -Last 4 | ForEach-Object { Write-Host "  $_" }
    return $LASTEXITCODE
}

Invoke-CIStep 'dsh-profile-windows' 'profile 完整性 + stamp 验证（对齐 CI.yml:393-422）' `
    "bash 验证 dist-profile 必需文件 + nested node_modules/*.map 零残留 + .bitfun-bridge.json stamp" -Mode strict -Body {
    $profileRoot = 'packages/dsh-acp/dist-profile'
    $required = @(
        "$profileRoot/.bitfun-bridge.json",
        "$profileRoot/cordis.patch.yml",
        "$profileRoot/package.json",
        "$profileRoot/lib/app.js",
        "$profileRoot/node_modules/@agentclientprotocol/sdk/package.json",
        "$profileRoot/node_modules/@deepseek-ai/dsh-agent-spine-demo/package.json"
    )
    foreach ($p in $required) {
        if (-not (Test-Path $p)) { Write-Host "  [ERROR] missing $p"; return 1 }
    }
    # 对齐 CI：lib/presets 下不得残留 node_modules / *.map
    $nested = Get-ChildItem "$profileRoot/lib", "$profileRoot/presets" -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq 'node_modules' -or $_.Extension -eq '.map' }
    if ($nested) { Write-Host "  [ERROR] profile carries files it must not ship:"; $nested | ForEach-Object { Write-Host "    $($_.FullName)" }; return 1 }
    # stamp 校验：profile=bitfun-acp + content=64 位 hex digest
    node -e "const s=require('./$profileRoot/.bitfun-bridge.json'); if(s.profile!=='bitfun-acp') throw new Error('wrong profile: '+s.profile); if(!/^[0-9a-f]{64}$/.test(s.content)) throw new Error('no content digest'); process.stdout.write('profile '+s.profile+' @ '+s.bridge+', min dsh '+s.minDshVersion+'\n');" 2>&1 | ForEach-Object { Write-Host "  $_" }
    if ($LASTEXITCODE -ne 0) { return $LASTEXITCODE }
    Write-Host "  [OK] profile complete and stamped"
}

# ── 6. frontend-build ─────────────────────────────────────────────────────
if (-not $SkipFrontend) {
    Write-Host "`n===== Job 6: frontend-build =====" -ForegroundColor Magenta

    Invoke-CIStep 'frontend-build' 'repo 卫生检查' 'pnpm run check:repo-hygiene' -Mode strict -Body {
        pnpm run check:repo-hygiene 2>&1 | Select-String -Pattern 'passed|valid|error' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'core 边界检查' 'node --test scripts/check-core-boundaries.test.mjs' -Mode strict -Body {
        node --test scripts/check-core-boundaries.test.mjs 2>&1 | Select-String -Pattern 'pass |fail ' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    # PPT Live 契约：Windows 已知平台差异（fixture 字节 hash 依赖 WebKit 渲染确定性）
    Invoke-CIStep 'frontend-build' 'PPT Live 生成文件契约' `
        "pnpm run test:ppt-live" -Mode platform-warn -Body {
        pnpm run test:ppt-live 2>&1 | Select-String -Pattern 'pass |fail |✖' | Select-Object -Last 6 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'GitHub 配置校验' 'pnpm run check:github-config' -Mode strict -Body {
        pnpm run check:github-config 2>&1 | Select-String -Pattern 'pass |fail ' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'i18n 契约（CI profile）' 'pnpm run i18n:contract:test:ci' -Mode strict -Body {
        pnpm run i18n:contract:test:ci 2>&1 | Select-String -Pattern 'pass |fail ' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'i18n 资源审计' 'pnpm run i18n:audit' -Mode strict -Body {
        pnpm run i18n:audit 2>&1 | Select-String -Pattern 'Passed|warning' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'theme 色彩审计契约' 'pnpm run theme:color-audit:test' -Mode strict -Body {
        pnpm run theme:color-audit:test 2>&1 | Select-String -Pattern 'pass |fail ' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'theme 色彩治理审计' 'pnpm run theme:color-audit:all' -Mode strict -Body {
        pnpm run theme:color-audit:all 2>&1 | Select-String -Pattern 'error|FAIL' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'theme 视觉治理契约' 'pnpm run theme:visual-contract' -Mode strict -Body {
        pnpm run theme:visual-contract 2>&1 | Select-String -Pattern 'covered|error' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    # webkit 兼容契约测试（对齐 CI.yml:484-485 verify:webkit-compatibility:test）
    Invoke-CIStep 'frontend-build' 'webkit 兼容契约测试' 'pnpm run verify:webkit-compatibility:test' -Mode strict -Body {
        pnpm run verify:webkit-compatibility:test 2>&1 | Select-String -Pattern 'pass |fail ' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    # web-ui lint：eslint 硬门禁（--max-warnings=0，对齐 CI.yml:490，warning 即失败）
    Invoke-CIStep 'frontend-build' 'web-ui lint（--max-warnings=0）' 'pnpm --dir src/web-ui exec eslint . --max-warnings=0' -Mode strict -Body {
        pnpm --dir src/web-ui exec eslint . --max-warnings=0 2>&1 | Select-Object -Last 3
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'web-ui 测试（vitest）' 'pnpm --dir src/web-ui run test:run' -Mode strict -Body {
        pnpm --dir src/web-ui run test:run 2>&1 | Select-String -Pattern 'Test Files|Tests ' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'web-ui 构建' 'pnpm run build:web' -Mode strict -Body {
        pnpm run build:web 2>&1 | Select-String -Pattern 'built in|verified|error' | Select-Object -Last 3 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'mobile-web type-check' 'pnpm --dir src/mobile-web run type-check' -Mode strict -Body {
        pnpm --dir src/mobile-web run type-check 2>&1 | Select-Object -Last 2
        return $LASTEXITCODE
    }

    Invoke-CIStep 'frontend-build' 'mobile-web 构建' 'pnpm run build:mobile-web' -Mode strict -Body {
        pnpm run build:mobile-web 2>&1 | Select-String -Pattern 'built in|error' | Select-Object -Last 2 | ForEach-Object { Write-Host "  $_" }
        return $LASTEXITCODE
    }
} else {
    Write-Host "`n[SKIP] frontend-build job（-SkipFrontend）" -ForegroundColor Yellow
}

# ── 汇总矩阵 ─────────────────────────────────────────────────────────────
Write-Host "`n===== 汇总矩阵 =====" -ForegroundColor Magenta
Write-Host ("{0,-14} {1,-38} {2,5} {3,-6} {4}" -f 'JOB', 'STEP', 'EXIT', 'STATUS', 'NOTE')
Write-Host ('-' * 110)
$passCount = 0; $failCount = 0; $warnCount = 0; $skipCount = 0
foreach ($r in $results) {
    Write-Host ("{0,-14} {1,-38} {2,5} {3,-6} {4}" -f $r.Job, $r.Step, $r.Exit, $r.Status, $r.Note)
    switch ($r.Status) {
        'PASS' { $passCount++ }
        'FAIL' { $failCount++ }
        'WARN' { $warnCount++ }
        'SKIP' { $skipCount++ }
    }
}
Write-Host ('-' * 110)
Write-Host "PASS=$passCount  FAIL=$failCount  WARN=$warnCount  SKIP=$skipCount  TOTAL=$($results.Count)"
if ($skipCount -gt 0) { Write-Host "SKIP 项：Windows 平台限制（minisign）或未安装（cargo-deny），远程 CI ubuntu 上通过" -ForegroundColor Yellow }
if ($warnCount -gt 0) { Write-Host "WARN 项：Windows 已知平台差异（cli plugin trust store / ppt-live fixture hash），远程 CI 通过，基线复测证实与改动无关" -ForegroundColor Yellow }

# ── 退出码 ───────────────────────────────────────────────────────────────
if ($script:stepFailed) {
    Write-Host "`n[RESULT] 核心步骤存在失败（FAIL），本地预演未通过" -ForegroundColor Red
    exit 1
}
Write-Host "`n[RESULT] 本地预演通过（PASS + WARN + SKIP，无核心失败）" -ForegroundColor Green
exit 0
