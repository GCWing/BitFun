[CmdletBinding()]
param(
    [string]$SourceBranch = "src-v2",
    [string]$TargetBranch = "",
    [switch]$SkipVerify = $false,
    [switch]$DryRun = $false
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

# ── helpers ──────────────────────────────────────────────────────────

function _git { git.exe -C $repoRoot @args }
function _git_ok { _git @args | Out-Null; return $LASTEXITCODE -eq 0 }

function info([string]$s)  { Write-Host "[taiji-migrate] $s" -ForegroundColor White }
function ok([string]$s)    { Write-Host "[taiji-migrate] $s" -ForegroundColor Green }
function warn([string]$s)  { Write-Host "[taiji-migrate] $s" -ForegroundColor Yellow }
function err([string]$s)   { Write-Host "[taiji-migrate] $s" -ForegroundColor Red }
function dim([string]$s)   { Write-Host "[taiji-migrate] $s" -ForegroundColor DarkGray }
function banner([string]$s){ Write-Host "[taiji-migrate] $s" -ForegroundColor Cyan }

function fatal([string]$msg) {
    Write-Host "[FATAL] $msg" -ForegroundColor Red
    Write-Host "Rollback: git checkout $originalBranch" -ForegroundColor Yellow
    exit 1
}

function assert-ok([string]$desc) {
    if ($LASTEXITCODE -ne 0) { fatal "$desc (exit code $LASTEXITCODE)" }
}

function get-next-version {
    $max = 0
    $branches = _git branch --list 'taiji-v*'
    foreach ($b in $branches) {
        if ($b -match 'taiji-v(\d+)') {
            $n = [int]$Matches[1]
            if ($n -gt $max) { $max = $n }
        }
    }
    return "taiji-v$($max + 1)"
}

# ── pre-flight ───────────────────────────────────────────────────────

$originalBranch = (_git branch --show-current).Trim()

banner '========================================'
banner ' Taiji Migration Tool v2.0'
banner '========================================'
info ''

# check clean
$status = _git status --porcelain
if ($status) { fatal "Working tree dirty. Commit or stash first." }
ok 'Workspace clean'

# fetch
warn 'Fetching origin/main ...'
_git fetch origin main | Out-Null
assert-ok 'git fetch origin/main'

$mainHead = (_git rev-parse --short origin/main).Trim()
$baseCommit = (_git merge-base HEAD origin/main).Trim()
$behindCount = [int](_git rev-list --count "$baseCommit..origin/main")

info "origin/main  : $mainHead"
info "current base : $(_git rev-parse --short $baseCommit)"
warn "commits ahead: $behindCount"

if (-not (_git_ok rev-parse --verify "refs/heads/$SourceBranch")) {
    fatal "Source branch '$SourceBranch' does not exist"
}

if (-not $TargetBranch) { $TargetBranch = get-next-version }
info "Source: $SourceBranch"
info "Target: $TargetBranch"
info ''

if ($DryRun) {
    warn 'DRY-RUN -- no changes made.'
    exit 0
}

# ── file lists ───────────────────────────────────────────────────────

$pureAdditions = @(
    'src/crates/taiji'
    'docs/plans/phase5-7-batch-a-methodology.md'
    'docs/plans/phase8-10-rebase-plan.md'
    'docs/plans/phase8-9-10-master-plan.md'
    'docs/plans/phase9-security-report.md'
    'docs/plans/session-analysis-20260721-batchB-report.md'
    'docs/plans/session-analysis-20260721-report.md'
    'docs/plans/session-progress-20260722.md'
    'docs/external-reference-code-map.md'
    'taiji-website'
    'test_data'
    'examples/example-pipeline.yaml'
    'MiniApp/Skills/miniapp-dev'
    'MiniApp/Demo'
    'scripts/migrate-taiji.ps1'
)

$integrationFiles = @(
    # batch A: pure new files
    'src/apps/desktop/src/api/ffmpeg_api.rs'
    'src/crates/assembly/core/src/agentic/tools/implementations/legion_control_tool.rs'
    'src/crates/assembly/core/src/agentic/agents/prompts/acp_agent.md'
    'src/crates/assembly/core/src/agentic/agents/definitions/subagents/acp_agent.rs'
    'src/crates/assembly/core/src/agentic/agents/team_presets.rs'
    'src/crates/interfaces/acp/src/client/cli_detect.rs'
    'src/crates/interfaces/acp/src/client/launch_policy.rs'
    'src/crates/interfaces/acp/src/client/probe.rs'
    'src/web-ui/src/app/layout/BeeColonyMonitor.scss'
    'src/web-ui/src/app/layout/BeeColonyMonitor.tsx'
    'src/web-ui/src/app/scenes/agents/components/CreateLegionPage.tsx'
    'src/web-ui/src/app/scenes/agents/components/LegionCard.scss'
    'src/web-ui/src/app/scenes/agents/components/LegionCard.tsx'
    'src/web-ui/src/app/scenes/agents/data/orchestration-patterns.ts'
    'src/web-ui/src/api/service-api/LegionPresetAPI.ts'
    'src/crates/assembly/core/builtin/assets/bee-colony-dag/index.html'
    'src/crates/assembly/core/builtin/assets/bee-colony-dag/meta.json'
    'src/crates/assembly/core/builtin/assets/bee-colony-dag/style.css'
    'src/crates/assembly/core/builtin/assets/bee-colony-dag/ui.js'
    # batch B1: infra
    'Cargo.toml'
    '.github/workflows/ci.yml'
    '.github/CODEOWNERS'
    'README.md'; 'README.zh-CN.md'
    'CONTRIBUTING.md'; 'CONTRIBUTING_CN.md'
    'SECURITY.md'; 'SECURITY_CN.md'
    # batch B2: core integration
    'src/crates/assembly/core/src/agentic/coordination/scheduler.rs'
    'src/crates/assembly/core/src/agentic/agents/mod.rs'
    'src/crates/assembly/core/src/agentic/tools/implementations/mod.rs'
    'src/crates/assembly/core/src/agentic/tools/implementations/file_edit_tool.rs'
    'src/crates/assembly/core/src/agentic/agents/prompts/team_mode.md'
    'src/crates/assembly/core/src/agentic/agents/definitions/subagents/mod.rs'
    'src/crates/execution/agent-runtime/src/scheduler.rs'
    'src/crates/execution/tool-execution/src/fs/edit_file.rs'
    # batch B3: desktop/CLI/WebUI
    'src/apps/desktop/Cargo.toml'
    'src/apps/desktop/src/api/browser_api.rs'
    'src/apps/desktop/src/api/mod.rs'
    'src/apps/cli/src/acp_cli.rs'
    'src/apps/cli/src/daemon/service.rs'
    'src/crates/interfaces/acp/Cargo.toml'
    'src/crates/interfaces/acp/src/client/builtin_clients.rs'
    'src/crates/interfaces/acp/src/client/config.rs'
    'src/crates/interfaces/acp/src/client/manager.rs'
    'src/crates/interfaces/acp/src/client/mod.rs'
    'src/crates/interfaces/acp/src/mcp/protocol/client_info.rs'
    'src/crates/interfaces/acp/src/mcp/protocol/transport_remote.rs'
    'src/web-ui/src/locales/en-US/scenes/agents.json'
    'src/web-ui/src/locales/zh-CN/scenes/agents.json'
    'src/web-ui/src/locales/zh-TW/scenes/agents.json'
)

# ── Step 1: create branch ────────────────────────────────────────────

warn "Step 1/5: Creating $TargetBranch from origin/main ..."
_git checkout -B $TargetBranch origin/main | Out-Null
_git clean -fd | Out-Null
assert-ok "git checkout -b $TargetBranch"
ok "Created $TargetBranch at $mainHead"

# ── Step 2: pure additions ───────────────────────────────────────────

$addCount = $pureAdditions.Count
warn "Step 2/5: Copying $addCount pure-additions ..."

foreach ($path in $pureAdditions) {
    $fullPath = Join-Path $repoRoot $path
    if (Test-Path $fullPath) {
        dim "  SKIP: $path (already exists on main)"
        continue
    }
    $existsOnSource = _git_ok cat-file -e "$SourceBranch`:$path"
    if (-not $existsOnSource) {
        dim "  SKIP: $path (not on $SourceBranch)"
        continue
    }
    _git checkout $SourceBranch -- $path | Out-Null
    if ($LASTEXITCODE -eq 0) {
        ok "  OK: $path"
    } else {
        fatal "Unexpected conflict: $path (pure additions should never conflict)"
    }
}

# ── Step 3: integration files ────────────────────────────────────────

$intCount = $integrationFiles.Count
warn "Step 3/5: Copying $intCount integrations ..."
$conflictFiles = @()
$okCount = 0
$skipCount = 0

foreach ($file in $integrationFiles) {
    $existsOnSource = _git_ok cat-file -e "$SourceBranch`:$file"
    if (-not $existsOnSource) {
        dim "  SKIP: $file"
        $skipCount++
        continue
    }
    _git checkout $SourceBranch -- $file | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $okCount++
    } else {
        $conflictFiles += $file
        err "  CONFLICT: $file"
    }
}

info "  Copied: $okCount | Skipped: $skipCount | Conflicts: $($conflictFiles.Count)"

if ($conflictFiles.Count -gt 0) {
    banner '========================================'
    err ' CONFLICT RESOLUTION REQUIRED'
    banner '========================================'
    warn 'For each file: git diff <file> -> edit -> git add <file>'
    foreach ($f in $conflictFiles) { err "  $f" }
    warn ''
    warn 'Common: Cargo.toml -> append taiji members; ci.yml -> append taiji jobs'
    warn "After resolving: re-run with -SkipVerify. Or: git checkout $originalBranch"
    exit 2
}

ok 'All integration files copied (zero conflicts)'

# ── Step 4: verify Cargo.toml ────────────────────────────────────────

warn 'Step 4/5: Verifying workspace members ...'
$cargoToml = Join-Path $repoRoot 'Cargo.toml'
$content = Get-Content $cargoToml -Raw

$requiredMembers = @(
    'taiji-bar', 'taiji-cli', 'taiji-engine', 'taiji-engine-py', 'taiji-content',
    'taiji-publisher', 'taiji-growth', 'taiji-alert', 'taiji-knowledge-graph',
    'taiji-blog-gen', 'taiji-example', 'taiji-llm', 'taiji-backtest',
    'taiji-executor', 'taiji-realtime', 'taiji-pattern', 'taiji-abnormal',
    'taiji-sentiment', 'taiji-orderflow', 'taiji-strategen'
)

$missing = @()
foreach ($m in $requiredMembers) {
    if ($content -notmatch [regex]::Escape($m)) {
        $missing += $m
    }
}

if ($missing.Count -gt 0) {
    warn "Missing members: $missing"
} else {
    ok 'All 20 taiji crates registered'
}

# ── Step 5: quality gate ─────────────────────────────────────────────

if ($SkipVerify) {
    dim 'Step 5/5: Verification SKIPPED'
} else {
    warn 'Step 5/5: Running quality gate ...'

    $excludeFlags = '--exclude taiji-dvmi --exclude taiji-magnet --exclude taiji-thrust --exclude taiji-risk'

    info '  [1/4] cargo check ...'
    $checkOutput = Invoke-Expression "cargo check --workspace $excludeFlags" *>&1
    if ($LASTEXITCODE -ne 0) {
        err 'cargo check FAILED'
        $checkOutput | Select-Object -Last 30 | ForEach-Object { Write-Host "    $_" }
        fatal 'Fix errors and re-run'
    }
    ok 'cargo check passed'

    info '  [2/4] cargo test ...'
    $testCrates = @(
        'taiji-engine','taiji-bar','taiji-backtest','taiji-executor','taiji-realtime',
        'taiji-pattern','taiji-abnormal','taiji-orderflow','taiji-sentiment',
        'taiji-strategen','taiji-llm','taiji-engine-py','taiji-content',
        'taiji-publisher','taiji-growth','taiji-alert','taiji-knowledge-graph',
        'taiji-cli','taiji-example'
    )
    $testArgs = ($testCrates | ForEach-Object { "-p $_" }) -join ' '
    $testOutput = Invoke-Expression "cargo test $testArgs" *>&1
    if ($LASTEXITCODE -ne 0) {
        err 'cargo test FAILED'
        $testOutput | Select-Object -Last 20 | ForEach-Object { Write-Host "    $_" }
        fatal 'Fix errors and re-run'
    }
    ok 'cargo test passed'

    info '  [3/4] cargo clippy ...'
    $clippyOutput = Invoke-Expression "cargo clippy --workspace $excludeFlags -- -D warnings" *>&1
    if ($LASTEXITCODE -ne 0) {
        err 'cargo clippy FAILED'
        $clippyOutput | Select-Object -Last 20 | ForEach-Object { Write-Host "    $_" }
        fatal 'Fix warnings and re-run'
    }
    ok 'cargo clippy passed'

    info '  [4/4] cargo fmt ...'
    cargo fmt --check --all 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        warn 'Formatting diffs found -- auto-fixing ...'
        cargo fmt --all 2>&1 | Out-Null
        assert-ok 'cargo fmt --all'
        ok 'cargo fmt applied'
    } else {
        ok 'cargo fmt passed'
    }
}

# ── done ─────────────────────────────────────────────────────────────

$mainHeadFinal = (_git rev-parse --short origin/main).Trim()
$targetHead = (_git rev-parse --short HEAD).Trim()

banner ''
banner '========================================'
banner ' Migration Complete'
banner '========================================'
info "Base  : $mainHeadFinal (origin/main)"
info "Branch: $TargetBranch ($targetHead)"
info ''

if (-not $SkipVerify) {
    warn 'Next steps:'
    info '  1. git diff origin/main..HEAD --stat'
    info "  2. git add -A && git commit -m 'taiji: migrate to main $mainHeadFinal'"
    info "  3. git push origin $TargetBranch"
    info "  4. git checkout $originalBranch"
} else {
    warn 'Verification skipped. Run manually:'
    info '  cargo check --workspace (exclude dvmi,magnet,thrust,risk)'
    info '  cargo test -p taiji-* (19 crates)'
    info '  cargo clippy --workspace (exclude dvmi,magnet,thrust,risk) -- -D warnings'
    info '  cargo fmt --check --all'
}
