# OpenBitFun 生产源站恢复手册

给换机后的人和 AI Agent：clone BitFun 之后按本文把当前 `ssh lwb` 这台源站
搬到新机器。命令默认从 BitFun 仓库根目录执行。

这是 **已有源站搬迁**，不是从零装一套空服务。不要对仍在服务的 `lwb` 重跑
clone、`crontab` 整表安装、市场 first-install 或空 volume 的 `deploy.sh`。

不要再发明一份脱离仓库的同步脚本副本。历史上的
`/root/repos/BitFun-AutoUpdate/` 只是旧主机上的运行拷贝，换机后不要重建。
当前源站只靠 cron，不必恢复 webhook listener。

## Agent 先读这个

1. 根目录 `AGENTS.md`，再读本文件。
2. MiniApp / Skin 分别只走它们自己的手册，不要从本目录重建那两个容器。
3. 不要 `cat`、下载或打印 `/etc/bitfun-*/market.env`、New API 数据目录、账号密码。
4. 不要在 `/root/repos/BitFun` 或 `/root/repos/BitFun-Website` 上跑 `git clean`。
   共享 BitFun checkout 上也禁止无确认的 `git reset --hard`（与 MiniApp 手册一致）。
   该 checkout 里可能有 Relay 的未跟踪 `.env`。
5. 本机 Nginx 只听 80。HTTPS 在前面的云 WAF / 负载均衡终止，不要在这台机器上
   擅自申请 Let's Encrypt 或改 443。
6. 未知 Host 必须落到 `00-default-server.conf` 的 404，不能落到 New API 或 Relay。
7. 每步失败立刻停。先做旧机只读导出，再在新机导入。

## 这台机器上有什么

| 产品 | 公网 | 源站 | 仓库内入口 |
| --- | --- | --- | --- |
| 官网 + 下载页 | `https://openbitfun.com/` 、`/download` | Nginx → `BitFun-Website/dist` | 本文「官网」 |
| BitFun Playbook | `https://playbook.openbitfun.com/` | Nginx → `/srv/bitfun-playbook/current` | 本文「BitFun Playbook」 |
| Release 镜像 | `https://openbitfun.com/release/` | cron → `/srv/bitfun-release` | 本文「Release 镜像」；脚本在 `scripts/openbitfun-release-sync.sh` |
| Relay | `https://remote.openbit.fun/relay` | `bitfun-relay:9700` | `src/apps/relay-server/README.md` + 本文 Nginx |
| MiniApp 市场 | `https://market.openbitfun.com/miniapp/` | `127.0.0.1:9710` | [../miniapp-market/README.md](../miniapp-market/README.md) |
| Skin 市场 | `https://market.openbitfun.com/skin/` | `127.0.0.1:9720` | [../skin-market/README.md](../skin-market/README.md) |
| New API | `https://api.openbit.fun/` | `0.0.0.0:33292` | **不在 BitFun 仓库**。见本文「New API」 |

当前生产 SSH 别名是 `lwb`（root）。下文旧机用 `OLD_HOST=lwb`，新机用
`NEW_HOST`。换机后可以把 `lwb` 指到新主机，但导出/导入期间必须同时能 SSH
到两台。

生产路径约定：

| 路径 | 用途 |
| --- | --- |
| `/root/repos/BitFun` | BitFun checkout。Relay 静态页和同步脚本从这里读 |
| `/root/repos/BitFun-Website` | 官网独立仓库 `GCWing/BitFun-Website` |
| `/srv/bitfun-release` | GitHub Release 镜像（禁止放进 Website `dist/`） |
| `/srv/bitfun-playbook` | Playbook 的不可变静态版本与 `current` 软链接 |
| `/srv/bitfun-miniapp-market` | MiniApp 专用 checkout / 数据 / 备份 |
| `/srv/bitfun-skin-market` | Skin 专用 checkout / 数据 / 备份 |
| `/etc/bitfun-miniapp-market/market.env` | MiniApp secrets，`root:root` `0600` |
| `/etc/bitfun-skin-market/market.env` | Skin secrets，`root:root` `0600` |
| `/root/repos/new_api_bak` | New API 镜像 tar + `data/` 卷。BitFun 不管 |

Git remote 不要混用：

- **换机新 clone** `https://github.com/GCWing/BitFun.git` 之后，唯一 remote 是
  `origin`，跟踪 `origin/main`。
- **当前仍在服务的 `lwb`** 是 `origin=bobleer/BitFun`、
  `upstream=GCWing/BitFun`。只在这台旧机上才使用 `upstream`。

脚本里的 `OPENBITFUN_BASE_URL` 写死为 `https://openbitfun.com/release`。
域名变了要改 `scripts/openbitfun-release-sync.sh`，不能只改 Nginx。

## A. 旧机只读导出

在能 SSH 到旧机的机器上执行。默认 `OLD_HOST=lwb`。导出目录在旧机
`/root/bitfun-host-export`，不要写进 Website `dist/` 或 BitFun checkout。

```bash
OLD_HOST=lwb
ssh "$OLD_HOST" 'set -eu
EXPORT=/root/bitfun-host-export
rm -rf "$EXPORT"
install -d -m 0700 "$EXPORT"

git -C /root/repos/BitFun remote -v >"$EXPORT/bitfun.remotes"
git -C /root/repos/BitFun rev-parse HEAD >"$EXPORT/bitfun.rev"
git -C /root/repos/BitFun-Website rev-parse HEAD >"$EXPORT/website.rev"
git -C /srv/bitfun-miniapp-market/app rev-parse HEAD >"$EXPORT/miniapp.rev"
git -C /srv/bitfun-skin-market/app rev-parse HEAD >"$EXPORT/skin.rev"
crontab -l >"$EXPORT/root.crontab"
uname -a >"$EXPORT/uname.txt"
docker ps --format "{{.Names}} {{.Image}} {{.Status}}" >"$EXPORT/docker.ps"
node -v >"$EXPORT/node.version" 2>/dev/null || true

docker run --rm \
  -v relay-server_relay-db:/data \
  -v "$EXPORT":/backup \
  alpine tar czf /backup/relay-server_relay-db.tar.gz -C /data .
docker run --rm \
  -v relay-server_room-web:/data \
  -v "$EXPORT":/backup \
  alpine tar czf /backup/relay-server_room-web.tar.gz -C /data .

test -s "$EXPORT/bitfun.rev"
test -s "$EXPORT/relay-server_relay-db.tar.gz"
test -s "$EXPORT/relay-server_room-web.tar.gz"
ls -la "$EXPORT"'
```

然后把导出目录、Release 镜像、市场数据、New API 和 secrets **拷到新机**。
`rsync` 不要加 `-v` 去扫 `market.env` 内容。

```bash
OLD_HOST=lwb
NEW_HOST=new-openbitfun   # 换成新机器 SSH 别名
ssh "$NEW_HOST" 'install -d -m 0700 /root/bitfun-host-export /root/repos /srv'

rsync -aH --numeric-ids "$OLD_HOST:/root/bitfun-host-export/" \
  "$NEW_HOST:/root/bitfun-host-export/"
rsync -aH --numeric-ids "$OLD_HOST:/srv/bitfun-release/" \
  "$NEW_HOST:/srv/bitfun-release/"
rsync -aH --numeric-ids \
  "$OLD_HOST:/srv/bitfun-miniapp-market/" \
  "$NEW_HOST:/srv/bitfun-miniapp-market/"
rsync -aH --numeric-ids \
  "$OLD_HOST:/srv/bitfun-skin-market/" \
  "$NEW_HOST:/srv/bitfun-skin-market/"
rsync -aH --numeric-ids \
  "$OLD_HOST:/etc/bitfun-miniapp-market/" \
  "$NEW_HOST:/etc/bitfun-miniapp-market/"
rsync -aH --numeric-ids \
  "$OLD_HOST:/etc/bitfun-skin-market/" \
  "$NEW_HOST:/etc/bitfun-skin-market/"
rsync -aH --numeric-ids \
  "$OLD_HOST:/root/repos/new_api_bak/" \
  "$NEW_HOST:/root/repos/new_api_bak/"
```

验收：新机上 `test -s /root/bitfun-host-export/relay-server_relay-db.tar.gz`、
`test -d /srv/bitfun-release/0.2.18`、`test -f /etc/bitfun-miniapp-market/market.env`。
不要 `cat` 任何 `market.env`。

## B. 新机导入

下面 `ssh` 目标都是 `$NEW_HOST`。先装 Docker、Nginx、cron，再按顺序做。
每块做完就跑该块验证。

### 1. 基础软件和 Node

生产官网构建用 **Node v20.20.2**，装在 `/root/.nvm`。非交互 SSH 默认不加载
nvm，所以每条 `npm` 命令都要先 `source`。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y docker.io nginx cron rsync curl ca-certificates python3
systemctl enable --now docker nginx cron
command -v docker
command -v nginx
systemctl is-active cron

export NVM_DIR="$HOME/.nvm"
if [ ! -s "$NVM_DIR/nvm.sh" ]; then
  curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
fi
# shellcheck disable=SC1091
. "$NVM_DIR/nvm.sh"
nvm install 20.20.2
nvm alias default 20.20.2
node -v
test "$(node -v)" = "v20.20.2"'
```

### 2. BitFun checkout

新机从 **GCWing/BitFun** clone，只用 `origin`。checkout 导出记录的 commit，
不要猜 `upstream/main`。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
BITFUN_REV="$(cat /root/bitfun-host-export/bitfun.rev)"
test -n "$BITFUN_REV"
if [ ! -d /root/repos/BitFun/.git ]; then
  git clone https://github.com/GCWing/BitFun.git /root/repos/BitFun
fi
cd /root/repos/BitFun
git remote -v
git fetch origin
git checkout --detach "$BITFUN_REV"
test "$(git rev-parse HEAD)" = "$BITFUN_REV"
test -x scripts/openbitfun-release-sync.sh'
```

`git remote -v` 应只有 `origin → github.com/GCWing/BitFun.git`。

### 3. Nginx 兜底（先拆 Ubuntu default）

新装 Nginx 自带 `sites-enabled/default`，也是 `default_server`。不删掉的话，
未知 Host 不会落到市场手册里的 404 兜底。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
rm -f /etc/nginx/sites-enabled/default
install -m 0644 \
  /root/repos/BitFun/deploy/miniapp-market/nginx-default-server.conf \
  /etc/nginx/sites-available/00-default-server.conf
ln -sfn /etc/nginx/sites-available/00-default-server.conf \
  /etc/nginx/sites-enabled/00-default-server.conf
nginx -t
systemctl reload nginx
code="$(curl -sS -o /dev/null -w "%{http_code}" \
  -H "Host: unconfigured.openbitfun.com" http://127.0.0.1/)"
test "$code" = "404"'
```

### 4. Release 镜像

先用旧机 rsync 过来的目录（保留 0.2.14 起的多版本，供旧 Desktop / Dispatch），
再跑一次 in-repo 同步补最新版。不要对空目录只 sync 一次就当完成。

Windows 网页安装包文件名以 GitHub `latest.json` 的 `manual_installers` 为准
（现在是 `BitFun_${version}_windows-x86_64-installer.exe`）。不要再写死
`bitfun-installer.exe`。

`release-sync.cron` **就是这台机器的整份 root crontab**。`crontab` 该文件会
替换所有 root cron。先备份，确认没有其它任务再装。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
test -d /srv/bitfun-release
test -f /srv/bitfun-release/downloads.json
touch /var/log/bitfun-release-sync.log
chmod 644 /var/log/bitfun-release-sync.log
crontab -l > /root/bitfun-host-export/new-host.crontab.bak 2>/dev/null || true
crontab /root/repos/BitFun/deploy/openbitfun-host/release-sync.cron
crontab -l
/root/repos/BitFun/scripts/openbitfun-release-sync.sh
python3 -c "import json; print(json.load(open(\"/srv/bitfun-release/downloads.json\"))[\"version\"])"'
```

`downloads.json` 的 `version` 必须等于
`https://github.com/GCWing/BitFun/releases/latest/download/latest.json` 的
`version`。日志里若再出现 `Failed to download bitfun-installer.exe`，说明跑到了
旧脚本，停下来改 cron，不要手工改清单。

锁文件默认是 `/var/lock/bitfun-release-sync.lock`，不要再指向
`/root/repos/BitFun-AutoUpdate/sync.lock`，也不要放进 `/srv/bitfun-release`
（该目录由 Nginx `/release/` 对外提供）。

可选 beta：

```bash
BITFUN_RELEASE_CHANNEL=beta /root/repos/BitFun/scripts/openbitfun-release-sync.sh
```

### 5. 官网

官网不在 BitFun 里。源码是 `https://github.com/GCWing/BitFun-Website`。
`npm run build` 会清空 `dist/`，所以 Release 镜像绝不能放进 `dist/release`。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
WEBSITE_REV="$(cat /root/bitfun-host-export/website.rev)"
test -n "$WEBSITE_REV"
if [ ! -d /root/repos/BitFun-Website/.git ]; then
  git clone https://github.com/GCWing/BitFun-Website.git /root/repos/BitFun-Website
fi
cd /root/repos/BitFun-Website
git fetch origin
git checkout --detach "$WEBSITE_REV"
export NVM_DIR="$HOME/.nvm"
# shellcheck disable=SC1091
. "$NVM_DIR/nvm.sh"
nvm use 20.20.2
npm install
npm run build
test -f dist/index.html
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-openbit.fun.conf \
  /etc/nginx/sites-available/openbit.fun
ln -sfn /etc/nginx/sites-available/openbit.fun /etc/nginx/sites-enabled/openbit.fun
nginx -t
systemctl reload nginx
curl -fsS -o /dev/null -w "%{http_code}\n" \
  -H "Host: openbitfun.com" http://127.0.0.1/'
```

更新官网：在 Website 仓库拉代码后只跑 `npm run build`，不要动 `/srv/bitfun-release`。

### 5.1 BitFun Playbook

Playbook 源码在 BitFun 仓库的 `website/`。功能与设置的唯一人工维护数据源是
`src/shared/interactive-capabilities/catalog.json`；网站只读取由它生成的
`docs/interactive-capabilities/capabilities.json`。构建与上传从开发机执行；源站只保存
不可变静态版本，不需要 Node 运行时。

```bash
pnpm run capabilities:check
pnpm run capabilities:test
pnpm run website:test
pnpm run website:build
PLAYBOOK_RELEASE="$(node -e 'const r=require("./website/dist/release.json");process.stdout.write(r.releaseId)')"
PLAYBOOK_SAMPLE_ID="$(node -e 'const c=require("./docs/interactive-capabilities/capabilities.json");process.stdout.write(c.capabilities[0].id)')"
test -n "$PLAYBOOK_RELEASE"
test -n "$PLAYBOOK_SAMPLE_ID"
ssh lwb "install -d -m 0755 /srv/bitfun-playbook/releases/$PLAYBOOK_RELEASE"
rsync -a --delete website/dist/ \
  "lwb:/srv/bitfun-playbook/releases/$PLAYBOOK_RELEASE/"
rsync -a deploy/openbitfun-host/nginx-playbook.openbitfun.com.conf \
  lwb:/tmp/nginx-playbook.openbitfun.com.conf
ssh lwb "set -eu
install -m 0644 /tmp/nginx-playbook.openbitfun.com.conf \
  /etc/nginx/sites-available/playbook.openbitfun.com
ln -sfn /srv/bitfun-playbook/releases/$PLAYBOOK_RELEASE \
  /srv/bitfun-playbook/current
ln -sfn /etc/nginx/sites-available/playbook.openbitfun.com \
  /etc/nginx/sites-enabled/playbook.openbitfun.com
nginx -t
systemctl reload nginx
curl --retry 5 --retry-delay 1 --retry-all-errors -fsS -o /dev/null \
  -H 'Host: playbook.openbitfun.com' http://127.0.0.1/
curl --retry 5 --retry-delay 1 --retry-all-errors -fsS -o /dev/null \
  -H 'Host: playbook.openbitfun.com' \
  http://127.0.0.1/capabilities/$PLAYBOOK_SAMPLE_ID/
curl --retry 5 --retry-delay 1 --retry-all-errors -fsS -o /dev/null \
  -H 'Host: playbook.openbitfun.com' \
  http://127.0.0.1/data/capabilities.json"
```

`systemctl reload` 返回时旧 worker 可能仍短暂接请求，所以源站验收必须带重试。
最后通过公网检查首页、任一详情页和 `/data/capabilities.json`；若源站 Host 检查为 200、
公网仍失败，应在云 WAF / DNS 增加该主机名，源站不要自行配置 443。

### 6. Relay

必须先恢复两个 volume，再启动容器。先跑 `deploy.sh` 会建空卷，账号、同步和
Pages 资产都会丢。不要在空库上用 `relay-admin` 新建用户来“代替”恢复。

当前生产 `.env` 只有镜像开关（`BITFUN_USE_CN_MIRROR` 等）。新机用
`BITFUN_MIRROR=auto`，让 `deploy.sh` 按主机网络自己选。不要写死
`--global-mirror`。不要 `cat` 旧 `.env`。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
EXPORT=/root/bitfun-host-export
test -s "$EXPORT/relay-server_relay-db.tar.gz"
test -s "$EXPORT/relay-server_room-web.tar.gz"

docker volume create relay-server_relay-db
docker volume create relay-server_room-web
docker run --rm \
  -v relay-server_relay-db:/data \
  -v "$EXPORT":/backup \
  alpine tar xzf /backup/relay-server_relay-db.tar.gz -C /data
docker run --rm \
  -v relay-server_room-web:/data \
  -v "$EXPORT":/backup \
  alpine tar xzf /backup/relay-server_room-web.tar.gz -C /data

cd /root/repos/BitFun/src/apps/relay-server
BITFUN_MIRROR=auto bash deploy.sh
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-remote.openbit.fun.conf \
  /etc/nginx/sites-available/remote.openbit.fun
ln -sfn /etc/nginx/sites-available/remote.openbit.fun \
  /etc/nginx/sites-enabled/remote.openbit.fun
nginx -t
systemctl reload nginx

curl -fsS http://127.0.0.1:9700/health
curl -fsS -o /dev/null -w "%{http_code}\n" \
  -H "Host: remote.openbit.fun" http://127.0.0.1/relay/health
docker exec bitfun-relay /app/relay-admin --db /app/data/bitfun_relay.db list-users'
```

`list-users` 必须能看到旧机已有账号（当前是 `bowen628`、`wgq@2012`）。
若只有空表，停下来，不要 `add-user`。

### 7. MiniApp / Skin（搬迁，不是首次安装）

不要跑市场手册里的「从 `market.env.example` 生成新 secret」。旧
`market.env`、SQLite、artifacts 已经 rsync 过来。

1. 确认 `/etc/bitfun-*-market/market.env` 权限仍是 `root:root` `0600`，不要打开文件。
2. 确认 `/srv/bitfun-*-market/{data,artifacts,backups}` 在。
3. 专用 checkout 回到导出的 commit，再按对应手册 **只 recreate 容器**，不要
   当绿场初始化。
4. Skin 仍从 MiniApp 专用 checkout 的契约出发，不要用 `/root/repos/BitFun`
   当 Skin 的运行 checkout。
5. 按市场手册安装它们自己的 Nginx 与 backup timer。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
MINIAPP_REV="$(cat /root/bitfun-host-export/miniapp.rev)"
SKIN_REV="$(cat /root/bitfun-host-export/skin.rev)"
test -n "$MINIAPP_REV"
test -n "$SKIN_REV"
stat -c "%U:%G %a %n" /etc/bitfun-miniapp-market/market.env \
  /etc/bitfun-skin-market/market.env
test -f /srv/bitfun-miniapp-market/data/market.sqlite
test -f /srv/bitfun-skin-market/data/market.sqlite
git -C /srv/bitfun-miniapp-market/app checkout --detach "$MINIAPP_REV"
git -C /srv/bitfun-skin-market/app checkout --detach "$SKIN_REV"
test "$(git -C /srv/bitfun-miniapp-market/app rev-parse HEAD)" = "$MINIAPP_REV"
test "$(git -C /srv/bitfun-skin-market/app rev-parse HEAD)" = "$SKIN_REV"'
```

然后只按：

- MiniApp：[../miniapp-market/README.md](../miniapp-market/README.md) 的构建 /
  recreate / 验证，不要走「初次开放市场」生成新 OAuth。
- Skin：[../skin-market/README.md](../skin-market/README.md) 同样只 recreate。

### 8. New API

这是改过的 `new-api` 镜像，**BitFun clone 恢复不了**。旧机事实：

- 镜像：`new-api:v1.0.0-rc.14-openbitfun-mod`
- 离线包：`/root/repos/new_api_bak/new-api-v1.0.0-rc.14-openbitfun-mod.tar`
- 数据：`/root/repos/new_api_bak/data` → 容器 `/data`
- 启动：`/root/repos/new_api_bak/start.sh`（宿主 `33292` → 容器 `3000`）

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
test -f /root/repos/new_api_bak/new-api-v1.0.0-rc.14-openbitfun-mod.tar
test -d /root/repos/new_api_bak/data
/root/repos/new_api_bak/load_image.sh
/root/repos/new_api_bak/start.sh
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-api.openbit.fun.conf \
  /etc/nginx/sites-available/api.openbit.fun
ln -sfn /etc/nginx/sites-available/api.openbit.fun \
  /etc/nginx/sites-enabled/api.openbit.fun
nginx -t
systemctl reload nginx
docker inspect --format "{{.State.Status}}" new-api'
```

没有 tar 和 data 就不要猜镜像、不要空库上线。

### 9. 一次性对齐本目录的 Nginx

若前面分步已装过官网 / Relay / API vhost，这一节只用来核对文件是否与仓库一致。
先 `nginx -t` 再 reload。市场 vhost 仍按市场手册安装，不要用本目录文件覆盖
`market.openbitfun.com.conf`。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
rm -f /etc/nginx/sites-enabled/default
install -m 0644 \
  /root/repos/BitFun/deploy/miniapp-market/nginx-default-server.conf \
  /etc/nginx/sites-available/00-default-server.conf
ln -sfn /etc/nginx/sites-available/00-default-server.conf \
  /etc/nginx/sites-enabled/00-default-server.conf
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-openbit.fun.conf \
  /etc/nginx/sites-available/openbit.fun
ln -sfn /etc/nginx/sites-available/openbit.fun /etc/nginx/sites-enabled/openbit.fun
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-playbook.openbitfun.com.conf \
  /etc/nginx/sites-available/playbook.openbitfun.com
ln -sfn /etc/nginx/sites-available/playbook.openbitfun.com \
  /etc/nginx/sites-enabled/playbook.openbitfun.com
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-remote.openbit.fun.conf \
  /etc/nginx/sites-available/remote.openbit.fun
ln -sfn /etc/nginx/sites-available/remote.openbit.fun \
  /etc/nginx/sites-enabled/remote.openbit.fun
install -m 0644 \
  /root/repos/BitFun/deploy/openbitfun-host/nginx-api.openbit.fun.conf \
  /etc/nginx/sites-available/api.openbit.fun
ln -sfn /etc/nginx/sites-available/api.openbit.fun \
  /etc/nginx/sites-enabled/api.openbit.fun
nginx -t
systemctl reload nginx'
```

### 10. 切 DNS / WAF

本机没有 443。把云 WAF / 负载均衡的源站指到新机器后，再跑公网验收。

## 整机验收

在源站本机用 `Host` 头打 80，不要依赖这台机器能否解析公网 DNS。

```bash
NEW_HOST=new-openbitfun
ssh "$NEW_HOST" 'set -eu
curl -fsS -o /dev/null -w "openbitfun.com/ %{http_code}\n" \
  -H "Host: openbitfun.com" http://127.0.0.1/
python3 - <<PY
import json, urllib.request
req = urllib.request.Request(
    "http://127.0.0.1/release/downloads.json",
    headers={"Host": "openbitfun.com"},
)
with urllib.request.urlopen(req, timeout=15) as resp:
    data = json.load(resp)
print("downloads.json", data["version"])
PY
curl -fsS -o /dev/null -w "remote /relay/health %{http_code}\n" \
  -H "Host: remote.openbit.fun" http://127.0.0.1/relay/health
curl -fsS -o /dev/null -w "unknown host %{http_code}\n" \
  -H "Host: unconfigured.openbitfun.com" http://127.0.0.1/
docker ps --format "{{.Names}} {{.Status}}"
crontab -l
docker exec bitfun-relay /app/relay-admin --db /app/data/bitfun_relay.db list-users'
```

预期：官网 200；`downloads.json` 版本等于 GitHub latest；Relay health 200；
未知 Host 404；`bitfun-relay` healthy；cron 指向
`/root/repos/BitFun/scripts/openbitfun-release-sync.sh`；`list-users` 含旧账号。

公网验收（从能解析 DNS 的机器）：

- `https://openbitfun.com/download` 显示的版本 = `downloads.json`
- `https://openbitfun.com/release/latest.json` 与 GitHub latest 同版本
- `https://remote.openbit.fun/relay/health`
- `https://market.openbitfun.com/miniapp/api/v1/health`
- `https://market.openbitfun.com/skin/`（按 Skin 手册）

## 日常更新（不是换机）

在 **当前 `lwb`** 上，GCWing 叫 `upstream`。在按本文新 clone 的机器上，GCWing
叫 `origin`。先 `git remote -v` 再选命令。

| 要做的事 | 怎么做 |
| --- | --- |
| 更新 BitFun 源码 | 先 `cp -a src/apps/relay-server/.env /tmp/relay-server.env.bak`（若存在）。`git fetch` 对应 remote，再 `git merge --ff-only <remote>/main`。不要默认 `reset --hard`，不要 `git clean`。 |
| 更新 Relay | 源码更新后 `cd src/apps/relay-server && BITFUN_MIRROR=auto bash deploy.sh`。账号 volume 会留下。 |
| 更新 Release 镜像脚本 | 只改仓库里的 `scripts/openbitfun-release-sync.sh`。cron 已经跑这份文件。 |
| 更新官网 | 在 `BitFun-Website` 拉代码，`source ~/.nvm/nvm.sh && nvm use 20.20.2 && npm run build`。 |
| 更新 Playbook | 在开发机校验能力契约并运行 `pnpm run website:build`，再按「BitFun Playbook」使用 `dist/release.json` 的版本目录上传和切换 `current`。 |
| 更新市场 | 只用对应市场手册。 |

更新 BitFun 源码**不会**自动更新官网、New API 或市场容器。
