import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '..');

test('generates GitHub URLs for both Linux CLI and Relay architectures', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-linux-manifest-'));
  const assets = path.join(temp, 'assets');
  const out = path.join(temp, 'linux-binaries.json');
  fs.mkdirSync(assets);

  for (const target of [
    'x86_64-unknown-linux-gnu',
    'aarch64-unknown-linux-gnu',
  ]) {
    for (const filename of [
      `openbitfun-cli-1.2.3-${target}.tar.gz`,
      `openbitfun-relay-server-${target}.tar.gz`,
    ]) {
      fs.writeFileSync(path.join(assets, filename), '');
      fs.writeFileSync(path.join(assets, `${filename}.sha256`), '');
    }
  }

  const result = spawnSync(
    process.execPath,
    [
      'scripts/generate-linux-binaries-manifest.mjs',
      '--assets-dir',
      assets,
      '--version',
      '1.2.3',
      '--tag',
      'v1.2.3',
      '--repo',
      'GCWing/OpenBitFun',
      '--out',
      out,
    ],
    { cwd: repoRoot, encoding: 'utf8' }
  );
  assert.equal(result.status, 0, result.stderr);

  const manifest = JSON.parse(fs.readFileSync(out, 'utf8'));
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.platforms.linux_x86_64, undefined);
  assert.match(
    manifest.platforms['linux-x86_64'].cli.url,
    /releases\/download\/v1\.2\.3\/openbitfun-cli-1\.2\.3-x86_64/
  );
  assert.match(
    manifest.platforms['linux-aarch64'].relay.sha256Url,
    /openbitfun-relay-server-aarch64-unknown-linux-gnu\.tar\.gz\.sha256$/
  );
});

test('publishes sigUrl when a signature is present, omits it otherwise', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-linux-manifest-sig-'));
  const assets = path.join(temp, 'assets');
  const out = path.join(temp, 'linux-binaries.json');
  fs.mkdirSync(assets);

  for (const target of ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu']) {
    for (const filename of [
      `openbitfun-cli-1.2.3-${target}.tar.gz`,
      `openbitfun-relay-server-${target}.tar.gz`,
    ]) {
      fs.writeFileSync(path.join(assets, filename), '');
      fs.writeFileSync(path.join(assets, `${filename}.sha256`), '');
    }
  }
  // Sign only the x86_64 CLI, so both branches are covered in one run.
  fs.writeFileSync(
    path.join(assets, 'openbitfun-cli-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sig'),
    ''
  );
  fs.writeFileSync(
    path.join(assets, 'openbitfun-cli-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256.sig'),
    ''
  );

  const result = spawnSync(
    process.execPath,
    [
      'scripts/generate-linux-binaries-manifest.mjs',
      '--assets-dir', assets,
      '--version', '1.2.3',
      '--tag', 'v1.2.3',
      '--repo', 'GCWing/OpenBitFun',
      '--out', out,
    ],
    { cwd: repoRoot, encoding: 'utf8' }
  );
  assert.equal(result.status, 0, result.stderr);

  const manifest = JSON.parse(fs.readFileSync(out, 'utf8'));
  assert.match(
    manifest.platforms['linux-x86_64'].cli.sigUrl,
    /openbitfun-cli-1\.2\.3-x86_64-unknown-linux-gnu\.tar\.gz\.sig$/
  );
  assert.match(
    manifest.platforms['linux-x86_64'].cli.sha256SigUrl,
    /openbitfun-cli-1\.2\.3-x86_64-unknown-linux-gnu\.tar\.gz\.sha256\.sig$/
  );
  assert.equal(manifest.platforms['linux-x86_64'].relay.sigUrl, undefined);
  assert.equal(manifest.platforms['linux-x86_64'].relay.sha256SigUrl, undefined);
  assert.equal(manifest.platforms['linux-aarch64'].cli.sigUrl, undefined);
});

test('rejects versions whose build metadata GitHub would rewrite in asset names', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-linux-manifest-meta-'));
  const assets = path.join(temp, 'assets');
  const out = path.join(temp, 'linux-binaries.json');
  fs.mkdirSync(assets);

  const version = '1.2.3-nightly.20260724+abc1234';
  for (const target of ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu']) {
    for (const filename of [
      `openbitfun-cli-${version}-${target}.tar.gz`,
      `openbitfun-relay-server-${target}.tar.gz`,
    ]) {
      fs.writeFileSync(path.join(assets, filename), '');
      fs.writeFileSync(path.join(assets, `${filename}.sha256`), '');
    }
  }

  const result = spawnSync(
    process.execPath,
    [
      'scripts/generate-linux-binaries-manifest.mjs',
      '--assets-dir',
      assets,
      '--version',
      version,
      '--tag',
      'nightly',
      '--repo',
      'GCWing/OpenBitFun',
      '--out',
      out,
    ],
    { cwd: repoRoot, encoding: 'utf8' }
  );
  assert.notEqual(result.status, 0, 'build metadata must not reach a release asset name');
  assert.match(result.stderr, /not preserved verbatim by GitHub/);
  assert.equal(fs.existsSync(out), false);
});

test('openbitfun sync mirrors both products and their checksums', () => {
  const syncScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
    'utf8'
  );

  assert.match(syncScript, /linux-binaries\.json/);
  assert.match(syncScript, /for product in \("cli", "relay"\)/);
  assert.match(
    syncScript,
    /for key in \("url", "sha256Url", "sha256SigUrl", "sigUrl"\)/
  );
  assert.match(syncScript, /OPENBITFUN_BASE_URL/);
  assert.match(syncScript, /WEBSITE_RELEASE_DIR.*linux-binaries\.json/);
  assert.match(syncScript, /mirror_dispatch_macos_cli_archives/);
  assert.match(syncScript, /x86_64-apple-darwin aarch64-apple-darwin/);
  assert.match(syncScript, /WEBSITE_RELEASE_DIR.*relay-image\.json/);
});

test('openbitfun sync mirrors the website installer from the exact updater release', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-windows-installer-mirror-'));
  const versionDir = path.join(temp, 'release', '1.2.3');
  const calls = path.join(temp, 'download-calls.tsv');
  fs.mkdirSync(versionDir, { recursive: true });

  const result = spawnSync(
    'bash',
    ['-c', `
      source "$SYNC_SCRIPT"
      VERSION_DIR="$TEST_VERSION_DIR"
      RELEASE_ASSET_BASE_URL="https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3"
      LATEST_JSON="$TEST_LATEST_JSON"
      download_asset() {
        printf '%s\\t%s\\n' "$1" "$2" >> "$DOWNLOAD_CALLS"
      }
      mirror_windows_installer
    `],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        DOWNLOAD_CALLS: calls,
        SYNC_SCRIPT: path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
        TEST_VERSION_DIR: versionDir,
        TEST_LATEST_JSON: JSON.stringify({
          manual_installers: {
            'windows-x86_64': {
              url: 'https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe',
              signature_url: 'https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe.sig',
            },
          },
        }),
      },
    }
  );
  assert.equal(result.status, 0, result.stderr);

  const downloads = fs.readFileSync(calls, 'utf8').trim().split('\n');
  assert.deepEqual(downloads, [
    `https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe\t${versionDir}/OpenBitFun_1.2.3_windows-x86_64-installer.exe`,
    `https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe.sig\t${versionDir}/OpenBitFun_1.2.3_windows-x86_64-installer.exe.sig`,
  ]);
});

test('openbitfun sync uses the canonical fixed installer fallback', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-installer-mirror-'));
  const versionDir = path.join(temp, 'release', '1.2.2');
  const calls = path.join(temp, 'download-calls.tsv');
  fs.mkdirSync(versionDir, { recursive: true });

  const result = spawnSync(
    'bash',
    ['-c', `
      source "$SYNC_SCRIPT"
      VERSION_DIR="$TEST_VERSION_DIR"
      RELEASE_ASSET_BASE_URL="https://github.com/GCWing/OpenBitFun/releases/download/v1.2.2"
      download_asset() {
        printf '%s\\t%s\\n' "$1" "$2" >> "$DOWNLOAD_CALLS"
      }
      mirror_windows_installer
    `],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        DOWNLOAD_CALLS: calls,
        SYNC_SCRIPT: path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
        TEST_VERSION_DIR: versionDir,
      },
    }
  );
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(fs.readFileSync(calls, 'utf8').trim().split('\n'), [
    `https://github.com/GCWing/OpenBitFun/releases/download/v1.2.2/openbitfun-installer.exe\t${versionDir}/openbitfun-installer.exe`,
    `https://github.com/GCWing/OpenBitFun/releases/download/v1.2.2/openbitfun-installer.exe.sig\t${versionDir}/openbitfun-installer.exe.sig`,
  ]);
});

test('openbitfun sync mirrors complete signed macOS CLI sets for Dispatch', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-macos-cli-mirror-'));
  const versionDir = path.join(temp, 'release', '1.2.3');
  const calls = path.join(temp, 'download-calls.tsv');
  const checksums = path.join(temp, 'checksum-list.txt');
  fs.mkdirSync(versionDir, { recursive: true });

  const result = spawnSync(
    'bash',
    ['-c', `
      source "$SYNC_SCRIPT"
      VERSION="1.2.3"
      VERSION_DIR="$TEST_VERSION_DIR"
      RELEASE_ASSET_BASE_URL="https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3"
      curl() { return 0; }
      download_asset() {
        printf '%s\\t%s\\n' "$1" "$2" >> "$DOWNLOAD_CALLS"
      }
      verify_mirrored_checksums() {
        cat > "$CHECKSUM_LIST"
      }
      mirror_dispatch_macos_cli_archives
    `],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        CHECKSUM_LIST: checksums,
        DOWNLOAD_CALLS: calls,
        SYNC_SCRIPT: path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
        TEST_VERSION_DIR: versionDir,
      },
    }
  );
  assert.equal(result.status, 0, result.stderr);

  const downloads = fs.readFileSync(calls, 'utf8').trim().split('\n');
  assert.equal(downloads.length, 8);
  for (const target of ['x86_64-apple-darwin', 'aarch64-apple-darwin']) {
    const archive = `openbitfun-cli-1.2.3-${target}.tar.gz`;
    for (const suffix of ['', '.sha256', '.sha256.sig', '.sig']) {
      assert.ok(
        downloads.includes(
          `https://github.com/GCWing/OpenBitFun/releases/download/v1.2.3/${archive}${suffix}\t${versionDir}/${archive}${suffix}`
        )
      );
    }
  }
  assert.deepEqual(fs.readFileSync(checksums, 'utf8').trim().split('\n'), [
    'openbitfun-cli-1.2.3-x86_64-apple-darwin.tar.gz.sha256',
    'openbitfun-cli-1.2.3-aarch64-apple-darwin.tar.gz.sha256',
  ]);
});

test('website download manifest uses installer while updater manifest keeps setup', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'openbitfun-website-downloads-'));
  const versionDir = path.join(temp, 'release', '1.2.3');
  const updaterPath = path.join(versionDir, 'latest.json');
  fs.mkdirSync(versionDir, { recursive: true });

  const updater = {
    version: '1.2.3',
    notes: '',
    pub_date: '2026-08-05T00:00:00Z',
    platforms: {
      'windows-x86_64': {
        url: 'https://openbitfun.test/release/1.2.3/OpenBitFun_1.2.3_windows-x86_64-setup.exe',
      },
      'darwin-aarch64': {
        url: 'https://openbitfun.test/release/1.2.3/OpenBitFun_1.2.3_darwin-aarch64.app.tar.gz',
      },
    },
    manual_installers: {
      'windows-x86_64': {
        url: 'https://openbitfun.test/release/1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe',
        signature_url: 'https://openbitfun.test/release/1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe.sig',
      },
    },
  };
  fs.writeFileSync(updaterPath, `${JSON.stringify(updater, null, 2)}\n`);

  const result = spawnSync(
    'bash',
    ['-c', `
      source "$SYNC_SCRIPT"
      VERSION_DIR="$TEST_VERSION_DIR"
      OPENBITFUN_BASE_URL="https://openbitfun.test/release"
      WINDOWS_INSTALLER_FILENAME="openbitfun-installer.exe"
      WEBSITE_DOWNLOADS_MANIFEST="downloads.json"
      write_website_download_manifest
    `],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        SYNC_SCRIPT: path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
        TEST_VERSION_DIR: versionDir,
      },
    }
  );
  assert.equal(result.status, 0, result.stderr);

  const updaterAfter = JSON.parse(fs.readFileSync(updaterPath, 'utf8'));
  const website = JSON.parse(
    fs.readFileSync(path.join(versionDir, 'downloads.json'), 'utf8')
  );
  assert.match(
    updaterAfter.platforms['windows-x86_64'].url,
    /windows-x86_64-setup\.exe$/
  );
  assert.equal(website.schemaVersion, 1);
  assert.equal(website.version, '1.2.3');
  assert.equal(
    website.platforms['windows-x86_64'].url,
    'https://openbitfun.test/release/1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe'
  );
  assert.equal(
    website.platforms['windows-x86_64'].signatureUrl,
    'https://openbitfun.test/release/1.2.3/OpenBitFun_1.2.3_windows-x86_64-installer.exe.sig'
  );
  assert.equal(
    website.platforms['darwin-aarch64'].url,
    updater.platforms['darwin-aarch64'].url
  );
});

test('stable Linux archives are mirrored before the much larger Desktop packages', () => {
  const syncScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
    'utf8'
  );

  const stableBranch = syncScript.indexOf('if [ "$RELEASE_CHANNEL" = "stable" ]; then');
  const linuxCall = syncScript.indexOf('\n    mirror_linux_binaries\n', stableBranch);
  const desktopLoop = syncScript.indexOf('Mirroring Desktop asset');
  assert.ok(stableBranch > 0, 'stable channel branch must exist');
  assert.ok(linuxCall > stableBranch, 'stable main path must call mirror_linux_binaries');
  assert.ok(desktopLoop > 0, 'Desktop asset mirroring must still exist');
  assert.ok(
    linuxCall < desktopLoop,
    'CLI/Relay archives must be mirrored first: Desktop packages are ~700MB per ' +
      'release, and until the Linux assets land the mirror advertises a version ' +
      'whose CLI/Relay bytes it cannot serve'
  );
});

test('the mirror retains enough releases for older Desktop builds', () => {
  const syncScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
    'utf8'
  );
  const keep = /^KEEP_VERSIONS=(\d+)$/m.exec(syncScript);
  assert.ok(keep, 'KEEP_VERSIONS must be set');
  // Dispatch confirms an exact release before installation; keep that version
  // available long enough for a later click or retry to finish safely.
  assert.ok(
    Number(keep[1]) >= 4,
    `KEEP_VERSIONS must retain several releases, got ${keep[1]}`
  );
});

test('openbitfun sync lock and cron use the in-repo script, not a server-only copy', () => {
  const syncScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
    'utf8'
  );
  const cronFile = fs.readFileSync(
    path.join(repoRoot, 'deploy/openbitfun-host/release-sync.cron'),
    'utf8'
  );
  assert.match(
    syncScript,
    /OPENBITFUN_RELEASE_SYNC_LOCK/,
    'lock path must be overridable so a new host can run the repo script'
  );
  assert.match(
    syncScript,
    /\/root\/repos\/OpenBitFun\/scripts\/openbitfun-release-sync\.sh/,
    'documented cron must invoke the OpenBitFun checkout, not a detached copy'
  );
  assert.doesNotMatch(
    syncScript,
    /LOCK_FILE="\/root\/repos\/OpenBitFun-AutoUpdate\/sync\.lock"/,
    'a hardcoded AutoUpdate lock forces every new host to recreate a server-only tree'
  );
  assert.match(
    cronFile,
    /\/root\/repos\/OpenBitFun\/scripts\/openbitfun-release-sync\.sh/,
    'installed crontab must invoke the OpenBitFun checkout, not a detached copy'
  );
  assert.doesNotMatch(
    cronFile,
    /OpenBitFun-AutoUpdate/,
    'the crontab file must not revive the server-only AutoUpdate copy'
  );
  assert.match(
    syncScript,
    /LOCK_FILE="\$\{OPENBITFUN_RELEASE_SYNC_LOCK:-\/var\/lock\/openbitfun-release-sync\.lock\}"/,
    'default lock must stay off the Nginx /release/ alias'
  );
});
