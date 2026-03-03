# Switch E2E Tests to Release Mode
# 切换 E2E 测试到 Release 模式

$releaseExe = "C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe"
$releaseBak = "C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe.bak"

Write-Host ""
Write-Host "=== 切换到 RELEASE 模式 ===" -ForegroundColor Cyan
Write-Host ""

# Check if backup exists
if (Test-Path $releaseBak) {
    # Restore release build
    Rename-Item $releaseBak $releaseExe
    Write-Host "✓ Release 构建已恢复" -ForegroundColor Green
    Write-Host "  $releaseBak" -ForegroundColor Gray
    Write-Host "  → $releaseExe" -ForegroundColor Gray
} elseif (Test-Path $releaseExe) {
    Write-Host "✓ Release 构建已存在" -ForegroundColor Yellow
    Write-Host "  当前已处于 RELEASE 模式" -ForegroundColor Yellow
} else {
    Write-Host "✗ Release 构建和备份都不存在" -ForegroundColor Red
    Write-Host "  需要重新构建: npm run desktop:build" -ForegroundColor Yellow
    Write-Host ""
    exit 1
}

Write-Host ""

# Verify release build exists
if (Test-Path $releaseExe) {
    $fileInfo = Get-Item $releaseExe
    Write-Host "✓ Release 构建验证通过" -ForegroundColor Green
    Write-Host "  路径: $releaseExe" -ForegroundColor Gray
    Write-Host "  大小: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
    Write-Host "  修改时间: $($fileInfo.LastWriteTime)" -ForegroundColor Gray
} else {
    Write-Host "✗ Release 构建验证失败" -ForegroundColor Red
    Write-Host ""
    exit 1
}

Write-Host ""
Write-Host "=== 当前状态 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "测试模式: RELEASE MODE" -ForegroundColor Green -BackgroundColor Black
Write-Host "测试将使用: $releaseExe" -ForegroundColor Gray
Write-Host ""

Write-Host "=== 下一步 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "运行测试:" -ForegroundColor Yellow
Write-Host "  cd tests/e2e" -ForegroundColor Gray
Write-Host "  npm run test:l0:all" -ForegroundColor Gray
Write-Host ""
Write-Host "提示: Release 模式不需要 dev server" -ForegroundColor Gray
Write-Host ""
