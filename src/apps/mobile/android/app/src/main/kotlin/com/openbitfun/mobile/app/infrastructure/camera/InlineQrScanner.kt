package com.openbitfun.mobile.app.infrastructure.camera

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage

/** App-owned camera surface. Its lifetime ends when hidden by manual pairing. */
@androidx.annotation.OptIn(ExperimentalGetImage::class)
@Composable
internal fun InlineQrScanner(
    modifier: Modifier,
    paused: Boolean = false,
    onCode: (String) -> Unit,
    onReady: () -> Unit,
    onPermissionDenied: () -> Unit,
    onUnavailable: () -> Unit,
) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val ready by rememberUpdatedState(onReady)
    val detected by rememberUpdatedState(onCode)
    val denied by rememberUpdatedState(onPermissionDenied)
    val unavailable by rememberUpdatedState(onUnavailable)
    var granted by remember { mutableStateOf(
        ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
    ) }
    DisposableEffect(owner) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) {
                granted = ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
            }
        }
        owner.lifecycle.addObserver(observer)
        onDispose { owner.lifecycle.removeObserver(observer) }
    }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it
        if (!it) denied()
    }
    LaunchedEffect(Unit) { if (!granted) permission.launch(Manifest.permission.CAMERA) }
    val view = remember(context) { PreviewView(context).apply {
        implementationMode = PreviewView.ImplementationMode.COMPATIBLE
        scaleType = PreviewView.ScaleType.FILL_CENTER
    } }
    AndroidView(factory = { view }, modifier = modifier)
    if (granted && !paused) {
        DisposableEffect(view, owner) {
            val executor = ContextCompat.getMainExecutor(context)
            val future = ProcessCameraProvider.getInstance(context)
            val scanner = BarcodeScanning.getClient(BarcodeScannerOptions.Builder()
                .setBarcodeFormats(Barcode.FORMAT_QR_CODE).build())
            val preview = Preview.Builder().build().also { it.setSurfaceProvider(view.surfaceProvider) }
            val analysis = ImageAnalysis.Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
            var disposed = false
            var delivered = false
            var failed = false
            var provider: ProcessCameraProvider? = null
            analysis.setAnalyzer(executor) { proxy ->
                val image = proxy.image
                if (disposed || delivered || failed || image == null) {
                    proxy.close()
                } else {
                    scanner.process(InputImage.fromMediaImage(image, proxy.imageInfo.rotationDegrees))
                        .addOnSuccessListener(executor) { codes ->
                            val value = codes.firstNotNullOfOrNull { it.rawValue?.takeIf(String::isNotBlank) }
                            if (!disposed && !delivered && value != null) {
                                delivered = true
                                detected(value)
                            }
                        }
                        .addOnFailureListener(executor) {
                            if (!disposed && !failed) { failed = true; unavailable() }
                        }
                        .addOnCompleteListener { proxy.close() }
                }
            }
            future.addListener({
                if (!disposed) {
                    try {
                        provider = future.get()
                        provider!!.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
                        ready()
                    } catch (_: Exception) {
                        if (!failed) { failed = true; unavailable() }
                    }
                }
            }, executor)
            onDispose {
                disposed = true
                analysis.clearAnalyzer()
                provider?.unbind(preview, analysis)
                scanner.close()
            }
        }
    }
}
