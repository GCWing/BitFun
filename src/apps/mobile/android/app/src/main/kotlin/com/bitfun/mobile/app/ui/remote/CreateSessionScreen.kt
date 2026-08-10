package com.bitfun.mobile.app.ui.remote

import android.app.Activity
import android.content.Intent
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.DrawableRes
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusManager
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.chat.ComposerBar
import com.bitfun.mobile.app.ui.common.CircleControl
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.bitfun.mobile.core.feature.session.CreateSessionPresenter
import com.bitfun.mobile.core.feature.session.RemoteSessionIntent
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

internal const val CREATE_SESSION_TEST_TAG: String = "create-session"
internal const val CREATE_SESSION_BACK_TEST_TAG: String = "create-session-back"
internal const val CREATE_SESSION_WORKSPACE_TEST_TAG: String = "create-session-workspace"

/**
 * [CreateSessionScreen] plus the one thing it cannot decide for itself: when the
 * session it asked for exists.
 *
 * The store answers a successful create by selecting the new session, so the
 * signal is that the selection changed from whatever it was when this screen
 * opened. Both hosts route on it identically, which is why it lives here rather
 * than twice in the screens that mount this.
 */
@Composable
internal fun CreateSessionRoute(
    sessionState: RemoteSessionUiState,
    workspaceState: RemoteWorkspaceUiState,
    phase: ConnectionPhase,
    deviceId: String,
    onBack: () -> Unit,
    onCreated: (String) -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    onIntent: (RemoteSessionIntent) -> Unit,
    modifier: Modifier,
) {
    val ready = sessionState as? RemoteSessionUiState.Ready
    val baseline = rememberSaveable { mutableStateOf(ready?.selectedSessionId) }
    val created = ready?.selectedSessionId
    val hasTimeline = ready?.timeline != null
    LaunchedEffect(created, hasTimeline) {
        if (created != null && created != baseline.value && hasTimeline) onCreated(created)
    }

    CreateSessionScreen(
        workspaceState = workspaceState,
        phase = phase,
        deviceId = deviceId,
        // Anything other than a settled list means the store is mid-request or
        // has nothing to create against, and either way the send would be lost.
        busy = ready?.busy ?: true,
        onBack = onBack,
        onWorkspaceIntent = onWorkspaceIntent,
        onIntent = onIntent,
        modifier = modifier,
    )
}

/**
 * The new-session screen, ported from `pages/components/RemoteCreateSessionView.ets`.
 *
 * It is deliberately almost empty: a way back, one line saying where the work
 * will happen, and the composer. The source has no title field and no agent
 * picker on this route because both are inferred — the agent follows from the
 * workspace ([CreateSessionPresenter.agentType]) and the title is whatever the
 * agent renames the session to after reading the first message.
 *
 * The pencil menu on the session list stays as it is: that is a different,
 * faster act — start a Code or Cowork session here in this project — and the
 * source keeps both too.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CreateSessionScreen(
    workspaceState: RemoteWorkspaceUiState,
    phase: ConnectionPhase,
    /** The desktop this would run on. Empty means there is nothing to create on. */
    deviceId: String,
    busy: Boolean,
    onBack: () -> Unit,
    onWorkspaceIntent: (RemoteWorkspaceIntent) -> Unit,
    onIntent: (RemoteSessionIntent) -> Unit,
    modifier: Modifier,
) {
    var draft by rememberSaveable { mutableStateOf("") }
    var workspacePath by rememberSaveable { mutableStateOf("") }
    // Not saveable: an open sheet is a finger part-way through a gesture.
    var pickerOpen by remember { mutableStateOf(false) }
    val focusManager = LocalFocusManager.current
    val ready = workspaceState as? RemoteWorkspaceUiState.Ready

    val voiceInput = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            val text = result.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull().orEmpty()
            if (text.isNotBlank()) {
                draft = listOf(draft.trim(), text.trim()).filter(String::isNotEmpty).joinToString(" ")
            }
        }
    }

    Column(modifier = modifier.fillMaxSize().testTag(CREATE_SESSION_TEST_TAG)) {
        Row(
            modifier = Modifier.fillMaxWidth().height(78.dp).padding(start = 18.dp, top = 14.dp),
            verticalAlignment = Alignment.Top,
        ) {
            CircleControl(
                icon = R.drawable.ic_symbol_chevron_left,
                glyphSize = 20,
                contentDescription = stringResource(R.string.create_back),
                onClick = onBack,
                modifier = Modifier.testTag(CREATE_SESSION_BACK_TEST_TAG),
            )
        }

        // The empty middle is load-bearing in the source: it is what the user
        // taps to put the keyboard away without leaving the screen.
        DismissFiller(focusManager = focusManager, modifier = Modifier.weight(1f))

        if (deviceId.isEmpty()) {
            Text(
                stringResource(R.string.create_no_device),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 28.dp, vertical = 4.dp),
            )
        }

        ContextRow(
            glyph = if (workspacePath.isEmpty()) {
                R.drawable.ic_symbol_message
            } else {
                R.drawable.ic_symbol_folder
            },
            label = when {
                workspaceState is RemoteWorkspaceUiState.Loading -> stringResource(R.string.sessions_loading)
                workspacePath.isEmpty() -> stringResource(R.string.create_chat)
                else -> ready?.workspaces?.firstOrNull { it.path == workspacePath }?.name.orEmpty()
                    .ifEmpty { workspacePath }
            },
            enabled = !busy,
            onClick = {
                focusManager.clearFocus()
                pickerOpen = true
            },
            modifier = Modifier.testTag(CREATE_SESSION_WORKSPACE_TEST_TAG),
        )

        ComposerBar(
            draft = draft,
            images = emptyList(),
            busy = busy,
            streaming = false,
            // No device means the remote genuinely is not reachable, so the send
            // dims itself the way it does during a dropout. The field stays
            // typable on purpose: the draft is worth keeping until a desktop
            // comes back, and the source blocks only the send too.
            phase = if (deviceId.isEmpty()) ConnectionPhase.DISCONNECTED else phase,
            // There is no session yet, so there is no per-session model to swap.
            model = null,
            capabilities = ChatComposerCapabilities.RemoteCreate,
            placeholder = stringResource(R.string.create_placeholder),
            onDraftChange = { draft = it },
            onRemoveImage = {},
            onAttach = {},
            onOpenModels = {},
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
                if (CreateSessionPresenter.canSubmit(draft, deviceId, busy)) {
                    onIntent(
                        RemoteSessionIntent.CreateSession(
                            agentType = CreateSessionPresenter.agentType(workspacePath),
                            title = "",
                            instruction = draft,
                            modelId = null,
                        ),
                    )
                    draft = ""
                }
            },
            onStop = {},
            modifier = Modifier,
        )
    }

    if (pickerOpen) {
        ModalBottomSheet(onDismissRequest = { pickerOpen = false }) {
            WorkspacePicker(
                workspaces = ready?.workspaces.orEmpty().map {
                    WorkspaceChoice(path = it.path, name = it.name)
                },
                selectedPath = workspacePath,
                onPick = { path ->
                    pickerOpen = false
                    workspacePath = path
                    // Applied now rather than at send: `set_workspace` is a round
                    // trip to the desktop, and doing it here means the row can
                    // show what the desktop actually settled on while the user is
                    // still typing. The source binds the same two workspaces —
                    // a project for code, the assistant's own for chat.
                    if (path.isEmpty()) {
                        if (ready?.selected?.kind != ASSISTANT_KIND) {
                            ready?.assistants?.firstOrNull()?.let {
                                onWorkspaceIntent(RemoteWorkspaceIntent.SelectAssistant(it.path))
                            }
                        }
                    } else {
                        onWorkspaceIntent(RemoteWorkspaceIntent.SelectWorkspace(path))
                    }
                },
                modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp),
            )
        }
    }
}

/** What the desktop calls the workspace it keeps its chat sessions in. */
private const val ASSISTANT_KIND = "assistant"

/**
 * One row of the picker.
 *
 * The workspace domain type carries a kind and a timestamp the picker has no use
 * for, and the app layer cannot see `core-domain` anyway.
 */
private data class WorkspaceChoice(val path: String, val name: String)

/**
 * The tap target that puts the keyboard away.
 *
 * No ripple and no role: it is empty space, and an indication here would read as
 * a control the user had missed.
 */
@Composable
private fun DismissFiller(focusManager: FocusManager, modifier: Modifier) {
    val interaction = remember { MutableInteractionSource() }
    Box(
        modifier = modifier
            .fillMaxWidth()
            .clickable(
                interactionSource = interaction,
                indication = null,
            ) { focusManager.clearFocus() },
    )
}

/**
 * One line saying where the session will run, as the source's `ContextRow` is:
 * a glyph for the kind of place, its name, and a chevron because it opens.
 */
@Composable
private fun ContextRow(
    @DrawableRes glyph: Int,
    label: String,
    enabled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(48.dp)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 28.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(
            painterResource(glyph),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(18.dp),
        )
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        Icon(
            painterResource(R.drawable.ic_symbol_chevron_right),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(14.dp),
        )
    }
}

/**
 * Where the session will run: chat, or one of the desktop's recent workspaces.
 *
 * Chat leads because it is the one option that needs no project, and because
 * choosing it is how the user says "just talk to me" rather than "work here".
 */
@Composable
private fun WorkspacePicker(
    workspaces: List<WorkspaceChoice>,
    selectedPath: String,
    onPick: (String) -> Unit,
    modifier: Modifier,
) {
    Column(modifier = modifier) {
        Text(
            stringResource(R.string.create_workspace_picker),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
        )
        PickerRow(
            title = stringResource(R.string.create_chat),
            subtitle = "",
            selected = selectedPath.isEmpty(),
            onClick = { onPick("") },
        )
        if (workspaces.isEmpty()) {
            Text(
                stringResource(R.string.create_no_workspaces),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp),
            )
            return@Column
        }
        workspaces.forEach { workspace ->
            PickerRow(
                title = workspace.name.ifBlank { workspace.path },
                // Two projects can share a name; the path is what tells them
                // apart, and it is the only place the user can check.
                subtitle = workspace.path,
                selected = workspace.path == selectedPath,
                onClick = { onPick(workspace.path) },
            )
        }
    }
}

@Composable
private fun PickerRow(
    title: String,
    subtitle: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle.isNotEmpty()) {
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (selected) {
            Icon(
                painterResource(R.drawable.ic_symbol_checkmark_circle),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(19.dp),
            )
        }
    }
}
