package com.bitfun.mobile.app.ui.chat

import androidx.annotation.DrawableRes
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideOut
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import com.bitfun.mobile.app.ui.theme.BitFunEaseOut
import com.bitfun.mobile.app.ui.theme.MotionQuickMillis
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignGeometry
import com.bitfun.mobile.app.ui.theme.bitFunColors

internal const val HEADER_ACTION_MENU_TEST_TAG: String = "header-action-menu"

/** One row in the fixed-width header menu shared by local and remote chat. */
internal data class BitFunHeaderAction(
    @DrawableRes val icon: Int,
    val label: String,
    val onClick: () -> Unit,
    val enabled: Boolean = true,
    val selected: Boolean = false,
    val destructive: Boolean = false,
    val dividerBefore: Boolean = false,
    val testTag: String? = null,
)

/**
 * The compact floating menu used by the HarmonyOS conversation header.
 *
 * Material's default menu is intentionally not used as-is: its narrow shape,
 * missing section label, and text-only rows made local and remote headers read
 * like different products. The actions remain platform-owned; this component
 * owns only the shared presentation contract.
 */
@Composable
internal fun BitFunHeaderActionMenu(
    expanded: Boolean,
    onDismiss: () -> Unit,
    sectionTitle: String,
    actions: List<BitFunHeaderAction>,
    modifier: Modifier = Modifier,
) {
    val visibility = remember { MutableTransitionState(false) }
    visibility.targetState = expanded
    val density = LocalDensity.current
    val targetSpace = with(density) { 8.dp.roundToPx() }
    val motionOffset = with(density) { 8.dp.roundToPx() }
    val positionProvider = remember(targetSpace) { HeaderMenuPositionProvider(targetSpace) }
    if (visibility.currentState || visibility.targetState) {
        Popup(
            popupPositionProvider = positionProvider,
            onDismissRequest = onDismiss,
            properties = PopupProperties(
                focusable = true,
                dismissOnBackPress = true,
                dismissOnClickOutside = true,
                clippingEnabled = true,
            ),
        ) {
            AnimatedVisibility(
                visibleState = visibility,
                enter = fadeIn(tween(MotionQuickMillis, easing = BitFunEaseOut)) +
                    slideIn(
                        animationSpec = tween(MotionQuickMillis, easing = BitFunEaseOut),
                        initialOffset = { IntOffset(motionOffset, -motionOffset) },
                    ),
                exit = fadeOut(tween(MotionQuickMillis, easing = BitFunEaseOut)) +
                    slideOut(
                        animationSpec = tween(MotionQuickMillis, easing = BitFunEaseOut),
                        targetOffset = { IntOffset(motionOffset, -motionOffset) },
                    ),
            ) {
                Surface(
                    modifier = modifier
                        .testTag(HEADER_ACTION_MENU_TEST_TAG)
                        .width(MobileDesignGeometry.PopoverWidth)
                        .clip(RoundedCornerShape(MobileDesignGeometry.PopoverRadius))
                        .border(
                            BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                            RoundedCornerShape(MobileDesignGeometry.PopoverRadius),
                        ),
                    shape = RoundedCornerShape(MobileDesignGeometry.PopoverRadius),
                    color = MaterialTheme.colorScheme.surfaceContainerLow,
                    tonalElevation = 0.dp,
                    shadowElevation = MobileDesignGeometry.PopoverShadowRadius,
                ) {
                    Column(
                        Modifier.padding(
                            horizontal = MobileDesignGeometry.PopoverPadding,
                            vertical = MobileDesignGeometry.PopoverVerticalPadding,
                        ),
                    ) {
                        Box(
                            modifier = Modifier.fillMaxWidth().height(28.dp).padding(start = 8.dp),
                            contentAlignment = Alignment.CenterStart,
                        ) {
                            Text(
                                sectionTitle,
                                fontSize = 13.sp,
                                fontWeight = FontWeight.Medium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        actions.forEachIndexed { index, action ->
                            if (index > 0 && shouldDivide(actions[index - 1], action)) {
                                HorizontalDivider(
                                    color = MaterialTheme.colorScheme.outlineVariant,
                                    modifier = Modifier.padding(vertical = 8.dp),
                                )
                            }
                            HeaderActionRow(action, onDismiss)
                        }
                    }
                }
            }
        }
    }
}

private class HeaderMenuPositionProvider(
    private val targetSpace: Int,
) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset {
        val x = (anchorBounds.right - popupContentSize.width)
            .coerceIn(0, (windowSize.width - popupContentSize.width).coerceAtLeast(0))
        val below = anchorBounds.bottom + targetSpace
        val y = if (below + popupContentSize.height <= windowSize.height) {
            below
        } else {
            (anchorBounds.top - targetSpace - popupContentSize.height).coerceAtLeast(0)
        }
        return IntOffset(x, y)
    }
}

private fun shouldDivide(previous: BitFunHeaderAction, current: BitFunHeaderAction): Boolean =
    current.dividerBefore ||
        (previous.label.contains("model", ignoreCase = true) &&
            !current.label.contains("model", ignoreCase = true))

@Composable
private fun HeaderActionRow(action: BitFunHeaderAction, onDismiss: () -> Unit) {
    val contentColor = when {
        !action.enabled -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.42f)
        action.destructive -> MaterialTheme.colorScheme.error
        else -> MaterialTheme.colorScheme.onSurface
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(MobileDesignGeometry.PopoverActionHeight)
            .clip(RoundedCornerShape(10.dp))
            .background(
                if (action.selected) MaterialTheme.colorScheme.surfaceVariant
                else bitFunColors.transparent,
            )
            .clickable(enabled = action.enabled) {
                action.onClick()
                onDismiss()
            }
            .alpha(if (action.enabled) 1f else 0.42f)
            .padding(horizontal = 8.dp)
            .then(action.testTag?.let { Modifier.testTag(it) } ?: Modifier),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(23.dp), contentAlignment = Alignment.Center) {
            Icon(
                painterResource(action.icon),
                contentDescription = null,
                tint = if (action.destructive) MaterialTheme.colorScheme.error else
                    MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(20.dp),
            )
        }
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            Text(
                action.label,
                color = contentColor,
                fontSize = 15.sp,
                maxLines = 1,
            )
        }
    }
}
