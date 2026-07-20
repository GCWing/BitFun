[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Target
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$installer = Join-Path $repoRoot 'src\apps\cli\install.ps1'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) "bitfun-cli-install-$([guid]::NewGuid().ToString('N'))"
$binDir = Join-Path $testRoot 'bin'

try {
    $env:CARGO_BUILD_TARGET = $Target
    & $installer -BinDir $binDir -SkipPathUpdate
    & $installer -BinDir $binDir -SkipPathUpdate

    & (Join-Path $binDir 'bitfun.exe') --version | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'Installed bitfun smoke check failed'
    }

    $primary = Join-Path $binDir 'bitfun.exe'
    $legacy = Join-Path $binDir 'bitfun-cli.exe'
    [IO.File]::WriteAllText($primary, 'previous primary')
    [IO.File]::WriteAllText($legacy, 'previous legacy')
    $primaryHash = (Get-FileHash -LiteralPath $primary -Algorithm SHA256).Hash
    $legacyHash = (Get-FileHash -LiteralPath $legacy -Algorithm SHA256).Hash

    $lock = [IO.File]::Open($legacy, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
    $failedAsExpected = $false
    try {
        & $installer -BinDir $binDir -SkipPathUpdate 2>$null
    }
    catch {
        $failedAsExpected = $true
    }
    finally {
        $lock.Dispose()
    }
    if (-not $failedAsExpected) {
        throw 'Installer unexpectedly succeeded while the legacy entrypoint was locked'
    }

    if ((Get-FileHash -LiteralPath $primary -Algorithm SHA256).Hash -cne $primaryHash) {
        throw 'Failed update did not restore the previous primary entrypoint'
    }
    if ((Get-FileHash -LiteralPath $legacy -Algorithm SHA256).Hash -cne $legacyHash) {
        throw 'Failed update did not preserve the previous legacy entrypoint'
    }
}
finally {
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
