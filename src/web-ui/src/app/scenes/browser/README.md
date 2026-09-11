# Embedded browser occlusion

Both `BrowserPanel` and `BrowserScene` use `useEmbeddedBrowserWebview` to own the
native child view. All show/hide/focus paths go through `nativeWebviewVisibility`.
DOM floating surfaces declare `data-openbitfun-native-webview-occlusion`; their
overlapping bounds suppress native visibility without destroying the page.

`BrowserPreviewCache` retains one decoded JPEG in memory beneath the native view.
It samples only while the native page and document are visible, waits one second
between completed captures, and allows at most one request in flight. Hiding never
waits for a screenshot. The resulting placeholder is decorative and cannot receive
input. Navigation, native viewport size changes and target replacement invalidate
the frame and reject stale responses. Frames are not persisted.

Native viewport edges are aligned to physical pixels using the controller's device
pixel ratio. The preview uses that same applied rectangle relative to its DOM host,
with square corners and explicit dimensions; generic image styles must not alter
its geometry. A one-physical-pixel change must reach the native view even at fractional DPI.

The desktop `browser_webview_capture_preview` command captures the specific child
view, bounds the longest edge to 1600 pixels, and encodes JPEG at quality 80 off
the async runtime worker. Windows and macOS use the existing WebView screenshot
adapter. Linux currently returns an explicit unsupported response. Unsupported or
older hosts retain the normal placeholder; transient capture failures back off to
five seconds and keep the last valid frame.

The command is controller-local in Peer Device Mode and never requests a screenshot
from the peer. SSH/Docker workspaces do not change ownership of this local UI view.
The web renderer keeps its existing iframe path. This command adds no preview
transport to mobile/IM remote control or detached jobs; peer hosts refuse it.

Focused verification:

```sh
pnpm --dir src/web-ui exec vitest run src/app/scenes/browser/browserPreviewCache.test.ts src/app/scenes/browser/nativeWebviewVisibility.test.ts src/app/scenes/browser/useEmbeddedBrowserWebview.test.tsx src/infrastructure/api/adapters/peer-device-adapter.test.ts
```

Manual checks: after rebuilding the desktop host, open a browser and let it settle,
then open session overview or a tab context menu over it. The last page frame should
remain in the uncovered region; closing the popup should restore the live page.
Also check navigation, resizing, multiple browser tabs and a video page. The frozen
image is not evidence that media playback or page execution has paused.
