package com.openbitfun.mobile.app

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.assertHeightIsEqualTo
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import com.openbitfun.mobile.app.R
import com.openbitfun.mobile.app.ui.chat.COMPOSER_INPUT_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.COMPOSER_SEND_TEST_TAG
import com.openbitfun.mobile.app.ui.chat.MODEL_CONTROL_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_CIRCLE_BACK_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_CIRCLE_MORE_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_CIRCLE_SIDEBAR_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_CIRCLE_STATES_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_COMPOSER_ATTACHMENTS_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_COMPOSER_FOCUSED_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_MODAL_BUSY_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_MODAL_ERROR_ACTION_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_MODAL_ERROR_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MOBILE_PREVIEW_MODAL_ERROR_TEXT_TEST_TAG
import com.openbitfun.mobile.app.ui.preview.MobilePreviewCircleStates
import com.openbitfun.mobile.app.ui.preview.MobilePreviewComposerAttachments
import com.openbitfun.mobile.app.ui.preview.MobilePreviewComposerFocused
import com.openbitfun.mobile.app.ui.preview.MobilePreviewModalBusy
import com.openbitfun.mobile.app.ui.preview.MobilePreviewModalError
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class MobilePreviewStatesVisualTest {
    @get:Rule
    val composeRule = createComposeRule()

    private val targetContext = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun focusedComposerExpandsToExposeModelControl() {
        composeRule.setContent { MobilePreviewComposerFocused() }
        composeRule.onNodeWithTag(MOBILE_PREVIEW_COMPOSER_FOCUSED_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(COMPOSER_INPUT_TEST_TAG)
            .assertIsDisplayed()
            .performTextInput("hello")
        composeRule.onNodeWithTag(MODEL_CONTROL_TEST_TAG).assertIsDisplayed()
    }

    @Test
    fun attachmentComposerShowsFallbackAttachmentSurfaceAndSend() {
        composeRule.setContent { MobilePreviewComposerAttachments() }
        composeRule.onNodeWithTag(MOBILE_PREVIEW_COMPOSER_ATTACHMENTS_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText(targetContext.getString(R.string.chat_image)).assertIsDisplayed()
        composeRule.onNodeWithTag(COMPOSER_SEND_TEST_TAG).assertIsDisplayed()
    }

    @Test
    fun busyModalShowsItsWorkingCopy() {
        composeRule.setContent { MobilePreviewModalBusy() }
        composeRule.onNodeWithTag(MOBILE_PREVIEW_MODAL_BUSY_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithText("Working…").assertIsDisplayed()
        composeRule.onNodeWithText("Please wait while the request finishes.").assertIsDisplayed()
    }

    @Test
    fun errorModalExposesRecoverySemantics() {
        composeRule.setContent { MobilePreviewModalError() }
        composeRule.onNodeWithTag(MOBILE_PREVIEW_MODAL_ERROR_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(MOBILE_PREVIEW_MODAL_ERROR_TEXT_TEST_TAG).assertIsDisplayed()
        composeRule.onNodeWithTag(MOBILE_PREVIEW_MODAL_ERROR_ACTION_TEST_TAG).assertIsDisplayed()
    }

    @Test
    fun circleStatesHaveStableTouchTargetsAndMoreActionIsSafe() {
        composeRule.setContent { MobilePreviewCircleStates() }
        composeRule.onNodeWithTag(MOBILE_PREVIEW_CIRCLE_STATES_TEST_TAG).assertIsDisplayed()
        listOf(
            MOBILE_PREVIEW_CIRCLE_SIDEBAR_TEST_TAG,
            MOBILE_PREVIEW_CIRCLE_MORE_TEST_TAG,
            MOBILE_PREVIEW_CIRCLE_BACK_TEST_TAG,
        ).forEach { tag ->
            composeRule.onNodeWithTag(tag)
                .assertIsDisplayed()
                .assertWidthIsEqualTo(44.dp)
                .assertHeightIsEqualTo(44.dp)
        }
        composeRule.onNodeWithContentDescription("More actions").performClick()
        composeRule.onNodeWithTag(MOBILE_PREVIEW_CIRCLE_MORE_TEST_TAG).assertIsDisplayed()
    }

    @Test
    fun darkModalStateProducesNonBlankCapturedPixels() {
        composeRule.setContent { MobilePreviewModalError(dark = true) }
        val image = composeRule.onNodeWithTag(MOBILE_PREVIEW_MODAL_ERROR_TEST_TAG).captureToImage()
        val pixelMap = image.toPixelMap()
        val center = pixelMap[image.width / 2, image.height / 2].toArgb()
        assertTrue(
            "Dark modal capture must have a non-transparent center",
            (center ushr 24) != 0,
        )
        assertTrue(
            "Dark modal capture must be dark-themed",
            meanLuminance(sampledArgb(image)) < 128.0,
        )
        val maxLuminance = (0 until image.height).maxOf { y ->
            (0 until image.width).maxOf { x -> argbLuminance(pixelMap[x, y].toArgb()) }
        }
        assertTrue(
            "Dark modal capture must contain drawn light-on-dark content",
            maxLuminance > 128.0,
        )
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

    private fun argbLuminance(argb: Int): Double {
        val red = (argb shr 16) and 0xff
        val green = (argb shr 8) and 0xff
        val blue = argb and 0xff
        return 0.2126 * red + 0.7152 * green + 0.0722 * blue
    }

    private fun meanLuminance(samples: List<Int>): Double = samples.map(::argbLuminance).average()
}
