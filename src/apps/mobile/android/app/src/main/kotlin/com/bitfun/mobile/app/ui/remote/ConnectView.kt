package com.bitfun.mobile.app.ui.remote

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.theme.bitFunColors
import com.bitfun.mobile.core.feature.pairing.PairingIntent
import com.bitfun.mobile.core.feature.pairing.PairingUiState
import com.bitfun.mobile.core.feature.pairing.inspectPairingLink
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

internal const val CONNECT_TEST_TAG: String = "connect"
internal const val CONNECT_MANUAL_TEST_TAG: String = "connect-manual"

/**
 * The connect page, ported from `pages/components/ConnectView.ets`.
 *
 * Scanning is the way in and typing is the fallback, as on HarmonyOS: the link
 * is long, opaque and easy to mistype, so the intro step offers the camera and
 * keeps the fields out of sight until someone asks for them. HarmonyOS draws its
 * own camera preview; here the scan is Play Services' full-screen one, so its
 * step is the system's rather than a page of ours.
 */
@Composable
internal fun ConnectView(
    state: PairingUiState,
    onSubmit: (PairingIntent.Submit) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier,
) {
    var manual by rememberSaveable { mutableStateOf(false) }
    var url by rememberSaveable { mutableStateOf("") }
    var userId by rememberSaveable { mutableStateOf("") }
    // Never rememberSaveable: a password must not reach saved instance state.
    var password by remember { mutableStateOf("") }
    var scanFailed by rememberSaveable { mutableStateOf(false) }

    // Ports `ConnectionErrorResult.shouldShowRemoteUrlInput`: a link that is
    // itself at fault puts the field back on screen, because the scan button
    // that got the user here cannot fix a link that has expired. Keyed on the
    // state rather than run on every recomposition, so tapping back out of the
    // form while the failure is still showing stays out.
    LaunchedEffect(state) {
        if (state is PairingUiState.Failed && state.failure.reopensLinkInput) manual = true
    }

    val hints = remember(url) { inspectPairingLink(url) }
    val connecting = state is PairingUiState.Connecting
    val context = LocalContext.current
    val scanner = remember(context) {
        GmsBarcodeScanning.getClient(
            context,
            GmsBarcodeScannerOptions.Builder()
                .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
                .enableAutoZoom()
                .build(),
        )
    }
    val scan = {
        scanFailed = false
        scanner.startScan()
            .addOnSuccessListener { barcode ->
                val scanned = barcode.rawValue.orEmpty().trim()
                if (scanned.isNotEmpty()) {
                    url = scanned
                    // A room that wants an account cannot be entered from the
                    // code alone, so the scan hands over to the form instead of
                    // failing a connect the user did not know needed a password.
                    if (inspectPairingLink(scanned).requiresAccount) {
                        manual = true
                    } else {
                        onSubmit(PairingIntent.Submit(scanned, userId, ""))
                    }
                }
            }
            .addOnFailureListener { scanFailed = true }
        Unit
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .testTag(CONNECT_TEST_TAG),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (manual) {
            ManualPairing(
                state = state,
                url = url,
                userId = userId,
                password = password,
                requiresAccount = hints.requiresAccount,
                suggestedUserId = hints.suggestedUserId,
                connecting = connecting,
                onUrlChange = { url = it },
                onUserIdChange = { userId = it },
                onPasswordChange = { password = it },
                onBack = { manual = false },
                onDismiss = onDismiss,
                onSubmit = { onSubmit(PairingIntent.Submit(url, userId, password)) },
            )
        } else {
            IntroPairing(
                state = state,
                scanFailed = scanFailed,
                connecting = connecting,
                onScan = scan,
                onManual = { manual = true },
                onDismiss = onDismiss,
            )
        }
    }
}

@Composable
private fun IntroPairing(
    state: PairingUiState,
    scanFailed: Boolean,
    connecting: Boolean,
    onScan: () -> Unit,
    onManual: () -> Unit,
    onDismiss: () -> Unit,
) {
    Hero()
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            stringResource(R.string.connect_title),
            style = MaterialTheme.typography.headlineSmall,
            textAlign = TextAlign.Center,
        )
        Centered(stringResource(R.string.connect_body))
        Centered(stringResource(R.string.connect_steps))

        if (state is PairingUiState.Failed) {
            PairingFailureCard(state, onDismiss)
        }
        if (scanFailed) {
            Centered(stringResource(R.string.connect_scan_failed))
        }

        Button(
            onClick = onScan,
            enabled = !connecting,
            modifier = Modifier.fillMaxWidth(),
        ) {
            if (connecting) {
                CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp))
                Text(stringResource(R.string.pairing_connecting))
            } else {
                Text(stringResource(R.string.connect_scan_action))
            }
        }
        OutlinedButton(
            onClick = onManual,
            enabled = !connecting,
            modifier = Modifier.fillMaxWidth().testTag(CONNECT_MANUAL_TEST_TAG),
        ) {
            Text(stringResource(R.string.connect_switch_manual))
        }
    }
}

@Composable
private fun ManualPairing(
    state: PairingUiState,
    url: String,
    userId: String,
    password: String,
    requiresAccount: Boolean,
    suggestedUserId: String,
    connecting: Boolean,
    onUrlChange: (String) -> Unit,
    onUserIdChange: (String) -> Unit,
    onPasswordChange: (String) -> Unit,
    onBack: () -> Unit,
    onDismiss: () -> Unit,
    onSubmit: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TextButton(onClick = onBack, enabled = !connecting) {
            Icon(
                painterResource(R.drawable.ic_symbol_chevron_left),
                contentDescription = null,
                modifier = Modifier.size(18.dp).padding(end = 4.dp),
            )
            Text(stringResource(R.string.connect_back))
        }
        Text(stringResource(R.string.pairing_title), style = MaterialTheme.typography.headlineSmall)
        Text(
            stringResource(R.string.pairing_subtitle),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        OutlinedTextField(
            value = url,
            onValueChange = onUrlChange,
            label = { Text(stringResource(R.string.pairing_url_label)) },
            singleLine = false,
            enabled = !connecting,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = userId,
            onValueChange = onUserIdChange,
            label = { Text(stringResource(R.string.pairing_user_label)) },
            placeholder = { Text(suggestedUserId) },
            singleLine = true,
            enabled = !connecting,
            modifier = Modifier.fillMaxWidth(),
        )
        if (requiresAccount) {
            OutlinedTextField(
                value = password,
                onValueChange = onPasswordChange,
                label = { Text(stringResource(R.string.pairing_password_label)) },
                supportingText = { Text(stringResource(R.string.pairing_password_hint)) },
                singleLine = true,
                enabled = !connecting,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                modifier = Modifier.fillMaxWidth(),
            )
        }

        if (state is PairingUiState.Failed) {
            PairingFailureCard(state, onDismiss)
        }

        Button(
            onClick = onSubmit,
            enabled = state.acceptsSubmit,
            modifier = Modifier.fillMaxWidth(),
        ) {
            if (connecting) {
                CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp))
                Text(stringResource(R.string.pairing_connecting))
            } else {
                Text(stringResource(R.string.pairing_connect))
            }
        }
    }
}

/**
 * The wash behind the heading, from `ConnectView.ets#HeroWash`.
 *
 * Three translucent blobs over a tinted band. The offsets are the source's own,
 * scaled to this shorter band; they are decoration, so they are placed rather
 * than laid out — a blob that reflowed with the text would stop being a wash.
 */
@Composable
private fun Hero() {
    val extras = bitFunColors
    Surface(
        color = extras.heroBackground,
        shape = RoundedCornerShape(bottomStart = 36.dp, bottomEnd = 36.dp),
        modifier = Modifier.fillMaxWidth().height(180.dp),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Blob(extras.heroSurface, 0.70f, 260.dp, 142.dp, 112.dp, 16.dp)
            Blob(extras.heroAccent, 0.42f, 188.dp, 134.dp, (-42).dp, 126.dp)
            Blob(extras.heroSecondary, 0.54f, 188.dp, 126.dp, 258.dp, 0.dp)
            Text(
                stringResource(R.string.pairing_title),
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun BoxScope.Blob(color: Color, alpha: Float, width: Dp, height: Dp, x: Dp, y: Dp) {
    Box(
        Modifier
            .align(Alignment.TopStart)
            .offset(x = x, y = y)
            .size(width, height)
            .clip(RoundedCornerShape(percent = 50))
            .background(color.copy(alpha = alpha)),
    )
}

@Composable
private fun Centered(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun PairingFailureCard(state: PairingUiState.Failed, onDismiss: () -> Unit) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                state.failure.message(),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
            )
            // The desktop's own wording, shown under our heading rather than
            // instead of it: it is written by the peer and is not localized.
            state.failure.remoteMessage?.let {
                Text(it, style = MaterialTheme.typography.bodySmall)
            }
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.pairing_dismiss))
            }
        }
    }
}
