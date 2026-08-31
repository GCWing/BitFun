package com.bitfun.mobile.app.ui.account

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.platform.deviceIdentity
import com.bitfun.mobile.app.viewmodel.AccountViewModel
import com.bitfun.mobile.core.feature.account.AccountFailureReason
import com.bitfun.mobile.core.feature.account.AccountIntent
import com.bitfun.mobile.core.feature.account.AccountUiState
import com.bitfun.mobile.app.ui.theme.bitFunColors

private val AccountCardShape = RoundedCornerShape(24.dp)

@Composable
internal fun AccountScreen(
    modifier: Modifier,
    onBack: () -> Unit = {},
    onDeviceSelected: (String) -> Unit = {},
    viewModel: AccountViewModel = viewModel(factory = AccountViewModel.Factory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    when (val current = state) {
        AccountUiState.Idle, AccountUiState.Restoring -> Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        AccountUiState.SigningIn, AccountUiState.SignedOut, is AccountUiState.Failed -> AccountLoginPage(
            state = current,
            onBack = onBack,
            onLogin = { relay, username, password -> viewModel.dispatch(AccountIntent.Login(relay, username, password)) },
            modifier = modifier,
        )
        is AccountUiState.Ready -> AccountProfilePage(
            state = current,
            onBack = onBack,
            onRefresh = { viewModel.dispatch(AccountIntent.RefreshDevices) },
            onSelect = { deviceId -> viewModel.selectDevice(deviceId); onDeviceSelected(deviceId) },
            onLogout = { viewModel.dispatch(AccountIntent.Logout) },
            modifier = modifier,
        )
    }
}

@Composable
private fun AccountLoginPage(
    state: AccountUiState,
    onBack: () -> Unit,
    onLogin: (String, String, String) -> Unit,
    modifier: Modifier,
) {
    var relayUrl by rememberSaveable { mutableStateOf("https://remote.openbitfun.com/relay") }
    var username by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }
    var passwordVisible by rememberSaveable { mutableStateOf(false) }
    val busy = state is AccountUiState.SigningIn
    val canSubmit = relayUrl.isNotBlank() && username.isNotBlank() && password.isNotEmpty() && !busy
    Box(modifier.fillMaxSize()) {
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState())
                .padding(start = 28.dp, end = 28.dp, top = 118.dp, bottom = 44.dp),
        ) {
            Text(stringResource(R.string.account_login_title), fontSize = 32.sp, lineHeight = 38.sp, fontWeight = FontWeight.Bold)
            Text(stringResource(R.string.account_login_body), fontSize = 15.sp, lineHeight = 22.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 12.dp, bottom = 42.dp))
            AccountInput(username, stringResource(R.string.account_username_placeholder), { username = it }, keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next))
            AccountInput(
                password,
                stringResource(R.string.account_password_placeholder),
                { password = it },
                visualTransformation = if (passwordVisible) VisualTransformation.None else PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Next),
                trailing = {
                    IconButton(onClick = { passwordVisible = !passwordVisible }) {
                        Icon(painterResource(if (passwordVisible) R.drawable.ic_symbol_eye else R.drawable.ic_symbol_eye_slash), contentDescription = null, modifier = Modifier.size(24.dp))
                    }
                },
                modifier = Modifier.padding(top = 14.dp),
            )
            Text(stringResource(R.string.account_login_server), fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 4.dp, top = 26.dp, bottom = 8.dp))
            AccountInput(
                relayUrl,
                stringResource(R.string.account_relay_url_placeholder),
                { relayUrl = it },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (canSubmit) onLogin(relayUrl, username, password) }),
                modifier = Modifier.height(52.dp),
            )
            (state as? AccountUiState.Failed)?.let { failure ->
                Text(stringResource(failure.reason.messageRes()), color = MaterialTheme.colorScheme.error, fontSize = 13.sp, lineHeight = 19.sp, modifier = Modifier.padding(top = 12.dp))
            }
            Spacer(Modifier.height(if (state is AccountUiState.Failed) 22.dp else 30.dp))
            Button(
                onClick = { onLogin(relayUrl, username, password) },
                enabled = canSubmit,
                shape = RoundedCornerShape(18.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                    disabledContainerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.28f),
                    disabledContentColor = MaterialTheme.colorScheme.onPrimary,
                ),
                modifier = Modifier.fillMaxWidth().height(56.dp),
            ) { Text(stringResource(if (busy) R.string.account_signing_in else R.string.account_sign_in), fontSize = 17.sp, fontWeight = FontWeight.Bold) }
        }
        AccountBackButton(onBack, Modifier.padding(start = 28.dp, top = 22.dp))
    }
}

@Composable
private fun AccountInput(
    value: String,
    placeholder: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    trailing: (@Composable (() -> Unit))? = null,
) {
    TextField(
        value = value,
        onValueChange = onValueChange,
        placeholder = { Text(placeholder, fontSize = 17.sp) },
        singleLine = true,
        visualTransformation = visualTransformation,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        trailingIcon = trailing,
        colors = TextFieldDefaults.colors(
            focusedContainerColor = MaterialTheme.colorScheme.surface,
            unfocusedContainerColor = MaterialTheme.colorScheme.surface,
            disabledContainerColor = MaterialTheme.colorScheme.surface,
            focusedIndicatorColor = bitFunColors.transparent,
            unfocusedIndicatorColor = bitFunColors.transparent,
            cursorColor = MaterialTheme.colorScheme.onSurface,
            focusedTextColor = MaterialTheme.colorScheme.onSurface,
            unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
            focusedPlaceholderColor = MaterialTheme.colorScheme.onSurfaceVariant,
            unfocusedPlaceholderColor = MaterialTheme.colorScheme.onSurfaceVariant,
        ),
        shape = RoundedCornerShape(18.dp),
        modifier = modifier.fillMaxWidth().height(58.dp),
    )
}

@Composable
private fun AccountProfilePage(
    state: AccountUiState.Ready,
    onBack: () -> Unit,
    onRefresh: () -> Unit,
    onSelect: (String) -> Unit,
    onLogout: () -> Unit,
    modifier: Modifier,
) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val installId = remember(context) { context.deviceIdentity().installId }
    Column(
        modifier = modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(start = 18.dp, end = 18.dp, top = 20.dp, bottom = 42.dp),
    ) {
        Row(Modifier.fillMaxWidth().height(56.dp), verticalAlignment = Alignment.CenterVertically) {
            AccountBackButton(onBack, Modifier)
            Text(stringResource(R.string.account_profile_title), fontSize = 20.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, modifier = Modifier.weight(1f))
            Spacer(Modifier.size(44.dp))
        }
        Spacer(Modifier.height(30.dp))
        Surface(color = MaterialTheme.colorScheme.surface, shape = RoundedCornerShape(28.dp), modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp)) {
            Column(Modifier.fillMaxWidth().padding(top = 24.dp, bottom = 24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
                AccountAvatar(70)
                Text(state.username, fontSize = 22.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(state.userId, fontSize = 14.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.fillMaxWidth(0.88f), textAlign = TextAlign.Center)
            }
        }
        Surface(color = MaterialTheme.colorScheme.surface, shape = AccountCardShape, modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp)) {
            Column(Modifier.padding(horizontal = 18.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.account_bitfun_account), fontSize = 17.sp, fontWeight = FontWeight.Bold)
                    Spacer(Modifier.weight(1f))
                    Text(stringResource(R.string.remote_settings_account_signed_in), fontSize = 14.sp, color = com.bitfun.mobile.app.ui.theme.bitFunColors.statusSuccess)
                }
                Text(stringResource(R.string.account_signed_in_body, state.username), fontSize = 14.sp, lineHeight = 20.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Surface(color = MaterialTheme.colorScheme.surface, shape = AccountCardShape, modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp)) {
            Column(Modifier.padding(horizontal = 18.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.account_devices_title), fontSize = 17.sp, fontWeight = FontWeight.Bold)
                    Spacer(Modifier.weight(1f))
                    Text(stringResource(if (state.refreshing) R.string.account_devices_loading else R.string.account_devices_refresh), fontSize = 13.sp, color = if (state.refreshing) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface, modifier = Modifier.clickable(enabled = !state.refreshing, onClick = onRefresh))
                }
                state.refreshFailure?.let { reason -> Text(stringResource(reason.messageRes()), fontSize = 13.sp, color = MaterialTheme.colorScheme.error) }
                if (state.devices.isEmpty()) {
                    Text(stringResource(R.string.account_devices_empty), fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                } else {
                    state.devices.forEach { device ->
                        val selected = device.id == state.selectedDeviceId
                        val reconnectable = selected && !device.online
                        Row(Modifier.fillMaxWidth().height(54.dp).clickable(enabled = device.online || reconnectable, onClick = { onSelect(device.id) }), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            Icon(painterResource(R.drawable.ic_symbol_desktop), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(24.dp))
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Text(device.name.ifBlank { device.id }, fontSize = 15.sp, fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(
                                    stringResource(
                                        when {
                                            selected && device.online -> R.string.account_device_current_control
                                            device.online -> R.string.account_online
                                            else -> R.string.account_offline
                                        },
                                    ),
                                    fontSize = 13.sp,
                                    color = if (device.online) com.bitfun.mobile.app.ui.theme.bitFunColors.statusSuccess else MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            if (reconnectable) Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = RoundedCornerShape(14.dp)) {
                                Text(stringResource(R.string.remote_settings_reconnect), fontSize = 14.sp, modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp))
                            } else if (device.online && !selected) Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = RoundedCornerShape(14.dp)) {
                                Text(stringResource(R.string.account_connect), fontSize = 14.sp, modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp))
                            }
                        }
                    }
                }
            }
        }
        Text(stringResource(R.string.account_profile_details), fontSize = 18.sp, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 18.dp, bottom = 8.dp))
        Surface(color = MaterialTheme.colorScheme.surface, shape = RoundedCornerShape(28.dp), modifier = Modifier.fillMaxWidth()) {
            Column {
                AccountDetailRow(stringResource(R.string.remote_settings_user_id), state.userId)
                androidx.compose.material3.HorizontalDivider(Modifier.padding(horizontal = 18.dp), color = MaterialTheme.colorScheme.outlineVariant)
                AccountDetailRow(stringResource(R.string.remote_settings_device_id), installId)
            }
        }
        Spacer(Modifier.height(30.dp))
        Surface(onClick = onLogout, color = MaterialTheme.colorScheme.surface, shape = AccountCardShape, modifier = Modifier.fillMaxWidth().height(62.dp)) {
            Row(Modifier.padding(horizontal = 20.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Icon(painterResource(R.drawable.ic_symbol_arrow_right_and_square), contentDescription = null, tint = MaterialTheme.colorScheme.error, modifier = Modifier.size(22.dp))
                Text(stringResource(R.string.account_sign_out), fontSize = 17.sp, fontWeight = FontWeight.Medium, color = MaterialTheme.colorScheme.error)
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}

@Composable
private fun AccountDetailRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, fontSize = 16.sp)
        Text(value, fontSize = 16.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.End, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun AccountAvatar(size: Int) {
    Box(Modifier.size(size.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant), contentAlignment = Alignment.Center) {
        Icon(painterResource(R.drawable.ic_symbol_person), contentDescription = null, modifier = Modifier.size((size * 0.52f).dp))
    }
}

@Composable
private fun AccountBackButton(onClick: () -> Unit, modifier: Modifier) {
    Surface(onClick = onClick, color = MaterialTheme.colorScheme.surface, shape = CircleShape, shadowElevation = 1.dp, modifier = modifier.size(44.dp)) {
        Box(contentAlignment = Alignment.Center) {
            Icon(painterResource(R.drawable.ic_symbol_chevron_left), contentDescription = stringResource(R.string.common_back), modifier = Modifier.size(23.dp))
        }
    }
}

internal fun AccountFailureReason.messageRes(): Int = when (this) {
    AccountFailureReason.INVALID_CREDENTIALS -> R.string.account_invalid_credentials
    AccountFailureReason.AUTHENTICATION -> R.string.account_authentication
    AccountFailureReason.RATE_LIMITED -> R.string.account_rate_limited
    AccountFailureReason.RELAY_UNAVAILABLE -> R.string.account_relay_unavailable
    AccountFailureReason.NETWORK -> R.string.account_network
    AccountFailureReason.TIMEOUT -> R.string.account_timeout
    AccountFailureReason.MALFORMED_RESPONSE -> R.string.account_malformed_response
    AccountFailureReason.SECURE_STORAGE -> R.string.account_secure_storage
}
