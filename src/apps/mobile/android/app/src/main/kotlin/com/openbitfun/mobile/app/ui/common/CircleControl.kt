package com.openbitfun.mobile.app.ui.common

import androidx.annotation.DrawableRes
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignGeometry

/**
 * The floating round control the source draws on a page rather than on a bar.
 *
 * `RemoteChatHeader.ets` and `RemoteCreateSessionView.ets` both open with one of
 * these in the top-left, which is what keeps whatever is centred — a title, or
 * nothing at all — the subject of the screen instead of one of three equal
 * things in a row. Shared rather than copied so the two screens cannot drift
 * apart by a dp.
 *
 * @param glyphWidth and [glyphHeight] are the box the glyph is *fitted* into,
 * matching HarmonyOS' `TemplateIcon`, which fits its asset with
 * `ImageFit.Contain`. Neither one is the drawn size on its own: a drawable is
 * scaled by the smaller of the two ratios and stays centred. Passing the
 * source's non-square numbers to a drawable that keeps this set's square 24x24
 * viewport therefore shrinks the whole glyph to the shorter side — a 23x7 box
 * draws a 24x24 vector at 7x7. Give the glyph its own aspect in the drawable,
 * as `ic_symbol_ellipsis` and `ic_symbol_chevron_left_wide` do, or pass a
 * square box.
 */
@Composable
internal fun CircleControl(
    @DrawableRes icon: Int,
    glyphSize: Int,
    glyphWidth: Int = glyphSize,
    glyphHeight: Int = glyphSize,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier,
) {
    Surface(
        onClick = onClick,
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        shadowElevation = 3.dp,
        modifier = modifier.size(MobileDesignGeometry.ControlTouchSize),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Icon(
                painterResource(icon),
                contentDescription = contentDescription,
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.size(width = glyphWidth.dp, height = glyphHeight.dp),
            )
        }
    }
}
