$ErrorActionPreference = 'Continue'
$base = "E:\Yuanban\BitFun-src\hook-dump"

# Files in priority order
$files = @(
    "src_schemas_hooks.ts",
    "src_utils_hooks_hooksConfigManager.ts",
    "src_utils_hooks_sessionHooks.ts",
    "src_utils_hooks_execPromptHook.ts",
    "src_utils_hooks_execAgentHook.ts",
    "src_utils_hooks_execHttpHook.ts",
    "src_utils_hooks_AsyncHookRegistry.ts",
    "src_utils_hooks_postSamplingHooks.ts",
    "src_services_tools_toolHooks.ts",
    "src_utils_hooks_hooksConfigSnapshot.ts",
    "src_commands_hooks_hooks.tsx",
    "src_costHook.ts",
    "src_hooks_useDeferredHookMessages.ts",
    "src_query_stopHooks.ts",
    "src_utils_hooks_hooksSettings.ts",
    "src_utils_hooks_registerFrontmatterHooks.ts",
    "src_utils_hooks_registerSkillHooks.ts",
    "src_utils_hooks_apiQueryHookHelper.ts",
    "src_utils_hooks_fileChangedWatcher.ts",
    "src_query_stopHooks.test.ts"
)

$out = "E:\Yuanban\BitFun-src\hook-dump\COMBINED.txt"
"" | Out-File $out -Encoding utf8

foreach ($f in $files) {
    $full = Join-Path $base $f
    if (Test-Path $full) {
        "`n`n========================================" | Out-File $out -Append -Encoding utf8
        "FILE: $f" | Out-File $out -Append -Encoding utf8
        "========================================" | Out-File $out -Append -Encoding utf8
        Get-Content $full | Out-File $out -Append -Encoding utf8
    } else {
        "`nMISSING: $f" | Out-File $out -Append -Encoding utf8
    }
}

Write-Host "Combined to $out"
Write-Host "Size: $((Get-Item $out).Length) bytes"
