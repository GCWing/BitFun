package com.openbitfun.mobile.app.platform

import org.junit.Assert.assertEquals
import org.junit.Test

class FoldFactsTest {
    @Test
    fun ordinaryDeviceHasNoFoldFacts() {
        assertEquals(
            FoldFacts(
                hasFoldingFeature = false,
                halfOpened = false,
                flatOpened = false,
                isFolded = false,
                isExpandedFoldable = false,
                hoverCandidate = false,
            ),
            reduceFoldFacts(hasHingeSensor = false, features = emptyList()),
        )
    }

    @Test
    fun flatOpenFeatureIsExpandedButNotFolded() {
        assertEquals(
            FoldFacts(true, false, true, false, true, false),
            reduceFoldFacts(
                hasHingeSensor = true,
                features = listOf(FoldFeatureFacts(FoldState.FLAT, isHorizontal = false)),
            ),
        )
    }

    @Test
    fun halfOpenHorizontalFeatureIsHoverCandidate() {
        assertEquals(
            FoldFacts(true, true, false, false, false, true),
            reduceFoldFacts(
                hasHingeSensor = false,
                features = listOf(FoldFeatureFacts(FoldState.HALF_OPENED, isHorizontal = true)),
            ),
        )
    }

    @Test
    fun halfOpenVerticalFeatureIsNotHoverCandidate() {
        assertEquals(
            FoldFacts(true, true, false, false, false, false),
            reduceFoldFacts(
                hasHingeSensor = false,
                features = listOf(FoldFeatureFacts(FoldState.HALF_OPENED, isHorizontal = false)),
            ),
        )
    }

    @Test
    fun hingeSensorWithoutWindowFeatureMeansCoverFolded() {
        assertEquals(
            FoldFacts(false, false, false, true, false, false),
            reduceFoldFacts(hasHingeSensor = true, features = emptyList()),
        )
    }

    @Test
    fun unknownFeatureIsNotTreatedAsExpandedOrHovering() {
        assertEquals(
            FoldFacts(true, false, false, false, false, false),
            reduceFoldFacts(
                hasHingeSensor = false,
                features = listOf(FoldFeatureFacts(FoldState.UNKNOWN, isHorizontal = true)),
            ),
        )
    }

    @Test
    fun mixedFlatAndUnknownFeaturesAreNotFlatOpen() {
        assertEquals(
            FoldFacts(true, false, false, false, false, false),
            reduceFoldFacts(
                hasHingeSensor = false,
                features = listOf(
                    FoldFeatureFacts(FoldState.FLAT, isHorizontal = false),
                    FoldFeatureFacts(FoldState.UNKNOWN, isHorizontal = false),
                ),
            ),
        )
    }

    @Test
    fun unknownFeatureSuppressesHoverForMixedHalfOpenFeatures() {
        assertEquals(
            FoldFacts(true, true, false, false, false, false),
            reduceFoldFacts(
                hasHingeSensor = false,
                features = listOf(
                    FoldFeatureFacts(FoldState.HALF_OPENED, isHorizontal = true),
                    FoldFeatureFacts(FoldState.UNKNOWN, isHorizontal = false),
                ),
            ),
        )
    }

    @Test
    fun multipleFeaturesRemainHalfOpenAndUseHorizontalHalfOpenForHover() {
        assertEquals(
            FoldFacts(true, true, false, false, false, true),
            reduceFoldFacts(
                hasHingeSensor = true,
                features = listOf(
                    FoldFeatureFacts(FoldState.FLAT, isHorizontal = false),
                    FoldFeatureFacts(FoldState.HALF_OPENED, isHorizontal = true),
                ),
            ),
        )
    }
}
