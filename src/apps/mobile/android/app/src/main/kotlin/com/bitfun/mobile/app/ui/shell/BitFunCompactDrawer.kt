package com.bitfun.mobile.app.ui.shell

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.ui.theme.BitFunEaseOut
import com.bitfun.mobile.app.ui.theme.MotionDrawerCloseMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerHideMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerOpenMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerRevealMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerScrimMillis
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch

private const val CONTENT_SCALE_X = 0.985f
private const val CONTENT_SCALE_Y = 0.992f
private const val CONTENT_RADIUS_DP = 28
private const val CONTENT_ELEVATION_DP = 18

/** Compact app shell motion shared with `AppShell.ets`. */
@Composable
internal fun BitFunCompactDrawer(
    open: Boolean,
    compact: Boolean,
    drawerWidth: Dp,
    onDismiss: () -> Unit,
    drawerContent: @Composable BoxScope.() -> Unit,
    content: @Composable BoxScope.() -> Unit,
) {
    BackHandler(enabled = open, onBack = onDismiss)

    val contentDuration = if (open) MotionDrawerOpenMillis else MotionDrawerCloseMillis
    // Translation, scale and corner radius share one progress value so they stay
    // in lockstep and the whole motion runs as layer-property updates instead of
    // recomposing the content once per frame.
    val contentProgress by animateFloatAsState(
        targetValue = if (open) 1f else 0f,
        animationSpec = tween(contentDuration, easing = BitFunEaseOut),
        label = "drawer-content-progress",
    )
    val density = LocalDensity.current
    val drawerWidthPx = with(density) { drawerWidth.toPx() }
    val contentRadiusPx = with(density) { CONTENT_RADIUS_DP.dp.toPx() }
    val contentElevationPx = with(density) { CONTENT_ELEVATION_DP.dp.toPx() }
    val interactionSource = remember { MutableInteractionSource() }

    // The drawer and scrim run on `Animatable` so their reveal/hide values are
    // read only inside `graphicsLayer` blocks (layer-only updates, no relayout or
    // recomposition). Composition is gated by plain booleans that flip only at
    // open/close boundaries, so the screen is not recomposed every frame.
    //
    // The drawer's content (sessions, devices, workspaces) is the expensive half
    // of the open animation, not the geometry. Re-composing it on every open is
    // the connected-state jank, so once it has been shown it stays composed and is
    // hidden off-screen with its semantics cleared instead of being disposed. That
    // is only safe while this compact drawer owns the sidebar; a wide layout has a
    // permanent sidebar, so the resident copy is dropped when `compact` turns off.
    val drawerProgress = remember { Animatable(0f) }
    val scrimProgress = remember { Animatable(0f) }
    var drawerComposed by remember { mutableStateOf(open) }
    var drawerRevealed by remember { mutableStateOf(open) }
    var scrimComposed by remember { mutableStateOf(open) }

    LaunchedEffect(open, compact) {
        if (!compact) {
            // Wide layout: the permanent sidebar owns this role. Drop any resident
            // copy so the window does not end up with a second, hidden Sidebar.
            drawerProgress.snapTo(0f)
            scrimProgress.snapTo(0f)
            drawerComposed = false
            drawerRevealed = false
            scrimComposed = false
        } else if (open) {
            drawerComposed = true
            drawerRevealed = true
            scrimComposed = true
            coroutineScope {
                launch {
                    drawerProgress.animateTo(
                        targetValue = 1f,
                        animationSpec = tween(MotionDrawerRevealMillis, easing = BitFunEaseOut),
                    )
                }
                launch {
                    scrimProgress.animateTo(
                        targetValue = 0.62f,
                        animationSpec = tween(MotionDrawerScrimMillis, easing = BitFunEaseOut),
                    )
                }
            }
        } else if (drawerComposed) {
            coroutineScope {
                launch {
                    drawerProgress.animateTo(
                        targetValue = 0f,
                        animationSpec = tween(MotionDrawerHideMillis, easing = BitFunEaseOut),
                    )
                }
                launch {
                    scrimProgress.animateTo(
                        targetValue = 0f,
                        animationSpec = tween(MotionDrawerScrimMillis, easing = BitFunEaseOut),
                    )
                }
            }
            drawerRevealed = false
            scrimComposed = false
        }
    }

    Box(Modifier.fillMaxSize()) {
        if (compact && drawerComposed) {
            Box(
                modifier = Modifier
                    .width(drawerWidth)
                    .fillMaxHeight()
                    .graphicsLayer {
                        alpha = drawerProgress.value
                        translationX = if (drawerRevealed) {
                            drawerWidthPx * -0.1f * (1f - drawerProgress.value)
                        } else {
                            -(drawerWidthPx + 1f)
                        }
                    }
                    .then(
                        if (drawerRevealed) {
                            Modifier
                        } else {
                            Modifier.clearAndSetSemantics {}
                        },
                    ),
                content = drawerContent,
            )
        }

        Box(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer {
                    translationX = drawerWidthPx * contentProgress
                    scaleX = 1f - (1f - CONTENT_SCALE_X) * contentProgress
                    scaleY = 1f - (1f - CONTENT_SCALE_Y) * contentProgress
                    transformOrigin = TransformOrigin(0f, 0.5f)
                    shape = RoundedCornerShape(contentRadiusPx * contentProgress)
                    clip = true
                    shadowElevation = if (open || contentProgress > 0f) contentElevationPx else 0f
                },
        ) {
            content()
            if (scrimComposed) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(androidx.compose.material3.MaterialTheme.colorScheme.background)
                        .graphicsLayer { alpha = scrimProgress.value }
                        .clickable(
                            enabled = open,
                            interactionSource = interactionSource,
                            indication = null,
                            onClick = onDismiss,
                        ),
                )
            }
        }
    }
}
