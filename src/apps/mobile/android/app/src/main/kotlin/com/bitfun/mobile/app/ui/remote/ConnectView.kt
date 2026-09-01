package com.bitfun.mobile.app.ui.remote

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.bitfun.mobile.app.R
import com.bitfun.mobile.app.ui.common.SignedOutConnectionActions
import com.bitfun.mobile.app.ui.theme.bitFunColors
import com.bitfun.mobile.core.feature.pairing.PairingIntent
import com.bitfun.mobile.core.feature.pairing.PairingUiState
import com.bitfun.mobile.core.feature.pairing.inspectPairingLink
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

internal const val CONNECT_TEST_TAG: String = "connect"
internal const val CONNECT_MANUAL_TEST_TAG: String = "connect-manual"
internal const val CONNECT_PAIRING_CODE_TEST_TAG: String = "connect-pairing-code"
internal const val CONNECT_SUBMIT_TEST_TAG: String = "connect-submit"

/**
 * The connect page, ported from `pages/components/ConnectView.ets`.
 *
 * Scanning is the way in and typing is the fallback, as on HarmonyOS: the link
 * is long, opaque and easy to mistype, so the intro step offers the camera and
 * keeps the fields out of sight until someone asks for them. HarmonyOS draws its
 * own camera preview. Android delegates capture to Play Services, but keeps the
 * matching scan step underneath it so cancellation returns to the same manual
 * fallback and back-navigation structure.
 */
@Composable
internal fun ConnectView(
    state: PairingUiState,
    onSubmit: (PairingIntent.Submit) -> Unit,
    onDismiss: () -> Unit,
    onBack: () -> Unit,
    onOpenAccount: () -> Unit,
    startScanning: Boolean = false,
    onScanStarted: () -> Unit = {},
    modifier: Modifier,
) {
    var manual by rememberSaveable { mutableStateOf(false) }
    var scanning by rememberSaveable { mutableStateOf(startScanning) }
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
            .addOnCanceledListener { scanFailed = true }
        Unit
    }
    LaunchedEffect(startScanning) {
        if (startScanning) {
            scanning = true
            scan()
            onScanStarted()
        }
    }

    Box(modifier = modifier.fillMaxSize().testTag(CONNECT_TEST_TAG)) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(top = 8.dp, bottom = 34.dp),
        ) {
            if (scanning) {
                ScanPairing(
                    scanFailed = scanFailed,
                    connecting = connecting,
                    onBack = { scanning = false },
                    onManual = { manual = true },
                )
            } else {
                IntroPairing(
                    state = state,
                    connecting = connecting,
                    onScan = {
                        scanning = true
                        scan()
                    },
                    onDismiss = onDismiss,
                    onBack = onBack,
                    onOpenAccount = onOpenAccount,
                )
            }
        }
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
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

@Composable
private fun ColumnScope.IntroPairing(
    state: PairingUiState,
    connecting: Boolean,
    onScan: () -> Unit,
    onDismiss: () -> Unit,
    onBack: () -> Unit,
    onOpenAccount: () -> Unit,
) {
    Box {
        Hero()
        Surface(
            onClick = onBack,
            shape = androidx.compose.foundation.shape.CircleShape,
            color = MaterialTheme.colorScheme.surfaceVariant,
            modifier = Modifier
                .align(Alignment.TopStart)
                .padding(start = 28.dp, top = 18.dp)
                .size(48.dp),
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    painterResource(R.drawable.ic_symbol_chevron_left),
                    contentDescription = stringResource(R.string.common_back),
                    tint = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.size(21.dp),
                )
            }
        }
    }
    Column(
        modifier = Modifier
            .weight(1f)
            .fillMaxWidth()
            .offset(y = (-10).dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(15.dp),
    ) {
        ConnectDesktopGlyph()
        Text(
            stringResource(R.string.connect_choose_connection),
            fontSize = 24.sp,
            lineHeight = 30.sp,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
        )
        if (state is PairingUiState.Failed) {
            PairingFailureCard(state, onDismiss)
        }
    }

    SignedOutConnectionActions(
        scanLabel = stringResource(R.string.sidebar_scan_to_connect),
        accountLabel = stringResource(R.string.sidebar_sign_in),
        onScan = onScan,
        onOpenAccount = onOpenAccount,
        modifier = Modifier
            .align(Alignment.CenterHorizontally)
            .padding(bottom = 14.dp)
            .fillMaxWidth(0.82f),
        enabled = !connecting,
        buttonHeight = 58.dp,
        spacing = 12.dp,
        fontSize = 20,
    )
}

@Composable
private fun ColumnScope.ScanPairing(
    scanFailed: Boolean,
    connecting: Boolean,
    onBack: () -> Unit,
    onManual: () -> Unit,
) {
    Box {
        Hero(height = 252.dp)
        Surface(
            onClick = onBack,
            shape = androidx.compose.foundation.shape.CircleShape,
            color = MaterialTheme.colorScheme.surfaceVariant,
            modifier = Modifier.align(Alignment.TopStart).padding(start = 28.dp, top = 18.dp).size(48.dp),
        ) {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    painterResource(R.drawable.ic_symbol_chevron_left),
                    contentDescription = stringResource(R.string.common_back),
                    tint = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.size(21.dp),
                )
            }
        }
    }
    Column(
        modifier = Modifier.weight(1f).fillMaxWidth().offset(y = (-50).dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        CameraFrame()
        Text(
            stringResource(R.string.connect_scan_title),
            fontSize = 24.sp,
            lineHeight = 30.sp,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        if (scanFailed) {
            Centered(stringResource(R.string.connect_scan_failed), fontSize = 13.sp, lineHeight = 18.sp)
        }
    }
    OutlinedButton(
        onClick = onManual,
        enabled = !connecting,
        modifier = Modifier
            .align(Alignment.CenterHorizontally)
            .fillMaxWidth(0.78f)
            .height(58.dp)
            .testTag(CONNECT_MANUAL_TEST_TAG),
        shape = RoundedCornerShape(35.dp),
    ) {
        Text(stringResource(R.string.connect_switch_manual), fontSize = 20.sp, fontWeight = FontWeight.Bold)
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
    modifier: Modifier,
) {
    val canSubmit = url.isNotBlank() && !connecting &&
        (!requiresAccount || ((userId.ifBlank { suggestedUserId }).isNotBlank() && password.isNotBlank()))
    val consumeTouches = remember { MutableInteractionSource() }
    Box(
        modifier = modifier
            .background(MaterialTheme.colorScheme.scrim)
            .clickable(enabled = !connecting, onClick = onBack),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth(0.82f)
                .widthIn(max = 520.dp)
                .clip(RoundedCornerShape(34.dp))
                .background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(34.dp))
                .clickable(
                    interactionSource = consumeTouches,
                    indication = null,
                    onClick = {},
                )
                .padding(start = 28.dp, end = 28.dp, top = 30.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Text(
                stringResource(R.string.connect_manual_title),
                fontSize = 24.sp,
                lineHeight = 30.sp,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                stringResource(R.string.connect_manual_body),
                fontSize = 17.sp,
                lineHeight = 24.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.fillMaxWidth(),
            )
            PairingPillField(
                value = url,
                onValueChange = onUrlChange,
                placeholder = stringResource(R.string.connect_pair_code_placeholder),
                height = 62.dp,
                fontSize = 20.sp,
                keyboardType = KeyboardType.Uri,
                enabled = !connecting,
                testTag = CONNECT_PAIRING_CODE_TEST_TAG,
            )
            if (requiresAccount) {
                PairingPillField(
                    value = userId,
                    onValueChange = onUserIdChange,
                    placeholder = suggestedUserId.ifBlank { stringResource(R.string.pairing_user_label) },
                    height = 56.dp,
                    fontSize = 18.sp,
                    enabled = !connecting,
                )
                PairingPillField(
                    value = password,
                    onValueChange = onPasswordChange,
                    placeholder = stringResource(R.string.pairing_password_label),
                    height = 56.dp,
                    fontSize = 18.sp,
                    keyboardType = KeyboardType.Password,
                    visualTransformation = PasswordVisualTransformation(),
                    enabled = !connecting,
                )
                Text(
                    stringResource(R.string.pairing_password_hint),
                    fontSize = 13.sp,
                    lineHeight = 18.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (state is PairingUiState.Failed) {
                PairingFailureCard(state, onDismiss)
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Button(
                    onClick = onBack,
                    enabled = !connecting,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant,
                        contentColor = MaterialTheme.colorScheme.onSurface,
                    ),
                    shape = RoundedCornerShape(29.dp),
                    modifier = Modifier.weight(1f).height(58.dp).testTag(CONNECT_SUBMIT_TEST_TAG),
                ) {
                    Text(stringResource(R.string.common_cancel), fontSize = 19.sp, fontWeight = FontWeight.Bold)
                }
                Button(
                    onClick = onSubmit,
                    enabled = canSubmit,
                    shape = RoundedCornerShape(29.dp),
                    modifier = Modifier.weight(1f).height(58.dp),
                ) {
                    if (connecting) {
                        CircularProgressIndicator(modifier = Modifier.padding(end = 8.dp))
                    }
                    Text(stringResource(R.string.connect_pair), fontSize = 19.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}

@Composable
private fun PairingPillField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    height: Dp,
    fontSize: androidx.compose.ui.unit.TextUnit,
    enabled: Boolean,
    keyboardType: KeyboardType = KeyboardType.Text,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    testTag: String? = null,
) {
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        enabled = enabled,
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
        visualTransformation = visualTransformation,
        textStyle = MaterialTheme.typography.bodyLarge.copy(
            fontSize = fontSize,
            color = MaterialTheme.colorScheme.onSurface,
        ),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.onSurface),
        modifier = Modifier
            .fillMaxWidth()
            .height(height)
            .clip(RoundedCornerShape(height / 2))
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(horizontal = 20.dp)
            .then(testTag?.let { Modifier.testTag(it) } ?: Modifier),
        decorationBox = { field ->
            Box(contentAlignment = Alignment.CenterStart) {
                if (value.isEmpty()) {
                    Text(
                        placeholder,
                        fontSize = fontSize,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                field()
            }
        },
    )
}

/**
 * The wash behind the heading, from `ConnectView.ets#HeroWash`.
 *
 * Three translucent blobs over a tinted 282dp band. The offsets are the source's
 * own; they are decoration, so they are placed rather than laid out — a blob
 * that reflowed with the text would stop being a wash.
 */
@Composable
private fun Hero(height: Dp = 282.dp) {
    val extras = bitFunColors
    Surface(
        color = extras.heroBackground,
        shape = RoundedCornerShape(36.dp),
        modifier = Modifier.fillMaxWidth().height(height),
    ) {
        Box {
            Blob(extras.heroSurface, 0.70f, 260.dp, 142.dp, 112.dp, 26.dp)
            Blob(extras.heroAccent, 0.42f, 188.dp, 134.dp, (-42).dp, 198.dp)
            Blob(extras.heroSecondary, 0.54f, 188.dp, 126.dp, 258.dp, 0.dp)
        }
    }
}

@Composable
private fun CameraFrame() {
    val accent = MaterialTheme.colorScheme.primary
    Box(
        modifier = Modifier
            .size(282.dp)
            .clip(RoundedCornerShape(40.dp))
            .background(bitFunColors.shadowMedium),
    ) {
        ScanCorner(accent, Alignment.TopStart, true, true)
        ScanCorner(accent, Alignment.TopEnd, false, true)
        ScanCorner(accent, Alignment.BottomStart, true, false)
        ScanCorner(accent, Alignment.BottomEnd, false, false)
    }
}

@Composable
private fun BoxScope.ScanCorner(color: Color, alignment: Alignment, left: Boolean, top: Boolean) {
    Box(
        Modifier
            .align(alignment)
            .padding(
                start = if (left) 32.dp else 0.dp,
                end = if (left) 0.dp else 32.dp,
                top = if (top) 32.dp else 0.dp,
                bottom = if (top) 0.dp else 32.dp,
            )
            .size(64.dp),
    ) {
        Box(
            Modifier
                .align(if (top) Alignment.TopCenter else Alignment.BottomCenter)
                .fillMaxWidth()
                .height(4.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(color),
        )
        Box(
            Modifier
                .align(if (left) Alignment.CenterStart else Alignment.CenterEnd)
                .width(4.dp)
                .height(64.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(color),
        )
    }
}

/** The two bordered rectangles used by Harmony's `DesktopGlyph()`. */
@Composable
private fun ConnectDesktopGlyph() {
    Box(Modifier.size(68.dp, 55.dp)) {
        Box(
            Modifier
                .offset(x = 5.dp)
                .size(58.dp, 39.dp)
                .border(5.dp, MaterialTheme.colorScheme.onSurface, RoundedCornerShape(8.dp)),
        )
        Box(
            Modifier
                .offset(x = 21.dp, y = 38.dp)
                .size(26.dp, 13.dp)
                .border(5.dp, MaterialTheme.colorScheme.onSurface),
        )
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
private fun Centered(text: String, fontSize: androidx.compose.ui.unit.TextUnit = 14.sp, lineHeight: androidx.compose.ui.unit.TextUnit = 21.sp) {
    Text(
        text,
        fontSize = fontSize,
        lineHeight = lineHeight,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth(0.84f),
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
