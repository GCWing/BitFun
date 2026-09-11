# Background command output

The backend retains a bounded raw ExecCommand stream with an output cursor.
`terminalSize` is reported by the local/remote execution provider and is the
geometry used to create its PTY. Missing geometry on legacy metadata means the
historical fixed 80x24 ExecCommand contract; explicit null means unknown and
disables accurate replay rather than guessing a controller-local geometry.

`BackgroundTerminalReplay` consumes snapshots and ordered increments using a
headless xterm at the source geometry. Keep CSI/OSC sequences and parser state
across chunks. In particular, ConPTY's last-column rewrites must overwrite cells,
not turn into line breaks. Snapshots/reset cursors replace the parser state.

`BackgroundTerminalProjection` renders the resulting text/SGR cells. Soft-wrapped
log rows can reflow at the panel width; coordinate-addressed screens keep source
columns and can scroll horizontally. Resizing this display never changes the
source PTY or the replay parser. Alternate screens are restored by the parser.
Copy output uses the parsed text;
copy raw output retains control sequences for diagnostics.

Poll only after the previous request and parser write finish. Discard responses
from an unmounted/session-switched view and drain completed output until its
cursor stops advancing. Reopening a view rebuilds from the retained snapshot.

A truncated snapshot is not a terminal checkpoint. It may lack cursor, screen,
or escape-parser context and must show an incomplete-replay notice. A continuously
attached parser retains its state when only the backend's old raw chunks expire.
The parser/display scrollback is bounded to 5000 rows. Source-side checkpoints
are not implemented by this viewer.

Focused non-UI regression verification:

```sh
pnpm --dir src/web-ui exec vitest run src/flow_chat/components/background-command/backgroundTerminalReplay.test.ts
```

The ConPTY fixture is a diagnostic recording with its machine-specific window
title removed. It covers the 80th-column overwrite, progress updates, cursor
redraw, alternate screen restoration, and arbitrary input chunk boundaries.
