package com.bitfun.mobile.app.ui.chat

import android.app.Activity
import android.content.Intent
import android.speech.RecognizerIntent
import android.util.Base64
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import com.bitfun.mobile.app.ui.common.AdaptiveModalSurface
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.layout.SettingsPlacement
import com.bitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.bitfun.mobile.core.feature.session.ComposerImage
import com.bitfun.mobile.core.feature.session.ConversationRowKind
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent.AnswerStructuredQuestion
import com.bitfun.mobile.core.feature.session.QuestionAnswer
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.session.conversationRows
import com.bitfun.mobile.core.feature.session.modelOptions
import com.bitfun.mobile.core.feature.session.selectedModelOption
import com.bitfun.mobile.core.feature.workspace.RemoteFileDownloadUiState
import java.util.UUID

internal const val CONVERSATION_TEST_TAG: String = "conversation"
internal const val CONVERSATION_BACK_TEST_TAG: String = "conversation-back"
internal const val CONVERSATION_LOADING_TEST_TAG: String = "conversation-loading"

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
 * Joins a dictated fragment onto whatever the composer already holds.
 *
 * Extracted so the voice path and its merge policy are unit-testable. It keeps
 * the single-space join the previous in-composition draft used: a blank side is
 * dropped rather than leaving a doubled or leading space.
 */
internal fun mergeComposerDraft(existing: String, spoken: String): String =
    listOf(existing.trim(), spoken.trim()).filter(String::isNotEmpty).joinToString(" ")

/**
 * One open session: the transcript and the composer, ported from
 * `pages/components/ConversationSurface.ets`.
 *
 * The transcript is a lazy list and the composer is pinned below it, so a
 * long session never pushes the input off screen. That is also why this surface
 * replaces the session list rather than sitting under it — the list's own scroll
 * cannot contain a lazy list.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ConversationView(
    state: RemoteSessionUiState.Ready,
    phase: ConnectionPhase,
    settingsPlacement: SettingsPlacement,
    onBack: () -> Unit,
    onOpenSidebar: (() -> Unit)? = null,
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
    download: RemoteFileDownloadUiState,
    onDownloadFile: (String, String) -> Unit,
    modifier: Modifier,
) {
    val timeline = state.timeline
    val activeTurn = timeline?.activeTurn
    val sessionId = state.selectedSessionId.orEmpty()
    val rows = remember(timeline) { timeline?.conversationRows().orEmpty() }
    val visibleRows = remember(rows) { rows.filter { it.kind != ConversationRowKind.EMPTY } }
    val uploadedFileCount = rows.sumOf { it.images.size }
    // Resolved here rather than inside the click: a Toast is raised from a
    // callback, and reading resources off `LocalContext` there reads them
    // without the composition's configuration.
    val uploadedFilesMessage = if (uploadedFileCount > 0) {
        stringResource(R.string.session_uploaded_files_count, uploadedFileCount)
    } else {
        stringResource(R.string.session_uploaded_files_empty)
    }
    // The remote composer's single source of truth is the store's draft. Typing,
    // voice, and send all round-trip through `state.draft` so a half-written
    // message survives session switches and process restarts via DraftStore.
    val draft = state.draft
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
                onIntent(RemoteSessionIntent.UpdateDraft(mergeComposerDraft(state.draft, text)))
            }
        }
    }

    Column(modifier = modifier.fillMaxSize().testTag(CONVERSATION_TEST_TAG)) {
        ConversationHeader(
            title = state.sessions.firstOrNull { it.id == sessionId }?.title.orEmpty(),
            contextTitle = contextTitle,
            canStop = activeTurn != null,
            enabled = !state.busy && sessionId.isNotEmpty(),
            onBack = onBack,
            onOpenSidebar = onOpenSidebar,
            onRename = { title ->
                onIntent(RemoteSessionIntent.RenameSession(sessionId, title))
            },
            onShowUploadedFiles = {
                Toast.makeText(context, uploadedFilesMessage, Toast.LENGTH_SHORT).show()
            },
            onStop = {
                onIntent(RemoteSessionIntent.CancelTurn(sessionId, activeTurn?.turnId))
            },
            modifier = Modifier,
        )

        if (phase != ConnectionPhase.CONNECTED) {
            ChatStatusBar(
                phase = phase,
                canStop = activeTurn != null,
                onStop = {
                    onIntent(RemoteSessionIntent.CancelTurn(sessionId, activeTurn?.turnId))
                },
            )
        }

        if (timeline == null) {
            ConversationLoadingState(modifier = Modifier.weight(1f).fillMaxWidth())
        } else if (visibleRows.isEmpty()) {
            ConversationEmptyState(modifier = Modifier.weight(1f).fillMaxWidth())
        } else {
            ConversationTimelineView(
                rows = visibleRows,
                hasMoreMessages = state.hasMoreMessages,
                onLoadOlder = { onIntent(RemoteSessionIntent.LoadOlderMessages) },
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
                onAnswerToolStructured = { toolId, answers ->
                    onIntent(AnswerStructuredQuestion(sessionId, toolId, answers))
                },
                onRetry = { text ->
                    onIntent(RemoteSessionIntent.SendMessage(sessionId, text, null))
                },
                onOpenFile = onOpenFile,
                previewingRemotePath = previewingRemotePath,
                previewLoading = previewLoading,
                download = download,
                onDownloadFile = onDownloadFile,
                downloadEnabled = !state.busy && phase == ConnectionPhase.CONNECTED,
                modifier = Modifier.weight(1f).fillMaxWidth(),
            )
        }

        ComposerBar(
            draft = draft,
            images = images,
            // An empty session id would send nowhere, so it reads as busy.
            busy = state.busy || sessionId.isEmpty(),
            streaming = activeTurn != null,
            phase = phase,
            model = timeline?.selectedModelOption(stringResource(R.string.models_unnamed)),
            modelOptions = timeline?.modelOptions(stringResource(R.string.models_unnamed)) ?: emptyList(),
            modelCatalogFailed = state.modelCatalogFailure != null,
            capabilities = ChatComposerCapabilities.RemoteChat,
            placeholder = stringResource(R.string.message_input_label),
            onDraftChange = { onIntent(RemoteSessionIntent.UpdateDraft(it)) },
            onRemoveImage = { id -> images = images.filterNot { it.id == id } },
            onOpenModels = { showSettings = true },
            onSelectModel = { modelId ->
                onIntent(RemoteSessionIntent.SelectModel(sessionId, modelId))
            },
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
                images = emptyList()
            },
            onStop = {
                onIntent(RemoteSessionIntent.CancelTurn(sessionId, activeTurn?.turnId))
            },
        )
    }

    if (showSettings) {
        AdaptiveModalSurface(
            visible = true,
            placement = settingsPlacement,
            onDismissRequest = { showSettings = false },
        ) { surfaceModifier ->
            RemoteSettingsSheet(
                state = state,
                sessionId = sessionId,
                onIntent = onIntent,
                modifier = surfaceModifier.padding(16.dp),
            )
        }
    }
}

@Composable
private fun ConversationLoadingState(modifier: Modifier) {
    Column(
        modifier = modifier.testTag(CONVERSATION_LOADING_TEST_TAG),
        horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        CircularProgressIndicator()
        Text(text = stringResource(R.string.chat_empty_loading))
    }
}
