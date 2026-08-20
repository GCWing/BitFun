package com.bitfun.mobile.app.platform

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.window.layout.FoldingFeature
import androidx.window.layout.WindowInfoTracker
import com.bitfun.mobile.core.feature.layout.ConversationLayoutPolicy
import com.bitfun.mobile.core.feature.layout.WindowCrease
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map

/**
 * The window as the layout policies measure it.
 *
 * @param widthDp the window's own width, not the display's: a freeform or split
 * window is narrower than the screen it sits on, and the layout follows the
 * window.
 * Fold APIs are projected into semantic facts here so shared code never imports
 * Android WindowManager types.
 */
internal data class WindowMetrics(
    val widthDp: Int,
    val heightDp: Int,
    val wideViewportMatched: Boolean,
    val isFolded: Boolean,
    val isExpandedFoldable: Boolean,
    val isHoverLayout: Boolean,
    val creases: List<WindowCrease>,
)

private data class AndroidFoldInfo(
    val creases: List<WindowCrease>,
    val hasFoldingFeature: Boolean,
    val hoverCandidate: Boolean,
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
    val containerSize = LocalWindowInfo.current.containerSize
    val widthDp = with(density) { containerSize.width.toDp().value.toInt() }
    val heightDp = with(density) { containerSize.height.toDp().value.toInt() }
    val hasHingeSensor = remember(context) {
        context.packageManager.hasSystemFeature(FEATURE_SENSOR_HINGE_ANGLE)
    }

    val activity = remember(context) { context.findActivity() }
    // No activity means no window to track — a @Preview, or a composable hosted
    // somewhere that has no fold to report anyway.
    val foldInfoFlow = remember(activity, density) {
        if (activity == null) {
            flowOf(AndroidFoldInfo(emptyList(), false, false))
        } else {
            WindowInfoTracker.getOrCreate(activity)
                .windowLayoutInfo(activity)
                .map { info ->
                    val features = info.displayFeatures.filterIsInstance<FoldingFeature>()
                    AndroidFoldInfo(
                        creases = features
                            .filter { it.orientation == FoldingFeature.Orientation.VERTICAL }
                            .map { feature ->
                                with(density) {
                                    WindowCrease(
                                        left = feature.bounds.left.toDp().value.toInt(),
                                        width = feature.bounds.width().toDp().value.toInt(),
                                    )
                                }
                            },
                        hasFoldingFeature = features.isNotEmpty(),
                        hoverCandidate = features.any { feature ->
                            feature.orientation == FoldingFeature.Orientation.HORIZONTAL &&
                                feature.state == FoldingFeature.State.HALF_OPENED
                        },
                    )
                }
        }
    }
    val foldInfo by foldInfoFlow.collectAsStateWithLifecycle(
        AndroidFoldInfo(emptyList(), false, false),
    )

    return WindowMetrics(
        widthDp = widthDp,
        heightDp = heightDp,
        wideViewportMatched = widthDp >= ConversationLayoutPolicy.MD_MIN_WIDTH,
        // Android exposes FLAT and HALF_OPENED while the app is visible; a
        // fully closed device runs on a narrow cover display instead.
        isFolded = false,
        isExpandedFoldable = foldInfo.hasFoldingFeature || hasHingeSensor,
        isHoverLayout = ConversationLayoutPolicy.useHoverOperate(
            foldInfo.hoverCandidate,
            widthDp,
            heightDp,
        ),
        creases = foldInfo.creases,
    )
}

private const val FEATURE_SENSOR_HINGE_ANGLE = "android.hardware.sensor.hinge_angle"

private fun Context.findActivity(): Activity? {
    var current: Context? = this
    while (current is ContextWrapper) {
        if (current is Activity) return current
        current = current.baseContext
    }
    return null
}
