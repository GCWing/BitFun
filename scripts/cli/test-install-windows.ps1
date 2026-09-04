[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Target
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$installer = Join-Path $repoRoot 'src\apps\cli\install.ps1'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) "openbitfun-cli-install-$([guid]::NewGuid().ToString('N'))"
$binDir = Join-Path $testRoot 'bin'

try {
    $env:CARGO_BUILD_TARGET = $Target
    & $installer -BinDir $binDir -SkipPathUpdate
    & $installer -BinDir $binDir -SkipPathUpdate

    & (Join-Path $binDir 'openbitfun.exe') --version | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'Installed openbitfun smoke check failed'
    }
    foreach ($entry in @('extension-host.js')) {
        if (-not (Test-Path -LiteralPath (Join-Path $binDir "resources\ext-host\$entry") -PathType Leaf)) {
            throw "Installed CLI is missing plugin Host resource: $entry"
        }
    }

    $primary = Join-Path $binDir 'openbitfun.exe'
    [IO.File]::WriteAllText($primary, 'previous primary')
    $primaryHash = (Get-FileHash -LiteralPath $primary -Algorithm SHA256).Hash

    $lock = [IO.File]::Open($primary, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
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
        throw 'Installer unexpectedly succeeded while the OpenBitFun CLI was locked'
    }

    if ((Get-FileHash -LiteralPath $primary -Algorithm SHA256).Hash -cne $primaryHash) {
        throw 'Failed update did not restore the previous primary entrypoint'
    }
}
finally {
    Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
