# UI Test IDs

This document records stable `data-testid` values used by BitFun UI automation.
Test IDs are grouped by product area and should be added only when an automated
workflow needs a stable locator.

Rules:

- Use `data-testid` only as a test locator. Do not branch product logic on it.
- Prefer the real interactive element: `button`, `input`, editable region, or dialog root.
- Keep `data-testid` values stable, lowercase, and hyphen-separated.
- For repeated items, use one shared `data-testid` plus a stable data attribute.
- Do not use visible text, CSS classes, coordinates, screenshots, or XPath paths as primary locators.

## App Shell

| Element name | data-testid | Notes |
|---|---|---|
| App layout root | `app-layout` | App load-ready anchor. |
| Main content area | `app-main-content` | Primary scene content container. |
| Navigation panel | `nav-panel` | Left navigation container. |

## Navigation

| Element name | data-testid | Notes |
|---|---|---|
| Footer more button | `nav-footer-more-btn` | Opens the footer overflow menu. |
| Footer menu | `nav-footer-menu` | Overflow menu opened from the footer more button. |
| Footer settings item | `nav-footer-settings-item` | Opens the Settings scene from the footer menu. |
| Footer shell button | `nav-footer-shell-btn` | Opens or closes the shell scene nav. |
| Footer browser button | `nav-footer-browser-btn` | Opens browser scene or browser panel depending on active context. |

## Chat

| Element name | data-testid | Notes |
|---|---|---|
| Chat input container | `chat-input-container` | Root container for the composer. |
| Chat input editable region | `chat-input-textarea` | Rich text editable region. |
| Chat send button | `chat-input-send-btn` | Send action when input is valid. |

## Settings

| Element name | data-testid | Notes |
|---|---|---|
| Settings scene root | `settings-scene` | Root content area for the Settings scene. Includes `data-settings-tab`. |
| Settings scene content | `settings-scene-content` | Active settings tab content wrapper. |
| Settings navigation root | `settings-nav` | Left-side settings navigation. |
| Settings navigation tab | `settings-nav-tab` | Repeated item. Pair with `data-settings-tab`. |

## Notifications

| Element name | data-testid | Notes |
|---|---|---|
| Notification button | `notification-button` | Opens or toggles the notification center. |
| Notification center dialog | `notification-center` | Notification center modal root. |
| Notification center close button | `notification-center-close-btn` | Closes the notification center. |
| Notification center active section | `notification-center-active-section` | Present only when active task notifications exist. |

## Flow Chat Header

| Element name | data-testid | Notes |
|---|---|---|
| Background subagents button | `flowchat-header-background-subagents` | Opens background subagent activity state. |
| Pull requests button | `flowchat-header-pull-requests` | Opens pull request related UI. |
| Turn list | `flowchat-header-turn-list` | Turn navigation list. |
| Previous turn button | `flowchat-header-turn-prev` | Moves to previous visible turn. |
| Next turn button | `flowchat-header-turn-next` | Moves to next visible turn. |

