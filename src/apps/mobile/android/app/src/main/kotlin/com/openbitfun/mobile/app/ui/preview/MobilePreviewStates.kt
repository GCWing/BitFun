package com.openbitfun.mobile.app.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.compose.ui.tooling.preview.Preview
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.chat.ComposerBar
import com.openbitfun.mobile.app.ui.common.AdaptiveModalSurface
import com.openbitfun.mobile.app.ui.common.CircleControl
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.layout.SettingsPlacementPolicy
import com.openbitfun.mobile.core.feature.layout.SettingsSheetKind
import com.openbitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.openbitfun.mobile.core.feature.session.ComposerImage
import com.openbitfun.mobile.core.feature.session.ModelOption

internal const val MOBILE_PREVIEW_COMPOSER_FOCUSED_TEST_TAG: String = "mobile-preview-composer-focused"
internal const val MOBILE_PREVIEW_COMPOSER_ATTACHMENTS_TEST_TAG: String = "mobile-preview-composer-attachments"
internal const val MOBILE_PREVIEW_MODAL_BUSY_TEST_TAG: String = "mobile-preview-modal-busy"
internal const val MOBILE_PREVIEW_MODAL_ERROR_TEST_TAG: String = "mobile-preview-modal-error"
internal const val MOBILE_PREVIEW_MODAL_ERROR_TEXT_TEST_TAG: String = "mobile-preview-modal-error-text"
internal const val MOBILE_PREVIEW_MODAL_ERROR_ACTION_TEST_TAG: String = "mobile-preview-modal-error-action"
internal const val MOBILE_PREVIEW_CIRCLE_STATES_TEST_TAG: String = "mobile-preview-circle-states"
internal const val MOBILE_PREVIEW_CIRCLE_SIDEBAR_TEST_TAG: String = "mobile-preview-circle-sidebar"
internal const val MOBILE_PREVIEW_CIRCLE_MORE_TEST_TAG: String = "mobile-preview-circle-more"
internal const val MOBILE_PREVIEW_CIRCLE_BACK_TEST_TAG: String = "mobile-preview-circle-back"

private val PreviewModel = ModelOption("preview-model", "Preview model", "Native model", true)

@Composable
private fun previewComposer(
    draft: String,
    images: List<ComposerImage>,
    modifier: Modifier,
) {
    ComposerBar(
        draft = draft,
        images = images,
        busy = false,
        streaming = false,
        phase = ConnectionPhase.CONNECTED,
        model = PreviewModel,
        capabilities = ChatComposerCapabilities.GeneralChat,
        placeholder = "Type a message",
        onDraftChange = {},
        onRemoveImage = {},
        onAttach = {},
        onVoice = {},
        onSend = {},
        onStop = {},
        onOpenModels = {},
        modifier = modifier,
    )
}

@Preview(name = "OpenBitFun Mobile · Composer Focused", showBackground = true)
@Composable
internal fun MobilePreviewComposerFocused(dark: Boolean = false) {
    OpenBitFunTheme(dark = dark) {
        Column(
            modifier = Modifier.fillMaxSize().testTag(MOBILE_PREVIEW_COMPOSER_FOCUSED_TEST_TAG),
        ) {
            previewComposer(
                draft = "Review the current message",
                images = emptyList(),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Preview(name = "OpenBitFun Mobile · Composer Attachments", showBackground = true)
@Composable
internal fun MobilePreviewComposerAttachments(dark: Boolean = false) {
    OpenBitFunTheme(dark = dark) {
        Column(
            modifier = Modifier.fillMaxSize().testTag(MOBILE_PREVIEW_COMPOSER_ATTACHMENTS_TEST_TAG),
        ) {
            previewComposer(
                draft = "",
                images = listOf(ComposerImage(id = "preview-1", dataUrl = "", mimeType = "")),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Preview(name = "OpenBitFun Mobile · Modal Busy", showBackground = true)
@Composable
internal fun MobilePreviewModalBusy(dark: Boolean = false) {
    OpenBitFunTheme(dark = dark) {
        AdaptiveModalSurface(
            visible = true,
            placement = SettingsPlacementPolicy.compactBottom(SettingsSheetKind.SETTINGS),
            onDismissRequest = {},
        ) { surfaceModifier ->
            Column(
                modifier = surfaceModifier
                    .padding(24.dp)
                    .testTag(MOBILE_PREVIEW_MODAL_BUSY_TEST_TAG),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text("Working…", style = MaterialTheme.typography.titleMedium)
                Text("Please wait while the request finishes.")
            }
        }
    }
}

@Preview(name = "OpenBitFun Mobile · Modal Error", showBackground = true)
@Composable
internal fun MobilePreviewModalError(dark: Boolean = false) {
    OpenBitFunTheme(dark = dark) {
        AdaptiveModalSurface(
            visible = true,
            placement = SettingsPlacementPolicy.compactBottom(SettingsSheetKind.SETTINGS),
            onDismissRequest = {},
        ) { surfaceModifier ->
            Column(
                modifier = surfaceModifier
                    .padding(24.dp)
                    .testTag(MOBILE_PREVIEW_MODAL_ERROR_TEST_TAG),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    "Connection failed",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.testTag(MOBILE_PREVIEW_MODAL_ERROR_TEXT_TEST_TAG),
                )
                Text("Try again when the connection is available.")
                TextButton(
                    onClick = {},
                    modifier = Modifier.testTag(MOBILE_PREVIEW_MODAL_ERROR_ACTION_TEST_TAG),
                ) {
                    Text("Try again")
                }
            }
        }
    }
}

@Preview(name = "OpenBitFun Mobile · Circle States", showBackground = true)
@Composable
internal fun MobilePreviewCircleStates(dark: Boolean = false) {
    OpenBitFunTheme(dark = dark) {
        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp)
                .testTag(MOBILE_PREVIEW_CIRCLE_STATES_TEST_TAG),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            CircleControl(
                icon = R.drawable.ic_symbol_menu_lines,
                glyphSize = 22,
                contentDescription = "Open sidebar",
                onClick = {},
                modifier = Modifier.testTag(MOBILE_PREVIEW_CIRCLE_SIDEBAR_TEST_TAG),
            )
            CircleControl(
                icon = R.drawable.ic_symbol_ellipsis,
                glyphSize = 22,
                contentDescription = "More actions",
                onClick = {},
                modifier = Modifier.testTag(MOBILE_PREVIEW_CIRCLE_MORE_TEST_TAG),
            )
            CircleControl(
                icon = R.drawable.ic_symbol_chevron_left,
                glyphSize = 22,
                contentDescription = "Back",
                onClick = {},
                modifier = Modifier.testTag(MOBILE_PREVIEW_CIRCLE_BACK_TEST_TAG),
            )
        }
    }
}
