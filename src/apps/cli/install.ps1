[CmdletBinding()]
param(
    [string]$BinDir = (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'BitFun\bin'),
    [switch]$SkipPathUpdate
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Resolve-RepoRoot {
    $candidate = (Resolve-Path $PSScriptRoot).Path
    while ($candidate) {
        $manifest = Join-Path $candidate 'Cargo.toml'
        $cliDirectory = Join-Path $candidate 'src\apps\cli'
        if ((Test-Path -LiteralPath $manifest -PathType Leaf) -and
            (Test-Path -LiteralPath $cliDirectory -PathType Container) -and
            (Select-String -LiteralPath $manifest -Pattern '^\[workspace\]' -Quiet)) {
            return $candidate
        }

        $parent = Split-Path -Parent $candidate
        if (-not $parent -or $parent -eq $candidate) {
            break
        }
        $candidate = $parent
    }

    throw "Could not locate the BitFun repository root from $PSScriptRoot"
}

function Resolve-TargetRoot([string]$RepoRoot) {
    if (-not $env:CARGO_TARGET_DIR) {
        return Join-Path $RepoRoot 'target'
    }
    if ([IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        return [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    }
    return [IO.Path]::GetFullPath((Join-Path $RepoRoot $env:CARGO_TARGET_DIR))
}

function Resolve-ReleaseDir([string]$RepoRoot) {
    $targetRoot = Resolve-TargetRoot $RepoRoot
    if ($env:CARGO_BUILD_TARGET) {
        return Join-Path $targetRoot "$($env:CARGO_BUILD_TARGET)\release"
    }
    return Join-Path $targetRoot 'release'
}

function Add-BinDirToUserPath([string]$Directory) {
    $normalizedDirectory = [IO.Path]::GetFullPath($Directory).TrimEnd('\')
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    $alreadyPresent = $entries | Where-Object {
        try {
            [IO.Path]::GetFullPath($_).TrimEnd('\') -ieq $normalizedDirectory
        }
        catch {
            $_.TrimEnd('\') -ieq $normalizedDirectory
        }
    }

    if (-not $alreadyPresent) {
        $updated = (@($entries) + $normalizedDirectory) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $updated, 'User')
        Write-Host "Added $normalizedDirectory to the user PATH."
    }
    else {
        Write-Host "$normalizedDirectory is already on the user PATH."
    }

}

function Assert-CommandSucceeded([string]$Description) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

function Assert-EntrypointPair([string]$Primary, [string]$Legacy) {
    & $Primary --version | Out-Null
    Assert-CommandSucceeded 'bitfun --version'

    $id = [guid]::NewGuid().ToString('N')
    $stdoutFile = Join-Path ([IO.Path]::GetTempPath()) "bitfun-install-$id.out"
    $stderrFile = Join-Path ([IO.Path]::GetTempPath()) "bitfun-install-$id.err"
    try {
        $legacyProcess = Start-Process -FilePath $Legacy -ArgumentList '--version' -Wait -PassThru -NoNewWindow `
            -RedirectStandardOutput $stdoutFile -RedirectStandardError $stderrFile
        if ($legacyProcess.ExitCode -ne 0) {
            throw "bitfun-cli --version failed with exit code $($legacyProcess.ExitCode)"
        }
        $legacyWarning = (Get-Content -LiteralPath $stderrFile -Raw).TrimEnd("`r", "`n")
        if ($legacyWarning -cne $script:deprecation) {
            throw "Deprecated entrypoint emitted an unexpected warning: $legacyWarning"
        }
    }
    finally {
        Remove-Item -LiteralPath $stdoutFile, $stderrFile -Force -ErrorAction SilentlyContinue
    }
}

function Assert-PluginHostResources([string]$Directory) {
    foreach ($entry in @('extension-host.js')) {
        $path = Join-Path $Directory $entry
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Plugin Host resource is missing: $path"
        }
    }
}

function Install-EntrypointPair(
    [string]$PrimarySource,
    [string]$LegacySource,
    [string]$PluginHostSource,
    [string]$Destination
) {
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    $stageDir = Join-Path $Destination ".bitfun-install-$([guid]::NewGuid().ToString('N'))"
    $stagedPrimary = Join-Path $stageDir 'bitfun.exe'
    $stagedLegacy = Join-Path $stageDir 'bitfun-cli.exe'
    $stagedPluginHost = Join-Path $stageDir 'ext-host'
    $primaryTarget = Join-Path $Destination 'bitfun.exe'
    $legacyTarget = Join-Path $Destination 'bitfun-cli.exe'
    $pluginHostTarget = Join-Path $Destination 'resources\ext-host'
    $primaryBackup = Join-Path $stageDir 'previous-bitfun.exe'
    $legacyBackup = Join-Path $stageDir 'previous-bitfun-cli.exe'
    $pluginHostBackup = Join-Path $stageDir 'previous-ext-host'
    $primaryBackedUp = $false
    $legacyBackedUp = $false
    $pluginHostBackedUp = $false
    $primaryCommitted = $false
    $legacyCommitted = $false
    $pluginHostCommitted = $false

    New-Item -ItemType Directory -Path $stageDir | Out-Null
    try {
        Copy-Item -LiteralPath $PrimarySource -Destination $stagedPrimary
        Copy-Item -LiteralPath $LegacySource -Destination $stagedLegacy
        New-Item -ItemType Directory -Path $stagedPluginHost | Out-Null
        Copy-Item -LiteralPath (Join-Path $PluginHostSource 'extension-host.js') -Destination $stagedPluginHost
        Assert-EntrypointPair $stagedPrimary $stagedLegacy
        Assert-PluginHostResources $stagedPluginHost

        if (Test-Path -LiteralPath $primaryTarget -PathType Leaf) {
            Move-Item -LiteralPath $primaryTarget -Destination $primaryBackup
            $primaryBackedUp = $true
        }
        if (Test-Path -LiteralPath $legacyTarget -PathType Leaf) {
            Move-Item -LiteralPath $legacyTarget -Destination $legacyBackup
            $legacyBackedUp = $true
        }
        if (Test-Path -LiteralPath $pluginHostTarget -PathType Container) {
            Move-Item -LiteralPath $pluginHostTarget -Destination $pluginHostBackup
            $pluginHostBackedUp = $true
        }

        Move-Item -LiteralPath $stagedPrimary -Destination $primaryTarget
        $primaryCommitted = $true
        Move-Item -LiteralPath $stagedLegacy -Destination $legacyTarget
        $legacyCommitted = $true
        New-Item -ItemType Directory -Path (Split-Path -Parent $pluginHostTarget) -Force | Out-Null
        Move-Item -LiteralPath $stagedPluginHost -Destination $pluginHostTarget
        $pluginHostCommitted = $true
        Assert-EntrypointPair $primaryTarget $legacyTarget
        Assert-PluginHostResources $pluginHostTarget
    }
    catch {
        $installError = $_
        if ($pluginHostCommitted) {
            Remove-Item -LiteralPath $pluginHostTarget -Recurse -Force -ErrorAction SilentlyContinue
        }
        if ($legacyCommitted) {
            Remove-Item -LiteralPath $legacyTarget -Force -ErrorAction SilentlyContinue
        }
        if ($primaryCommitted) {
            Remove-Item -LiteralPath $primaryTarget -Force -ErrorAction SilentlyContinue
        }
        if ($legacyBackedUp) {
            Move-Item -LiteralPath $legacyBackup -Destination $legacyTarget -Force
        }
        if ($primaryBackedUp) {
            Move-Item -LiteralPath $primaryBackup -Destination $primaryTarget -Force
        }
        if ($pluginHostBackedUp) {
            New-Item -ItemType Directory -Path (Split-Path -Parent $pluginHostTarget) -Force | Out-Null
            Move-Item -LiteralPath $pluginHostBackup -Destination $pluginHostTarget -Force
        }
        throw "CLI installation failed; the previous entrypoint pair was restored. $installError"
    }
    finally {
        Remove-Item -LiteralPath $stageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$repoRoot = Resolve-RepoRoot
$releaseDir = Resolve-ReleaseDir $repoRoot
$primarySource = Join-Path $releaseDir 'bitfun.exe'
$legacySource = Join-Path $releaseDir 'bitfun-cli.exe'
$pluginHostSource = Join-Path $repoRoot 'src\apps\extension-host\dist'
$primaryInstalled = Join-Path $BinDir 'bitfun.exe'
$legacyInstalled = Join-Path $BinDir 'bitfun-cli.exe'
$deprecation = 'Warning: `bitfun-cli` is deprecated; use `bitfun` instead.'

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo was not found. Install Rust from https://rustup.rs and re-run.'
}
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    throw 'rustc was not found. Install Rust from https://rustup.rs and re-run.'
}

Write-Host '=== BitFun CLI Install ==='
Write-Host "Repo: $repoRoot"
Write-Host "Install dir: $BinDir"

Push-Location $repoRoot
try {
    Write-Host '[1/3] Building the bitfun and deprecated bitfun-cli entrypoints...'
    & cargo build -p bitfun-cli --release
    Assert-CommandSucceeded 'cargo build'
}
finally {
    Pop-Location
}

foreach ($source in @($primarySource, $legacySource)) {
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Built executable was not found at $source"
    }
}
Assert-PluginHostResources $pluginHostSource

Write-Host '[2/3] Installing executables...'
Install-EntrypointPair $primarySource $legacySource $pluginHostSource $BinDir
Write-Host "Installed: $primaryInstalled"
Write-Host "Installed deprecated compatibility entrypoint: $legacyInstalled"
Write-Host "Installed plugin Host resources: $(Join-Path $BinDir 'resources\ext-host')"

if (-not $SkipPathUpdate) {
    Add-BinDirToUserPath $BinDir
}
else {
    Write-Host 'Skipped the user PATH update (-SkipPathUpdate).'
}

Write-Host '[3/3] Verifying both entrypoints...'
Assert-EntrypointPair $primaryInstalled $legacyInstalled
Assert-PluginHostResources (Join-Path $BinDir 'resources\ext-host')

Write-Host '=== Install complete ==='
Write-Host 'Open a new terminal, then run: bitfun'
Write-Host "Current PowerShell: `$env:Path = `"$([IO.Path]::GetFullPath($BinDir));`$env:Path`"; bitfun"
Write-Host "Direct path: $primaryInstalled"
Write-Host 'Deprecated compatibility command: bitfun-cli'
