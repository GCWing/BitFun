package com.bitfun.mobile.app.ui.shell

import android.content.Intent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.PermanentDrawerSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScaffoldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.platform.rememberWindowMetrics
import com.bitfun.mobile.app.state.MobileSurface
import com.bitfun.mobile.app.state.SettingsMode
import com.bitfun.mobile.app.state.rememberAppShellState
import com.bitfun.mobile.app.ui.account.AccountScreen
import com.bitfun.mobile.app.ui.chat.GeneralChatScreen
import com.bitfun.mobile.app.ui.remote.FilePreviewSurface
import com.bitfun.mobile.app.ui.remote.AccountRemoteScreen
import com.bitfun.mobile.app.ui.remote.PairingScreen
import com.bitfun.mobile.app.ui.settings.GeneralSettingsScreen
import com.bitfun.mobile.app.ui.settings.SettingsScreen
import com.bitfun.mobile.app.ui.shell.sidebar.AppSidebar
import com.bitfun.mobile.app.viewmodel.AccountViewModel
import com.bitfun.mobile.app.viewmodel.GeneralChatViewModel
import com.bitfun.mobile.app.viewmodel.PairingViewModel
import com.bitfun.mobile.core.feature.account.AccountIntent
import com.bitfun.mobile.core.feature.account.AccountUiState
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.RemoteControlPresenter
import com.bitfun.mobile.core.feature.connection.RemoteControlSource
import com.bitfun.mobile.core.feature.connection.connectionPhase
import com.bitfun.mobile.core.feature.generalchat.GeneralChatIntent
import com.bitfun.mobile.core.feature.layout.ConversationLayoutPolicy
import com.bitfun.mobile.core.feature.layout.FilePreviewPlacement
import com.bitfun.mobile.core.feature.layout.FilePreviewPlacementPolicy
import com.bitfun.mobile.core.feature.pairing.PairingIntent
import com.bitfun.mobile.core.feature.pairing.PairingUiState
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.shell.SidebarSessionRow
import com.bitfun.mobile.core.feature.workspace.RemoteFilePreviewUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState
import kotlinx.coroutines.launch

internal const val MENU_TEST_TAG: String = "shell-menu"

/** Present only while the sidebar is permanent, which is the whole assertion. */
internal const val MASTER_DETAIL_TEST_TAG: String = "shell-master-detail"

/**
 * Keeps the detail pane's content off a hinge it would otherwise be laid across.
 *
 * The policy answers in window coordinates; the padding here is what that offset
 * is once the master pane has already taken its own width. A flat screen asks
 * for no padding at all, and so does a fold that falls behind the master pane —
 * in both cases the pane is its own content.
 */
private fun Modifier.dodgingCrease(
    paneWidth: Int,
    contentOffset: Int,
    contentWidth: Int,
): Modifier {
    if (contentWidth <= 0) return this
    val leading = contentOffset.coerceAtLeast(0)
    val trailing = (paneWidth - contentOffset - contentWidth).coerceAtLeast(0)
    if (leading == 0 && trailing == 0) return this
    return padding(start = leading.dp, end = trailing.dp)
}

/**
 * What goes between two panes.
 *
 * A hinge is already a seam, so nothing is drawn on one; a flat window gets the
 * hairline the policy did not reserve room for, out of the pane that can spare it.
 */
@Composable
private fun PaneSeparator(gapWidth: Int) {
    if (gapWidth > 0) Spacer(Modifier.width(gapWidth.dp)) else VerticalDivider(Modifier.fillMaxHeight())
}

/**
 * The app shell: a drawer beside the content, with settings and the account as
 * sheets over it. Ported from `pages/components/AppShell.ets`.
 *
 * Both view models are resolved here as well as inside their screens; they are
 * activity-scoped, so this is the same instance and not a second store. Reading
 * them at the shell is what lets the drawer show connection state and decide
 * between its signed-in and signed-out chrome, and it is also why swapping the
 * content surface cannot drop an open conversation — the timeline lives in the
 * view model, not in the composable that was removed.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MobileScreen() {
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val shell = rememberAppShellState()

    val pairingViewModel: PairingViewModel = viewModel(factory = PairingViewModel.Factory)
    val accountViewModel: AccountViewModel = viewModel(factory = AccountViewModel.Factory)
    val generalChatViewModel: GeneralChatViewModel =
        viewModel(factory = GeneralChatViewModel.Factory)
    val pairingState by pairingViewModel.state.collectAsStateWithLifecycle()
    val pairingWorkspaceState by pairingViewModel.workspaceState.collectAsStateWithLifecycle()
    val accountWorkspaceState by accountViewModel.workspaceState.collectAsStateWithLifecycle()
    val accountState by accountViewModel.state.collectAsStateWithLifecycle()
    val pairingRemoteState by pairingViewModel.remoteState.collectAsStateWithLifecycle()
    val accountRemoteState by accountViewModel.remoteState.collectAsStateWithLifecycle()
    val generalChatState by generalChatViewModel.state.collectAsStateWithLifecycle()
    val pairingPhase: ConnectionPhase = pairingState.connectionPhase()
    val readyAccount = accountState as? AccountUiState.Ready
    val accountUserId = readyAccount?.userId

    // Which desktop this phone is driving is the one fact neither store holds on
    // its own: the pairing store knows a room, the account store knows a device,
    // and only together do they make one connection with a provenance. The
    // shared presenter decides which of the two wins, so the sheet renders it
    // rather than working it out a second time.
    val controlSummary = remember(pairingState, pairingPhase, readyAccount) {
        RemoteControlPresenter.summarize(
            pairingPhase = pairingPhase,
            pairedRoomLabel = (pairingState as? PairingUiState.Paired)
                ?.workspace?.roomLabel.orEmpty(),
            accountDeviceId = readyAccount?.selectedDeviceId.orEmpty(),
            accountDeviceName = readyAccount?.selectedDeviceName.orEmpty(),
        )
    }
    val phase = controlSummary.phase
    val activeWorkspaceState = when (controlSummary.source) {
        RemoteControlSource.QR_PAIRING -> pairingWorkspaceState
        RemoteControlSource.ACCOUNT_DEVICE -> accountWorkspaceState
        RemoteControlSource.NONE -> RemoteWorkspaceUiState.Idle
    }

    fun dispatchActiveWorkspace(intent: RemoteWorkspaceIntent) {
        when (controlSummary.source) {
            RemoteControlSource.QR_PAIRING -> pairingViewModel.dispatchWorkspace(intent)
            RemoteControlSource.ACCOUNT_DEVICE -> accountViewModel.dispatchWorkspace(intent)
            RemoteControlSource.NONE -> Unit
        }
    }

    // The export labels belong to whichever surface asked for one, so they are
    // read here: the drawer can export a conversation the content area is not
    // showing, and the sheet that hands it over is the shell's.
    val untitledTitle = stringResource(R.string.sidebar_untitled)
    val userLabel = stringResource(R.string.general_chat_role_user)
    val assistantLabel = stringResource(R.string.general_chat_role_assistant)
    val context = LocalContext.current

    // Handing the export to the share sheet is the platform half of the intent;
    // clearing it immediately keeps a rotation from re-opening the chooser. At
    // the shell rather than in the chat screen so that exporting from the drawer
    // works while the remote surface is the one on screen.
    LaunchedEffect(generalChatState.export) {
        val export = generalChatState.export ?: return@LaunchedEffect
        val share = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TITLE, export.title)
            putExtra(Intent.EXTRA_SUBJECT, export.title)
            putExtra(Intent.EXTRA_TEXT, export.markdown)
        }
        context.startActivity(Intent.createChooser(share, export.title))
        generalChatViewModel.dispatch(GeneralChatIntent.ClearExport)
    }

    // The account's models follow whoever is signed in. Keyed on the user id
    // rather than on the whole state so that a device-list refresh does not
    // re-fetch the settings blob, and so that signing out hands over null —
    // which is what drops the previous account's models rather than leaving one
    // user's providers listed for the next.
    LaunchedEffect(accountUserId) {
        generalChatViewModel.bindCloudSettings(
            accountUserId?.let { accountViewModel.cloudSettingsSource() },
        )
    }

    // Handed over whole: which rows are pinned, recent or archived, and which the
    // search leaves standing, is `SidebarPresentation`'s answer inside the drawer.
    // Filtering here would decide it twice and let the archive count drift.
    val sidebarSessions = remember(generalChatState.sessions) {
        generalChatState.sessions.map { session ->
            SidebarSessionRow(
                id = session.id,
                title = session.title,
                status = session.status,
                pinned = session.pinned,
                createdAt = session.createdAt,
                updatedAt = session.updatedAt,
                messageCount = session.messageCount,
            )
        }
    }

    fun closeDrawer() {
        // A no-op while the sidebar is permanent, which is why the sidebar's
        // callbacks are the same lambdas in both shapes: only the container that
        // holds it changes, not what its rows do.
        scope.launch { drawerState.close() }
    }

    // Whether there is room for the sidebar to stay. The policy is the same one
    // `AppRootPresentation.ets` asks, given the same three facts.
    val window = rememberWindowMetrics()
    val wide = ConversationLayoutPolicy.useMasterDetail(
        viewportWidth = window.widthDp,
        largeScreenDevice = window.largeScreenDevice,
        creases = window.creases,
    )
    val geometry = ConversationLayoutPolicy.resolveWideGeometry(window.widthDp, window.creases)

    // A file the agent referenced is the third thing that wants the window, and
    // the one that decides whether the other two still fit. Only the remote
    // surface has one to place: a file opened from the account sheet belongs to
    // that sheet's own store and stays inside it.
    val preview = (activeWorkspaceState as? RemoteWorkspaceUiState.Ready)?.preview
    val previewVisible = shell.surface == MobileSurface.REMOTE &&
        preview != null && preview !is RemoteFilePreviewUiState.None
    val previewLayout = FilePreviewPlacementPolicy.resolveLayout(
        previewVisible = previewVisible,
        largeScreenLayout = wide,
        viewportWidth = window.widthDp,
        creases = window.creases,
        // So the list does not jump sideways the moment a file opens beside it.
        preferredMasterWidth = geometry.masterPaneWidth,
    )
    val sidebarWidth = when {
        !wide -> 0
        previewLayout.placement == FilePreviewPlacement.Hidden -> geometry.masterPaneWidth
        // A focus split gave the list's room to the file; a full-page preview
        // never had room for either.
        else -> previewLayout.masterPaneWidth
    }
    // The button and the pane are the same sidebar; exactly one of them is real.
    val showMenu = sidebarWidth == 0

    val sidebar: @Composable () -> Unit = {
        AppSidebar(
            accountUserId = accountUserId,
            connectionPhase = phase,
            remoteActive = shell.surface == MobileSurface.REMOTE,
            sessions = sidebarSessions,
            // Only while general chat is on screen: the highlight names
            // what the content area is showing, not what the store last
            // opened behind the remote surface.
            selectedSessionId = generalChatState.sessionId
                .takeIf { shell.surface == MobileSurface.GENERAL_CHAT },
            query = shell.sidebarQuery,
            searchOpen = shell.searchOpen,
            onQueryChange = shell::search,
            onToggleSearch = shell::toggleSearch,
            onEnterCode = {
                shell.show(MobileSurface.REMOTE)
                closeDrawer()
            },
            onNewChat = {
                generalChatViewModel.dispatch(GeneralChatIntent.NewSession)
                shell.show(MobileSurface.GENERAL_CHAT)
                closeDrawer()
            },
            onOpenSession = { session ->
                generalChatViewModel.dispatch(GeneralChatIntent.SelectSession(session.id))
                shell.show(MobileSurface.GENERAL_CHAT)
                closeDrawer()
            },
            onRenameSession = { id, title ->
                generalChatViewModel.dispatch(GeneralChatIntent.RenameSession(id, title))
            },
            onArchiveSession = { id, archived ->
                generalChatViewModel.dispatch(GeneralChatIntent.ArchiveSession(id, archived))
            },
            onExportSession = { session ->
                generalChatViewModel.dispatch(
                    GeneralChatIntent.ExportSession(
                        session.id,
                        untitledTitle,
                        userLabel,
                        assistantLabel,
                    ),
                )
            },
            onDeleteSession = { id ->
                generalChatViewModel.dispatch(GeneralChatIntent.DeleteSession(id))
            },
            onOpenSettings = {
                // The gear is one button over two pages, split on what is behind
                // it, as `AppRootOverlaySurfaces.openSettings()` splits it: from a
                // remote conversation it asks about the desktop, and from the
                // app's own chat it asks about the app.
                shell.openSettings(
                    when (shell.surface) {
                        MobileSurface.REMOTE -> SettingsMode.REMOTE
                        MobileSurface.GENERAL_CHAT -> SettingsMode.GENERAL
                    },
                )
                closeDrawer()
            },
            onOpenAccount = {
                shell.openAccount()
                closeDrawer()
            },
            modifier = Modifier,
        )
    }

    val content: @Composable () -> Unit = {
        Scaffold(
            // The manifest asks for `adjustResize`, but an edge-to-edge window
            // is never resized by it — the keyboard simply draws on top, and
            // what it draws on top of is the composer. Adding the IME to the
            // insets the content already lifts itself out of is what actually
            // keeps the input bar above the keyboard. `union` rather than a
            // second padding: the IME inset already contains the navigation
            // bar's, and adding them would leave a gap the height of the bar.
            contentWindowInsets = ScaffoldDefaults.contentWindowInsets.union(WindowInsets.ime),
            topBar = {
                TopAppBar(
                    title = {
                        Text(
                            stringResource(
                                when (shell.surface) {
                                    MobileSurface.GENERAL_CHAT -> R.string.navigation_general_chat
                                    MobileSurface.REMOTE -> R.string.navigation_remote
                                },
                            ),
                        )
                    },
                    navigationIcon = {
                        // Nothing to open when the sidebar never left: a button
                        // that reveals what is already on screen is a button that
                        // does nothing.
                        if (showMenu) {
                            IconButton(
                                onClick = { scope.launch { drawerState.open() } },
                                modifier = Modifier.testTag(MENU_TEST_TAG),
                            ) {
                                Icon(
                                    // Two bars, drawn off the source's own
                                    // `gpt_home_menu_glyph` rather than the
                                    // three Material stacks up.
                                    painterResource(R.drawable.ic_symbol_menu_lines),
                                    contentDescription = stringResource(R.string.shell_open_sidebar),
                                    modifier = Modifier.size(width = 22.dp, height = 14.dp),
                                )
                            }
                        }
                    },
                    actions = {
                        if (shell.surface == MobileSurface.REMOTE) {
                            Text(
                                stringResource(phase.labelRes()),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    },
                )
            },
        ) { insets ->
            Box(Modifier.padding(insets)) {
                when (shell.surface) {
                    MobileSurface.GENERAL_CHAT -> GeneralChatScreen(Modifier)
                    MobileSurface.REMOTE -> when (controlSummary.source) {
                        RemoteControlSource.ACCOUNT_DEVICE -> AccountRemoteScreen(
                            remoteState = accountRemoteState,
                            workspaceState = accountWorkspaceState,
                            deviceId = readyAccount?.selectedDeviceId.orEmpty(),
                            deviceName = controlSummary.desktopName,
                            accountUsername = readyAccount?.username.orEmpty(),
                            onSessionIntent = accountViewModel::dispatchSession,
                            onWorkspaceIntent = accountViewModel::dispatchWorkspace,
                            modifier = Modifier,
                        )

                        RemoteControlSource.QR_PAIRING,
                        RemoteControlSource.NONE,
                        -> PairingScreen(Modifier)
                    }
                }
            }
        }
    }

    val previewPane: @Composable () -> Unit = {
        preview?.let { current ->
            FilePreviewSurface(
                preview = current,
                onIntent = ::dispatchActiveWorkspace,
                modifier = Modifier.padding(16.dp),
            )
        }
    }

    // The drawer wraps both shapes rather than only the compact one: a window
    // that loses its permanent sidebar to a preview still has a menu button, and
    // that button needs something to open.
    ModalNavigationDrawer(
        drawerState = drawerState,
        gesturesEnabled = showMenu,
        drawerContent = { if (showMenu) ModalDrawerSheet { sidebar() } },
    ) {
        if (previewLayout.placement == FilePreviewPlacement.CompactFullPage) {
            // Two documents in one phone width is two columns of hyphenation, so
            // the file covers the conversation — the same trade
            // `FilePreviewSurface.ets` makes. Covering the page means covering
            // the app bar with it, so the way out has to be the system's own
            // back gesture as well as the card's button, and the insets the
            // scaffold was applying have to be applied here instead.
            BackHandler {
                dispatchActiveWorkspace(RemoteWorkspaceIntent.DismissPreview)
            }
            Surface(Modifier.fillMaxSize()) {
                Box(Modifier.safeDrawingPadding()) { previewPane() }
            }
        } else {
            Row(Modifier.fillMaxSize()) {
                if (sidebarWidth > 0) {
                    PermanentDrawerSheet(
                        Modifier.width(sidebarWidth.dp).testTag(MASTER_DETAIL_TEST_TAG),
                    ) { sidebar() }
                    PaneSeparator(
                        if (previewVisible) {
                            previewLayout.masterConversationGap
                        } else {
                            geometry.masterDetailGap
                        },
                    )
                }
                if (previewLayout.placement == FilePreviewPlacement.Hidden) {
                    val hasMaster = sidebarWidth > 0
                    Box(
                        Modifier
                            .weight(1f)
                            .dodgingCrease(
                                paneWidth = window.widthDp - sidebarWidth - geometry.masterDetailGap,
                                contentOffset = if (hasMaster) {
                                    geometry.detailContentOffset
                                } else {
                                    geometry.collapsedDetailContentOffset
                                },
                                contentWidth = if (hasMaster) {
                                    geometry.detailContentWidth
                                } else {
                                    geometry.collapsedDetailContentWidth
                                },
                            ),
                    ) { content() }
                } else {
                    Box(Modifier.width(previewLayout.conversationPaneWidth.dp)) { content() }
                    PaneSeparator(previewLayout.conversationPreviewGap)
                    Box(Modifier.width(previewLayout.previewPaneWidth.dp)) { previewPane() }
                }
            }
        }
    }

    if (shell.showSettings) {
        ModalBottomSheet(
            onDismissRequest = shell::dismissSettings,
            // Full height on open, never half: the source's sheet is a page, and
            // a half-open one puts the permission modes — the reason to come here
            // — below the fold behind a drag the page gives no sign is needed.
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            // The source's own sheet chrome: PAGE_BG rather than a raised
            // container, corners at 34, and no drag handle — the page carries a
            // close button of its own, and a handle above a centred title would
            // push it off the vertical the source centres it on.
            containerColor = MaterialTheme.colorScheme.background,
            shape = RoundedCornerShape(topStart = 34.dp, topEnd = 34.dp),
            dragHandle = null,
        ) {
            when (shell.settingsMode) {
                SettingsMode.GENERAL -> GeneralSettingsScreen(
                    modifier = Modifier.fillMaxWidth(),
                    accountUserId = accountUserId,
                    accountUsername = readyAccount?.username.orEmpty(),
                    config = generalChatState.config,
                    models = generalChatState.models,
                    activeModelId = generalChatState.activeModelId,
                    configFailure = generalChatState.configFailure,
                    connectionTest = generalChatState.connectionTest,
                    onChatIntent = generalChatViewModel::dispatch,
                    onSaveConfig = { intent ->
                        generalChatViewModel.dispatch(intent)
                        generalChatViewModel.state.value.configFailure == null
                    },
                    onOpenAccount = shell::openAccount,
                    onClose = shell::dismissSettings,
                )

                SettingsMode.REMOTE -> SettingsScreen(
                    modifier = Modifier.fillMaxWidth(),
                    accountUserId = accountUserId,
                    summary = controlSummary,
                    // The permission mode belongs to the desktop the summary
                    // named, so it has to be asked of that desktop's store —
                    // asking the other one would answer for a connection this
                    // page is not describing, or for none at all.
                    remoteState = when (controlSummary.source) {
                        RemoteControlSource.QR_PAIRING -> pairingRemoteState
                        RemoteControlSource.ACCOUNT_DEVICE -> accountRemoteState
                        RemoteControlSource.NONE -> RemoteSessionUiState.Idle
                    },
                    onSessionIntent = when (controlSummary.source) {
                        RemoteControlSource.QR_PAIRING -> pairingViewModel::dispatchSession
                        RemoteControlSource.ACCOUNT_DEVICE -> accountViewModel::dispatchSession
                        RemoteControlSource.NONE -> {
                            {}
                        }
                    },
                    onClose = shell::dismissSettings,
                    onOpenAccount = shell::openAccount,
                    onDisconnect = { pairingViewModel.dispatch(PairingIntent.Disconnect) },
                    onReconnect = {
                        // A room is re-checked where it stands; a device is asked
                        // for again, which is the same command its row in the
                        // account sends. Neither re-pairs behind the user's back.
                        when (controlSummary.source) {
                            RemoteControlSource.QR_PAIRING ->
                                pairingViewModel.dispatch(PairingIntent.Verify)

                            RemoteControlSource.ACCOUNT_DEVICE -> {
                                val deviceId = readyAccount?.selectedDeviceId
                                if (deviceId != null) {
                                    accountViewModel.dispatch(
                                        AccountIntent.SelectDevice(deviceId),
                                    )
                                }
                            }

                            RemoteControlSource.NONE -> Unit
                        }
                    },
                    onConnectByLink = {
                        shell.dismissSettings()
                        shell.show(MobileSurface.REMOTE)
                    },
                )
            }
        }
    }
    if (shell.showAccount) {
        ModalBottomSheet(
            onDismissRequest = shell::dismissAccount,
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            containerColor = MaterialTheme.colorScheme.background,
            shape = RoundedCornerShape(topStart = 34.dp, topEnd = 34.dp),
        ) {
            AccountScreen(Modifier.fillMaxWidth())
        }
    }
}
