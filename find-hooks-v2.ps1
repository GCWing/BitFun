$ErrorActionPreference = 'Continue'
$srcDir = "E:\Yuanban\cc-haha-src"

# Find files with "hook" in name, excluding node_modules, dist, .git
Write-Host "=== Files with 'hook' in name (source only) ==="
Get-ChildItem $srcDir -Recurse -File -ErrorAction SilentlyContinue | Where-Object {
    $_.Name -match 'hook' -and
    $_.FullName -notmatch '\\node_modules\\' -and
    $_.FullName -notmatch '\\dist\\' -and
    $_.FullName -notmatch '\\.git\\' -and
    $_.FullName -notmatch '\\__pycache__\\'
} | Select-Object FullName | Format-Table -AutoSize

Write-Host "=== Files containing 'HookResult' (source only) ==="
Get-ChildItem $srcDir -Recurse -File -ErrorAction SilentlyContinue | Where-Object {
    $_.Extension -match '\.(ts|js|tsx|jsx)$' -and
    $_.FullName -notmatch '\\node_modules\\' -and
    $_.FullName -notmatch '\\dist\\' -and
    $_.FullName -notmatch '\\.git\\'
} | ForEach-Object {
    $content = Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue
    if ($content -match 'HookResult') {
        Write-Host "HookResult: $($_.FullName)"
    }
}

Write-Host "=== Files containing 'behavior_guard' (source only) ==="
Get-ChildItem $srcDir -Recurse -File -ErrorAction SilentlyContinue | Where-Object {
    $_.Extension -match '\.(ts|js|tsx|jsx)$' -and
    $_.FullName -notmatch '\\node_modules\\' -and
    $_.FullName -notmatch '\\dist\\' -and
    $_.FullName -notmatch '\\.git\\'
} | ForEach-Object {
    $content = Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue
    if ($content -match 'behavior_guard') {
        Write-Host "behavior_guard: $($_.FullName)"
    }
}

Write-Host "=== DONE ==="
