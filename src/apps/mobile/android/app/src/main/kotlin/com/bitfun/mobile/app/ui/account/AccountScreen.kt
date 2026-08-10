package com.bitfun.mobile.app.ui.account

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.chat.ConversationView
import com.bitfun.mobile.app.ui.remote.CreateSessionRoute
import com.bitfun.mobile.app.ui.remote.RemoteSessionListContent
import com.bitfun.mobile.app.ui.remote.RemoteWorkspacePanel
import com.bitfun.mobile.app.ui.remote.previewLoading
import com.bitfun.mobile.app.ui.remote.previewingRemotePath
import com.bitfun.mobile.app.viewmodel.AccountViewModel
import com.bitfun.mobile.core.feature.account.AccountFailureReason
import com.bitfun.mobile.core.feature.account.AccountIntent
import com.bitfun.mobile.core.feature.account.AccountUiState
import com.bitfun.mobile.core.feature.connection.ConnectionPhase
import com.bitfun.mobile.core.feature.session.ConversationHeaderPresenter
import com.bitfun.mobile.core.feature.session.RemoteSessionUiState
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceIntent
import com.bitfun.mobile.core.feature.workspace.RemoteWorkspaceUiState

@Composable
internal fun AccountScreen(
    modifier: Modifier,
    viewModel: AccountViewModel = viewModel(factory = AccountViewModel.Factory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val remoteState by viewModel.remoteState.collectAsStateWithLifecycle()
    val workspaceState by viewModel.workspaceState.collectAsStateWithLifecycle()
    var relayUrl by rememberSaveable { mutableStateOf("https://remote.openbitfun.com/relay") }
    var username by rememberSaveable { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var openSessionId by rememberSaveable { mutableStateOf<String?>(null) }
    var creating by rememberSaveable { mutableStateOf(false) }

    // An open session takes the whole surface: its transcript scrolls itself, so
    // it cannot live inside the account form's scroll.
    val conversation = (remoteState as? RemoteSessionUiState.Ready)?.takeIf {
        openSessionId != null && it.selectedSessionId == openSessionId && it.timeline != null
    }
    if (conversation != null) {
        ConversationView(
            state = conversation,
            // A session store only exists here once a device is selected and its
            // link is up, so reaching this branch is itself the connected state.
            phase = ConnectionPhase.CONNECTED,
            onBack = { openSessionId = null },
            onIntent = viewModel::dispatchSession,
            // The device being driven is the whole point of this screen, so the
            // header says its name rather than the workspace's branch.
            contextTitle = ConversationHeaderPresenter.contextTitle(
                desktopName = (state as? AccountUiState.Ready)?.selectedDeviceName.orEmpty(),
                workspaceBranch = (workspaceState as? RemoteWorkspaceUiState.Ready)
                    ?.selected?.gitBranch.orEmpty(),
            ),
            onOpenFile = { path, label ->
                viewModel.dispatchWorkspace(
                    RemoteWorkspaceIntent.OpenFile(path, label, conversation.selectedSessionId.orEmpty()),
                )
            },
            previewingRemotePath = workspaceState.previewingRemotePath(),
            previewLoading = workspaceState.previewLoading(),
            modifier = modifier,
        )
        return
    }

    // Same reason as the conversation above: the create screen pins a composer
    // to the bottom, which cannot live inside the account form's own scroll.
    if (creating) {
        CreateSessionRoute(
            sessionState = remoteState,
            workspaceState = workspaceState,
            phase = ConnectionPhase.CONNECTED,
            deviceId = (state as? AccountUiState.Ready)?.selectedDeviceId.orEmpty(),
            onBack = { creating = false },
            onCreated = {
                creating = false
                openSessionId = it
            },
            onWorkspaceIntent = viewModel::dispatchWorkspace,
            onIntent = viewModel::dispatchSession,
            modifier = modifier,
        )
        return
    }

    Column(
        modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(stringResource(R.string.account_title), style = MaterialTheme.typography.headlineSmall)
        when (val current = state) {
            AccountUiState.Idle, AccountUiState.Restoring, AccountUiState.SigningIn -> CircularProgressIndicator()
            AccountUiState.SignedOut, is AccountUiState.Failed -> {
                if (current is AccountUiState.Failed) {
                    Text(stringResource(current.reason.messageRes()), color = MaterialTheme.colorScheme.error)
                }
                val canSubmit = username.isNotBlank() && password.isNotEmpty()
                fun submit() {
                    viewModel.dispatch(AccountIntent.Login(relayUrl, username, password))
                    password = ""
                }
                // All three are single-line, as the source's `TextInput`s are.
                // Not cosmetic: a multi-line field takes the return key for a
                // newline, so a credential can carry whitespace the relay will
                // reject and the user cannot see.
                OutlinedTextField(
                    value = relayUrl,
                    onValueChange = { relayUrl = it },
                    label = { Text(stringResource(R.string.account_relay_url)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Next,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = username,
                    onValueChange = { username = it },
                    label = { Text(stringResource(R.string.account_username)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text(stringResource(R.string.account_password)) },
                    visualTransformation = PasswordVisualTransformation(),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done,
                    ),
                    // The keyboard's own key does what the button does, and is
                    // just as unable to send an empty credential.
                    keyboardActions = KeyboardActions(onDone = { if (canSubmit) submit() }),
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(
                    onClick = ::submit,
                    enabled = canSubmit,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.account_sign_in)) }
            }
            is AccountUiState.Ready -> {
                AccountIdentityCard(userId = current.userId, modifier = Modifier)
                AccountDetailsCard(userId = current.userId, modifier = Modifier)
                AccountDeviceSection(
                    state = current,
                    onRefresh = { viewModel.dispatch(AccountIntent.RefreshDevices) },
                    onSelect = viewModel::selectDevice,
                )
                TextButton(onClick = { viewModel.dispatch(AccountIntent.Logout) }) {
                    Text(stringResource(R.string.account_sign_out))
                }
                if (current.selectedDeviceId != null) {
                    RemoteSessionListContent(
                        state = remoteState,
                        workspaceState = workspaceState,
                        onIntent = viewModel::dispatchSession,
                        onOpen = { openSessionId = it },
                        onCreate = { creating = true },
                    )
                    RemoteWorkspacePanel(
                        workspaceState,
                        (remoteState as? RemoteSessionUiState.Ready)?.selectedSessionId.orEmpty(),
                        viewModel::dispatchWorkspace,
                    )
                }
            }
        }
    }
}

/**
 * The account's devices, as `ConnectAccountDevicePage.ets` lists them: a header
 * carrying the one action the list has, then a row per device that says where it
 * is before it says whether you can go there.
 *
 * The store has already dropped this phone and the user's other phones, so every
 * row here is something that could in principle be driven — an offline one is a
 * desktop that is not running, which is worth saying and is not the same as an
 * empty list.
 */
@Composable
private fun AccountDeviceSection(
    state: AccountUiState.Ready,
    onRefresh: () -> Unit,
    onSelect: (String) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            stringResource(R.string.account_devices_title),
            style = MaterialTheme.typography.titleSmall,
            modifier = Modifier.weight(1f),
        )
        // Presence is a snapshot, so the list is stale as soon as it lands and
        // the only honest thing to offer is a way to ask again. The same button
        // is the retry after a failure, because it is the same request.
        TextButton(onClick = onRefresh, enabled = !state.refreshing) {
            Text(
                stringResource(
                    when {
                        state.refreshing -> R.string.account_devices_loading
                        state.refreshFailure != null -> R.string.account_devices_retry
                        else -> R.string.account_devices_refresh
                    },
                ),
            )
        }
    }
    state.refreshFailure?.let { reason ->
        Text(
            stringResource(reason.messageRes()),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
        )
    }
    if (state.devices.isEmpty()) {
        Text(
            stringResource(R.string.account_devices_empty),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    state.devices.forEach { device ->
        val selected = device.id == state.selectedDeviceId
        // Deliberately not `&& !selected`, matching `canSelectAccountDevice`:
        // tapping the device already being controlled re-binds it, which is the
        // only retry the row offers once its first load failed.
        val selectable = device.online
        val presence = stringResource(
            if (device.online) R.string.account_online else R.string.account_offline,
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(enabled = selectable) { onSelect(device.id) }
                .padding(vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                device.name.ifBlank { device.id },
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                // An offline row is readable but plainly not a destination.
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = if (device.online) 1f else 0.62f),
            )
            Text(
                if (selected) {
                    stringResource(R.string.account_device_controlling, presence)
                } else {
                    presence
                },
                style = MaterialTheme.typography.bodySmall,
                color = if (device.online) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
            )
        }
    }
}

private fun AccountFailureReason.messageRes(): Int = when (this) {
    AccountFailureReason.INVALID_CREDENTIALS -> R.string.account_invalid_credentials
    AccountFailureReason.AUTHENTICATION -> R.string.account_authentication
    AccountFailureReason.RATE_LIMITED -> R.string.account_rate_limited
    AccountFailureReason.RELAY_UNAVAILABLE -> R.string.account_relay_unavailable
    AccountFailureReason.NETWORK -> R.string.account_network
    AccountFailureReason.TIMEOUT -> R.string.account_timeout
    AccountFailureReason.MALFORMED_RESPONSE -> R.string.account_malformed_response
    AccountFailureReason.SECURE_STORAGE -> R.string.account_secure_storage
}
