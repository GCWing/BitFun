# OpenBitFun Native Mobile Apps

This directory contains the native mobile product surfaces for OpenBitFun:

- `android/`: Android application code and resources.
- `ios/`: iOS application code and resources.
- `harmonyos/`: HarmonyOS application code and resources.

The mobile apps are remote controllers: GitHub login and the account device
directory select a desktop or CLI host that owns configuration and Agent Runtime
execution. Phones submit tasks and display results; they do not synchronize model
configuration or execute agents locally.

Each platform directory owns its native UI, lifecycle, permissions, packaging,
and platform adapters. Product logic and stable contracts should remain in the
platform-agnostic Rust layers and be exposed to these apps through explicit
interfaces.

## Image messages

All three apps can send images with or without text. Camera photos are decoded
on the phone and converted to a supported format before upload. Failed sends
retain the draft and images; acknowledgement removes only the submitted content.

Model selection belongs to the connected host. A primary model that supports
images receives their pixels directly. For a text-only primary model, select an
enabled image-understanding model in the host settings and keep `analyze_image`
enabled for the agent. The receiving runtime saves inline attachments so the
same images remain available after restoring a conversation, including sessions
in SSH workspaces. An unavailable model or unreadable image produces an error.

## Shared visual contract

HarmonyOS is the current visual baseline. The source contract in
[`design-system/`](design-system/README.md) records the stable HarmonyOS colors,
type scale, geometry, breakpoints, motion, component anatomy, and deterministic
preview scenarios. A generator emits native constants for ArkUI, Compose, and
SwiftUI; each platform still owns its native component implementation.

```bash
pnpm run mobile:ui:generate
pnpm run mobile:ui:check
pnpm run mobile:ui:preview
```

The preview command opens a local three-column desktop surface for HarmonyOS,
Android, and iOS. It renders the same scenario from the contract and can overlay
native simulator or IDE-preview captures for pixel-level comparison.

## Native feature parity

HarmonyOS is the behavioral and visual reference for the mobile controller.
The Android and iOS implementations share capability negotiation, account display
metadata, execution-mode IDs, speech draft merging, and completion observation in
`shared/core-feature`; native views and OS adapters remain platform-owned.

| Capability | Android and iOS behavior | Compatibility and verification boundary |
|---|---|---|
| Execution modes | Minimal / Standard / Ultimate, with the HarmonyOS density glyph; workspace menus also retain Cowork | Enabled only by live `get_workspace_info.capabilities` containing `harness_profiles_v1`; older hosts keep Code / Cowork. Capability absence in old payloads is covered by a round-trip test. |
| Account profile | Public GitHub login and avatar replace the numeric-ID placeholder | The immutable Relay user ID still authorizes devices. Display metadata uses a separate encrypted 24-hour cache; offline refreshes preserve credentials and cached display. |
| Task completion | Notify for a previously observed successful remote turn while backgrounded | Identity includes target, session and turn. Replayed, failed and cancelled turns do not notify. These are local notifications, not server push. iOS observes within its OS background-task allowance; Android observes while the controller process remains alive. Neither promises notification after process termination. |
| Speech input | Continue an existing draft without trimming its whitespace or inserting spaces into Chinese text | iOS owns Speech/AVAudio lifecycle in a platform adapter and cancels on route, target or scene changes. Android uses the system recognition activity. |
| Conversation UI | Shared native tokens, neutral send/stop discs, supplementary voice action with a nonempty draft, plain assistant body | Native previews reuse production message rendering; compare the same scenario under `design-system`, then verify real app menus separately. |
| Remote operations | Existing session creation, tools/approvals/questions, models, images, remote file preview/download, and compact/wide layouts remain native | Simulator screenshots are presentation evidence, not evidence of a live SSH workspace, peer-device, or detached-dispatch session. |
| HarmonyOS watch provisioning | Remains HarmonyOS-specific | Android/iOS do not advertise watch provisioning without a supported platform transport and negotiated host contract. |

Focused checks are documented in `AGENTS.md` and each platform guide. Keep
comparison screenshots local under the design-system snapshot convention, or
with the task's local artifacts; do not commit account or device captures.

## Notification permission onboarding

iOS, Android 13+, and HarmonyOS offer an optional task-completion notification
introduction on first launch. Enable opens the system authorization dialog;
Later dismisses the introduction. The choice is stored per installation and
survives account changes and process restarts. Already-authorized installations
skip the introduction. Sending or observing a task never requests notification
permission. System settings remain the recovery path after skipping or denying. iOS also
provides a Notifications entry in app settings: it requests undecided permission
or opens system notification settings for an existing decision.
Camera and microphone access stays contextual to scanning and voice input.
Notification authorization does not extend the platform background-execution
limits described above.
