package com.bitfun.mobile.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignColors
import com.bitfun.mobile.app.ui.theme.generated.MobileDesignTypography

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
private val LightTokens = MobileDesignColors.Light
private val DarkTokens = MobileDesignColors.Dark
private val InkLight = LightTokens.Ink
private val InkDark = DarkTokens.Ink
private val ContentOnAction = LightTokens.ContentOnAction

private val LightScheme = lightColorScheme(
    primary = LightTokens.PrimaryAction,
    onPrimary = ContentOnAction,
    primaryContainer = LightTokens.Soft,
    onPrimaryContainer = InkLight,
    secondary = LightTokens.Accent,
    onSecondary = ContentOnAction,
    secondaryContainer = LightTokens.Soft,
    onSecondaryContainer = InkLight,
    // file_link: the one saturated hue in the palette. Material has no link
    // role, so it lands on tertiary — which is also the "busy" connection dot.
    tertiary = LightTokens.FileLink,
    onTertiary = ContentOnAction,
    background = LightTokens.PageBg,
    onBackground = InkLight,
    surface = LightTokens.Card,
    onSurface = InkLight,
    surfaceVariant = LightTokens.Soft,
    onSurfaceVariant = LightTokens.Muted,
    // The whole container family, not only the two a card reads. Material fills
    // any role left unset from its own purple baseline, and the roles nothing in
    // this app names by hand are exactly the ones its components reach for on
    // their own — `ModalBottomSheet` takes surfaceContainerLow, elevation takes
    // surfaceTint, a snackbar takes inverseSurface. Leaving them out painted a
    // lilac sheet under a paper-coloured page.
    surfaceContainerLowest = LightTokens.Card,
    surfaceContainerLow = LightTokens.FloatingPanelBg,
    surfaceContainer = LightTokens.FloatingPanelBg,
    surfaceContainerHigh = LightTokens.Soft,
    surfaceContainerHighest = LightTokens.Line,
    surfaceBright = LightTokens.PageBg,
    surfaceDim = LightTokens.Line,
    // No tint: the source's cards are flat fills, and a tinted overlay would put
    // the ink colour back over every raised surface.
    surfaceTint = LightTokens.Card,
    inverseSurface = DarkTokens.Soft,
    inverseOnSurface = InkDark,
    inversePrimary = LightTokens.Line,
    outline = LightTokens.Subtle,
    outlineVariant = LightTokens.Line,
    error = LightTokens.StatusDanger,
    onError = ContentOnAction,
    // The HarmonyOS palette has no error container; rather than invent a hue,
    // a failure card is the same soft surface with the error colour on it.
    errorContainer = LightTokens.Soft,
    onErrorContainer = LightTokens.StatusDanger,
    scrim = LightTokens.Scrim,
)

private val DarkScheme = darkColorScheme(
    primary = DarkTokens.PrimaryAction,
    onPrimary = ContentOnAction,
    primaryContainer = DarkTokens.Soft,
    onPrimaryContainer = InkDark,
    secondary = DarkTokens.Accent,
    onSecondary = ContentOnAction,
    secondaryContainer = DarkTokens.Soft,
    onSecondaryContainer = InkDark,
    tertiary = DarkTokens.FileLink,
    onTertiary = DarkTokens.PageBg,
    background = DarkTokens.PageBg,
    onBackground = InkDark,
    surface = DarkTokens.Card,
    onSurface = InkDark,
    surfaceVariant = DarkTokens.Soft,
    onSurfaceVariant = DarkTokens.Muted,
    surfaceContainerLowest = DarkTokens.StartWindowBackground,
    surfaceContainerLow = DarkTokens.FloatingPanelBg,
    surfaceContainer = DarkTokens.FloatingPanelBg,
    surfaceContainerHigh = DarkTokens.Soft,
    surfaceContainerHighest = DarkTokens.Line,
    surfaceBright = DarkTokens.Accent,
    surfaceDim = DarkTokens.PageBg,
    surfaceTint = DarkTokens.Card,
    inverseSurface = InkDark,
    inverseOnSurface = DarkTokens.Card,
    inversePrimary = DarkTokens.Line,
    outline = DarkTokens.Subtle,
    outlineVariant = DarkTokens.Line,
    error = DarkTokens.StatusDanger,
    onError = ContentOnAction,
    errorContainer = DarkTokens.Soft,
    onErrorContainer = DarkTokens.StatusDanger,
    scrim = DarkTokens.Scrim,
)

/**
 * Palette entries Material has no role for.
 *
 * [success] is the connection dot's "connected" green — it is not `primary`,
 * because primary here is near-black ink and a black dot reads as "off".
 */
internal data class BitFunColors(
    val transparent: Color,
    val statusSuccess: Color,
    val shellScrim: Color,
    val mediaBackground: Color,
    val mediaScrim: Color,
    val mediaControlBackground: Color,
    val toastBackground: Color,
    val shadowSubtle: Color,
    val shadowMedium: Color,
    val shadowStrong: Color,
    val floatingBorder: Color,
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
    transparent = LightTokens.Transparent,
    statusSuccess = LightTokens.StatusSuccess,
    shellScrim = LightTokens.ShellScrim,
    mediaBackground = LightTokens.MediaBackground,
    mediaScrim = LightTokens.MediaScrim,
    mediaControlBackground = LightTokens.MediaControlBackground,
    toastBackground = LightTokens.ToastBackground,
    shadowSubtle = LightTokens.ShadowSubtle,
    shadowMedium = LightTokens.ShadowMedium,
    shadowStrong = LightTokens.ShadowStrong,
    floatingBorder = LightTokens.FloatingBorder,
    heroBackground = LightTokens.ConnectHeroBg,
    heroSurface = LightTokens.ConnectHeroSurface,
    heroAccent = LightTokens.ConnectHeroAccent,
    heroSecondary = LightTokens.ConnectHeroSecondary,
    code = CodeSyntaxColors(
        lineNumber = LightTokens.CodeLineNumber,
        keyword = LightTokens.CodeKeyword,
        string = LightTokens.CodeString,
        number = LightTokens.CodeNumber,
        comment = LightTokens.CodeComment,
        function = LightTokens.CodeFunction,
        type = LightTokens.CodeType,
        constant = LightTokens.CodeConstant,
        property = LightTokens.CodeProperty,
        targetBackground = LightTokens.CodeTargetBg,
    ),
)

private val DarkExtras = BitFunColors(
    transparent = DarkTokens.Transparent,
    statusSuccess = DarkTokens.StatusSuccess,
    shellScrim = DarkTokens.ShellScrim,
    mediaBackground = DarkTokens.MediaBackground,
    mediaScrim = DarkTokens.MediaScrim,
    mediaControlBackground = DarkTokens.MediaControlBackground,
    toastBackground = DarkTokens.ToastBackground,
    shadowSubtle = DarkTokens.ShadowSubtle,
    shadowMedium = DarkTokens.ShadowMedium,
    shadowStrong = DarkTokens.ShadowStrong,
    floatingBorder = DarkTokens.FloatingBorder,
    heroBackground = DarkTokens.ConnectHeroBg,
    heroSurface = DarkTokens.ConnectHeroSurface,
    heroAccent = DarkTokens.ConnectHeroAccent,
    heroSecondary = DarkTokens.ConnectHeroSecondary,
    code = CodeSyntaxColors(
        lineNumber = DarkTokens.CodeLineNumber,
        keyword = DarkTokens.CodeKeyword,
        string = DarkTokens.CodeString,
        number = DarkTokens.CodeNumber,
        comment = DarkTokens.CodeComment,
        function = DarkTokens.CodeFunction,
        type = DarkTokens.CodeType,
        constant = DarkTokens.CodeConstant,
        property = DarkTokens.CodeProperty,
        targetBackground = DarkTokens.CodeTargetBg,
    ),
)

private val LocalBitFunColors = staticCompositionLocalOf { LightExtras }

/**
 * Harmony's mobile surfaces use a small, explicit type scale rather than the
 * platform Material defaults. Keeping the roles here makes every remaining
 * Material control start from the same geometry as the ArkUI counterpart.
 */
private val BitFunTypography = androidx.compose.material3.Typography(
    displayLarge = MobileDesignTypography.DisplayLarge,
    displayMedium = MobileDesignTypography.DisplayMedium,
    displaySmall = MobileDesignTypography.DisplaySmall,
    headlineLarge = MobileDesignTypography.HeadlineLarge,
    headlineMedium = MobileDesignTypography.HeadlineMedium,
    headlineSmall = MobileDesignTypography.HeadlineSmall,
    titleLarge = MobileDesignTypography.TitleLarge,
    titleMedium = MobileDesignTypography.TitleMedium,
    titleSmall = MobileDesignTypography.TitleSmall,
    bodyLarge = MobileDesignTypography.BodyLarge,
    bodyMedium = MobileDesignTypography.BodyMedium,
    bodySmall = MobileDesignTypography.BodySmall,
    labelLarge = MobileDesignTypography.LabelLarge,
    labelMedium = MobileDesignTypography.LabelMedium,
    labelSmall = MobileDesignTypography.LabelSmall,
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
