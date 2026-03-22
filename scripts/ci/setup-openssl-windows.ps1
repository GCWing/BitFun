# Downloads FireDaemon OpenSSL (prebuilt) and sets OPENSSL_* for cargo/openssl-sys on Windows.
# Used by GitHub Actions; developers may run locally if paths are not already set.
# Keep $Version in sync with OPENSSL_VERSION in scripts/ensure-openssl-windows.mjs.
$ErrorActionPreference = "Stop"
$Version = "3.5.5"
$Url = "https://download.firedaemon.com/FireDaemon-OpenSSL/openssl-$Version.zip"
$TempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$Root = Join-Path $TempRoot "firedaemon-openssl"
$Zip = Join-Path $TempRoot "openssl-$Version.zip"

New-Item -ItemType Directory -Force -Path $Root | Out-Null
if (-not (Test-Path $Zip)) {
    Invoke-WebRequest -Uri $Url -OutFile $Zip
}
Expand-Archive -Path $Zip -DestinationPath $Root -Force

$X64 = Join-Path $Root "x64"
$LibCrypto = Join-Path $X64 "lib\libcrypto.lib"
if (-not (Test-Path $LibCrypto)) {
    throw "Expected prebuilt OpenSSL at $LibCrypto (extract layout changed?)"
}

$Dir = (Resolve-Path $X64).Path
if ($env:GITHUB_ENV) {
    Add-Content -Path $env:GITHUB_ENV -Value "OPENSSL_DIR=$Dir"
    Add-Content -Path $env:GITHUB_ENV -Value "OPENSSL_LIB_DIR=$Dir\lib"
    Add-Content -Path $env:GITHUB_ENV -Value "OPENSSL_STATIC=1"
} else {
    $env:OPENSSL_DIR = $Dir
    $env:OPENSSL_LIB_DIR = "$Dir\lib"
    $env:OPENSSL_STATIC = "1"
    Write-Host "Set OPENSSL_DIR=$Dir (current session only)"
}
