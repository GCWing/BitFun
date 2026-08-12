import { buttonAppearanceDescriptor } from '@/component-library/components/Button/appearance';
import { cardAppearanceDescriptor } from '@/component-library/components/Card/appearance';
import { inputAppearanceDescriptor } from '@/component-library/components/Input/appearance';
import { modalAppearanceDescriptor } from '@/component-library/components/Modal/appearance';
import { tooltipAppearanceDescriptor } from '@/component-library/components/Tooltip/appearance';
import { iconButtonAppearanceDescriptor } from '@/component-library/components/IconButton/appearance';
import { checkboxAppearanceDescriptor } from '@/component-library/components/Checkbox/appearance';
import { switchAppearanceDescriptor } from '@/component-library/components/Switch/appearance';
import { textareaAppearanceDescriptor } from '@/component-library/components/Textarea/appearance';
import { numberInputAppearanceDescriptor } from '@/component-library/components/NumberInput/appearance';
import { searchAppearanceDescriptor } from '@/component-library/components/Search/appearance';
import { tabsAppearanceDescriptor } from '@/component-library/components/Tabs/appearance';
import { selectAppearanceDescriptor } from '@/component-library/components/Select/appearance';
import { alertAppearanceDescriptor } from '@/component-library/components/Alert/appearance';
import { badgeAppearanceDescriptor } from '@/component-library/components/Badge/appearance';
import { tagAppearanceDescriptor } from '@/component-library/components/Tag/appearance';
import { avatarAppearanceDescriptor } from '@/component-library/components/Avatar/appearance';
import { codeEditorAppearanceDescriptor } from '@/component-library/components/CodeEditor/appearance';
import { configPageAppearanceDescriptor } from '@/component-library/components/ConfigPage/appearance';
import { confirmDialogAppearanceDescriptor } from '@/component-library/components/ConfirmDialog/appearance';
import { cubeLoadingAppearanceDescriptor } from '@/component-library/components/CubeLoading/appearance';
import { cubeLogoAppearanceDescriptor } from '@/component-library/components/CubeLogo/appearance';
import { dotMatrixLoaderAppearanceDescriptor } from '@/component-library/components/DotMatrixLoader/appearance';
import { emptyAppearanceDescriptor } from '@/component-library/components/Empty/appearance';
import { filterPillAppearanceDescriptor } from '@/component-library/components/FilterPill/appearance';
import { inputDialogAppearanceDescriptor } from '@/component-library/components/InputDialog/appearance';
import { streamTextAppearanceDescriptor } from '@/component-library/components/StreamText/appearance';
import { taskRunningIndicatorAppearanceDescriptor } from '@/component-library/components/TaskRunningIndicator/appearance';
import { textStrokeEffectAppearanceDescriptor } from '@/component-library/components/TextStrokeEffect/appearance';
import { windowControlsAppearanceDescriptor } from '@/component-library/components/WindowControls/appearance';
import { markdownAppearanceDescriptor } from '@/component-library/components/Markdown/appearance';
import { flowChatCardAppearanceDescriptor } from '@/component-library/components/FlowChatCards/appearance';
import { chatInputAppearanceDescriptor } from '@/flow_chat/components/ChatInput.appearance';
import { composerVoiceInputAppearanceDescriptor } from '@/flow_chat/components/voice/ComposerVoiceInputButton.appearance';
import { sessionUsagePanelAppearanceDescriptor } from '@/flow_chat/components/usage/appearance';
import { fileOperationToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/FileOperationToolCard.appearance';
import { richTextInputAppearanceDescriptor } from '@/flow_chat/components/RichTextInput.appearance';
import { modelRoundItemAppearanceDescriptor } from '@/flow_chat/components/modern/ModelRoundItem.appearance';
import { deepReviewActionBarAppearanceDescriptor } from '@/flow_chat/deep-review/action-bar/appearance';
import { modelSelectorAppearanceDescriptor } from '@/flow_chat/components/ModelSelector.appearance';
import { reasoningPresetSelectorAppearanceDescriptor } from '@/flow_chat/components/ReasoningPresetSelector.appearance';
import { flowChatHeaderAppearanceDescriptor } from '@/flow_chat/components/modern/FlowChatHeader.appearance';
import { flowChatTurnRailAppearanceDescriptor } from '@/flow_chat/components/modern/FlowChatTurnRail.appearance';
import { sessionFilesBadgeAppearanceDescriptor } from '@/flow_chat/components/modern/SessionFilesBadge.appearance';
import { codeReviewToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/CodeReviewToolCard.appearance';
import { createAgentPageAppearanceDescriptor } from '@/app/scenes/agents/components/CreateAgentPage.appearance';
import { keyboardShortcutsAppearanceDescriptor } from '@/app/scenes/settings/components/KeyboardShortcutsTab.appearance';
import { taskToolDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/TaskToolDisplay.appearance';
import { basicsConfigAppearanceDescriptor } from '@/infrastructure/config/components/BasicsConfig.appearance';
import { markdownEditorAppearanceDescriptor } from '@/tools/editor/components/MarkdownEditor.appearance';
import { planViewerAppearanceDescriptor } from '@/tools/editor/components/PlanViewer.appearance';
import { appLayoutAppearanceDescriptor } from '@/app/layout/AppLayout.appearance';
import { skillGroupPickerAppearanceDescriptor } from '@/app/scenes/agents/components/SkillGroupPicker.appearance';
import { workingCopyViewAppearanceDescriptor } from '@/app/scenes/git/views/WorkingCopyView.appearance';
import { assistantConfigPageAppearanceDescriptor } from '@/app/scenes/profile/views/AssistantConfigPage.appearance';
import { assistantDefaultsPageAppearanceDescriptor } from '@/app/scenes/profile/views/AssistantDefaultsPage.appearance';
import { taskDetailPanelAppearanceDescriptor } from '@/flow_chat/components/TaskDetailPanel/TaskDetailPanel.appearance';
import { toolbarModeAppearanceDescriptor } from '@/flow_chat/components/toolbar-mode/ToolbarMode.appearance';
import { mcpToolDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/MCPToolDisplay.appearance';
import { terminalToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/TerminalToolCard.appearance';
import { skillsConfigAppearanceDescriptor } from '@/infrastructure/config/components/SkillsConfig.appearance';
import { diffEditorAppearanceDescriptor } from '@/tools/editor/components/DiffEditor.appearance';
import { agentCompanionDesktopPetAppearanceDescriptor } from '@/app/components/AgentCompanionDesktopPet/AgentCompanionDesktopPet.appearance';
import { toolGroupPickerAppearanceDescriptor } from '@/app/scenes/agents/components/ToolGroupPicker.appearance';
import { inlineDiffPreviewAppearanceDescriptor } from '@/flow_chat/components/InlineDiffPreview.appearance';
import { exportImageAppearanceDescriptor } from '@/flow_chat/components/modern/ExportImageButton.appearance';
import { userMessageItemAppearanceDescriptor } from '@/flow_chat/components/modern/UserMessageItem.appearance';
import { sessionUsageReportCardAppearanceDescriptor } from '@/flow_chat/components/usage/SessionUsageReportCard.appearance';
import { askUserQuestionCardAppearanceDescriptor } from '@/flow_chat/tool-cards/AskUserQuestionCard.appearance';
import { createPlanDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/CreatePlanDisplay.appearance';
import { execProcessToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/ExecProcessToolCardView.appearance';
import { editorConfigAppearanceDescriptor } from '@/infrastructure/config/components/EditorConfig.appearance';
import { appearanceConfigAppearanceDescriptor } from '@/infrastructure/config/components/AppearanceConfig.appearance';
import { lspConfigAppearanceDescriptor } from '@/infrastructure/config/components/LspConfig.appearance';
import { configAppearanceDescriptor } from '@/infrastructure/config/appearance';
import { fontPreferenceAppearanceDescriptor } from '@/infrastructure/font-preference/appearance';
import { languageSelectorAppearanceDescriptor } from '@/infrastructure/i18n/appearance';
import { peerDeviceAppearanceDescriptor } from '@/infrastructure/peer-device/appearance';
import { updateAppearanceDescriptor } from '@/infrastructure/update/appearance';
import { componentPreviewAppearanceDescriptor } from '@/component-library/preview/appearance';
import { announcementAppearanceDescriptor } from '@/shared/announcement-system/appearance';
import { contextMenuAppearanceDescriptor } from '@/shared/context-menu-system/appearance';
import { contextListAppearanceDescriptor } from '@/shared/context-system/appearance';
import { notificationAppearanceDescriptor } from '@/shared/notification-system/appearance';
import { canvasToolAppearanceDescriptor } from '@/tools/bitfun-canvas/appearance';
import { generativeWidgetAppearanceDescriptor } from '@/tools/generative-widget/appearance';
import { editorToolAppearanceDescriptor } from '@/tools/editor/appearance';
import { fileSystemAppearanceDescriptor } from '@/tools/file-system/appearance';
import { gitToolAppearanceDescriptor } from '@/tools/git/appearance';
import { lspAppearanceDescriptor } from '@/tools/lsp/appearance';
import { terminalToolAppearanceDescriptor } from '@/tools/terminal/appearance';
import { workspaceToolAppearanceDescriptor } from '@/tools/workspace/appearance';
import { relayDeployAppearanceDescriptor } from '@/features/relay-deploy/appearance';
import { marketAccountControlsAppearanceDescriptor } from '@/features/market-account/appearance';
import { sshRemoteAppearanceDescriptor } from '@/features/ssh-remote/appearance';
import { workbenchAppearanceDescriptor } from '@/app/appearance';
import { welcomeAppearanceDescriptor } from '@/app/scenes/welcome/appearance';
import { shellAppearanceDescriptor } from '@/app/scenes/shell/appearance';
import { agentsAppearanceDescriptor } from '@/app/scenes/agents/appearance';
import { assistantAppearanceDescriptor } from '@/app/scenes/assistant/appearance';
import { browserAppearanceDescriptor } from '@/app/scenes/browser/appearance';
import { fileViewerAppearanceDescriptor } from '@/app/scenes/file-viewer/appearance';
import { gitAppearanceDescriptor } from '@/app/scenes/git/appearance';
import { miniAppAppearanceDescriptor, miniAppGalleryAppearanceDescriptor } from '@/app/scenes/miniapps/appearance';
import { insightsAppearanceDescriptor } from '@/app/scenes/my-agent/appearance';
import { pagesAppearanceDescriptor } from '@/app/scenes/pages/appearance';
import { panelViewAppearanceDescriptor } from '@/app/scenes/panel-view/appearance';
import { profileAppearanceDescriptor } from '@/app/scenes/profile/appearance';
import { sessionAppearanceDescriptor } from '@/app/scenes/session/appearance';
import { settingsAppearanceDescriptor } from '@/app/scenes/settings/appearance';
import { skillsAppearanceDescriptor } from '@/app/scenes/skills/appearance';
import { terminalAppearanceDescriptor } from '@/app/scenes/terminal/appearance';
import { aboutDialogAppearanceDescriptor } from '@/app/components/AboutDialog/appearance';
import { navPanelAppearanceDescriptor } from '@/app/components/NavPanel/appearance';
import { sessionsSectionAppearanceDescriptor } from '@/app/components/NavPanel/sections/sessions/appearance';
import { contentCanvasAppearanceDescriptor } from '@/app/components/panels/content-canvas/appearance';
import { filesPanelAppearanceDescriptor } from '@/app/components/panels/FilesPanel.appearance';
import { reviewPlatformAppearanceDescriptor } from '@/app/components/panels/review-platform/appearance';
import { remoteAccountPanelAppearanceDescriptor, remoteConnectDialogAppearanceDescriptor } from '@/app/components/RemoteConnectDialog/appearance';
import { scheduledJobsViewAppearanceDescriptor } from '@/app/components/scheduled-jobs/appearance';
import { todosSceneAppearanceDescriptor } from '@/app/scenes/todos/appearance';
import { flexiblePanelAppearanceDescriptor } from '@/app/components/panels/base/FlexiblePanel.appearance';
import { btwSessionPanelAppearanceDescriptor } from '@/flow_chat/components/btw/BtwSessionPanel.appearance';
import { modernFlowChatAppearanceDescriptor, virtualMessageListAppearanceDescriptor } from '@/flow_chat/components/modern/appearance';
import { aiModelConfigAppearanceDescriptor } from '@/infrastructure/config/components/AIModelConfig.appearance';
import { reasoningConfigPanelAppearanceDescriptor } from '@/infrastructure/config/components/ReasoningConfigPanel.appearance';
import { reasoningPresetEditorAppearanceDescriptor } from '@/infrastructure/config/components/ReasoningPresetEditor.appearance';
import { externalSourcesConfigAppearanceDescriptor } from '@/infrastructure/config/components/ExternalSourcesConfig.appearance';
import { acpAgentsConfigAppearanceDescriptor } from '@/infrastructure/config/components/AcpAgentsConfig.appearance';
import { sessionConfigAppearanceDescriptor } from '@/infrastructure/config/components/SessionConfig.appearance';
import { sessionTitleConfigAppearanceDescriptor } from '@/infrastructure/config/components/SessionTitleConfig.appearance';
import { mcpToolsConfigAppearanceDescriptor } from '@/infrastructure/config/components/McpToolsConfig.appearance';
import { externalMcpOverviewAppearanceDescriptor } from '@/infrastructure/config/components/ExternalMcpOverview.appearance';
import { voiceInputDiagnosticsAppearanceDescriptor } from '@/infrastructure/config/components/VoiceInputDiagnostics.appearance';
import { assistantCardAppearanceDescriptor } from '@/app/scenes/profile/views/AssistantCard.appearance';
import { miniAppCustomizePanelAppearanceDescriptor } from '@/app/scenes/miniapps/customization/MiniAppCustomizePanel.appearance';
import { userMessageEditComposerAppearanceDescriptor } from '@/flow_chat/components/modern/UserMessageEditComposer.appearance';
import { monacoAppearanceAdapter } from '../adapters/MonacoAppearanceAdapter';
import { xtermAppearanceAdapter } from '../adapters/XtermAppearanceAdapter';
import { mermaidAppearanceAdapter } from '../adapters/MermaidAppearanceAdapter';
import { widgetAppearanceAdapter } from '../adapters/WidgetAppearanceAdapter';
import { canvasAppearanceAdapter } from '../adapters/CanvasAppearanceAdapter';
import { cssTokenAppearanceAdapter } from '../adapters/CssTokenAppearanceAdapter';
import { tiptapEditorAppearanceDescriptor } from '@/tools/editor/meditor/components/TiptapEditor.appearance';
import { workspaceProjectPermissionsDialogAppearanceDescriptor } from '@/app/components/NavPanel/sections/workspaces/WorkspaceProjectPermissionsDialog.appearance';
import { workspaceSessionBatchModalAppearanceDescriptor } from '@/app/components/NavPanel/sections/workspaces/WorkspaceSessionBatchModal.appearance';
import { archivedSessionsConfigAppearanceDescriptor } from '@/app/scenes/settings/components/ArchivedSessionsConfig.appearance';
import { settingsNavAppearanceDescriptor } from '@/app/scenes/settings/SettingsNav.appearance';
import { backgroundCommandOutputPanelAppearanceDescriptor } from '@/flow_chat/components/background-command/BackgroundCommandOutputPanel.appearance';
import { chatInputPixelPetAppearanceDescriptor } from '@/flow_chat/components/ChatInputPixelPet.appearance';
import { fileMentionPickerAppearanceDescriptor } from '@/flow_chat/components/FileMentionPicker.appearance';
import { sessionFileModificationsBarAppearanceDescriptor } from '@/flow_chat/components/modern/SessionFileModificationsBar.appearance';
import { editorBreadcrumbAppearanceDescriptor } from '@/tools/editor/components/EditorBreadcrumb.appearance';
import { gitBranchHistoryAppearanceDescriptor } from '@/tools/git/components/GitBranchHistoryView/GitBranchHistoryView.appearance';
import { gitDiffViewAppearanceDescriptor } from '@/tools/git/components/GitDiffView/GitDiffView.appearance';
import { gitSettingsViewAppearanceDescriptor } from '@/tools/git/components/GitSettingsView/GitSettingsView.appearance';
import { quickActionsConfigAppearanceDescriptor } from '@/infrastructure/config/components/QuickActionsConfig.appearance';
import { statusBarPopoversAppearanceDescriptor } from '@/tools/editor/components/StatusBarPopovers/StatusBarPopovers.appearance';
import { mEditorAppearanceDescriptor } from '@/tools/editor/meditor/components/MEditor.appearance';
import { referencesPanelAppearanceDescriptor } from '@/tools/lsp/components/ReferencesPanel/ReferencesPanel.appearance';
import { navSearchDialogAppearanceDescriptor } from '@/app/components/NavPanel/NavSearchDialog.appearance';
import { workspaceRelatedPathsDialogAppearanceDescriptor } from '@/app/components/NavPanel/sections/workspaces/WorkspaceRelatedPathsDialog.appearance';
import { branchSelectModalAppearanceDescriptor } from '@/app/components/panels/BranchSelectModal.appearance';
import { floatingMiniChatAppearanceDescriptor } from '@/app/layout/FloatingMiniChat.appearance';
import { miniAppBubbleWelcomeAppearanceDescriptor } from '@/app/layout/MiniAppBubbleWelcome.appearance';
import { branchesViewAppearanceDescriptor } from '@/app/scenes/git/views/BranchesView.appearance';
import { miniAppGalleryViewAppearanceDescriptor } from '@/app/scenes/miniapps/views/MiniAppGalleryView.appearance';
import { miniAppMarketViewAppearanceDescriptor } from '@/app/scenes/miniapps/views/MiniAppMarketView.appearance';
import { miniAppSubmissionsViewAppearanceDescriptor } from '@/app/scenes/miniapps/views/MiniAppSubmissionsView.appearance';
import { shellNavAppearanceDescriptor } from '@/app/scenes/shell/ShellNav.appearance';
import { missionControlAppearanceDescriptor } from '@/app/components/panels/content-canvas/mission-control/MissionControl.appearance';
import { canvasTabAppearanceDescriptor } from '@/app/components/panels/content-canvas/tab-bar/Tab.appearance';
import { canvasTabBarAppearanceDescriptor } from '@/app/components/panels/content-canvas/tab-bar/TabBar.appearance';
import { chatInputWorkspaceStripAppearanceDescriptor } from '@/flow_chat/components/ChatInputWorkspaceStrip.appearance';
import { codePreviewAppearanceDescriptor } from '@/flow_chat/components/CodePreview.appearance';
import { exploreGroupAppearanceDescriptor } from '@/flow_chat/components/modern/ExploreGroupRenderer.appearance';
import { pendingQueuePanelAppearanceDescriptor } from '@/flow_chat/components/PendingQueuePanel.appearance';
import { subagentProjectionAppearanceDescriptor } from '@/flow_chat/components/subagent/SubagentProjectionView.appearance';
import { threadGoalDialogsAppearanceDescriptor } from '@/flow_chat/components/thread-goal/ThreadGoalDialogs.appearance';
import { userSteeringBubbleAppearanceDescriptor } from '@/flow_chat/components/UserSteeringBubble.appearance';
import { welcomePanelAppearanceDescriptor } from '@/flow_chat/components/WelcomePanel.appearance';
import { generativeWidgetToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/GenerativeWidgetToolCard.appearance';
import { gitToolDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/GitToolDisplay.appearance';
import { snapshotFullscreenDiffViewerAppearanceDescriptor } from '@/flow_chat/tool-cards/SnapshotFullscreenDiffViewer.appearance';
import { mermaidBlockAppearanceDescriptor } from '@/component-library/components/Markdown/MermaidBlock.appearance';
import { defaultModelConfigAppearanceDescriptor } from '@/infrastructure/config/components/DefaultModelConfig.appearance';
import { globalPermissionRulesDialogAppearanceDescriptor } from '@/infrastructure/config/components/GlobalPermissionRulesDialog.appearance';
import { mcpResourceBrowserAppearanceDescriptor } from '@/infrastructure/config/components/MCPResourceBrowser.appearance';
import { editorStatusBarAppearanceDescriptor } from '@/tools/editor/components/EditorStatusBar.appearance';
import { imageViewerAppearanceDescriptor } from '@/tools/editor/components/ImageViewer.appearance';
import { branchQuickSwitchAppearanceDescriptor } from '@/app/components/NavPanel/components/BranchQuickSwitch.appearance';
import { workspaceListSectionAppearanceDescriptor } from '@/app/components/NavPanel/sections/workspaces/WorkspaceListSection.appearance';
import { workspaceItemAppearanceDescriptor } from '@/app/components/NavPanel/sections/workspaces/WorkspaceItem.appearance';
import { newProjectDialogAppearanceDescriptor } from '@/app/components/NewProjectDialog/NewProjectDialog.appearance';
import { canvasEditorAreaAppearanceDescriptor } from '@/app/components/panels/content-canvas/editor-area/EditorArea.appearance';
import { canvasEditorGroupAppearanceDescriptor } from '@/app/components/panels/content-canvas/editor-area/EditorGroup.appearance';
import { canvasThumbnailAppearanceDescriptor } from '@/app/components/panels/content-canvas/mission-control/ThumbnailCard.appearance';
import { canvasTabOverflowAppearanceDescriptor } from '@/app/components/panels/content-canvas/tab-bar/TabOverflowMenu.appearance';
import { browserPanelAppearanceDescriptor } from '@/app/scenes/browser/BrowserPanel.appearance';
import { nurseryGalleryAppearanceDescriptor } from '@/app/scenes/profile/views/NurseryGallery.appearance';
import { gitGraphViewAppearanceDescriptor } from '@/app/scenes/git/views/GraphView.appearance';
import { navBarAppearanceDescriptor } from '@/app/components/NavBar/NavBar.appearance';
import { splashScreenAppearanceDescriptor } from '@/app/components/SplashScreen/SplashScreen.appearance';
import { chatPaneAppearanceDescriptor } from '@/app/scenes/session/ChatPane.appearance';
import { auxPaneAppearanceDescriptor } from '@/app/scenes/session/AuxPane.appearance';
import { bottomTerminalPaneAppearanceDescriptor } from '@/app/scenes/session/BottomTerminalPane.appearance';
import { scheduledJobsModalAppearanceDescriptor } from '@/app/components/scheduled-jobs/ScheduledJobsModal.appearance';
import { sceneBarAppearanceDescriptor } from '@/app/components/SceneBar/SceneBar.appearance';
import { panelHeaderAppearanceDescriptor } from '@/app/components/panels/base/PanelHeader.appearance';
import { terminalEditModalAppearanceDescriptor } from '@/app/components/panels/TerminalEditModal.appearance';
import { galleryLayoutAppearanceDescriptor } from '@/app/components/GalleryLayout/GalleryLayout.appearance';
import { skillCardAppearanceDescriptor } from '@/app/scenes/skills/components/SkillCard.appearance';
import { miniAppCardAppearanceDescriptor } from '@/app/scenes/miniapps/components/MiniAppCard.appearance';
import { agentCardAppearanceDescriptor } from '@/app/scenes/agents/components/AgentCard.appearance';
import { coreAgentCardAppearanceDescriptor } from '@/app/scenes/agents/components/CoreAgentCard.appearance';
import { agentCapabilityTooltipAppearanceDescriptor } from '@/app/scenes/agents/components/AgentCapabilityTooltip.appearance';
import { gitNavAppearanceDescriptor } from '@/app/scenes/git/GitNav.appearance';
import { fileViewerNavAppearanceDescriptor } from '@/app/scenes/file-viewer/FileViewerNav.appearance';
import { assistantQuickInputAppearanceDescriptor } from '@/app/scenes/profile/views/AssistantQuickInput.appearance';
import { galleryDetailModalAppearanceDescriptor } from '@/app/components/GalleryLayout/GalleryDetailModal.appearance';
import { mcpInteractionDialogAppearanceDescriptor } from '@/app/components/MCPInteractionDialog/MCPInteractionDialog.appearance';
import { remoteConnectDisclaimerAppearanceDescriptor } from '@/app/components/RemoteConnectDialog/RemoteConnectDisclaimer.appearance';
import { diffFullscreenViewerAppearanceDescriptor } from '@/app/components/panels/DiffFullscreenViewer.appearance';
import { notificationButtonAppearanceDescriptor } from '@/app/components/TitleBar/NotificationButton.appearance';
import { contextCompressionCardAppearanceDescriptor } from '@/component-library/components/FlowChatCards/ContextCompressionCard/appearance';
import { reviewSessionSummaryCardAppearanceDescriptor } from '@/flow_chat/tool-cards/ReviewSessionSummaryCard.appearance';
import { deepReviewConsentDialogAppearanceDescriptor } from '@/flow_chat/components/DeepReviewConsentDialog.appearance';
import { flowTextBlockAppearanceDescriptor } from '@/flow_chat/components/FlowTextBlock.appearance';
import { permissionRequestPanelAppearanceDescriptor } from '@/flow_chat/components/modern/PermissionRequestPanel.appearance';
import { userMessageAppearanceDescriptor } from '@/flow_chat/components/UserMessage.appearance';
import { toolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/BaseToolCard.appearance';
import { canvasToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/CanvasToolCard.appearance';
import { computerUseToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/ComputerUseToolCard.appearance';
import { defaultToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/DefaultToolCard.appearance';
import { getFileDiffDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/GetFileDiffDisplay.appearance';
import { miniAppToolDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/MiniAppToolDisplay.appearance';
import { modelThinkingDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/ModelThinkingDisplay.appearance';
import { todoWriteDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/TodoWriteDisplay.appearance';
import { toolTimeoutIndicatorAppearanceDescriptor } from '@/flow_chat/tool-cards/ToolTimeoutIndicator.appearance';
import { webFetchCardAppearanceDescriptor } from '@/flow_chat/tool-cards/WebFetchCard.appearance';
import { compactToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/CompactToolCard.appearance';
import { toolCardStatusSlotAppearanceDescriptor } from '@/flow_chat/tool-cards/ToolCardStatusSlot.appearance';
import { toolCardHeaderActionsAppearanceDescriptor } from '@/flow_chat/tool-cards/ToolCardHeaderActions.appearance';
import { acpPermissionActionsAppearanceDescriptor } from '@/flow_chat/tool-cards/AcpPermissionActions.appearance';
import { contextCompressionDisplayAppearanceDescriptor } from '@/flow_chat/tool-cards/ContextCompressionDisplay.appearance';
import { getToolSpecCardAppearanceDescriptor } from '@/flow_chat/tool-cards/GetToolSpecCard.appearance';
import { viewImageToolCardAppearanceDescriptor } from '@/flow_chat/tool-cards/ViewImageToolCard.appearance';
import { acpPlanPanelAppearanceDescriptor } from '@/flow_chat/components/AcpPlanPanel.appearance';
import { backgroundCommandInputDialogAppearanceDescriptor } from '@/flow_chat/components/background-command/BackgroundCommandInputDialog.appearance';
import { chatEmptyStateAppearanceDescriptor } from '@/flow_chat/components/ChatEmptyState.appearance';
import { copyOutputButtonAppearanceDescriptor } from '@/flow_chat/components/CopyOutputButton.appearance';
import { copyableTextPreviewAppearanceDescriptor } from '@/flow_chat/components/CopyableTextPreview.appearance';
import { currentSessionTitleAppearanceDescriptor } from '@/flow_chat/components/CurrentSessionTitle.appearance';
import { coworkExampleCardsAppearanceDescriptor } from '@/flow_chat/components/CoworkExampleCards.appearance';
import { imageAnalysisCardAppearanceDescriptor } from '@/flow_chat/components/ImageAnalysisCard.appearance';
import { scrollToBottomButtonAppearanceDescriptor } from '@/flow_chat/components/ScrollToBottomButton.appearance';
import { scrollToLatestBarAppearanceDescriptor } from '@/flow_chat/components/ScrollToLatestBar.appearance';
import { scrollToTurnHeaderButtonAppearanceDescriptor } from '@/flow_chat/components/ScrollToTurnHeaderButton.appearance';
import { tokenUsageIndicatorAppearanceDescriptor } from '@/flow_chat/components/TokenUsageIndicator.appearance';
import { toolApprovalBarAppearanceDescriptor } from '@/flow_chat/components/ToolApprovalBar.appearance';
import { snapshotRollbackButtonAppearanceDescriptor } from '@/flow_chat/components/SnapshotRollbackButton.appearance';
import { smartRecommendationsAppearanceDescriptor } from '@/flow_chat/components/smart-recommendations/SmartRecommendations.appearance';
import { stickyTaskIndicatorAppearanceDescriptor } from '@/flow_chat/components/StickyTaskIndicator.appearance';
import { turnHistoryPanelAppearanceDescriptor } from '@/flow_chat/components/TurnHistoryPanel.appearance';
import { turnRollbackButtonAppearanceDescriptor } from '@/flow_chat/components/TurnRollbackButton.appearance';
import { sessionRuntimeStatusEntryAppearanceDescriptor } from '@/flow_chat/components/usage/SessionRuntimeStatusEntry.appearance';
import { runtimeStatusSlotAppearanceDescriptor } from '@/flow_chat/components/modern/RuntimeStatusSlot.appearance';
import { sessionMenuAppearanceDescriptor } from '@/flow_chat/components/session-menu/SessionMenu.appearance';
import {
  dispatchInstallDialogAppearanceDescriptor,
  dispatchResultDialogAppearanceDescriptor,
  dispatchTargetPickerAppearanceDescriptor,
} from '@/features/dispatch/appearance';
import { voiceInputConfigAppearanceDescriptor } from '@/infrastructure/config/components/VoiceInputConfig.appearance';
import { worktreesConfigAppearanceDescriptor } from '@/infrastructure/config/components/WorktreesConfig.appearance';
import { turnCompletionNoticeAppearanceDescriptor } from '@/flow_chat/components/modern/TurnCompletionNoticeItem.appearance';
import { turnFailureNoticeAppearanceDescriptor } from '@/flow_chat/components/modern/TurnFailureNoticeItem.appearance';
import { virtualItemAppearanceDescriptor } from '@/flow_chat/components/modern/VirtualItemRenderer.appearance';
import { AppearanceRegistry } from './AppearanceRegistry';

export function createDefaultAppearanceRegistry(): AppearanceRegistry {
  return new AppearanceRegistry()
    .registerComponent(buttonAppearanceDescriptor)
    .registerComponent(cardAppearanceDescriptor)
    .registerComponent(inputAppearanceDescriptor)
    .registerComponent(modalAppearanceDescriptor)
    .registerComponent(tooltipAppearanceDescriptor)
    .registerComponent(iconButtonAppearanceDescriptor)
    .registerComponent(checkboxAppearanceDescriptor)
    .registerComponent(switchAppearanceDescriptor)
    .registerComponent(textareaAppearanceDescriptor)
    .registerComponent(numberInputAppearanceDescriptor)
    .registerComponent(searchAppearanceDescriptor)
    .registerComponent(tabsAppearanceDescriptor)
    .registerComponent(selectAppearanceDescriptor)
    .registerComponent(alertAppearanceDescriptor)
    .registerComponent(badgeAppearanceDescriptor)
    .registerComponent(tagAppearanceDescriptor)
    .registerComponent(avatarAppearanceDescriptor)
    .registerComponent(codeEditorAppearanceDescriptor)
    .registerComponent(configPageAppearanceDescriptor)
    .registerComponent(confirmDialogAppearanceDescriptor)
    .registerComponent(cubeLoadingAppearanceDescriptor)
    .registerComponent(cubeLogoAppearanceDescriptor)
    .registerComponent(dotMatrixLoaderAppearanceDescriptor)
    .registerComponent(emptyAppearanceDescriptor)
    .registerComponent(filterPillAppearanceDescriptor)
    .registerComponent(inputDialogAppearanceDescriptor)
    .registerComponent(streamTextAppearanceDescriptor)
    .registerComponent(taskRunningIndicatorAppearanceDescriptor)
    .registerComponent(textStrokeEffectAppearanceDescriptor)
    .registerComponent(windowControlsAppearanceDescriptor)
    .registerComponent(markdownAppearanceDescriptor)
    .registerComponent(flowChatCardAppearanceDescriptor)
    .registerComponent(chatInputAppearanceDescriptor)
    .registerComponent(composerVoiceInputAppearanceDescriptor)
    .registerComponent(sessionUsagePanelAppearanceDescriptor)
    .registerComponent(fileOperationToolCardAppearanceDescriptor)
    .registerComponent(richTextInputAppearanceDescriptor)
    .registerComponent(modelRoundItemAppearanceDescriptor)
    .registerComponent(deepReviewActionBarAppearanceDescriptor)
    .registerComponent(modelSelectorAppearanceDescriptor)
    .registerComponent(reasoningPresetSelectorAppearanceDescriptor)
    .registerComponent(flowChatHeaderAppearanceDescriptor)
    .registerComponent(flowChatTurnRailAppearanceDescriptor)
    .registerComponent(sessionFilesBadgeAppearanceDescriptor)
    .registerComponent(codeReviewToolCardAppearanceDescriptor)
    .registerComponent(createAgentPageAppearanceDescriptor)
    .registerComponent(keyboardShortcutsAppearanceDescriptor)
    .registerComponent(taskToolDisplayAppearanceDescriptor)
    .registerComponent(basicsConfigAppearanceDescriptor)
    .registerComponent(markdownEditorAppearanceDescriptor)
    .registerComponent(planViewerAppearanceDescriptor)
    .registerComponent(appLayoutAppearanceDescriptor)
    .registerComponent(skillGroupPickerAppearanceDescriptor)
    .registerComponent(workingCopyViewAppearanceDescriptor)
    .registerComponent(assistantConfigPageAppearanceDescriptor)
    .registerComponent(assistantDefaultsPageAppearanceDescriptor)
    .registerComponent(taskDetailPanelAppearanceDescriptor)
    .registerComponent(toolbarModeAppearanceDescriptor)
    .registerComponent(mcpToolDisplayAppearanceDescriptor)
    .registerComponent(terminalToolCardAppearanceDescriptor)
    .registerComponent(skillsConfigAppearanceDescriptor)
    .registerComponent(diffEditorAppearanceDescriptor)
    .registerComponent(agentCompanionDesktopPetAppearanceDescriptor)
    .registerComponent(toolGroupPickerAppearanceDescriptor)
    .registerComponent(inlineDiffPreviewAppearanceDescriptor)
    .registerComponent(exportImageAppearanceDescriptor)
    .registerComponent(userMessageItemAppearanceDescriptor)
    .registerComponent(sessionUsageReportCardAppearanceDescriptor)
    .registerComponent(askUserQuestionCardAppearanceDescriptor)
    .registerComponent(createPlanDisplayAppearanceDescriptor)
    .registerComponent(execProcessToolCardAppearanceDescriptor)
    .registerComponent(editorConfigAppearanceDescriptor)
    .registerComponent(appearanceConfigAppearanceDescriptor)
    .registerComponent(lspConfigAppearanceDescriptor)
    .registerComponent(configAppearanceDescriptor)
    .registerComponent(fontPreferenceAppearanceDescriptor)
    .registerComponent(languageSelectorAppearanceDescriptor)
    .registerComponent(peerDeviceAppearanceDescriptor)
    .registerComponent(updateAppearanceDescriptor)
    .registerComponent(componentPreviewAppearanceDescriptor)
    .registerComponent(announcementAppearanceDescriptor)
    .registerComponent(contextMenuAppearanceDescriptor)
    .registerComponent(contextListAppearanceDescriptor)
    .registerComponent(notificationAppearanceDescriptor)
    .registerComponent(canvasToolAppearanceDescriptor)
    .registerComponent(generativeWidgetAppearanceDescriptor)
    .registerComponent(editorToolAppearanceDescriptor)
    .registerComponent(fileSystemAppearanceDescriptor)
    .registerComponent(gitToolAppearanceDescriptor)
    .registerComponent(lspAppearanceDescriptor)
    .registerComponent(terminalToolAppearanceDescriptor)
    .registerComponent(workspaceToolAppearanceDescriptor)
    .registerComponent(gitGraphViewAppearanceDescriptor)
    .registerComponent(navBarAppearanceDescriptor)
    .registerComponent(splashScreenAppearanceDescriptor)
    .registerComponent(chatPaneAppearanceDescriptor)
    .registerComponent(auxPaneAppearanceDescriptor)
    .registerComponent(bottomTerminalPaneAppearanceDescriptor)
    .registerComponent(scheduledJobsModalAppearanceDescriptor)
    .registerComponent(sceneBarAppearanceDescriptor)
    .registerComponent(panelHeaderAppearanceDescriptor)
    .registerComponent(terminalEditModalAppearanceDescriptor)
    .registerComponent(galleryLayoutAppearanceDescriptor)
    .registerComponent(skillCardAppearanceDescriptor)
    .registerComponent(miniAppCardAppearanceDescriptor)
    .registerComponent(agentCardAppearanceDescriptor)
    .registerComponent(coreAgentCardAppearanceDescriptor)
    .registerComponent(agentCapabilityTooltipAppearanceDescriptor)
    .registerComponent(gitNavAppearanceDescriptor)
    .registerComponent(fileViewerNavAppearanceDescriptor)
    .registerComponent(assistantQuickInputAppearanceDescriptor)
    .registerComponent(galleryDetailModalAppearanceDescriptor)
    .registerComponent(mcpInteractionDialogAppearanceDescriptor)
    .registerComponent(remoteConnectDisclaimerAppearanceDescriptor)
    .registerComponent(diffFullscreenViewerAppearanceDescriptor)
    .registerComponent(notificationButtonAppearanceDescriptor)
    .registerComponent(contextCompressionCardAppearanceDescriptor)
    .registerComponent(reviewSessionSummaryCardAppearanceDescriptor)
    .registerComponent(relayDeployAppearanceDescriptor)
    .registerComponent(marketAccountControlsAppearanceDescriptor)
    .registerComponent(sshRemoteAppearanceDescriptor)
    .registerComponent(aboutDialogAppearanceDescriptor)
    .registerComponent(navPanelAppearanceDescriptor)
    .registerComponent(sessionsSectionAppearanceDescriptor)
    .registerComponent(contentCanvasAppearanceDescriptor)
    .registerComponent(filesPanelAppearanceDescriptor)
    .registerComponent(reviewPlatformAppearanceDescriptor)
    .registerComponent(remoteConnectDialogAppearanceDescriptor)
    .registerComponent(remoteAccountPanelAppearanceDescriptor)
    .registerComponent(scheduledJobsViewAppearanceDescriptor)
    .registerComponent(flexiblePanelAppearanceDescriptor)
    .registerComponent(btwSessionPanelAppearanceDescriptor)
    .registerComponent(modernFlowChatAppearanceDescriptor)
    .registerComponent(virtualMessageListAppearanceDescriptor)
    .registerComponent(aiModelConfigAppearanceDescriptor)
    .registerComponent(reasoningConfigPanelAppearanceDescriptor)
    .registerComponent(reasoningPresetEditorAppearanceDescriptor)
    .registerComponent(externalSourcesConfigAppearanceDescriptor)
    .registerComponent(acpAgentsConfigAppearanceDescriptor)
    .registerComponent(sessionConfigAppearanceDescriptor)
    .registerComponent(sessionTitleConfigAppearanceDescriptor)
    .registerComponent(mcpToolsConfigAppearanceDescriptor)
    .registerComponent(externalMcpOverviewAppearanceDescriptor)
    .registerComponent(voiceInputDiagnosticsAppearanceDescriptor)
    .registerComponent(assistantCardAppearanceDescriptor)
    .registerComponent(miniAppCustomizePanelAppearanceDescriptor)
    .registerComponent(userMessageEditComposerAppearanceDescriptor)
    .registerComponent(tiptapEditorAppearanceDescriptor)
    .registerComponent(workspaceProjectPermissionsDialogAppearanceDescriptor)
    .registerComponent(workspaceSessionBatchModalAppearanceDescriptor)
    .registerComponent(archivedSessionsConfigAppearanceDescriptor)
    .registerComponent(settingsNavAppearanceDescriptor)
    .registerComponent(backgroundCommandOutputPanelAppearanceDescriptor)
    .registerComponent(chatInputPixelPetAppearanceDescriptor)
    .registerComponent(fileMentionPickerAppearanceDescriptor)
    .registerComponent(sessionFileModificationsBarAppearanceDescriptor)
    .registerComponent(editorBreadcrumbAppearanceDescriptor)
    .registerComponent(gitBranchHistoryAppearanceDescriptor)
    .registerComponent(gitDiffViewAppearanceDescriptor)
    .registerComponent(gitSettingsViewAppearanceDescriptor)
    .registerComponent(quickActionsConfigAppearanceDescriptor)
    .registerComponent(statusBarPopoversAppearanceDescriptor)
    .registerComponent(mEditorAppearanceDescriptor)
    .registerComponent(referencesPanelAppearanceDescriptor)
    .registerComponent(navSearchDialogAppearanceDescriptor)
    .registerComponent(workspaceRelatedPathsDialogAppearanceDescriptor)
    .registerComponent(branchSelectModalAppearanceDescriptor)
    .registerComponent(floatingMiniChatAppearanceDescriptor)
    .registerComponent(miniAppBubbleWelcomeAppearanceDescriptor)
    .registerComponent(branchesViewAppearanceDescriptor)
    .registerComponent(miniAppGalleryViewAppearanceDescriptor)
    .registerComponent(miniAppMarketViewAppearanceDescriptor)
    .registerComponent(miniAppSubmissionsViewAppearanceDescriptor)
    .registerComponent(shellNavAppearanceDescriptor)
    .registerComponent(missionControlAppearanceDescriptor)
    .registerComponent(canvasTabAppearanceDescriptor)
    .registerComponent(canvasTabBarAppearanceDescriptor)
    .registerComponent(chatInputWorkspaceStripAppearanceDescriptor)
    .registerComponent(codePreviewAppearanceDescriptor)
    .registerComponent(exploreGroupAppearanceDescriptor)
    .registerComponent(pendingQueuePanelAppearanceDescriptor)
    .registerComponent(subagentProjectionAppearanceDescriptor)
    .registerComponent(threadGoalDialogsAppearanceDescriptor)
    .registerComponent(userSteeringBubbleAppearanceDescriptor)
    .registerComponent(welcomePanelAppearanceDescriptor)
    .registerComponent(generativeWidgetToolCardAppearanceDescriptor)
    .registerComponent(gitToolDisplayAppearanceDescriptor)
    .registerComponent(snapshotFullscreenDiffViewerAppearanceDescriptor)
    .registerComponent(mermaidBlockAppearanceDescriptor)
    .registerComponent(defaultModelConfigAppearanceDescriptor)
    .registerComponent(globalPermissionRulesDialogAppearanceDescriptor)
    .registerComponent(mcpResourceBrowserAppearanceDescriptor)
    .registerComponent(editorStatusBarAppearanceDescriptor)
    .registerComponent(imageViewerAppearanceDescriptor)
    .registerComponent(branchQuickSwitchAppearanceDescriptor)
    .registerComponent(workspaceListSectionAppearanceDescriptor)
    .registerComponent(workspaceItemAppearanceDescriptor)
    .registerComponent(newProjectDialogAppearanceDescriptor)
    .registerComponent(canvasEditorAreaAppearanceDescriptor)
    .registerComponent(canvasEditorGroupAppearanceDescriptor)
    .registerComponent(canvasThumbnailAppearanceDescriptor)
    .registerComponent(canvasTabOverflowAppearanceDescriptor)
    .registerComponent(browserPanelAppearanceDescriptor)
    .registerComponent(nurseryGalleryAppearanceDescriptor)
    .registerComponent(deepReviewConsentDialogAppearanceDescriptor)
    .registerComponent(flowTextBlockAppearanceDescriptor)
    .registerComponent(permissionRequestPanelAppearanceDescriptor)
    .registerComponent(userMessageAppearanceDescriptor)
    .registerComponent(toolCardAppearanceDescriptor)
    .registerComponent(canvasToolCardAppearanceDescriptor)
    .registerComponent(computerUseToolCardAppearanceDescriptor)
    .registerComponent(defaultToolCardAppearanceDescriptor)
    .registerComponent(getFileDiffDisplayAppearanceDescriptor)
    .registerComponent(miniAppToolDisplayAppearanceDescriptor)
    .registerComponent(modelThinkingDisplayAppearanceDescriptor)
    .registerComponent(todoWriteDisplayAppearanceDescriptor)
    .registerComponent(toolTimeoutIndicatorAppearanceDescriptor)
    .registerComponent(webFetchCardAppearanceDescriptor)
    .registerComponent(compactToolCardAppearanceDescriptor)
    .registerComponent(toolCardStatusSlotAppearanceDescriptor)
    .registerComponent(toolCardHeaderActionsAppearanceDescriptor)
    .registerComponent(acpPermissionActionsAppearanceDescriptor)
    .registerComponent(contextCompressionDisplayAppearanceDescriptor)
    .registerComponent(getToolSpecCardAppearanceDescriptor)
    .registerComponent(viewImageToolCardAppearanceDescriptor)
    .registerComponent(acpPlanPanelAppearanceDescriptor)
    .registerComponent(backgroundCommandInputDialogAppearanceDescriptor)
    .registerComponent(chatEmptyStateAppearanceDescriptor)
    .registerComponent(copyOutputButtonAppearanceDescriptor)
    .registerComponent(copyableTextPreviewAppearanceDescriptor)
    .registerComponent(currentSessionTitleAppearanceDescriptor)
    .registerComponent(coworkExampleCardsAppearanceDescriptor)
    .registerComponent(imageAnalysisCardAppearanceDescriptor)
    .registerComponent(scrollToBottomButtonAppearanceDescriptor)
    .registerComponent(scrollToLatestBarAppearanceDescriptor)
    .registerComponent(scrollToTurnHeaderButtonAppearanceDescriptor)
    .registerComponent(tokenUsageIndicatorAppearanceDescriptor)
    .registerComponent(toolApprovalBarAppearanceDescriptor)
    .registerComponent(snapshotRollbackButtonAppearanceDescriptor)
    .registerComponent(smartRecommendationsAppearanceDescriptor)
    .registerComponent(stickyTaskIndicatorAppearanceDescriptor)
    .registerComponent(turnHistoryPanelAppearanceDescriptor)
    .registerComponent(turnRollbackButtonAppearanceDescriptor)
    .registerComponent(sessionRuntimeStatusEntryAppearanceDescriptor)
    .registerComponent(runtimeStatusSlotAppearanceDescriptor)
    .registerComponent(sessionMenuAppearanceDescriptor)
    .registerComponent(dispatchInstallDialogAppearanceDescriptor)
    .registerComponent(dispatchResultDialogAppearanceDescriptor)
    .registerComponent(dispatchTargetPickerAppearanceDescriptor)
    .registerComponent(voiceInputConfigAppearanceDescriptor)
    .registerComponent(worktreesConfigAppearanceDescriptor)
    .registerComponent(turnCompletionNoticeAppearanceDescriptor)
    .registerComponent(turnFailureNoticeAppearanceDescriptor)
    .registerComponent(virtualItemAppearanceDescriptor)
    .registerScene(workbenchAppearanceDescriptor)
    .registerScene(welcomeAppearanceDescriptor)
    .registerScene(shellAppearanceDescriptor)
    .registerScene(agentsAppearanceDescriptor)
    .registerScene(assistantAppearanceDescriptor)
    .registerScene(browserAppearanceDescriptor)
    .registerScene(fileViewerAppearanceDescriptor)
    .registerScene(gitAppearanceDescriptor)
    .registerScene(miniAppGalleryAppearanceDescriptor)
    .registerScene(miniAppAppearanceDescriptor)
    .registerScene(insightsAppearanceDescriptor)
    .registerScene(pagesAppearanceDescriptor)
    .registerScene(panelViewAppearanceDescriptor)
    .registerScene(profileAppearanceDescriptor)
    .registerScene(sessionAppearanceDescriptor)
    .registerScene(settingsAppearanceDescriptor)
    .registerScene(skillsAppearanceDescriptor)
    .registerScene(terminalAppearanceDescriptor)
    .registerScene(todosSceneAppearanceDescriptor)
    .registerRenderer(cssTokenAppearanceAdapter)
    .registerRenderer(monacoAppearanceAdapter)
    .registerRenderer(xtermAppearanceAdapter)
    .registerRenderer(mermaidAppearanceAdapter)
    .registerRenderer(widgetAppearanceAdapter)
    .registerRenderer(canvasAppearanceAdapter)
    .freeze();
}
