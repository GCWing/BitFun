package com.bitfun.mobile.app.ui.common

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignGeometry
import com.bitfun.mobile.app.ui.theme.bitFunColors
import com.bitfun.mobile.core.feature.layout.SettingsPlacement
import com.bitfun.mobile.core.feature.layout.SettingsPlacementMode

/**
 * Native modal lifecycle around the shared mobile overlay visual contract.
 *
 * Compact and hover windows keep Material's modal sheet semantics. A side
 * placement uses a full-window Dialog so back handling, focus containment and
 * accessibility isolation remain native while the surface docks to the
 * physical trailing region selected by the shared policy.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AdaptiveModalSurface(
    visible: Boolean,
    placement: SettingsPlacement,
    onDismissRequest: () -> Unit,
    content: @Composable (Modifier) -> Unit,
) {
    if (!visible) return

    if (placement.mode == SettingsPlacementMode.SIDE) {
        Dialog(
            onDismissRequest = onDismissRequest,
            properties = DialogProperties(
                dismissOnBackPress = true,
                dismissOnClickOutside = true,
                usePlatformDefaultWidth = false,
                decorFitsSystemWindows = false,
            ),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(MaterialTheme.colorScheme.scrim)
                    .clickable(onClick = onDismissRequest)
                    .safeDrawingPadding()
                    .imePadding(),
                contentAlignment = Alignment.CenterEnd,
            ) {
                Surface(
                    color = MaterialTheme.colorScheme.background,
                    shape = RoundedCornerShape(MobileDesignGeometry.SheetSideRadius),
                    shadowElevation = MobileDesignGeometry.PopoverShadowRadius,
                    modifier = Modifier
                        .width(placement.width.dp)
                        .height(placement.height.dp)
                        .clickable(interactionSource = null, indication = null, onClick = {}),
                ) {
                    content(Modifier.fillMaxSize())
                }
            }
        }
        return
    }

    ModalBottomSheet(
        onDismissRequest = onDismissRequest,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.background,
        shape = RoundedCornerShape(
            topStart = MobileDesignGeometry.SheetTopRadius,
            topEnd = MobileDesignGeometry.SheetTopRadius,
        ),
        dragHandle = null,
    ) {
        val modifier = if (placement.mode == SettingsPlacementMode.FOLD_OPERATE && placement.height > 0) {
            Modifier.fillMaxWidth().height(placement.height.dp)
        } else {
            Modifier.fillMaxWidth().fillMaxHeight(0.94f)
        }
        content(modifier)
    }
}
