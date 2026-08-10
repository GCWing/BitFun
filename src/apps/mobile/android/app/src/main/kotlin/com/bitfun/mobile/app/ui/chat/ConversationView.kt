package com.bitfun.mobile.app.ui.chat

import android.app.Activity
import android.content.Intent
import android.speech.RecognizerIntent
import android.util.Base64
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.settings.RemoteSettingsSheet
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.connection.ConnectionStatusPresenter
import com.bitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.bitfun.mobile.core.feature.session.ComposerImage
import com.bitfun.mobile.core.feature.session.ConversationRowKind
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.session.conversationRows
import com.bitfun.mobile.core.feature.session.selectedModelOption
import java.util.UUID

internal const val CONVERSATION_TEST_TAG: String = "conversation"
internal const val CONVERSATION_BACK_TEST_TAG: String = "conversation-back"

/**
 * The transcript itself, tagged so a test can scroll it to a row.
 *
 * Separate from [CONVERSATION_TEST_TAG] because that one sits on the whole
 * surface, header and composer included, and it is not the scrollable.
 */
internal const val CONVERSATION_LIST_TEST_TAG: String = "conversation-list"

/** The relay refuses anything larger, and refusing here is a better error. */
private const val MAX_IMAGE_BYTES = 8 * 1024 * 1024

/**
 * One open session: the transcript and the composer, ported from
 * `pages/components/ConversationSurface.ets`.
 *
 * The transcript is a [LazyColumn] and the composer is pinned below it, so a
 * long session never pushes the input off screen. That is also why this surface
 * replaces the session list rather than sitting under it — the list's own scroll
 * cannot contain a lazy list.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ConversationView(
    state: RemoteSessionUiState.Ready,
    phase: ConnectionPhase,
    onBack: () -> Unit,
    onIntent: (RemoteSessionIntent) -> Unit,
    /**
     * Where this transcript is running — a desktop name, or the brand and the
     * branch. Built by `ConversationHeaderPresenter` at the call site, because
     * only the screen knows whether it reached this session through a pairing
     * or through an account device.
     */
    contextTitle: String,
    /** A file the agent named, taken from a markdown link. Path, then label. */
    onOpenFile: (String, String) -> Unit,
    /**
     * The file the preview surface currently holds, normalised, and whether it
     * is still arriving. Passed as two scalars rather than the workspace state:
     * this screen is about the transcript, and the only thing it needs from the
     * preview is which of its own file cards is the one on screen.
     */
    previewingRemotePath: String,
    previewLoading: Boolean,
    modifier: Modifier,
) {
    val timeline = state.timeline ?: return
    val sessionId = state.selectedSessionId.orEmpty()
    val rows = remember(timeline) { timeline.conversationRows() }
    val listState = rememberLazyListState()
    var draft by rememberSaveable(sessionId) { mutableStateOf("") }
    var images by remember(sessionId) { mutableStateOf<List<ComposerImage>>(emptyList()) }
    var showSettings by rememberSaveable(sessionId) { mutableStateOf(false) }
    val context = LocalContext.current

    val photoPicker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) {
            val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            val mime = context.contentResolver.getType(uri) ?: "image/jpeg"
            if (bytes != null && bytes.size <= MAX_IMAGE_BYTES && images.size < MAX_COMPOSER_IMAGES) {
                images = images + ComposerImage(
                    id = "android-" + UUID.randomUUID(),
                    dataUrl = "data:" + mime + ";base64," + Base64.encodeToString(bytes, Base64.NO_WRAP),
                    mimeType = mime,
                )
            }
        }
    }
    val voiceInput = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            val text = result.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull().orEmpty()
            if (text.isNotBlank()) {
                draft = listOf(draft.trim(), text.trim()).filter(String::isNotEmpty).joinToString(" ")
            }
        }
    }

    // Follow the tail: new rows arrive while the user is reading the latest one.
    LaunchedEffect(rows.size) {
        if (rows.isNotEmpty()) listState.animateScrollToItem(rows.lastIndex)
    }

    Column(modifier = modifier.fillMaxSize().testTag(CONVERSATION_TEST_TAG)) {
        ConversationHeader(
            title = state.sessions.firstOrNull { it.id == sessionId }?.title.orEmpty(),
            contextTitle = contextTitle,
            canStop = timeline.activeTurn != null,
            enabled = !state.busy && sessionId.isNotEmpty(),
            onBack = onBack,
            onRename = { title ->
                onIntent(RemoteSessionIntent.RenameSession(sessionId, title))
            },
            onOpenSettings = { showSettings = true },
            onStop = {
                onIntent(RemoteSessionIntent.CancelTurn(sessionId, timeline.activeTurn?.turnId))
            },
            modifier = Modifier,
        )

        LazyColumn(
            state = listState,
            modifier = Modifier.weight(1f).fillMaxWidth().testTag(CONVERSATION_LIST_TEST_TAG),
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
                        onApproveTool = { toolId ->
                            onIntent(RemoteSessionIntent.ApproveTool(sessionId, toolId))
                        },
                        onRejectTool = { toolId, reason ->
                            onIntent(RemoteSessionIntent.RejectTool(sessionId, toolId, reason))
                        },
                        onCancelTool = { toolId, reason ->
                            onIntent(RemoteSessionIntent.CancelTool(sessionId, toolId, reason))
                        },
                        onAnswerTool = { toolId, answer ->
                            onIntent(RemoteSessionIntent.AnswerQuestion(sessionId, toolId, answer))
                        },
                        // A retry is the same message again; the failed row stays
                        // where it is so the order the user typed in survives.
                        onRetry = { text ->
                            onIntent(RemoteSessionIntent.SendMessage(sessionId, text, null))
                        },
                        // A link in an agent turn names a file on the desktop,
                        // so it opens the same preview the file field opens.
                        onOpenLink = { path, label -> onOpenFile(path, label) },
                        previewingRemotePath = previewingRemotePath,
                        previewLoading = previewLoading,
                        modifier = Modifier,
                    )
                }
            }
        }

        ComposerBar(
            draft = draft,
            images = images,
            // An empty session id would send nowhere, so it reads as busy.
            busy = state.busy || sessionId.isEmpty(),
            streaming = timeline.activeTurn != null,
            phase = phase,
            model = timeline.selectedModelOption(stringResource(R.string.models_unnamed)),
            capabilities = ChatComposerCapabilities.RemoteChat,
            placeholder = stringResource(R.string.message_input_label),
            onDraftChange = { draft = it },
            onRemoveImage = { id -> images = images.filterNot { it.id == id } },
            onOpenModels = { showSettings = true },
            modifier = Modifier,
            onAttach = {
                photoPicker.launch(
                    PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                )
            },
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
            onSend = {
                onIntent(
                    RemoteSessionIntent.SendMessage(
                        sessionId,
                        draft,
                        images.takeIf { it.isNotEmpty() },
                    ),
                )
                draft = ""
                images = emptyList()
            },
            onStop = {
                onIntent(RemoteSessionIntent.CancelTurn(sessionId, timeline.activeTurn?.turnId))
            },
        )
    }

    if (showSettings) {
        ModalBottomSheet(onDismissRequest = { showSettings = false }) {
            RemoteSettingsSheet(
                state = state,
                sessionId = sessionId,
                onIntent = onIntent,
                modifier = Modifier.fillMaxWidth().padding(16.dp),
            )
        }
    }
}
