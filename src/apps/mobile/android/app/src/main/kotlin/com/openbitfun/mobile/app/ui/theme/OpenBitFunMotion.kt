package com.openbitfun.mobile.app.ui.theme

import androidx.compose.animation.core.CubicBezierEasing
import com.openbitfun.mobile.app.ui.theme.generated.MobileDesignMotion

/**
 * Motion contract for the mobile surfaces.
 *
 * [MotionQuickMillis] and [MotionStructureMillis] consume the generated
 * design-system tokens from [MobileDesignMotion]. Compose's composable animation
 * APIs and [androidx.compose.animation.core.Animatable.animateTo] honor
 * [androidx.compose.animation.core.MotionDurationScale] from the coroutine
 * context, and the Android window recomposer derives that scale from the system
 * `Settings.Global.ANIMATOR_DURATION_SCALE` ("Remove animations"), so reduced
 * motion needs no call-site scaling. The drawer keeps its real durations under
 * that scale rather than switching to a different, shorter animation.
 *
 * The remaining drawer durations mirror the HarmonyOS `AppShell.ets` timings and
 * intentionally exceed the minimal Quick/Structure scale; [MotionDrawerHideMillis]
 * reuses the structure token because its value is already the same.
 */
internal const val MotionQuickMillis: Int = MobileDesignMotion.Quick
internal const val MotionStructureMillis: Int = MobileDesignMotion.Structure
internal const val MotionDrawerScrimMillis: Int = 210
internal const val MotionDrawerOpenMillis: Int = 320
internal const val MotionDrawerCloseMillis: Int = 250
internal const val MotionDrawerRevealMillis: Int = 300
internal const val MotionDrawerHideMillis: Int = MotionStructureMillis

internal val OpenBitFunEaseOut = CubicBezierEasing(0f, 0f, 0.58f, 1f)
internal val OpenBitFunEaseInOut = CubicBezierEasing(0.42f, 0f, 0.58f, 1f)
