package com.bitfun.mobile.app.ui.shell

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateDpAsState
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.bitfun.mobile.app.ui.theme.BitFunEaseOut
import com.bitfun.mobile.app.ui.theme.MotionDrawerCloseMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerHideMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerOpenMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerRevealMillis
import com.bitfun.mobile.app.ui.theme.MotionDrawerScrimMillis

/** Compact app shell motion shared with `AppShell.ets`. */
@Composable
internal fun BitFunCompactDrawer(
    open: Boolean,
    drawerWidth: Dp,
    onDismiss: () -> Unit,
    drawerContent: @Composable BoxScope.() -> Unit,
    content: @Composable BoxScope.() -> Unit,
) {
    BackHandler(enabled = open, onBack = onDismiss)

    val drawerDuration = if (open) MotionDrawerRevealMillis else MotionDrawerHideMillis
    val contentDuration = if (open) MotionDrawerOpenMillis else MotionDrawerCloseMillis
    val drawerProgress by animateFloatAsState(
        targetValue = if (open) 1f else 0f,
        animationSpec = tween(drawerDuration, easing = BitFunEaseOut),
        label = "drawer-reveal",
    )
    val contentOffset by animateDpAsState(
        targetValue = if (open) drawerWidth else 0.dp,
        animationSpec = tween(contentDuration, easing = BitFunEaseOut),
        label = "drawer-content-offset",
    )
    val contentScaleX by animateFloatAsState(
        targetValue = if (open) 0.985f else 1f,
        animationSpec = tween(contentDuration, easing = BitFunEaseOut),
        label = "drawer-content-scale-x",
    )
    val contentScaleY by animateFloatAsState(
        targetValue = if (open) 0.992f else 1f,
        animationSpec = tween(contentDuration, easing = BitFunEaseOut),
        label = "drawer-content-scale-y",
    )
    val contentRadius by animateDpAsState(
        targetValue = if (open) 28.dp else 0.dp,
        animationSpec = tween(contentDuration, easing = BitFunEaseOut),
        label = "drawer-content-radius",
    )
    val scrimAlpha by animateFloatAsState(
        targetValue = if (open) 0.62f else 0f,
        animationSpec = tween(MotionDrawerScrimMillis, easing = BitFunEaseOut),
        label = "drawer-scrim",
    )
    val density = LocalDensity.current
    val interactionSource = remember { MutableInteractionSource() }

    Box(Modifier.fillMaxSize()) {
        if (open || drawerProgress > 0.001f) {
            Box(
                modifier = Modifier
                    .width(drawerWidth)
                    .fillMaxHeight()
                    .graphicsLayer {
                        alpha = drawerProgress
                        translationX = with(density) { drawerWidth.toPx() } * -0.1f * (1f - drawerProgress)
                    },
                content = drawerContent,
            )
        }

        val shape = RoundedCornerShape(contentRadius)
        Box(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer {
                    translationX = with(density) { contentOffset.toPx() }
                    scaleX = contentScaleX
                    scaleY = contentScaleY
                    transformOrigin = androidx.compose.ui.graphics.TransformOrigin(0f, 0.5f)
                }
                .shadow(
                    elevation = if (open || contentOffset > 0.dp) 18.dp else 0.dp,
                    shape = shape,
                    clip = false,
                )
                .clip(shape),
        ) {
            content()
            if (open || scrimAlpha > 0.001f) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(
                            androidx.compose.material3.MaterialTheme.colorScheme.background.copy(
                                alpha = scrimAlpha,
                            ),
                        )
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
