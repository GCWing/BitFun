[中文](./verify-downloads.zh-CN.md) | **English**

# Verify OpenBitFun downloads

Signed OpenBitFun releases provide a detached `<asset>.sig` file for each covered
desktop installer or CLI archive. Release `v1.0.0`, for example, provides
signatures for its desktop and CLI downloads.

OpenBitFun uses this pinned minisign public key:

- Key ID: `50F47CBE6CC0A376`
- Public key: `RWR2o8Bsvnz0UOBc3NoTVW06wdiGM7pLP3LpiL4A3Sp4nxkBsWlJRTxn`

The same key is published as `minisign.pub` with signed releases and is built
into official OpenBitFun update paths. The commands below pin the key directly so
the signature and key are not both trusted only because they came from the same
download location.

## macOS or Linux

Install [minisign](https://github.com/jedisct1/minisign/releases), then run the
following in a new empty directory. Replace both values with the exact tag and
asset name shown on the release page when verifying another download.

```bash
VERSION=v1.0.0
ASSET=openbitfun-cli-1.0.0-aarch64-unknown-linux-gnu.tar.gz
BASE="https://github.com/GCWing/OpenBitFun/releases/download/$VERSION"
PUBLIC_KEY=RWR2o8Bsvnz0UOBc3NoTVW06wdiGM7pLP3LpiL4A3Sp4nxkBsWlJRTxn

curl --fail --location --remote-name "$BASE/$ASSET"
curl --fail --location --remote-name "$BASE/$ASSET.sig"
base64 --decode <"$ASSET.sig" >"$ASSET.minisig"
minisign -Vm "$ASSET" -P "$PUBLIC_KEY" -x "$ASSET.minisig"
```

A valid download prints `Signature and comment signature verified` and exits
with status 0. Do not run or install the asset if verification fails.

## Windows PowerShell

Install minisign, open a new empty directory, and use the exact release tag and
asset name you downloaded:

```powershell
$Version = "v1.0.0"
$Asset = "OpenBitFun_1.0.0_windows-x86_64-setup.exe"
$Base = "https://github.com/GCWing/OpenBitFun/releases/download/$Version"
$PublicKey = "RWR2o8Bsvnz0UOBc3NoTVW06wdiGM7pLP3LpiL4A3Sp4nxkBsWlJRTxn"

Invoke-WebRequest "$Base/$Asset" -OutFile $Asset
Invoke-WebRequest "$Base/${Asset}.sig" -OutFile "${Asset}.sig"
$EncodedSignature = (Get-Content "${Asset}.sig" -Raw).Trim()
[IO.File]::WriteAllBytes("${Asset}.minisig", [Convert]::FromBase64String($EncodedSignature))
minisign -Vm $Asset -P $PublicKey -x "${Asset}.minisig"
if ($LASTEXITCODE -ne 0) { throw "OpenBitFun download signature verification failed" }
```

## What the `.sig` file means

OpenBitFun release `.sig` files are base64-wrapped **minisign signatures**. Decode
one layer before giving the result to the minisign CLI, as shown above. A valid
signature proves that the file's exact bytes match a signature made by the
pinned OpenBitFun release key; changing even one byte makes verification fail.

This is not platform code signing. In particular, an OpenBitFun `.sig` is not an
Apple Developer ID signature or notarization ticket, and it is not Windows
Authenticode. Gatekeeper and SmartScreen can therefore show their own warnings
independently of a successful minisign check. Signature verification also does
not replace your normal review of the software and its dependencies.
