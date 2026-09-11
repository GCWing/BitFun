package com.openbitfun.mobile.app.ui.common

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignGeometry

@Composable
internal fun ConnectionSheetHeader(onClose: () -> Unit) = ConnectionSheetHeader(onClose, false)

@Composable
internal fun ConnectionSheetHeader(onClose: () -> Unit, uniformGlyph: Boolean) {
    Box(Modifier.fillMaxWidth().height(MobileDesignGeometry.SheetHeaderHeight).padding(end = 8.dp)) {
        IconButton(onClick = onClose, modifier = Modifier.align(Alignment.CenterEnd)
            .size(MobileDesignGeometry.ControlTouchSize)) {
            if (uniformGlyph) {
                val ink = MaterialTheme.colorScheme.onSurface
                val closeLabel = stringResource(R.string.common_close)
                androidx.compose.foundation.Canvas(Modifier.size(18.dp).semantics { contentDescription = closeLabel }) {
                    drawLine(ink, androidx.compose.ui.geometry.Offset(2.dp.toPx(), 2.dp.toPx()),
                        androidx.compose.ui.geometry.Offset(16.dp.toPx(), 16.dp.toPx()), 1.5.dp.toPx(), androidx.compose.ui.graphics.StrokeCap.Round)
                    drawLine(ink, androidx.compose.ui.geometry.Offset(16.dp.toPx(), 2.dp.toPx()),
                        androidx.compose.ui.geometry.Offset(2.dp.toPx(), 16.dp.toPx()), 1.5.dp.toPx(), androidx.compose.ui.graphics.StrokeCap.Round)
                }
            } else Icon(painterResource(R.drawable.ic_symbol_xmark), stringResource(R.string.common_close),
                modifier = Modifier.size(18.dp))
        }
    }
}

@Composable
internal fun ConnectionSheetFooter(label: String, primary: Boolean = false, enabled: Boolean = true,
    modifier: Modifier = Modifier, elevated: Boolean = true, onClick: () -> Unit) {
    Box(modifier.fillMaxWidth().padding(start = MobileDesignGeometry.SheetHorizontalPadding,
        end = MobileDesignGeometry.SheetHorizontalPadding, top = 10.dp, bottom = 24.dp),
        contentAlignment = Alignment.Center) {
        Button(onClick = onClick, enabled = enabled,
            modifier = Modifier.widthIn(max = 520.dp).fillMaxWidth().height(MobileDesignGeometry.SheetActionHeight),
            shape = RoundedCornerShape(24.dp),
            border = if (primary) null else BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
            colors = ButtonDefaults.buttonColors(
                containerColor = if (primary) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surface,
                contentColor = if (primary) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface),
            elevation = ButtonDefaults.buttonElevation(defaultElevation = if (elevated) 3.dp else 0.dp)) {
            Text(label, style = MaterialTheme.typography.labelLarge.connectionSheetTextStyle())
        }
    }
}

// These sheet roles reserve their full contract line box, including for CJK fallback fonts.
internal fun androidx.compose.ui.text.TextStyle.connectionSheetTextStyle() = copy(
    platformStyle = androidx.compose.ui.text.PlatformTextStyle(includeFontPadding = false),
    lineHeightStyle = androidx.compose.ui.text.style.LineHeightStyle(
        alignment = androidx.compose.ui.text.style.LineHeightStyle.Alignment.Center,
        trim = androidx.compose.ui.text.style.LineHeightStyle.Trim.Both,
        mode = androidx.compose.ui.text.style.LineHeightStyle.Mode.Tight,
    ),
)
