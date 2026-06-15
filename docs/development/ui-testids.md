[中文](ui-testids-CN.md) | **English**

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
- Prefer stable product identifiers in companion `data-*` attributes, such as `data-workspace-id`, `data-session-id`, `data-agent-id`, `data-skill-key`, or `data-settings-tab`.

## Coverage Planning

### Must Add

These areas are high-value UI automation entry points and should have stable IDs
before adding or expanding cross-platform pytest cases.

| Area | Scope | Rationale |
|---|---|---|
| App shell | App root, main content, scene viewport | App load and routing readiness anchors. |
| Navigation | Top actions, footer menu, workspace menu, workspace rows, session rows | Main path for opening settings, sessions, projects, agents, skills, and workspace-scoped actions. |
| Welcome scene | Scene root, open/new project buttons, recent workspace list | Default startup path currently lands here on OH. |
| Notifications | Notification button, center root, close button, active section | Current smoke coverage and async task visibility. |
| Settings | Scene root, nav tabs, active content | Current smoke coverage and future configuration tests. |
| Session and Flow Chat | Session scene, chat/aux panes, message list, composer | Main product workflow once session creation is stable. |
| Agents and Skills | Scene roots, zones/tabs, filters, cards, key actions | High-value navigation and marketplace/agent setup flows. |

### Optional Add

Add these when a concrete test needs them.

| Area | Scope | Rationale |
|---|---|---|
| Deep Review / BTW detail panels | Review action bars, reviewer/member details, report export actions | Valuable for deeper behavior tests, but not needed for app smoke. |
| Tool cards | Specific approve/retry/open-detail controls | Add per tool workflow instead of tagging every rendered result field. |
| File, Git, Terminal, Browser panels | Panel roots, primary toolbar actions, selected list rows | Useful once panel-specific pytest coverage exists. |
| Settings form controls | Specific model/provider fields and save/reset buttons | Add with configuration tests; avoid tagging every display-only label. |
| Mini apps | Gallery root, app cards, runner root | Add when Mini App flows enter the automation plan. |

### Not Recommended

Avoid adding IDs to these surfaces unless there is a clear automated workflow.

| Scope | Reason |
|---|---|
| Decorative icons, badges, counters, shadows, animations | Not meaningful interaction or state anchors. |
| Every text node, paragraph, and static label | Creates maintenance cost and duplicates i18n-visible content. |
| Generated markdown/code content and model output spans | Output is dynamic and should be asserted through higher-level state. |
| Coordinates, canvas pixels, screenshot-only markers, or native window controls | Cross-platform WebView automation should stay DOM and `data-testid` based. |
| Localized text copied into `data-testid` or required as the primary locator | Breaks when locales or copy change. |

## Naming

- Use area prefixes: `app-*`, `scene-*`, `nav-*`, `welcome-*`, `settings-*`, `notification-*`, `session-*`, `chat-*`, `flowchat-*`, `agents-*`, `skills-*`.
- Use action suffixes for buttons: `*-btn`, `*-toggle`, `*-open`, `*-close`, `*-submit`, `*-cancel`, `*-delete`.
- Use structure suffixes for containers: `*-scene`, `*-panel`, `*-list`, `*-grid`, `*-menu`, `*-content`, `*-zone`.
- For repeated rows/cards, reuse one `data-testid` and pair it with a stable attribute, for example:
  - `nav-workspace-item` + `data-workspace-id`
  - `nav-session-item` + `data-session-id`
  - `settings-nav-tab` + `data-settings-tab`
  - `agents-agent-card` + `data-agent-id`
  - `skills-installed-card` + `data-skill-key`
  - `skills-market-card` + `data-skill-install-id`

## App Shell

| Element name | data-testid | Notes |
|---|---|---|
| App layout root | `app-layout` | App load-ready anchor. |
| Main content area | `app-main-content` | Primary scene content container. |
| Navigation panel | `nav-panel` | Left navigation container. |
| Scene viewport root | `scene-viewport` | Scene host root. |
| Scene viewport clip | `scene-viewport-clip` | Mounted scene clip area. |
| Empty scene viewport | `scene-viewport-empty` | Rendered when no tabs are open. |
| Mounted scene wrapper | `scene-viewport-scene` | Repeated item. Pair with `data-scene-id` and `data-scene-active`. |

## Welcome

| Element name | data-testid | Notes |
|---|---|---|
| Welcome scene root | `welcome-scene` | Default startup scene anchor. |
| Open project button | `welcome-open-project-btn` | Opens the file/folder picker. |
| New project button | `welcome-new-project-btn` | Opens the new project flow. |
| Recent workspace list | `welcome-recent-workspace-list` | Present when recent workspaces exist. |
| Recent workspace row | `welcome-recent-workspace-row` | Repeated item. Pair with `data-workspace-id`. |
| Recent workspace open button | `welcome-recent-workspace-open` | Repeated item. Pair with `data-workspace-id`. |
| Recent workspace remove button | `welcome-recent-workspace-remove` | Repeated item. Pair with `data-workspace-id`. |
| Recent workspace empty state | `welcome-recent-workspace-empty` | Present when no recent workspace is available. |

## Navigation

| Element name | data-testid | Notes |
|---|---|---|
| Nav search trigger | `nav-search-trigger` | Opens navigation search. |
| New code session button | `nav-new-code-session-btn` | Creates or opens a code session for the active project workspace. |
| New cowork session button | `nav-new-cowork-session-btn` | Creates or opens a cowork session for the active project workspace. |
| Assistant button | `nav-assistant-btn` | Opens assistant/persona scene. |
| Extensions toggle | `nav-extensions-toggle` | Expands Agents/Skills entries. |
| Agents button | `nav-agents-btn` | Opens Agents scene. |
| Skills button | `nav-skills-btn` | Opens Skills scene. |
| Navigation sections | `nav-sections` | Container for workspace/session sections. |
| Navigation bottom bar | `nav-bottom-bar` | Container for Mini App/footer region. |
| Footer more button | `nav-footer-more-btn` | Opens the footer overflow menu. |
| Footer menu | `nav-footer-menu` | Overflow menu opened from the footer more button. |
| Footer settings item | `nav-footer-settings-item` | Opens the Settings scene from the footer menu. |
| Footer shell button | `nav-footer-shell-btn` | Opens or closes the shell scene nav. |
| Footer browser button | `nav-footer-browser-btn` | Opens browser scene or browser panel depending on active context. |

## Navigation Workspaces

| Element name | data-testid | Notes |
|---|---|---|
| Workspace add button | `nav-workspace-add-btn` | Opens workspace add/recent menu. |
| Workspace add menu | `nav-workspace-menu` | Portal menu opened from add button. |
| Workspace menu open project | `nav-workspace-menu-open-project` | Opens project picker. |
| Workspace menu new project | `nav-workspace-menu-new-project` | Opens new project flow. |
| Workspace menu remote SSH | `nav-workspace-menu-remote-ssh` | Opens SSH remote connect flow. |
| Workspace menu recent workspace | `nav-workspace-menu-recent-workspace` | Repeated item. Pair with `data-workspace-id`. |
| Workspace list | `nav-workspace-list` | Repeated by list type. Pair with `data-workspace-list`. |
| Workspace list empty state | `nav-workspace-list-empty` | Pair with `data-workspace-list`. |
| Workspace drop target | `nav-workspace-drop-target` | Repeated drag target. Pair with `data-workspace-id`. |
| Workspace row | `nav-workspace-item` | Repeated item. Pair with `data-workspace-id`, `data-workspace-kind`, and `data-workspace-active`. |
| Workspace card | `nav-workspace-card` | Clickable row body. Pair with `data-workspace-id`. |
| Workspace sessions toggle | `nav-workspace-sessions-toggle` | Expands/collapses session rows. Pair with `data-workspace-id`. |
| Workspace name button | `nav-workspace-name-btn` | Activates workspace or toggles sessions. Pair with `data-workspace-id`. |
| Workspace files button | `nav-workspace-files-btn` | Opens file viewer for workspace. Pair with `data-workspace-id`. |
| Workspace search index button | `nav-workspace-search-index-btn` | Opens search index status modal when present. Pair with `data-workspace-id`. |
| Workspace row menu button | `nav-workspace-menu-btn` | Opens row action menu. Pair with `data-workspace-id`. |
| Workspace row menu | `nav-workspace-item-menu` | Portal menu for one workspace. Pair with `data-workspace-id`. |
| Workspace create session | `nav-workspace-menu-create-session` | Assistant workspace session action. |
| Workspace create code session | `nav-workspace-menu-create-code-session` | Normal workspace code session action. |
| Workspace create cowork session | `nav-workspace-menu-create-cowork-session` | Normal workspace cowork session action. |
| Workspace create ACP session | `nav-workspace-menu-create-acp-session` | Repeated item. Pair with `data-acp-client-id`. |
| Workspace create init session | `nav-workspace-menu-create-init-session` | Starts AGENTS.md/init session. |
| Workspace related paths | `nav-workspace-menu-related-paths` | Opens related paths dialog. |
| Workspace new worktree | `nav-workspace-menu-new-worktree` | Opens worktree creation dialog. |
| Workspace delete worktree | `nav-workspace-menu-delete-worktree` | Deletes linked worktree workspace. |
| Workspace copy path | `nav-workspace-menu-copy-path` | Copies workspace path. |
| Workspace reveal | `nav-workspace-menu-reveal` | Reveals workspace in file explorer. |
| Workspace close | `nav-workspace-menu-close` | Closes workspace. |
| Workspace reset assistant | `nav-workspace-menu-reset-assistant` | Resets default assistant workspace. |
| Workspace delete assistant | `nav-workspace-menu-delete-assistant` | Deletes named assistant workspace. |
| Workspace session region | `nav-workspace-session-region` | Contains sessions for one workspace. Pair with `data-workspace-id`. |

## Navigation Sessions

| Element name | data-testid | Notes |
|---|---|---|
| Session list | `nav-session-list` | Workspace-scoped list. Pair with `data-workspace-id`. |
| Session row | `nav-session-item` | Repeated item. Pair with `data-session-id`, `data-session-kind`, `data-session-level`, and `data-session-active`. |
| Session menu button | `nav-session-menu-btn` | Opens row action menu. Pair with `data-session-id`. |
| Session menu | `nav-session-menu` | Portal menu for one session. Pair with `data-session-id`. |
| Session rename item | `nav-session-menu-rename` | Starts session rename. |
| Session delete item | `nav-session-menu-delete` | Deletes session. |
| Session list toggle | `nav-session-list-toggle` | Expands/collapses long session lists. |

## Session And Chat

| Element name | data-testid | Notes |
|---|---|---|
| Session scene root | `session-scene` | Session scene anchor. |
| Session chat pane | `session-chat-pane` | Left chat pane within session scene. |
| Session right pane resizer | `session-right-pane-resizer` | Splitter between chat and aux pane. |
| Session aux pane | `session-aux-pane` | Right content canvas pane. Includes `data-mode`. |
| Chat pane root | `chat-pane` | FlowChat host pane. |
| FlowChat container | `flowchat-container` | FlowChat root. Includes `data-session-id`. |
| FlowChat messages region | `flowchat-messages` | Message list/welcome panel host. |
| FlowChat message list | `flowchat-message-list` | Virtual message list root when messages exist. |
| FlowChat empty message list | `flowchat-message-list-empty` | Empty virtual list state. |
| FlowChat message item | `flowchat-message-item` | Repeated virtual item. Pair with `data-turn-id`, `data-item-type`, and `data-item-index`. |
| Chat input container | `chat-input-container` | Root container for the composer. |
| Chat input editable region | `chat-input-textarea` | Rich text editable region. |
| Chat send button | `chat-input-send-btn` | Send action when input is valid. |
| Chat cancel button | `chat-input-cancel-btn` | Cancels in-progress send/generation when present. |
| Chat input workspace strip | `chat-input-workspace-strip` | Active workspace strip above composer. |
| Chat input target switcher | `chat-input-target-switcher` | Target/mode switcher. |
| Chat input image strip | `chat-input-image-strip` | Attached image strip. |
| Chat input start BTW button | `chat-input-boost-start-btw` | Starts the BTW flow when present. |
| Chat model selector button | `chat-model-selector-btn` | Opens the session model selector. |
| Chat model selector menu | `chat-model-selector-menu` | Model selector dropdown root. |
| Chat model selector option | `chat-model-selector-option` | Repeated item. Pair with `data-model-id`, `data-model-name`, and `data-selected`. |
| Chat user message | `chat-user-message` | Repeated user message. Pair with `data-turn-id`, `data-status`, and `data-failed`. |
| Chat user message content | `chat-user-message-content` | User message text content. Pair with `data-turn-id`. |
| Chat assistant message | `chat-assistant-message` | Repeated model round container. Pair with `data-turn-id`, `data-round-id`, `data-status`, `data-model-id`, `data-model-alias`, and `data-streaming`. |
| Chat assistant message content | `chat-assistant-message-content` | Assistant text block. Pair with `data-turn-id`, `data-flow-item-id`, `data-status`, and `data-streaming`. |
| Pending queue panel | `pending-queue-panel` | Pending background task queue. |

## Settings

| Element name | data-testid | Notes |
|---|---|---|
| Settings scene root | `settings-scene` | Root content area for the Settings scene. Includes `data-settings-tab`. |
| Settings scene content | `settings-scene-content` | Active settings tab content wrapper. |
| Settings navigation root | `settings-nav` | Left-side settings navigation. |
| Settings navigation tab | `settings-nav-tab` | Repeated item. Pair with `data-settings-tab`. |

## Settings Models

| Element name | data-testid | Notes |
|---|---|---|
| Model list | `settings-model-list` | Container for configured model rows. |
| Create first model config button | `settings-model-create-first-config-btn` | Starts the first model provider setup from the empty state. |
| Custom model config button | `settings-model-custom-config-btn` | Starts custom provider configuration. Includes `data-provider-id="custom"`. |
| Model provider option | `settings-model-provider-option` | Repeated provider card. Pair with `data-provider-id`, for example `openbitfun`. |
| Model provider name input | `settings-model-provider-name-input` | Provider/config display name field, such as a mock LLM provider name. |
| Model API key input | `settings-model-api-key-input` | API key field in the model configuration form. Do not hardcode real keys in tests; load them from local config. |
| Model base URL input | `settings-model-base-url-input` | API base URL field for custom/OpenAI-compatible providers. |
| Model request format select | `settings-model-request-format-select` | Request format selector, for example OpenAI-compatible vs Anthropic. |
| Model select button | `settings-model-select-btn` | Opens the model selection dropdown. |
| Model selection menu | `settings-model-select-menu` | Model selection dropdown root. |
| Model selection option | `settings-model-option` | Repeated dropdown item. Pair with `data-model-id`, `data-model-name`, and `data-selected`. |
| Manual model name input | `settings-model-manual-name-input` | Manual/custom model name entry field. |
| Add custom model button | `settings-model-add-custom-btn` | Adds the manual model name into the selected model list. |
| Selected model list | `settings-model-selected-list` | Selected model draft list. Includes `data-selected-count`. |
| Selected model empty state | `settings-model-selected-list-empty` | Empty selected model draft state. Includes `data-selected-count="0"`. |
| Selected model row | `settings-model-selected-row` | Repeated selected model draft. Pair with `data-model-id`, `data-model-name`, `data-selected`, and `data-expanded`. |
| Selected model remove button | `settings-model-selected-remove-btn` | Removes a selected model draft. Pair with `data-model-id` and `data-model-name`. |
| Model save button | `settings-model-save-btn` | Saves the model provider/configuration form. |
| Model row | `settings-model-row` | Repeated saved model row. Pair with `data-model-id`, `data-model-name`, and `data-config-id`. |
| Model test status | `settings-model-test-status` | Repeated saved model test status. Pair with `data-model-id`, `data-model-name`, `data-config-id`, and `data-status` (`success` or `error`). |

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

## Agents

| Element name | data-testid | Notes |
|---|---|---|
| Agents scene root | `agents-scene` | Agents gallery page root. |
| Agents zones container | `agents-zones` | Container for all agent zones. |
| Core anchor button | `agents-anchor-core` | Scrolls to core agents zone. |
| Teams anchor button | `agents-anchor-teams` | Scrolls to teams zone. |
| Custom agents anchor button | `agents-anchor-custom` | Scrolls to custom agents zone. |
| Agents search button | `agents-search-btn` | Search suffix button. |
| Core agents zone | `agents-core-zone` | Core agents section. |
| Teams zone | `agents-teams-zone` | Agent teams section. |
| Custom agents zone | `agents-custom-zone` | Custom/subagent section. |
| Review team configure button | `agents-review-team-configure-btn` | Opens review team configuration. |
| Agent source filter | `agents-source-filter` | Repeated item. Pair with `data-agent-source`. |
| Agent kind filter | `agents-kind-filter` | Repeated item. Pair with `data-agent-kind`. |
| Create agent button | `agents-create-agent-btn` | Opens custom agent creation page. |
| Core agent card | `agents-core-agent-card` | Repeated item. Pair with `data-agent-id` and `data-agent-kind`. |
| Agent team card | `agents-team-card` | Repeated item. Pair with `data-team-id`. |
| Agent card | `agents-agent-card` | Repeated item. Pair with `data-agent-id`, `data-agent-kind`, and `data-subagent-source`. |
| BTW stop review button | `btw-session-panel-stop-review` | Stops review session from BTW panel. |
| BTW origin button | `btw-session-panel-origin-button` | Opens origin session from BTW panel. |

## Skills

| Element name | data-testid | Notes |
|---|---|---|
| Skills scene root | `skills-scene` | Skills scene root. |
| Skills tabs root | `skills-tabs` | Installed/discover tabs container. |
| Installed tab | `skills-tab-installed` | Includes `data-skills-tab-active`. |
| Discover tab | `skills-tab-discover` | Includes `data-skills-tab-active`. |
| Installed panel | `skills-installed-panel` | Installed skills view root. |
| Installed sidebar | `skills-installed-sidebar` | Installed category sidebar. |
| Installed category | `skills-installed-category` | Repeated item. Pair with `data-skill-category`. |
| Installed content | `skills-installed-content` | Main installed skills content. |
| Installed search | `skills-installed-search` | Installed skills search root. |
| Hide duplicates button | `skills-hide-duplicates-btn` | Includes `data-active`. |
| Add local skill button | `skills-add-local-btn` | Opens add skill form. |
| Installed loading state | `skills-installed-loading` | Loading skeleton container. |
| Installed error state | `skills-installed-error` | Error state container. |
| Installed empty state | `skills-installed-empty` | Empty state container. |
| Installed grid | `skills-installed-grid` | Installed skills card grid. |
| Installed skill card | `skills-installed-card` | Repeated item. Pair with `data-skill-key`, `data-skill-level`, and `data-skill-builtin`. |
| Installed card path button | `skills-installed-card-path` | Repeated item. Pair with `data-skill-key`. |
| Installed card delete button | `skills-installed-card-delete` | Repeated item. Pair with `data-skill-key`. |
| Installed pagination | `skills-installed-pagination` | Installed list pagination root. |
| Installed previous page | `skills-installed-page-prev` | Previous page button. |
| Installed next page | `skills-installed-page-next` | Next page button. |
| Discover panel | `skills-discover-panel` | Marketplace view root. |
| Discover search | `skills-discover-search` | Marketplace search root. |
| Discover content | `skills-discover-content` | Marketplace content area. |
| Discover loading state | `skills-discover-loading` | Initial loading skeleton container. |
| Discover page loading state | `skills-discover-page-loading` | Loading state for page changes. |
| Discover error state | `skills-discover-error` | Error state container. |
| Discover empty state | `skills-discover-empty` | Empty state container. |
| Discover grid | `skills-discover-grid` | Marketplace card grid. |
| Market skill card | `skills-market-card` | Repeated item. Pair with `data-skill-install-id` and `data-skill-installed`. |
| Skill card action | `skills-card-action` | Repeated card action. Pair with `data-skill-action`. |
| Discover pagination | `skills-discover-pagination` | Marketplace pagination root. |
| Discover previous page | `skills-discover-page-prev` | Previous page button. |
| Discover next page | `skills-discover-page-next` | Next page button. |
| Detail delete button | `skills-detail-delete-btn` | Deletes selected installed skill. |
| Detail installed button | `skills-detail-installed-btn` | Disabled installed marker for marketplace detail. |
| Detail project download button | `skills-detail-download-project-btn` | Downloads market skill to project scope. |
| Detail user download button | `skills-detail-download-user-btn` | Downloads market skill to user scope. |
| Detail path button | `skills-detail-path-btn` | Reveals installed skill path. |
| Detail external link | `skills-detail-external-link` | Opens marketplace link. |
| Add form | `skills-add-form` | Add local skill modal content. |
| Add path input | `skills-add-path-input` | Local skill path input. |
| Add browse button | `skills-add-browse-btn` | Opens path picker. |
| Add validation result | `skills-add-validation` | Includes `data-validation-valid`. |
| Add cancel button | `skills-add-cancel-btn` | Closes add form. |
| Add submit button | `skills-add-submit-btn` | Adds validated local skill. |
