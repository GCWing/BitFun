**中文** | [English](./verify-downloads.md)

# 校验 OpenBitFun 下载文件

带签名的 OpenBitFun Release 会为覆盖到的桌面安装包或 CLI 归档提供独立的
`<文件名>.sig`。例如，`v1.0.0` 已为桌面端和 CLI 下载文件提供签名。

OpenBitFun 固定使用以下 minisign 公钥：

- Key ID：`50F47CBE6CC0A376`
- 公钥：`RWR2o8Bsvnz0UOBc3NoTVW06wdiGM7pLP3LpiL4A3Sp4nxkBsWlJRTxn`

带签名的 Release 还会发布包含同一把公钥的 `minisign.pub`，OpenBitFun 官方更新
路径也内置了这把公钥。下面的命令直接固定公钥，避免仅仅因为签名和公钥来自
同一个下载位置就同时信任两者。

## macOS 或 Linux

先安装 [minisign](https://github.com/jedisct1/minisign/releases)，然后在一个新建
的空目录中运行以下命令。校验其他版本时，请将两个变量同时替换为 Release 页面
显示的准确 tag 和文件名。

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

校验成功时会输出 `Signature and comment signature verified`，并以状态码 0 退出。
如果校验失败，请不要运行或安装该文件。

## Windows PowerShell

安装 minisign 后，打开一个新建的空目录，并使用你所下载文件对应的准确 Release
tag 和文件名：

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
if ($LASTEXITCODE -ne 0) { throw "OpenBitFun 下载文件签名校验失败" }
```

## `.sig` 文件代表什么

OpenBitFun Release 的 `.sig` 是经过一层 base64 包装的 **minisign 签名**。交给
minisign 命令行工具之前，需要像上面的命令一样先解码一层。校验成功表示文件的
每个字节都与 OpenBitFun 固定发布公钥对应的签名一致；哪怕只修改一个字节，校验也会
失败。

这不是操作系统级代码签名。OpenBitFun 的 `.sig` 既不是 Apple Developer ID 签名或
公证票据，也不是 Windows Authenticode。因此，即使 minisign 校验成功，Gatekeeper
或 SmartScreen 仍可能独立显示提示。签名校验也不能替代你对软件及其依赖的正常
审查。
