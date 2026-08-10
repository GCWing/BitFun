package com.bitfun.mobile.app.platform

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.window.layout.FoldingFeature
import androidx.window.layout.WindowInfoTracker
import com.bitfun.mobile.core.feature.layout.WindowCrease
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map

/**
 * The window as the layout policies measure it.
 *
 * @param widthDp the window's own width, not the display's: a freeform or split
 * window is narrower than the screen it sits on, and the layout follows the
 * window.
 * @param largeScreenDevice `smallestScreenWidthDp >= 600`, the qualifier Android
 * has used for "tablet" since resource buckets existed. A *device* property on
 * purpose, matching the `deviceType == 'tablet'` that HarmonyOS passes: a phone
 * turned sideways is a wide window on a small device, and the policy already
 * knows the width.
 */
internal data class WindowMetrics(
    val widthDp: Int,
    val largeScreenDevice: Boolean,
    val creases: List<WindowCrease>,
)

/**
 * Reads the current window and the hinges crossing it.
 *
 * Only vertical creases are kept, and only their leading edge and thickness —
 * the same filter `AppRootPresentation.ets` applies to
 * `display.getCurrentFoldCreaseRegion()`, for the same reason: a crease running
 * across the window splits nothing a master/detail layout cares about. The
 * conversion to dp happens here rather than in the policy, so the shared code
 * never has to know what a pixel is on this device.
 */
@Composable
internal fun rememberWindowMetrics(): WindowMetrics {
    val context = LocalContext.current
    val density = LocalDensity.current
    val configuration = LocalConfiguration.current
    val containerSize = LocalWindowInfo.current.containerSize

    val activity = remember(context) { context.findActivity() }
    // No activity means no window to track — a @Preview, or a composable hosted
    // somewhere that has no fold to report anyway.
    val creaseFlow = remember(activity) {
        if (activity == null) {
            flowOf(emptyList())
        } else {
            WindowInfoTracker.getOrCreate(activity)
                .windowLayoutInfo(activity)
                .map { info ->
                    info.displayFeatures
                        .filterIsInstance<FoldingFeature>()
                        .filter { it.orientation == FoldingFeature.Orientation.VERTICAL }
                        .map { feature ->
                            with(density) {
                                WindowCrease(
                                    left = feature.bounds.left.toDp().value.toInt(),
                                    width = feature.bounds.width().toDp().value.toInt(),
                                )
                            }
                        }
                }
        }
    }
    val creases by creaseFlow.collectAsStateWithLifecycle(emptyList())

    return WindowMetrics(
        widthDp = with(density) { containerSize.width.toDp().value.toInt() },
        largeScreenDevice = configuration.smallestScreenWidthDp >= LARGE_SCREEN_MIN_WIDTH_DP,
        creases = creases,
    )
}

/** The sw600dp bucket, spelled out rather than left as a bare 600. */
private const val LARGE_SCREEN_MIN_WIDTH_DP = 600

private fun Context.findActivity(): Activity? {
    var current: Context? = this
    while (current is ContextWrapper) {
        if (current is Activity) return current
        current = current.baseContext
    }
    return null
}
