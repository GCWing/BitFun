$ErrorActionPreference = 'Continue'

# 1. Try git pull in various locations
$locations = @(
    "E:\Yuanban\cc-haha",
    "E:\Yuanban\cc-haha\Claude Code Haha",
    "E:\Yuanban\cc-haha-src"
)

foreach ($loc in $locations) {
    Write-Host "=== Checking: $loc ==="
    if (Test-Path "$loc\.git") {
        Write-Host "Git repo found. Pulling..."
        git -C $loc pull --rebase 2>&1
        Write-Host "Git pull done for $loc"
    } else {
        Write-Host "Not a git repo (no .git folder)"
    }
    Write-Host ""
}

# 2. List top-level directory structure
Write-Host "=== Listing E:\Yuanban\cc-haha top-level ==="
Get-ChildItem "E:\Yuanban\cc-haha" -Depth 1 | Select-Object Name, PSIsContainer | Format-Table -AutoSize

Write-Host "=== Listing E:\Yuanban\cc-haha-src top-level ==="
Get-ChildItem "E:\Yuanban\cc-haha-src" -Depth 1 -ErrorAction SilentlyContinue | Select-Object Name, PSIsContainer | Format-Table -AutoSize

# 3. Find all hook-related files
Write-Host "=== Searching for hook-related files (name contains 'hook') ==="
Get-ChildItem "E:\Yuanban\cc-haha" -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'hook' } | Select-Object FullName | Format-Table -AutoSize

Write-Host "=== Searching for files containing 'HookResult' or 'Abort' or 'behavior_guard' ==="
$searchDirs = @("E:\Yuanban\cc-haha", "E:\Yuanban\cc-haha-src")
foreach ($dir in $searchDirs) {
    if (Test-Path $dir) {
        Write-Host "--- Searching in $dir ---"
        Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue | Where-Object {
            $_.Extension -match '\.(ts|js|tsx|jsx|rs|py|go|java)$'
        } | ForEach-Object {
            $content = Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue
            if ($content -match 'HookResult|Abort|behavior_guard') {
                Write-Host "MATCH: $($_.FullName)"
            }
        }
    }
}

Write-Host "=== DONE ==="
