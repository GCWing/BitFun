# sync-upstream.ps1 — 从 BitFun upstream 同步最新 commit 到 taiji-quant 开源基座
#
# 工作流:
#   1. taiji 工作空间 rebase taiji-v1 到最新 origin/main
#   2. 计算新增 commit 的 diff（taiji-quant 上次同步点 → 最新 origin/main）
#   3. robocopy 变更文件到 taiji-quant，排除闭源 crate
#   4. 恢复 taiji-quant 元数据（版本号、包名）
#   5. cargo check 验证
#
# 用法:
#   .\scripts\sync-upstream.ps1
#   .\scripts\sync-upstream.ps1 -SkipBuild  # 跳过编译验证
#   .\scripts\sync-upstream.ps1 -DryRun     # 只显示会同步什么，不实际执行
#
param(
    [string]$TaijiWorkspace = "E:\finance-trading\lvpa\software\taiji",
    [string]$TaijiQuantWorkspace = "E:\finance-trading\lvpa\software\taiji-quant",
    [string]$SyncStateFile = "E:\finance-trading\lvpa\software\taiji-quant\.sync-upstream-state.json",
    [switch]$SkipBuild,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$ClosedCrates = @("taiji-dvmi", "taiji-magnet", "taiji-thrust", "taiji-risk")
$ExcludeDirs = @(".git", ".bitfun", "target", "node_modules") + $ClosedCrates

function Write-Step { param([string]$Msg) Write-Host "`n=== $Msg ===" -ForegroundColor Cyan }

# ── Step 0: 读取同步状态 ──────────────────────────────────────────
Write-Step "读取同步状态"
$lastSyncCommit = $null
if (Test-Path $SyncStateFile) {
    $state = Get-Content $SyncStateFile -Raw | ConvertFrom-Json
    $lastSyncCommit = $state.last_synced_commit
    Write-Host "  上次同步点: $lastSyncCommit"
} else {
    Write-Host "  首次同步，无历史记录"
}

# ── Step 1: taiji 工作空间 — fetch + rebase ────────────────────────
Write-Step "taiji 工作空间: fetch upstream"
Push-Location $TaijiWorkspace
try {
    git fetch origin +refs/heads/main:refs/remotes/origin/main 2>&1 | Out-Null
    $upstreamHead = git rev-parse origin/main
    Write-Host "  origin/main HEAD: $upstreamHead"

    # 检查是否需要 rebase
    $currentBranch = git branch --show-current
    $mergeBase = git merge-base $currentBranch origin/main
    if ($mergeBase -eq $upstreamHead) {
        Write-Host "  taiji 已是最新，无需 rebase"
    } else {
        Write-Host "  执行 rebase..."
        if ($DryRun) {
            Write-Host "  [DRY RUN] git rebase origin/main $currentBranch"
        } else {
            git rebase origin/main $currentBranch 2>&1 | Out-Null
            Write-Host "  Rebase 完成"
        }
    }

    # 推送到 fork
    Write-Host "  推送 taiji-v1 → mengdie/main..."
    if ($DryRun) {
        Write-Host "  [DRY RUN] git push mengdie taiji-v1:main --force"
    } else {
        git push mengdie taiji-v1:main --force 2>&1 | Out-Null
        Write-Host "  推送完成"
    }
} finally {
    Pop-Location
}

# ── Step 2: 计算新增 commit ────────────────────────────────────────
Write-Step "计算增量 commit"
if ($lastSyncCommit) {
    $newCommits = git -C $TaijiWorkspace log $lastSyncCommit..origin/main --oneline
} else {
    Write-Host "  首次同步，同步 origin/main 全部内容"
    $newCommits = "FULL_SYNC"
}

if (-not $newCommits -or $newCommits.Count -eq 0) {
    Write-Host "  无新增 commit，taiji-quant 已是最新"
    exit 0
}

Write-Host "  新增 commit:"
$newCommits | ForEach-Object { Write-Host "    $_" }

# ── Step 3: 同步文件到 taiji-quant ─────────────────────────────────
Write-Step "同步文件到 taiji-quant"
$xdExclude = ($ExcludeDirs | ForEach-Object { "/XD $_" }) -join " "

if ($DryRun) {
    Write-Host "  [DRY RUN] robocopy $TaijiWorkspace → $TaijiQuantWorkspace"
    Write-Host "  排除: $($ExcludeDirs -join ', ')"
} else {
    $robocopyArgs = @(
        $TaijiWorkspace,
        $TaijiQuantWorkspace,
        "/E", "/NP", "/NFL", "/NDL",
        "/XF", "Cargo.lock", "pnpm-lock.yaml"
    )
    # 添加排除目录
    foreach ($dir in $ExcludeDirs) {
        $robocopyArgs += "/XD"
        $robocopyArgs += $dir
    }

    & robocopy @robocopyArgs 2>&1 | Select-Object -Last 3
    Write-Host "  文件同步完成"
}

# ── Step 4: 恢复 taiji-quant 元数据 ─────────────────────────────────
Write-Step "恢复 taiji-quant 元数据"
if (-not $DryRun) {
    # Cargo.toml
    $cargoPath = Join-Path $TaijiQuantWorkspace "Cargo.toml"
    $cargo = Get-Content $cargoPath -Raw
    $cargo = $cargo -replace 'version = "0\.2\.\d+".*', 'version = "0.1.0"'
    $cargo = $cargo -replace 'authors = \["BitFun Team"\]', 'authors = ["Taiji Quant Team"]'
    [System.IO.File]::WriteAllText($cargoPath, $cargo, [System.Text.UTF8Encoding]::new($false))

    # package.json
    $pkgPath = Join-Path $TaijiQuantWorkspace "package.json"
    $pkg = Get-Content $pkgPath -Raw
    $pkg = $pkg -replace '"name": "BitFun"', '"name": "taiji-quant"'
    $pkg = $pkg -replace '"version": "0\.2\.\d+"', '"version": "0.1.0"'
    [System.IO.File]::WriteAllText($pkgPath, $pkg, [System.Text.UTF8Encoding]::new($false))

    Write-Host "  Cargo.toml + package.json 元数据已恢复"
}

# ── Step 5: 保存同步状态 ───────────────────────────────────────────
Write-Step "保存同步状态"
if (-not $DryRun) {
    $state = @{
        last_synced_commit = $upstreamHead
        synced_at          = (Get-Date -Format "yyyy-MM-ddTHH:mm:sszzz")
        commit_count       = if ($newCommits -is [array]) { $newCommits.Count } else { 1 }
    }
    $state | ConvertTo-Json | Set-Content $SyncStateFile -Encoding UTF8
    Write-Host "  已保存: last_synced_commit = $upstreamHead"
}

# ── Step 6: 全编译验证 ────────────────────────────────────────────────
if (-not $SkipBuild -and -not $DryRun) {
    Write-Step "全编译验证"
    Push-Location $TaijiQuantWorkspace
    
    $failed = $false
    
    # 6a. Rust 编译
    Write-Host "  [1/4] cargo check --workspace..."
    $env:OPENSSL_DIR = "C:\Program Files\PostgreSQL\17"
    cargo check --workspace 2>&1 | Select-Object -Last 3
    if ($LASTEXITCODE -ne 0) { Write-Host "  ERROR: cargo check 失败!" -ForegroundColor Red; $failed = $true }
    
    # 6b. Rust 测试
    Write-Host "  [2/4] cargo test (关键 crate)..."
    cargo test -p bitfun-services-core --lib -- tree 2>&1 | Select-Object -Last 2
    cargo test -p bitfun-core --lib -- call_impl_allows 2>&1 | Select-Object -Last 2
    
    # 6c. 前端类型检查
    Write-Host "  [3/4] pnpm type-check:web..."
    pnpm run type-check:web 2>&1 | Select-Object -Last 5
    
    # 6d. mobile-web 构建
    Write-Host "  [4/4] pnpm prepare:mobile-web..."
    pnpm run prepare:mobile-web 2>&1 | Select-Object -Last 5
    
    if ($failed) { exit 1 }
    Write-Host "  全编译验证通过"
    
    Pop-Location
} elseif ($DryRun) {
    Write-Host "  [DRY RUN] 跳过编译"
}

# ── Step 7: Git commit + push ─────────────────────────────────────────
if (-not $DryRun) {
    Write-Step "提交并推送"
    Push-Location $TaijiQuantWorkspace
    try {
        $commitMsg = "sync: upstream $(Get-Date -Format 'yyyy-MM-dd') — $($newCommits.Count) commits from GCWing/BitFun main"
        git add -A
        git commit -m $commitMsg 2>&1 | Out-Null
        git push origin master 2>&1 | Out-Null
        Write-Host "  已推送到 origin/master"
    } finally {
        Pop-Location
    }
} elseif ($DryRun) {
    Write-Host "  [DRY RUN] git commit + push origin"
}

Write-Host "`n=== 同步完成 ===" -ForegroundColor Green
