package com.bitfun.mobile.app.ui.theme

import androidx.compose.animation.core.CubicBezierEasing

/** Motion values shared with the HarmonyOS presentation components. */
internal const val MotionQuickMillis: Int = 180
internal const val MotionStructureMillis: Int = 220
internal const val MotionDrawerScrimMillis: Int = 210
internal const val MotionDrawerOpenMillis: Int = 320
internal const val MotionDrawerCloseMillis: Int = 250
internal const val MotionDrawerRevealMillis: Int = 300
internal const val MotionDrawerHideMillis: Int = 220

internal val BitFunEaseOut = CubicBezierEasing(0f, 0f, 0.58f, 1f)
internal val BitFunEaseInOut = CubicBezierEasing(0.42f, 0f, 0.58f, 1f)
