# OpenBitFun Data Migrator

[中文](README.zh-CN.md)

A separate, optional desktop utility for importing old **BitFun** data into
**OpenBitFun**. It runs without installing or opening the main application, has
its own window and settings, and never starts or restarts Desktop. OpenBitFun
does not bundle, download, or launch it automatically.

## Download and run

Look for **OpenBitFun Data Migrator** releases with a `data-migrator-v*` tag on
the [release page](https://github.com/GCWing/OpenBitFun/releases?q=data-migrator-v&expanded=true).
These releases have their own version and assets; the main application's
installer does not contain the tool. If no migrator release is listed, build
from source using the commands below.

| Platform | Download | Launch |
| --- | --- | --- |
| Windows x64 | `openbitfun-data-migrator-v<version>-windows-x64.zip` | Extract, then double-click `openbitfun-data-migrator.exe` |
| macOS Apple Silicon | `openbitfun-data-migrator-v<version>-macos-arm64.dmg` | Open the DMG and its Data Migrator app |
| macOS Intel | `openbitfun-data-migrator-v<version>-macos-x64.dmg` | Open the DMG and its Data Migrator app |
| Linux x64 | `openbitfun-data-migrator-v<version>-linux-x64.AppImage` | Make executable and launch in a desktop session |

Windows needs the Microsoft Edge WebView2 runtime. macOS uses the system WebView;
Linux packages are built on Ubuntu 22.04. No login or network connection is
needed for migration. ARM Windows/Linux packages are not currently produced.

1. Close BitFun, OpenBitFun, their CLI instances, and background data writers.
2. Open Data Migrator. Check the **source and destination** directories. All
   four locations on each side can be edited; apply changes before scanning.
3. Select the data groups, scan, then run the preflight plan.
4. Review the destination and conflicts, then start migration. If known writers
   remain open, the tool waits for them to stop; it does not terminate them.
5. Read the report. Sign in again or repair paths where indicated, close the
   tool, and open OpenBitFun yourself.

The report lists Sessions, workspace records, and assistant directories separately.
Expected auxiliary exclusions (such as request traces and lock files) do not
contribute to these displayed counts. Historical runs use their saved manifests
for the same breakdown; missing manifests are shown as unavailable.

The UI uses the shared design-system tokens bundled offline, follows the system
light/dark/high-contrast setting, and offers English, Simplified Chinese, and
Traditional Chinese.

## Data and compatibility

The declared source range is BitFun `>=0.2.0,<1.0.0`; the archived integration
fixture is **0.2.19**. This is format-based support, not a claim that every old
release has been tested. The source must pass the probe and selected domain
validators. Unsupported or corrupt data is kept and reported.

Scanning isolates invalid settings/model entries, workspace registrations, memory
rows/files, SSH profiles, Remote Connect files/Bots, Sessions, Skills, MiniApps,
and Agent definitions. Readable Turns within a damaged Session are recovered;
derived Turn counts and workspace reference lists are rebuilt. Identical Turn
copies are deduplicated; conflicting Turn identities are omitted with warnings.
Nested user memory notes are supported. Legacy memory jobs are not read or
imported: the runtime creates jobs on demand, and destination jobs stay intact.
Optional Skills and MiniApps do not block importing Agent definitions.

Item failures during staging are omitted from the committed manifest where the
item has an independent storage boundary. Source data remains read-only and the
report shows omissions and partial history recovery. Unreadable destination
stores, unsafe paths, unsupported schemas, changed inputs, and transaction/write
failures still protect the affected domain; independent domains can continue
after successful rollback. A failed rollback stops execution. Review warnings
and the report before retrying.


The destination is OpenBitFun: configuration schema **1**, workspace registry
format **1**, coordination database schema **2**, and the session, memory,
extension, and connection formats accepted by the shared storage owners in
this source revision. Unknown product/configuration schemas and newer SQLite
or session schemas fail validation. A future storage format requires a new
migrator release; matching application and tool version numbers is unnecessary.
Custom branded products are not supported by this tool.

Migration covers settings and credentials; user Agents, Skills and MiniApps;
workspaces, sessions and task records; memories; and local connection/device
records. Existing destination values take priority or conflicts are preserved
under a new identity according to the domain policy. Runtime caches, locks,
process discovery files, built-in executable content and request traces are
excluded. Credentials that cannot be decrypted on the destination require
sign-in again.

Source data is never automatically deleted. Writes use consistent snapshots,
staging, validation, backups, a migration lock and atomic replacement. Keep
both applications closed until the run finishes. Cancellation and window close
requests wait for an engine-declared safe boundary; already verified domains
may remain imported.


Agent coordination imports do not require historical Session/Turn references,
parent relationships, counters, timestamps, or Swarm lineage to remain valid.
Records that cannot be decoded or inserted under the target table constraints
are skipped individually. Tasks without a mapped Agent are skipped instead of
being attached to an unrelated target primary key. Existing target records still
win conflicts; imported Agent primary keys and task references are remapped.
Importing historical task state does not start or resume an Agent execution.

## Resume and diagnose

Plans, journals, reports, backups and staging live under:

```text
<destination settings-and-data>/data/migrations/bitfun-to-openbitfun/runs/<run-id>/
```

Reopen the tool, select the original directories, and use **Saved migration
tasks → Review / resume task**. Recovery requires a valid plan and unchanged
source fingerprint and resumes through the journal; it does not expire after
ten minutes. Completed reports can be reopened. New scans create new tasks and
never replace earlier journals. Old handoff-based plans remain readable even
if their `request.json` has expired; select the original locations before
resuming them. Unreadable files are not deleted or reset.

The tool remembers selected locations in its own `com.openbitfun.data-migrator`
application configuration directory. It does not write main-app onboarding or
reminder preferences. **Export failure diagnostics** writes a sanitized file
containing result codes and journal phases; full local reports and backups
can contain sensitive data and should stay private.

This tool only operates on files accessible on the computer where it runs.
Remote workspace execution, remote control, Peer Device Mode and Detached
Dispatch are not execution surfaces for it. Run it on the data-owning computer;
importing stored connection records does not connect to or migrate a remote host.

## Build and release

From the repository root with Rust, Node, pnpm and the platform's Tauri build prerequisites:

```bash
pnpm install
pnpm run data-migrator:dev       # independent window; no Desktop or dev server
pnpm run data-migrator:build     # independent release bundle
cargo build -p openbitfun-data-migrator --bin openbitfun-data-migrator
```

Direct Cargo builds embed the committed UI and design-system CSS. After changing
the token/theme owners run `pnpm run data-migrator:theme:generate`; the packaging
entry does this automatically. Desktop development/build commands do not build
the migrator. Shared Rust crates remain in the same source workspace to preserve
storage compatibility; there is no dependency on the main application's Core,
runtime assembly, Web UI, installer or updater.

The tool version is maintained in its own `Cargo.toml` and `tauri.conf.json`.
The **Data Migrator Package** workflow builds four platform artifacts manually
or on `data-migrator-v<version>` tags. Tag builds require the separate
`DATA_MIGRATOR_SIGNING_PRIVATE_KEY`, `DATA_MIGRATOR_SIGNING_PRIVATE_KEY_PASSWORD`
and `DATA_MIGRATOR_SIGNING_PUBKEY` secrets, verify checksums/signatures, and create
a **draft** release for review. Manual workflow runs only upload CI artifacts.
Publishing migrator releases does not start main-app packaging or update feeds.

Each asset has a SHA-256 sidecar and a base64-encoded minisign `.sig`; the
release also carries `SHA256SUMS` and `data-migrator.minisign.pub`. Verify the key
against the maintainer's trusted key before checking signatures. Detached
signatures are distinct from Apple/Authenticode platform signing; the workflow
does not currently configure those certificates or macOS notarization.

Focused checks and architecture rules are in [AGENTS.md](AGENTS.md).
