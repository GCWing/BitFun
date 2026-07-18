$ErrorActionPreference = 'Continue'
$outDir = "E:\Yuanban\BitFun-src\hook-dump"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$files = @(
    "src\types\hooks.ts",
    "src\utils\hooks.ts",
    "src\schemas\hooks.ts",
    "src\utils\hooks\hookEvents.ts",
    "src\utils\hooks\hookHelpers.ts",
    "src\utils\hooks\hooksConfigManager.ts",
    "src\utils\hooks\sessionHooks.ts",
    "src\utils\hooks\execPromptHook.ts",
    "src\utils\hooks\execAgentHook.ts",
    "src\utils\hooks\execHttpHook.ts",
    "src\utils\hooks\AsyncHookRegistry.ts",
    "src\utils\hooks\postSamplingHooks.ts",
    "src\utils\hooks\hooksSettings.ts",
    "src\utils\hooks\hooksConfigSnapshot.ts",
    "src\utils\hooks\registerFrontmatterHooks.ts",
    "src\utils\hooks\registerSkillHooks.ts",
    "src\utils\hooks\apiQueryHookHelper.ts",
    "src\utils\hooks\fileChangedWatcher.ts",
    "src\commands\hooks\hooks.tsx",
    "src\services\tools\toolHooks.ts",
    "src\hooks\useDeferredHookMessages.ts",
    "src\query\stopHooks.ts",
    "src\query\stopHooks.test.ts",
    "src\costHook.ts"
)

$base = "E:\Yuanban\cc-haha-src"

foreach ($f in $files) {
    $full = Join-Path $base $f
    if (Test-Path $full) {
        $lines = @(Get-Content $full)
        $outName = $f -replace '[\\/]', '_'
        $outPath = Join-Path $outDir $outName
        "FILE: $full" | Out-File $outPath -Encoding utf8
        "LINES: $($lines.Count)" | Out-File $outPath -Append -Encoding utf8
        "==========" | Out-File $outPath -Append -Encoding utf8
        Get-Content $full | Out-File $outPath -Append -Encoding utf8
        Write-Host "Copied: $f ($($lines.Count) lines) -> $outPath"
    } else {
        Write-Host "NOT FOUND: $full"
    }
}

Write-Host "DONE. Files in $outDir"
