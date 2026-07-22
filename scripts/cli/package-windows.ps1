[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [string]$Target,

    [string]$ReleaseDir,
    [string]$OutputDir,
    [string]$GitHubOutput = $env:GITHUB_OUTPUT
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $ReleaseDir) {
    $ReleaseDir = Join-Path $repoRoot "target\$Target\release"
}
if (-not $OutputDir) {
    $OutputDir = $repoRoot
}
$ReleaseDir = [IO.Path]::GetFullPath($ReleaseDir)
$OutputDir = [IO.Path]::GetFullPath($OutputDir)

$primary = Join-Path $ReleaseDir 'bitfun.exe'
$legacy = Join-Path $ReleaseDir 'bitfun-cli.exe'
$deprecation = 'Warning: `bitfun-cli` is deprecated; use `bitfun` instead.'

function Assert-LastExitCode([string]$Description) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
    }
}

function Assert-LegacyEntrypoint([string]$Executable) {
    $id = [guid]::NewGuid().ToString('N')
    $stdout = Join-Path ([IO.Path]::GetTempPath()) "bitfun-cli-$id.out"
    $stderr = Join-Path ([IO.Path]::GetTempPath()) "bitfun-cli-$id.err"
    try {
        $process = Start-Process -FilePath $Executable -ArgumentList '--version' -Wait -PassThru -NoNewWindow `
            -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        if ($process.ExitCode -ne 0) {
            throw "Deprecated bitfun-cli entrypoint failed with exit code $($process.ExitCode)"
        }
        $warning = (Get-Content -LiteralPath $stderr -Raw).TrimEnd("`r", "`n")
        if ($warning -cne $deprecation) {
            throw "Unexpected deprecated entrypoint warning: $warning"
        }
    }
    finally {
        Remove-Item -LiteralPath $stdout, $stderr -Force -ErrorAction SilentlyContinue
    }
}

function Assert-NoRedistributableRuntime([string]$Executable) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw 'vswhere.exe was not found; cannot verify the Windows runtime dependency contract'
    }
    $installPath = (& $vswhere -latest -products '*' `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath | Select-Object -First 1)
    $dumpbin = Get-ChildItem -LiteralPath (Join-Path $installPath 'VC\Tools\MSVC') -Recurse `
        -Filter dumpbin.exe | Where-Object { $_.FullName -match '\\bin\\Hostx64\\x64\\dumpbin\.exe$' } |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $dumpbin) {
        throw 'dumpbin.exe was not found; cannot verify the Windows runtime dependency contract'
    }

    $dependents = (& $dumpbin.FullName /nologo /dependents $Executable 2>&1 | Out-String)
    Assert-LastExitCode "dumpbin dependency check for $Executable"
    if ($dependents -match '(?im)^\s*(?:VCRUNTIME|MSVCP)[^\s]*\.dll\s*$') {
        throw "$Executable requires the Visual C++ Redistributable; build it with static CRT linkage"
    }
}

& $primary --version
Assert-LastExitCode 'bitfun --version'
& $primary --help | Out-Null
Assert-LastExitCode 'bitfun --help'
Assert-LegacyEntrypoint $legacy
Assert-NoRedistributableRuntime $primary
Assert-NoRedistributableRuntime $legacy

$stageName = "bitfun-cli-$Version-$Target"
$stageDir = Join-Path (Join-Path $OutputDir 'dist-cli') $stageName
New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
Copy-Item -LiteralPath $primary -Destination $stageDir -Force
Copy-Item -LiteralPath $legacy -Destination $stageDir -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination $stageDir -Force -ErrorAction SilentlyContinue
Copy-Item -LiteralPath (Join-Path $repoRoot 'src\apps\cli\README.md') `
    -Destination (Join-Path $stageDir 'README.md') -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'README.md') `
    -Destination (Join-Path $stageDir 'PROJECT-README.md') -Force

$themes = Join-Path $repoRoot 'src\apps\cli\themes'
$prompts = Join-Path $repoRoot 'src\apps\cli\prompts'
if (Test-Path -LiteralPath $themes -PathType Container) {
    Copy-Item -LiteralPath $themes -Destination (Join-Path $stageDir 'themes') -Recurse -Force
}
if (Test-Path -LiteralPath $prompts -PathType Container) {
    Copy-Item -LiteralPath $prompts -Destination (Join-Path $stageDir 'prompts') -Recurse -Force
}

$archive = Join-Path $OutputDir "$stageName.zip"
Compress-Archive -Path $stageDir -DestinationPath $archive -CompressionLevel Optimal -Force
$hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
$checksum = "$archive.sha256"
"$hash  $(Split-Path -Leaf $archive)" | Set-Content -LiteralPath $checksum -Encoding ascii

$recordedHash = ((Get-Content -LiteralPath $checksum -Raw).Trim() -split '\s+')[0]
if ($recordedHash -cne $hash) {
    throw 'Packaged archive checksum mismatch'
}

$extractDir = Join-Path ([IO.Path]::GetTempPath()) "bitfun-cli-package-$([guid]::NewGuid().ToString('N'))"
try {
    Expand-Archive -LiteralPath $archive -DestinationPath $extractDir
    $primaryCandidates = @(Get-ChildItem -LiteralPath $extractDir -Recurse -Filter 'bitfun.exe')
    $legacyCandidates = @(Get-ChildItem -LiteralPath $extractDir -Recurse -Filter 'bitfun-cli.exe')
    if ($primaryCandidates.Count -ne 1 -or $legacyCandidates.Count -ne 1) {
        throw 'Expected exactly one of each CLI entrypoint in the packaged archive'
    }
    foreach ($readme in @('README.md', 'PROJECT-README.md')) {
        if (-not (Test-Path -LiteralPath (Join-Path $primaryCandidates[0].DirectoryName $readme) -PathType Leaf)) {
            throw "Packaged archive is missing $readme"
        }
    }

    & $primaryCandidates[0].FullName --version
    Assert-LastExitCode 'packaged bitfun --version'
    & $primaryCandidates[0].FullName --help | Out-Null
    Assert-LastExitCode 'packaged bitfun --help'
    Assert-LegacyEntrypoint $legacyCandidates[0].FullName
}
finally {
    Remove-Item -LiteralPath $extractDir -Recurse -Force -ErrorAction SilentlyContinue
}

if ($GitHubOutput) {
    "archive=$(Split-Path -Leaf $archive)" >> $GitHubOutput
    "checksum=$(Split-Path -Leaf $checksum)" >> $GitHubOutput
}

Write-Host "Packaged and verified: $archive"
