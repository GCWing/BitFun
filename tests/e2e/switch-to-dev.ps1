# Switch E2E Tests to Dev Mode
# 切换 E2E 测试到 Dev 模式

$releaseExe = "C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe"
$releaseBak = "C:\Users\wuxiao\BitFun\target\release\bitfun-desktop.exe.bak"
$debugExe = "C:\Users\wuxiao\BitFun\target\debug\bitfun-desktop.exe"

Write-Host ""
Write-Host "=== 切换到 DEV 模式 ===" -ForegroundColor Cyan
Write-Host ""

# Check if release build exists
if (Test-Path $releaseExe) {
    # Rename release build
    Rename-Item $releaseExe $releaseBak
    Write-Host "✓ Release 构建已重命名为 .bak" -ForegroundColor Green
    Write-Host "  $releaseExe" -ForegroundColor Gray
    Write-Host "  → $releaseBak" -ForegroundColor Gray
} elseif (Test-Path $releaseBak) {
    Write-Host "✓ Release 构建已经被重命名" -ForegroundColor Yellow
    Write-Host "  当前已处于 DEV 模式" -ForegroundColor Yellow
} else {
    Write-Host "! Release 构建不存在" -ForegroundColor Yellow
}

Write-Host ""

# Check if debug build exists
if (Test-Path $debugExe) {
    Write-Host "✓ Debug 构建存在" -ForegroundColor Green
    Write-Host "  $debugExe" -ForegroundColor Gray
} else {
    Write-Host "✗ Debug 构建不存在" -ForegroundColor Red
    Write-Host "  请先运行: npm run dev" -ForegroundColor Yellow
    Write-Host ""
    exit 1
}

Write-Host ""
Write-Host "=== 当前状态 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "测试模式: DEV MODE" -ForegroundColor Green -BackgroundColor Black
Write-Host "测试将使用: $debugExe" -ForegroundColor Gray
Write-Host ""

# Check if dev server is running
Write-Host "检查 Dev Server 状态..." -ForegroundColor Yellow
try {
    $connection = Test-NetConnection -ComputerName localhost -Port 1422 -InformationLevel Quiet -WarningAction SilentlyContinue -ErrorAction SilentlyContinue
    if ($connection) {
        Write-Host "✓ Dev server 正在运行 (端口 1422)" -ForegroundColor Green
    } else {
        Write-Host "✗ Dev server 未运行" -ForegroundColor Red
        Write-Host "  建议启动: npm run dev" -ForegroundColor Yellow
        Write-Host "  (测试仍可运行，但建议启动 dev server)" -ForegroundColor Gray
    }
} catch {
    Write-Host "? 无法检测 dev server 状态" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== 下一步 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. (可选) 启动 dev server:" -ForegroundColor Yellow
Write-Host "   npm run dev" -ForegroundColor Gray
Write-Host ""
Write-Host "2. 运行测试:" -ForegroundColor Yellow
Write-Host "   cd tests/e2e" -ForegroundColor Gray
Write-Host "   npm run test:l0:all" -ForegroundColor Gray
Write-Host ""
Write-Host "3. 完成后切换回 Release 模式:" -ForegroundColor Yellow
Write-Host "   ./switch-to-release.ps1" -ForegroundColor Gray
Write-Host ""
