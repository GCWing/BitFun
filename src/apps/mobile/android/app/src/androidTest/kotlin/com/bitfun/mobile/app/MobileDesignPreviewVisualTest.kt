package com.bitfun.mobile.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.platform.app.InstrumentationRegistry
import com.bitfun.mobile.app.ui.preview.MOBILE_DESIGN_GALLERY_PLATFORM_TEST_TAG
import com.bitfun.mobile.app.ui.preview.MOBILE_DESIGN_GALLERY_TEST_TAG
import com.bitfun.mobile.app.ui.preview.MobileDesignGallery
import com.bitfun.mobile.app.ui.preview.generated.MobilePreviewScenario
import com.bitfun.mobile.app.ui.preview.generated.MobilePreviewScenarios
import com.bitfun.mobile.app.ui.theme.BitFunTheme
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignColors
import com.bitfun.mobile.app.R
import kotlin.math.abs
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class MobileDesignPreviewVisualTest {
    @get:Rule
    val composeRule = createComposeRule()

    private val targetContext = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun connectedConversationScenarioExposesStableSemanticsAndPixels() {
        composeRule.setContent {
            MobileDesignGallery(MobilePreviewScenarios.ConnectedConversation, dark = false)
        }
        assertGallerySemantics(MobilePreviewScenarios.ConnectedConversation)
        val gallery = composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_TEST_TAG)
            .captureToImage()
        assertNonBlankAndVaried(gallery)
    }

    @Test
    fun streamingDarkScenarioExposesStableSemanticsAndPixels() {
        composeRule.setContent {
            MobileDesignGallery(MobilePreviewScenarios.StreamingDark, dark = true)
        }
        assertGallerySemantics(MobilePreviewScenarios.StreamingDark)
        val gallery = composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_TEST_TAG)
            .captureToImage()
        assertNonBlankAndVaried(gallery)
    }

    @Test
    fun reconnectingWideScenarioExposesStableSemanticsAndPixels() {
        composeRule.setContent {
            MobileDesignGallery(MobilePreviewScenarios.ReconnectingWide, dark = false)
        }
        assertGallerySemantics(MobilePreviewScenarios.ReconnectingWide)
        assertNonBlankAndVaried(
            composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_TEST_TAG).captureToImage()
        )
    }

    @Test
    fun darkGalleryIsDarkerThanLightGallery() {
        var scenario by mutableStateOf<MobilePreviewScenario>(MobilePreviewScenarios.ConnectedConversation)
        composeRule.setContent {
            MobileDesignGallery(scenario, dark = scenario.appearance == "dark")
        }
        val lightGallery = composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_TEST_TAG)
            .captureToImage()

        composeRule.runOnIdle { scenario = MobilePreviewScenarios.StreamingDark }
        composeRule.waitForIdle()
        val darkGallery = composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_TEST_TAG)
            .captureToImage()

        assertNonBlankAndVaried(lightGallery)
        assertNonBlankAndVaried(darkGallery)
        assertTrue(
            "Light gallery should be brighter than dark gallery",
            meanLuminance(lightGallery) > meanLuminance(darkGallery),
        )
    }

    @Test
    fun lightThemeBackgroundProbeMatchesContract() {
        assertBackgroundProbe(dark = false, expected = MobileDesignColors.Light.PageBg)
    }

    @Test
    fun darkThemeBackgroundProbeMatchesContract() {
        assertBackgroundProbe(dark = true, expected = MobileDesignColors.Dark.PageBg)
    }

    private fun assertGallerySemantics(scenario: MobilePreviewScenario) {
        composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(MOBILE_DESIGN_GALLERY_PLATFORM_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText("${scenario.viewportWidth} × ${scenario.viewportHeight}")
            .assertIsDisplayed()
        composeRule.onNodeWithText(scenario.headerTitle).assertIsDisplayed()
        composeRule.onNodeWithText(scenario.headerSubtitle).assertIsDisplayed()
        scenario.messages.forEach { message ->
            composeRule.onNodeWithText(message.text).assertIsDisplayed()
        }
        if (scenario.composerDraft.isEmpty()) {
            composeRule.onNodeWithText(scenario.composerPlaceholder).assertIsDisplayed()
        } else {
            composeRule.onNodeWithText(scenario.composerDraft).assertIsDisplayed()
        }
        if (scenario == MobilePreviewScenarios.StreamingDark) {
            composeRule.onNodeWithContentDescription(targetContext.getString(R.string.message_stop))
                .assertIsDisplayed()
        }
    }

    @OptIn(ExperimentalTestApi::class, ExperimentalComposeUiApi::class)
    private fun assertBackgroundProbe(dark: Boolean, expected: androidx.compose.ui.graphics.Color) {
        composeRule.setContent {
            BitFunTheme(dark = dark) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(MaterialTheme.colorScheme.background)
                        .testTag("token-probe"),
                )
            }
        }
        val image = composeRule.onNodeWithTag("token-probe").captureToImage()
        val actual = image.toPixelMap()[image.width / 2, image.height / 2].toArgb()
        val expectedArgb = expected.toArgb()
        assertTrue(
            "Theme background pixel differs: expected $expectedArgb, actual $actual",
            channelDistance(actual, expectedArgb) <= 4,
        )
    }

    private fun assertNonBlankAndVaried(image: ImageBitmap) {
        assertTrue("Captured gallery must have pixels", image.width > 0 && image.height > 0)
        val samples = sampledArgb(image)
        assertTrue("Captured gallery must contain visual variation", samples.distinct().size > 1)
    }

    private fun sampledArgb(image: ImageBitmap): List<Int> {
        val map = image.toPixelMap()
        return (0 until 8).flatMap { row ->
            (0 until 8).map { column ->
                val x = column * (image.width - 1) / 7
                val y = row * (image.height - 1) / 7
                map[x, y].toArgb()
            }
        }
    }

    private fun meanLuminance(image: ImageBitmap): Double = sampledArgb(image).map { argb ->
        val red = (argb shr 16) and 0xff
        val green = (argb shr 8) and 0xff
        val blue = argb and 0xff
        0.2126 * red + 0.7152 * green + 0.0722 * blue
    }.average()

    private fun channelDistance(first: Int, second: Int): Int = maxOf(
        abs(((first shr 16) and 0xff) - ((second shr 16) and 0xff)),
        abs(((first shr 8) and 0xff) - ((second shr 8) and 0xff)),
        abs((first and 0xff) - (second and 0xff)),
    )
}
