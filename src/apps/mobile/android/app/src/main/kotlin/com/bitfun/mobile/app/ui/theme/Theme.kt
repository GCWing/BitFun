package com.bitfun.mobile.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * The palette, ported from the HarmonyOS client's `Theme.ets` plus its
 * `resources/base` and `resources/dark` colour elements.
 *
 * Stock `lightColorScheme()` / `darkColorScheme()` is Material's purple baseline,
 * which is not what the other client looks like: BitFun's is a warm paper-and-ink
 * palette. Every screen here already speaks in Material roles rather than
 * literals, so aligning the two clients is a matter of what those roles resolve
 * to — nothing below this file had to change to get the new colours.
 *
 * The tokens Material has no role for live in [BitFunColors]. Only the ones a
 * screen actually paints with are carried over; `connect_scan_accent` stays
 * behind because it decorates a viewfinder HarmonyOS draws itself and Play
 * Services draws for us. A colour nothing reads is a colour nobody maintains.
 */
private val InkLight = Color(0xFF171717)
private val InkDark = Color(0xFFF4F3EF)
private val White = Color(0xFFFFFFFF)

private val LightScheme = lightColorScheme(
    primary = Color(0xFF111111), // primary_action
    onPrimary = White, // primary_action_text
    primaryContainer = Color(0xFFF4F3F0), // soft
    onPrimaryContainer = InkLight,
    secondary = Color(0xFF111111), // accent
    onSecondary = White,
    secondaryContainer = Color(0xFFF4F3F0),
    onSecondaryContainer = InkLight,
    // file_link: the one saturated hue in the palette. Material has no link
    // role, so it lands on tertiary — which is also the "busy" connection dot.
    tertiary = Color(0xFF2563EB),
    onTertiary = White,
    background = Color(0xFFFDFDFB), // page_bg
    onBackground = InkLight,
    surface = White, // card
    onSurface = InkLight,
    surfaceVariant = Color(0xFFF4F3F0), // soft
    onSurfaceVariant = Color(0xFF706F6A), // muted
    // The whole container family, not only the two a card reads. Material fills
    // any role left unset from its own purple baseline, and the roles nothing in
    // this app names by hand are exactly the ones its components reach for on
    // their own — `ModalBottomSheet` takes surfaceContainerLow, elevation takes
    // surfaceTint, a snackbar takes inverseSurface. Leaving them out painted a
    // lilac sheet under a paper-coloured page.
    surfaceContainerLowest = White,
    surfaceContainerLow = Color(0xFFF7F7F5), // floating_panel_bg
    surfaceContainer = Color(0xFFF7F7F5), // floating_panel_bg
    surfaceContainerHigh = Color(0xFFF4F3F0), // soft
    surfaceContainerHighest = Color(0xFFEFEEE9),
    surfaceBright = Color(0xFFFDFDFB),
    surfaceDim = Color(0xFFEFEEE9),
    // No tint: the source's cards are flat fills, and a tinted overlay would put
    // the ink colour back over every raised surface.
    surfaceTint = White,
    inverseSurface = Color(0xFF2D2C28),
    inverseOnSurface = InkDark,
    inversePrimary = Color(0xFFE9E7E2),
    outline = Color(0xFFA5A39B), // subtle
    outlineVariant = Color(0xFFE9E7E2), // line
    error = Color(0xFFE04F4F), // red
    onError = White,
    // The HarmonyOS palette has no error container; rather than invent a hue,
    // a failure card is the same soft surface with the error colour on it.
    errorContainer = Color(0xFFF4F3F0),
    onErrorContainer = Color(0xFFE04F4F),
)

private val DarkScheme = darkColorScheme(
    primary = Color(0xFF454540), // primary_action
    onPrimary = White,
    primaryContainer = Color(0xFF2D2C28), // soft
    onPrimaryContainer = InkDark,
    secondary = Color(0xFF5B5954), // accent
    onSecondary = White,
    secondaryContainer = Color(0xFF2D2C28),
    onSecondaryContainer = InkDark,
    tertiary = Color(0xFF60A5FA), // file_link
    onTertiary = Color(0xFF151514),
    background = Color(0xFF151514), // page_bg
    onBackground = InkDark,
    surface = Color(0xFF252522), // card
    onSurface = InkDark,
    surfaceVariant = Color(0xFF2D2C28), // soft
    onSurfaceVariant = Color(0xFFAAA8A0), // muted
    surfaceContainerLowest = Color(0xFF101010),
    surfaceContainerLow = Color(0xFF1E1E1C), // floating_panel_bg
    surfaceContainer = Color(0xFF1E1E1C), // floating_panel_bg
    surfaceContainerHigh = Color(0xFF2D2C28), // soft
    surfaceContainerHighest = Color(0xFF35342F),
    surfaceBright = Color(0xFF3A3936),
    surfaceDim = Color(0xFF151514),
    surfaceTint = Color(0xFF252522),
    inverseSurface = InkDark,
    inverseOnSurface = Color(0xFF252522),
    inversePrimary = Color(0xFF363531),
    outline = Color(0xFF77756E), // subtle
    outlineVariant = Color(0xFF363531), // line
    error = Color(0xFFFF6B6B), // red
    onError = White,
    errorContainer = Color(0xFF2D2C28),
    onErrorContainer = Color(0xFFFF6B6B),
)

/**
 * Palette entries Material has no role for.
 *
 * [success] is the connection dot's "connected" green — it is not `primary`,
 * because primary here is near-black ink and a black dot reads as "off".
 */
internal data class BitFunColors(
    val success: Color,
    val heroBackground: Color,
    val heroSurface: Color,
    val heroAccent: Color,
    val heroSecondary: Color,
    val code: CodeSyntaxColors,
)

/**
 * What `CodeSyntaxTokenKind` looks like, one entry per kind that is not plain
 * text. Straight from the `code_*` colour elements of the HarmonyOS client.
 *
 * [targetBackground] is not a token colour: it paints behind whichever lines the
 * agent's reference named, so a `file.kt:80-92` preview shows *where* rather than
 * only *what*.
 */
internal data class CodeSyntaxColors(
    val lineNumber: Color,
    val keyword: Color,
    val string: Color,
    val number: Color,
    val comment: Color,
    val function: Color,
    val type: Color,
    val constant: Color,
    val property: Color,
    val targetBackground: Color,
)

private val LightExtras = BitFunColors(
    success = Color(0xFF27C46A),
    heroBackground = Color(0xFFE6EDFF),
    heroSurface = Color(0xFFF8FAFF),
    heroAccent = Color(0xFF9DB4FF),
    heroSecondary = Color(0xFFC9C5FF),
    code = CodeSyntaxColors(
        lineNumber = Color(0xFFAAA69D),
        keyword = Color(0xFF8F3F71),
        string = Color(0xFF477A4A),
        number = Color(0xFF9A5B13),
        comment = Color(0xFF7A8078),
        function = Color(0xFF2C6693),
        type = Color(0xFF865A20),
        constant = Color(0xFFA04444),
        property = Color(0xFF466D78),
        targetBackground = Color(0xFFFFF1BE),
    ),
)

private val DarkExtras = BitFunColors(
    success = Color(0xFF3BD47B),
    heroBackground = Color(0xFF2B2B29),
    heroSurface = Color(0xFF252522),
    heroAccent = Color(0xFF4A4944),
    heroSecondary = Color(0xFF3C3B38),
    code = CodeSyntaxColors(
        lineNumber = Color(0xFF77756E),
        keyword = Color(0xFFD99AC4),
        string = Color(0xFF9BCB9D),
        number = Color(0xFFE3B36D),
        comment = Color(0xFF96958D),
        function = Color(0xFF8CBCE0),
        type = Color(0xFFD5B27F),
        constant = Color(0xFFE79A9A),
        property = Color(0xFF9CC8D0),
        targetBackground = Color(0xFF5A4E24),
    ),
)

private val LocalBitFunColors = staticCompositionLocalOf { LightExtras }

/**
 * Harmony's mobile surfaces use a small, explicit type scale rather than the
 * platform Material defaults. Keeping the roles here makes every remaining
 * Material control start from the same geometry as the ArkUI counterpart.
 */
private val BitFunTypography = androidx.compose.material3.Typography(
    displayLarge = TextStyle(fontSize = 24.sp, lineHeight = 30.sp, fontWeight = FontWeight.Bold),
    displayMedium = TextStyle(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.Bold),
    displaySmall = TextStyle(fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.Bold),
    headlineLarge = TextStyle(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.Bold),
    headlineMedium = TextStyle(fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.Bold),
    headlineSmall = TextStyle(fontSize = 18.sp, lineHeight = 24.sp, fontWeight = FontWeight.Bold),
    titleLarge = TextStyle(fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.Bold),
    titleMedium = TextStyle(fontSize = 17.sp, lineHeight = 22.sp, fontWeight = FontWeight.Medium),
    titleSmall = TextStyle(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    bodyLarge = TextStyle(fontSize = 16.sp, lineHeight = 24.sp),
    bodyMedium = TextStyle(fontSize = 14.sp, lineHeight = 21.sp),
    bodySmall = TextStyle(fontSize = 13.sp, lineHeight = 19.sp),
    labelLarge = TextStyle(fontSize = 15.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    labelMedium = TextStyle(fontSize = 14.sp, lineHeight = 18.sp, fontWeight = FontWeight.Medium),
    labelSmall = TextStyle(fontSize = 12.sp, lineHeight = 16.sp),
)

/** The extra palette for the theme in scope. Reads like `MaterialTheme.colorScheme`. */
internal val bitFunColors: BitFunColors
    @Composable @ReadOnlyComposable get() = LocalBitFunColors.current

@Composable
internal fun BitFunTheme(dark: Boolean, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalBitFunColors provides if (dark) DarkExtras else LightExtras) {
        MaterialTheme(
            colorScheme = if (dark) DarkScheme else LightScheme,
            typography = BitFunTypography,
            content = content,
        )
    }
}
