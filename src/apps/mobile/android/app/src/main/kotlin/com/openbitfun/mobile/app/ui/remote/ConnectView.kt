package com.openbitfun.mobile.app.ui.remote

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.Canvas
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
import com.openbitfun.mobile.app.infrastructure.camera.InlineQrScanner
import com.openbitfun.mobile.app.ui.common.ConnectionSheetHeader
import com.openbitfun.mobile.app.ui.common.ConnectionSheetFooter
import com.openbitfun.mobile.app.ui.common.connectionSheetTextStyle
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.common.SignedOutConnectionActions
import com.openbitfun.mobile.app.ui.theme.openBitFunColors

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
 * own camera preview; Android uses a lifecycle-bound inline camera adapter.
 */
@Composable
internal fun ConnectView(
    onSubmit: (String) -> Unit,
    onBack: () -> Unit,
    onOpenAccount: () -> Unit,
    startScanning: Boolean = false,
    onScanStarted: () -> Unit = {},
    modifier: Modifier,
) {
    var manual by rememberSaveable { mutableStateOf(false) }
    var scanning by rememberSaveable { mutableStateOf(startScanning) }
    var url by rememberSaveable { mutableStateOf("") }
    var scanError by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(startScanning) {
        if (startScanning) { scanning = true; onScanStarted() }
    }

    BackHandler(enabled = manual || scanning) {
        if (manual) manual = false else if (startScanning) onBack() else scanning = false
    }

    Box(modifier = modifier.fillMaxSize().testTag(CONNECT_TEST_TAG)) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .then(if (!scanning) Modifier.verticalScroll(rememberScrollState()) else Modifier)
                .then(if (!scanning) Modifier.padding(top = 8.dp, bottom = 28.dp) else Modifier),
        ) {
            if (scanning) {
                ScanPairing(
                    scanError = scanError,
                    paused = manual,
                    onDetected = { url = it; onSubmit(it) },
                    onScanError = { scanError = it },
                    connecting = false,
                    onBack = onBack,
                    onManual = { manual = true },
                )
            } else {
                IntroPairing(
                    connecting = false,
                    onScan = {
                        scanning = true
                        scanError = null
                    },
                    onBack = onBack,
                    onOpenAccount = onOpenAccount,
                )
            }
        }
        if (manual) {
            ManualPairing(
                url = url,
                connecting = false,
                onUrlChange = { url = it },
                onBack = { manual = false },
                onSubmit = { onSubmit(url) },
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

@Composable
private fun ColumnScope.IntroPairing(
    connecting: Boolean,
    onScan: () -> Unit,
    onBack: () -> Unit,
    onOpenAccount: () -> Unit,
) {
    Box {
        Hero(height = 250.dp)
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
            .fillMaxWidth()
            .offset(y = (-24).dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        ConnectDesktopGlyph()
        Text(
            stringResource(R.string.sidebar_connect_desktop),
            style = MaterialTheme.typography.displayLarge,
            textAlign = TextAlign.Center,
        )
        SignedOutConnectionActions(
            scanLabel = stringResource(R.string.sidebar_scan_to_connect),
            accountLabel = stringResource(R.string.sidebar_sign_in),
            onScan = onScan,
            onOpenAccount = onOpenAccount,
            modifier = Modifier.fillMaxWidth(0.82f),
            enabled = !connecting,
            buttonHeight = 58.dp,
            spacing = 18.dp,
            fontSize = 16,
            primaryScan = true,
        )
    }
}

@Composable
private fun ColumnScope.ScanPairing(
    scanError: Int?, paused: Boolean,
    onDetected: (String) -> Unit, onScanError: (Int?) -> Unit,
    connecting: Boolean, onBack: () -> Unit, onManual: () -> Unit,
) {
    ConnectionSheetHeader(onBack)
    Box(Modifier.weight(1f).fillMaxWidth().padding(start = 20.dp, end = 20.dp, bottom = 18.dp),
        contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 520.dp).fillMaxWidth().verticalScroll(rememberScrollState()),
            horizontalAlignment = Alignment.CenterHorizontally) {
            Text(stringResource(R.string.connect_scan_title), style = MaterialTheme.typography.displayMedium.connectionSheetTextStyle(),
                textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth())
            Text(stringResource(R.string.connect_scan_body), style = MaterialTheme.typography.bodyMedium.connectionSheetTextStyle(),
                color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 24.dp))
            CameraFrame(paused, onDetected, onScanError)
            Row(Modifier.padding(top = 18.dp).fillMaxWidth().height(30.dp)
                .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(15.dp))
                .padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(8.dp).background(MaterialTheme.colorScheme.outline,
                    androidx.compose.foundation.shape.CircleShape))
                Text(stringResource(R.string.connect_scan_hint), style = MaterialTheme.typography.bodySmall.connectionSheetTextStyle(),
                    color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1,
                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
            }
            if (scanError != null) {
                Text(stringResource(scanError), style = MaterialTheme.typography.bodySmall.connectionSheetTextStyle(),
                    color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 14.dp).fillMaxWidth()
                        .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(16.dp))
                        .padding(horizontal = 14.dp, vertical = 12.dp))
            }
        }
    }
    ConnectionSheetFooter(stringResource(R.string.connect_switch_manual), enabled = !connecting,
        modifier = Modifier.testTag(CONNECT_MANUAL_TEST_TAG), onClick = onManual)
}

@Composable
private fun ManualPairing(
    url: String,
    connecting: Boolean,
    onUrlChange: (String) -> Unit,
    onBack: () -> Unit,
    onSubmit: () -> Unit,
    modifier: Modifier,
) {
    val canSubmit = url.isNotBlank() && !connecting
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
    val extras = openBitFunColors
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
private fun CameraFrame(paused: Boolean, onDetected: (String) -> Unit, onScanError: (Int?) -> Unit) {
    val colors = openBitFunColors
    Box(Modifier.size(248.dp).clip(RoundedCornerShape(28.dp))
        .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(28.dp))) {
        InlineQrScanner(Modifier.fillMaxSize(), paused, onDetected, { onScanError(null) },
            { onScanError(R.string.connect_camera_permission_denied) },
            { onScanError(R.string.connect_camera_unavailable) })
        Canvas(Modifier.fillMaxSize()) {
            drawRect(colors.shadowMedium)
            val inset = 20.dp.toPx()
            val size = 56.dp.toPx()
            val stroke = 4.dp.toPx()
            val length = 36.dp.toPx()
            for (right in listOf(false, true)) for (bottom in listOf(false, true)) {
                val x = if (right) this.size.width - inset - size else inset
                val y = if (bottom) this.size.height - inset - size else inset
                drawRoundRect(colors.scanAccent,
                    androidx.compose.ui.geometry.Offset(x + if (right) size - length else 0f,
                        y + if (bottom) size - stroke else 0f),
                    androidx.compose.ui.geometry.Size(length, stroke), androidx.compose.ui.geometry.CornerRadius(2.dp.toPx()))
                drawRoundRect(colors.scanAccent,
                    androidx.compose.ui.geometry.Offset(x + if (right) size - stroke else 0f,
                        y + if (bottom) size - length else 0f),
                    androidx.compose.ui.geometry.Size(stroke, length), androidx.compose.ui.geometry.CornerRadius(2.dp.toPx()))
            }
        }
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
