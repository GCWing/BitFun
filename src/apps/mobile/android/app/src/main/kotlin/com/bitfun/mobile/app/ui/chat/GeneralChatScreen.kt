package com.bitfun.mobile.app.ui.chat

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.session.RenameSessionDialog
import com.bitfun.mobile.app.ui.settings.ModelServiceScreen
import com.bitfun.mobile.app.viewmodel.GeneralChatViewModel
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.generalchat.GeneralChatFailureReason
import com.bitfun.mobile.core.feature.generalchat.GeneralChatIntent
import com.bitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.bitfun.mobile.core.feature.session.ConversationRowKind
import com.bitfun.mobile.core.feature.session.ModelOption
import com.bitfun.mobile.core.feature.session.conversationRows

internal const val GENERAL_CHAT_TEST_TAG: String = "general-chat"
internal const val GENERAL_CHAT_MENU_TEST_TAG: String = "general-chat-menu"
internal const val GENERAL_CHAT_MODEL_TEST_TAG: String = "general-chat-model"

/**
 * The general-chat surface, the counterpart of `ConversationView` for a
 * conversation that has no desktop behind it.
 *
 * It renders the same rows through the same bubbles and the same composer as the
 * remote session: both surfaces project the same shared timeline, and letting
 * them diverge visually is how the two clients drift apart. What differs is the
 * header menu — rename, export and delete are this store's own operations — and
 * the composer's capabilities.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun GeneralChatScreen(
    modifier: Modifier,
    viewModel: GeneralChatViewModel = viewModel(factory = GeneralChatViewModel.Factory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val rows = remember(state.timeline) { state.timeline.conversationRows() }
    val listState = rememberLazyListState()
    val context = LocalContext.current
    var menuOpen by rememberSaveable { mutableStateOf(false) }
    var showModelService by rememberSaveable { mutableStateOf(false) }
    var renaming by rememberSaveable { mutableStateOf(false) }
    var confirmDelete by rememberSaveable { mutableStateOf(false) }

    val untitled = stringResource(R.string.sidebar_untitled)
    val userLabel = stringResource(R.string.general_chat_role_user)
    val assistantLabel = stringResource(R.string.general_chat_role_assistant)
    val localModelSource = stringResource(R.string.model_selector_local)
    val accountModelSource = stringResource(R.string.model_selector_account)
    val title = state.sessions.firstOrNull { it.id == state.sessionId }
        ?.title
        ?.takeIf(String::isNotBlank)
        ?: untitled
    val composerModels = remember(
        state.models,
        state.activeModelId,
        localModelSource,
        accountModelSource,
    ) {
        state.models.map { model ->
            ModelOption(
                id = model.id,
                primaryLabel = model.label,
                secondaryLabel = when (model.source) {
                    com.bitfun.mobile.core.feature.generalchat.GeneralChatModelSource.LOCAL ->
                        localModelSource
                    com.bitfun.mobile.core.feature.generalchat.GeneralChatModelSource.ACCOUNT ->
                        accountModelSource
                },
                selected = model.id == state.activeModelId,
            )
        }
    }

    val voiceInput = rememberLauncherForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            val text = result.data
                ?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)
                ?.firstOrNull()
                .orEmpty()
            if (text.isNotBlank()) {
                val merged = listOf(state.draft.trim(), text.trim())
                    .filter(String::isNotEmpty)
                    .joinToString(" ")
                viewModel.dispatch(GeneralChatIntent.UpdateDraft(merged))
            }
        }
    }

    // An export's share sheet is raised by `MobileScreen`, which is mounted
    // whichever surface is showing: the drawer can export a conversation this
    // screen is not on, and two consumers would raise two choosers.
    LaunchedEffect(rows.size) {
        if (rows.isNotEmpty()) listState.animateScrollToItem(rows.lastIndex)
    }

    Column(modifier = modifier.fillMaxSize().testTag(GENERAL_CHAT_TEST_TAG)) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).padding(start = 8.dp),
            )
            TextButton(
                onClick = { showModelService = true },
                modifier = Modifier.testTag(GENERAL_CHAT_MODEL_TEST_TAG),
            ) {
                Text(
                    // The model that would answer, not the local form's name —
                    // an account model is an answer too, and labelling the button
                    // "not configured" beside a chat that works would send the
                    // user into a form they never needed to open.
                    state.models.firstOrNull { it.id == state.activeModelId }?.label
                        ?: stringResource(R.string.model_service_not_configured),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Box {
                IconButton(
                    onClick = { menuOpen = true },
                    modifier = Modifier.testTag(GENERAL_CHAT_MENU_TEST_TAG),
                ) {
                    Icon(
                        painterResource(R.drawable.ic_symbol_ellipsis),
                        contentDescription = stringResource(R.string.general_chat_menu),
                    )
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    // Every one of these acts on a stored conversation; a session
                    // that has never been sent to has no row to rename, pin,
                    // archive, share or delete.
                    val stored = state.sessions.any { it.id == state.sessionId }
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.session_rename)) },
                        enabled = stored,
                        onClick = {
                            menuOpen = false
                            renaming = true
                        },
                    )
                    // Pinning and archiving are the two the source keeps in this
                    // menu rather than in the row's sheet: both are statements
                    // about the conversation you are reading, made while you read
                    // it, and the sidebar is where you see the result.
                    val current = state.sessions.firstOrNull { it.id == state.sessionId }
                    val archived = current?.status.equals(ARCHIVED, ignoreCase = true)
                    DropdownMenuItem(
                        text = {
                            Text(
                                stringResource(
                                    if (current?.pinned == true) {
                                        R.string.session_unpin
                                    } else {
                                        R.string.session_pin
                                    },
                                ),
                            )
                        },
                        enabled = stored,
                        onClick = {
                            menuOpen = false
                            viewModel.dispatch(
                                GeneralChatIntent.PinSession(
                                    state.sessionId,
                                    current?.pinned != true,
                                ),
                            )
                        },
                    )
                    DropdownMenuItem(
                        text = {
                            Text(
                                stringResource(
                                    if (archived) R.string.session_unarchive else R.string.session_archive,
                                ),
                            )
                        },
                        enabled = stored,
                        onClick = {
                            menuOpen = false
                            viewModel.dispatch(
                                GeneralChatIntent.ArchiveSession(state.sessionId, !archived),
                            )
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.general_chat_export)) },
                        enabled = stored,
                        onClick = {
                            menuOpen = false
                            viewModel.dispatch(
                                GeneralChatIntent.ExportSession(
                                    state.sessionId,
                                    untitled,
                                    userLabel,
                                    assistantLabel,
                                ),
                            )
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.session_delete)) },
                        enabled = stored,
                        onClick = {
                            menuOpen = false
                            confirmDelete = true
                        },
                    )
                }
            }
        }

        if (!state.configured) {
            UnconfiguredNotice(onConfigure = { showModelService = true })
        }

        LazyColumn(
            state = listState,
            modifier = Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(rows, key = { it.id }) { row ->
                if (row.kind == ConversationRowKind.EMPTY) {
                    ConversationEmptyState(Modifier)
                } else {
                    ChatMessageBubble(
                        row = row,
                        enabled = !state.busy,
                        // General chat has no tools: the provider stream carries
                        // text only, so no row can ever ask for an approval.
                        onApproveTool = {},
                        onRejectTool = { _, _ -> },
                        onCancelTool = { _, _ -> },
                        onAnswerTool = { _, _ -> },
                        onRetry = { text ->
                            viewModel.dispatch(GeneralChatIntent.UpdateDraft(text))
                            viewModel.dispatch(GeneralChatIntent.Send)
                        },
                        // No desktop behind this surface, so a link to a file has
                        // nothing to open — only a web address goes anywhere, and
                        // it leaves the app. Anything else is left as plain text.
                        onOpenLink = { url, _ -> openWebLink(context, url) },
                        // No preview surface here either, so no card is ever the
                        // open one; the projector only matches `computer://`, so
                        // a general-chat turn produces no cards to mark anyway.
                        previewingRemotePath = "",
                        previewLoading = false,
                        modifier = Modifier,
                    )
                }
            }
        }

        state.failure?.let { failure ->
            Text(
                stringResource(failure.messageRes()),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }

        ComposerBar(
            draft = state.draft,
            images = emptyList(),
            busy = state.busy,
            streaming = state.timeline.activeTurn != null,
            // Never consulted: general chat's capabilities do not require a
            // desktop, so the phase cannot gate the send.
            phase = ConnectionPhase.DISCONNECTED,
            model = composerModels.firstOrNull { it.selected },
            modelOptions = composerModels,
            capabilities = ChatComposerCapabilities.GeneralChat,
            placeholder = stringResource(R.string.message_input_label),
            onDraftChange = { viewModel.dispatch(GeneralChatIntent.UpdateDraft(it)) },
            onRemoveImage = {},
            onAttach = {},
            onOpenModels = { showModelService = true },
            onSelectModel = { viewModel.dispatch(GeneralChatIntent.SelectModel(it)) },
            onSend = { viewModel.dispatch(GeneralChatIntent.Send) },
            onStop = { viewModel.dispatch(GeneralChatIntent.Cancel) },
            modifier = Modifier,
            onVoice = {
                voiceInput.launch(
                    Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                        putExtra(
                            RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
                        )
                    },
                )
            },
        )
    }

    if (showModelService) {
        val dismissModelService = {
            showModelService = false
            viewModel.dispatch(GeneralChatIntent.ClearConfigFailure)
            viewModel.dispatch(GeneralChatIntent.ClearConnectionTest)
        }
        ModalBottomSheet(
            onDismissRequest = dismissModelService,
            // Straight to full height. Material's default opens a tall sheet at
            // half height first, which would show this panel's header over a
            // blank half and hide the rows it exists to show until the user
            // dragged it — the source's sheet has one height and arrives at it.
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
            // The panel brings its own header with the close button in it, and a
            // handle above that header would be a second way out drawn over the
            // first — the source's sheet has none either.
            dragHandle = null,
            containerColor = MaterialTheme.colorScheme.surface,
            shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        ) {
            ModelServiceScreen(
                config = state.config,
                models = state.models,
                activeModelId = state.activeModelId,
                failure = state.configFailure,
                connectionTest = state.connectionTest,
                onIntent = viewModel::dispatch,
                onSave = { intent ->
                    viewModel.dispatch(intent)
                    // Read back rather than waiting for recomposition: dispatch
                    // is synchronous, and a refused save must keep the form up
                    // with the reason next to the field that caused it.
                    viewModel.state.value.configFailure == null
                },
                onClose = dismissModelService,
                // The source's `height('78%')`: tall enough for the form and the
                // keyboard under it, short enough that the conversation it was
                // opened from is still visible above.
                //
                // Measured off the screen rather than asked of the parent: a
                // sheet hands its content an unbounded height, so a fraction of
                // it is a fraction of nothing and the panel would collapse onto
                // whatever its rows happen to add up to.
                modifier = Modifier
                    .fillMaxWidth()
                    .height(LocalConfiguration.current.screenHeightDp.dp * 0.78f),
            )
        }
    }

    if (renaming) {
        RenameSessionDialog(
            currentTitle = state.sessions.firstOrNull { it.id == state.sessionId }?.title.orEmpty(),
            onConfirm = { viewModel.dispatch(GeneralChatIntent.RenameSession(state.sessionId, it)) },
            onDismiss = { renaming = false },
        )
    }

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(stringResource(R.string.session_delete)) },
            text = { Text(stringResource(R.string.general_chat_delete_confirm)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        viewModel.dispatch(GeneralChatIntent.DeleteSession(state.sessionId))
                        confirmDelete = false
                    },
                ) {
                    Text(stringResource(R.string.session_delete))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = false }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}

/** Shown above the transcript rather than instead of it: an unconfigured provider
 *  does not hide a conversation the user can still read back. */
@Composable
private fun UnconfiguredNotice(onConfigure: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            stringResource(R.string.general_chat_setup_hint),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        TextButton(onClick = onConfigure) {
            Text(stringResource(R.string.general_chat_setup))
        }
    }
}

/**
 * Hands a link to whatever the phone opens web addresses with.
 *
 * Only `http` and `https`: everything else a model can write into a link — a
 * path, a `file:` url, an intent scheme — either points at a machine this app
 * cannot reach or is a way to aim this app's own components from text a server
 * sent. A device with no browser at all is the remaining failure, and doing
 * nothing is the right answer there too.
 */
private fun openWebLink(context: Context, url: String) {
    val uri = runCatching { Uri.parse(url) }.getOrNull() ?: return
    if (uri.scheme?.lowercase() !in setOf("http", "https")) return
    runCatching {
        context.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

internal fun GeneralChatFailureReason.messageRes(): Int = when (this) {
    GeneralChatFailureReason.UNCONFIGURED -> R.string.general_chat_unconfigured
    GeneralChatFailureReason.AUTHENTICATION -> R.string.general_chat_authentication
    GeneralChatFailureReason.RATE_LIMITED -> R.string.general_chat_rate_limited
    GeneralChatFailureReason.SERVICE_UNAVAILABLE -> R.string.general_chat_service_unavailable
    GeneralChatFailureReason.INVALID_RESPONSE -> R.string.general_chat_invalid_response
    GeneralChatFailureReason.NETWORK -> R.string.general_chat_network
}

/** The status the store writes for a filed-away conversation. */
private const val ARCHIVED = "archived"
