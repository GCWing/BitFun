Optional, independently downloaded BitFun → OpenBitFun data migrator.

Extract the Windows ZIP and double-click the migrator, or open the macOS/Linux
package. Close all BitFun/OpenBitFun data writers first. Review source,
destination and the preflight plan before importing. Source data is retained;
interrupted runs can be resumed from Saved migration tasks.

Declared source range: BitFun >=0.2.0,<1.0.0. Archived integration fixture: 0.2.19.
The tool uses explicit storage-format validation and rejects unsupported data.
No main application installation, login, network or restart handoff is required.

See the README files at this release tag for compatibility, recovery, build and
signature verification details. Every package carries a SHA-256 sidecar and a
verified detached minisign signature. Platform code signing/notarization is not
configured by this workflow. This draft requires maintainer review and platform
launch checks before publication.
