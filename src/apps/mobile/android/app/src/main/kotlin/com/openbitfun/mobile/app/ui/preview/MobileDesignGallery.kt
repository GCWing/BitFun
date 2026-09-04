package com.openbitfun.mobile.app.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.openbitfun.mobile.app.ui.chat.ComposerBar
import com.openbitfun.mobile.app.ui.chat.ConversationHeader
import com.openbitfun.mobile.app.ui.preview.generated.MobilePreviewMessage
import com.openbitfun.mobile.app.ui.preview.generated.MobilePreviewScenario
import com.openbitfun.mobile.app.ui.preview.generated.MobilePreviewScenarios
import com.openbitfun.mobile.app.ui.theme.OpenBitFunTheme
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignGeometry
import com.openbitfun.mobile.core.feature.connection.ConnectionPhase
import com.openbitfun.mobile.core.feature.session.ChatComposerCapabilities
import com.openbitfun.mobile.core.feature.session.ModelOption

/**
 * Stable preview semantics: the gallery root, platform label, and message timeline
 * are tagged so visual-parity instrumentation can locate each region.
 */
internal const val MOBILE_DESIGN_GALLERY_TEST_TAG: String = "mobile-design-gallery"
internal const val MOBILE_DESIGN_GALLERY_PLATFORM_TEST_TAG: String = "mobile-design-gallery-platform"
internal const val MOBILE_DESIGN_GALLERY_TIMELINE_TEST_TAG: String = "mobile-design-gallery-timeline"

@Composable
internal fun MobileDesignGallery(scenario: MobilePreviewScenario, dark: Boolean) {
    OpenBitFunTheme(dark = dark) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding()
                .background(MaterialTheme.colorScheme.background)
                .testTag(MOBILE_DESIGN_GALLERY_TEST_TAG),
        ) {
            PlatformLabel(scenario)
            ConversationHeader(
                title = scenario.headerTitle,
                contextTitle = scenario.headerSubtitle,
                canStop = scenario.streaming,
                enabled = true,
                onBack = {},
                onOpenSidebar = {},
                onRename = {},
                onStop = {},
                modifier = Modifier.background(MaterialTheme.colorScheme.background),
            )
            Column(
                verticalArrangement = Arrangement.spacedBy(MobileDesignGeometry.MessageSpacing),
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .testTag(MOBILE_DESIGN_GALLERY_TIMELINE_TEST_TAG)
                    .padding(
                        horizontal = MobileDesignGeometry.ContentGutter,
                        vertical = MobileDesignGeometry.TimelineTopPadding,
                    ),
            ) {
                scenario.messages.forEach { message -> PreviewMessageBubble(message) }
            }
            ComposerBar(
                draft = scenario.composerDraft,
                images = emptyList(),
                busy = scenario.streaming,
                streaming = scenario.streaming,
                phase = if (scenario.connectionPhase == "reconnecting") {
                    ConnectionPhase.RECONNECTING
                } else {
                    ConnectionPhase.CONNECTED
                },
                model = ModelOption("preview-model", "OpenBitFun Preview", "Native model", true),
                modelOptions = emptyList(),
                capabilities = ChatComposerCapabilities.RemoteChat,
                placeholder = scenario.composerPlaceholder,
                onDraftChange = {},
                onRemoveImage = {},
                onAttach = {},
                onVoice = {},
                onSend = {},
                onStop = {},
                onOpenModels = {},
                modifier = Modifier,
            )
        }
    }
}

internal fun mobileDesignScenario(id: String?): MobilePreviewScenario = when (id) {
    MobilePreviewScenarios.StreamingDark.id -> MobilePreviewScenarios.StreamingDark
    MobilePreviewScenarios.ReconnectingWide.id -> MobilePreviewScenarios.ReconnectingWide
    else -> MobilePreviewScenarios.ConnectedConversation
}

@Composable
private fun PlatformLabel(scenario: MobilePreviewScenario) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier
            .fillMaxWidth()
            .height(MobileDesignGeometry.ConnectionStripHeight)
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant)
            .padding(horizontal = MobileDesignGeometry.ContentGutter)
            .testTag(MOBILE_DESIGN_GALLERY_PLATFORM_TEST_TAG),
    ) {
        Text("Android", style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Medium, color = MaterialTheme.colorScheme.onSurface)
        Text(
            "NATIVE",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier
                .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(10.dp))
                .padding(horizontal = 8.dp, vertical = 4.dp),
        )
        Spacer(Modifier.weight(1f))
        Text(
            "${scenario.viewportWidth} × ${scenario.viewportHeight}",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun PreviewMessageBubble(message: MobilePreviewMessage) {
    Row(modifier = Modifier.fillMaxWidth()) {
        if (message.role == "user") Spacer(Modifier.weight(1f))
        Text(
            text = message.text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier
                .widthIn(max = MobileDesignGeometry.MessageBubbleMaxWidth)
                .background(
                    if (message.role == "user") MaterialTheme.colorScheme.surfaceVariant
                    else MaterialTheme.colorScheme.surface,
                    RoundedCornerShape(MobileDesignGeometry.MessageBubbleRadius),
                )
                .border(
                    1.dp,
                    MaterialTheme.colorScheme.outlineVariant,
                    RoundedCornerShape(MobileDesignGeometry.MessageBubbleRadius),
                )
                .padding(
                    horizontal = MobileDesignGeometry.MessageBubbleHorizontalPadding,
                    vertical = MobileDesignGeometry.MessageBubbleVerticalPadding,
                ),
        )
        if (message.role != "user") Spacer(Modifier.weight(1f))
    }
}

@Preview(name = "OpenBitFun Mobile · Compact", widthDp = 390, heightDp = 844, showBackground = true)
@Composable
private fun MobileDesignCompactPreview() {
    MobileDesignGallery(MobilePreviewScenarios.ConnectedConversation, dark = false)
}

@Preview(name = "OpenBitFun Mobile · Dark", widthDp = 390, heightDp = 844, showBackground = true)
@Composable
private fun MobileDesignDarkPreview() {
    MobileDesignGallery(MobilePreviewScenarios.StreamingDark, dark = true)
}

@Preview(name = "OpenBitFun Mobile · Reconnecting Wide", widthDp = 1024, heightDp = 768, showBackground = true)
@Composable
private fun MobileDesignReconnectingWidePreview() {
    MobileDesignGallery(MobilePreviewScenarios.ReconnectingWide, dark = false)
}
