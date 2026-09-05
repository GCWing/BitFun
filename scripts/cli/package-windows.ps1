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

$primary = Join-Path $ReleaseDir 'openbitfun.exe'
$pluginHostDist = Join-Path $repoRoot 'src\apps\extension-host\dist'
$pluginHostResourceRelative = 'resources\ext-host'

function Assert-PluginHostResources([string]$Directory) {
    foreach ($entry in @('extension-host.js')) {
        $path = Join-Path $Directory $entry
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Plugin Host resource is missing: $path"
        }
    }
}

function Assert-LastExitCode([string]$Description) {
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE"
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
Assert-LastExitCode 'openbitfun --version'
& $primary --help | Out-Null
Assert-LastExitCode 'openbitfun --help'
Assert-NoRedistributableRuntime $primary
Assert-PluginHostResources $pluginHostDist

$stageName = "openbitfun-cli-$Version-$Target"
$stageDir = Join-Path (Join-Path $OutputDir 'dist-cli') $stageName
New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
Copy-Item -LiteralPath $primary -Destination $stageDir -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination $stageDir -Force -ErrorAction SilentlyContinue
Copy-Item -LiteralPath (Join-Path $repoRoot 'src\apps\cli\README.md') `
    -Destination (Join-Path $stageDir 'README.md') -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'README.md') `
    -Destination (Join-Path $stageDir 'PROJECT-README.md') -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'THIRD_PARTY_NOTICES.md') `
    -Destination (Join-Path $stageDir 'THIRD_PARTY_NOTICES.md') -Force
$modelsDevNotices = Join-Path $stageDir 'third-party\models.dev'
New-Item -ItemType Directory -Path $modelsDevNotices -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot 'src\crates\services\services-integrations\assets\models-dev.LICENSE.txt') `
    -Destination (Join-Path $modelsDevNotices 'LICENSE.txt') -Force
Copy-Item -LiteralPath (Join-Path $repoRoot 'src\crates\services\services-integrations\assets\models-dev.provenance.json') `
    -Destination (Join-Path $modelsDevNotices 'provenance.json') -Force

$themes = Join-Path $repoRoot 'src\apps\cli\themes'
$prompts = Join-Path $repoRoot 'src\apps\cli\prompts'
if (Test-Path -LiteralPath $themes -PathType Container) {
    Copy-Item -LiteralPath $themes -Destination (Join-Path $stageDir 'themes') -Recurse -Force
}
if (Test-Path -LiteralPath $prompts -PathType Container) {
    Copy-Item -LiteralPath $prompts -Destination (Join-Path $stageDir 'prompts') -Recurse -Force
}
$pluginHostResources = Join-Path $stageDir $pluginHostResourceRelative
New-Item -ItemType Directory -Path $pluginHostResources -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $pluginHostDist 'extension-host.js') -Destination $pluginHostResources -Force
Assert-PluginHostResources $pluginHostResources

$archive = Join-Path $OutputDir "$stageName.zip"
Compress-Archive -Path $stageDir -DestinationPath $archive -CompressionLevel Optimal -Force
$hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
$checksum = "$archive.sha256"
"$hash  $(Split-Path -Leaf $archive)" | Set-Content -LiteralPath $checksum -Encoding ascii

$recordedHash = ((Get-Content -LiteralPath $checksum -Raw).Trim() -split '\s+')[0]
if ($recordedHash -cne $hash) {
    throw 'Packaged archive checksum mismatch'
}

$extractDir = Join-Path ([IO.Path]::GetTempPath()) "openbitfun-cli-package-$([guid]::NewGuid().ToString('N'))"
try {
    Expand-Archive -LiteralPath $archive -DestinationPath $extractDir
    $primaryCandidates = @(Get-ChildItem -LiteralPath $extractDir -Recurse -Filter 'openbitfun.exe')
    if ($primaryCandidates.Count -ne 1) {
        throw 'Expected exactly one OpenBitFun CLI entrypoint in the packaged archive'
    }
    foreach ($requiredFile in @(
        'README.md',
        'PROJECT-README.md',
        'THIRD_PARTY_NOTICES.md',
        'third-party\models.dev\LICENSE.txt',
        'third-party\models.dev\provenance.json',
        'resources\ext-host\extension-host.js'
    )) {
        if (-not (Test-Path -LiteralPath (Join-Path $primaryCandidates[0].DirectoryName $requiredFile) -PathType Leaf)) {
            throw "Packaged archive is missing $requiredFile"
        }
    }

    & $primaryCandidates[0].FullName --version
    Assert-LastExitCode 'packaged openbitfun --version'
    & $primaryCandidates[0].FullName --help | Out-Null
    Assert-LastExitCode 'packaged openbitfun --help'
}
finally {
    Remove-Item -LiteralPath $extractDir -Recurse -Force -ErrorAction SilentlyContinue
}

if ($GitHubOutput) {
    "archive=$(Split-Path -Leaf $archive)" >> $GitHubOutput
    "checksum=$(Split-Path -Leaf $checksum)" >> $GitHubOutput
}

Write-Host "Packaged and verified: $archive"
