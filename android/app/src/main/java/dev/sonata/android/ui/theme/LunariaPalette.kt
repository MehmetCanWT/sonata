/*
 * Lunaria Moon Palette & Open-Source App Typography for Sonata Android.
 *
 * Inspired by Lunaria annua (honesty / silver dollar / moonflower) under
 * nocturnal moonlight: translucent mother-of-pearl silver-lilac, luminous
 * moonstone, muted periwinkle-violet, and deep lunar eclipse obsidian.
 *
 * Typography uses Plus Jakarta Sans (SIL Open Font License 1.1) for app UI
 * chrome while leaving the synced Lyrics display font completely untouched.
 */
package dev.sonata.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import dev.sonata.android.R

/**
 * Botanical & nocturnal moonlight color tokens for Sonata's Lunaria theme.
 *
 * Designed for flat sheets (Account sheet gradient), neutral Settings cards,
 * and glass-compatible dark/light/AMOLED surfaces with crisp text contrast.
 */
object LunariaColors {
    // ---- Core Lunaria Botanical & Moonstone Accents ----

    /** Luminous moonstone highlight — primary luminous silver-pearl tone. */
    val MoonGlow = Color(0xFFEDE8F8)

    /** Translucent silver-dollar seed pod tone — muted pearl-lilac for secondary labels & borders. */
    val SilverPetal = Color(0xFFC9C2DE)

    /** Spring Lunaria blossom hue — soft twilight lavender-violet accent. */
    val PetalLavender = Color(0xFFB79CED)

    /** Iridescent periwinkle-violet mist — cool moonlight glow for focus rings & subtle halos. */
    val LunarMist = Color(0xFF8F96D9)

    // ---- Nocturnal Eclipse Surfaces (Dark Mode) ----

    /** Deep lunar eclipse indigo-obsidian — primary dark screen floor. */
    val EclipseBase = Color(0xFF0A0910)

    /** Elevated nocturnal indigo-slate — cards, sheets, and resting bars. */
    val EclipseSurface = Color(0xFF14121E)

    /** Interactive card & pill fill on dark eclipse surfaces. */
    val EclipseCard = Color(0xFF1D1A2B)

    // ---- Account Sheet Flat Gradient (NO Liquid Glass) ----

    /** Top stop of the Account sheet's flat nocturnal Lunaria gradient. */
    val AccountGradientTop = Color(0xFF1A162B)

    /** Bottom stop of the Account sheet's flat nocturnal Lunaria gradient. */
    val AccountGradientBottom = Color(0xFF0B0A12)

    // ---- Settings Screen Flat Neutral Grays (NO Liquid Glass) ----

    /** Flat pure-black Settings background matching dark Android reference screens. */
    val SettingsBg = Color(0xFF000000)

    /** Flat neutral dark-gray card fill for Settings groups. */
    val SettingsCard = Color(0xFF161618)

    /** Subtle neutral divider inside Settings cards. */
    val SettingsDivider = Color(0xFF262629)

    // ---- Light Mode Lunaria Mother-of-Pearl Surfaces ----

    /** Soft pearlescent alabaster background for Light mode. */
    val PearlBase = Color(0xFFF9F8FC)

    /** Crisp translucent-feel light surface for sheets and elevated bars. */
    val PearlSurface = Color(0xFFF1EFF7)

    /** Light mode card fill with subtle silver-lilac undertone. */
    val PearlCard = Color(0xFFE8E4F2)

    /** High-contrast twilight ink for primary text in Light mode. */
    val PearlInk = Color(0xFF161320)

    /** Muted slate-plum for secondary metadata in Light mode. */
    val PearlMutedInk = Color(0xFF5D5770)

    /** Rich violet-indigo primary accent for Light mode legibility. */
    val PetalVioletDeep = Color(0xFF6B4EC4)

    // ---- Gradients ----

    /** Signature Lunaria gradient (PetalLavender -> LunarMist) for active CTAs and highlights. */
    val AccentGradient: Brush = Brush.linearGradient(
        colors = listOf(PetalLavender, LunarMist),
    )

    /** Flat nocturnal gradient exclusively for the Account sheet backdrop. */
    val AccountSheetGradient: Brush = Brush.verticalGradient(
        colors = listOf(AccountGradientTop, AccountGradientBottom),
    )
}

/**
 * Resolved Lunaria palette tokens for the active theme variant (Dark, AMOLED, or Light).
 */
@Immutable
data class LunariaThemeTokens(
    val isDark: Boolean,
    val isAmoled: Boolean,
    val moonGlow: Color,
    val silverPetal: Color,
    val petalLavender: Color,
    val lunarMist: Color,
    val eclipseBase: Color,
    val eclipseSurface: Color,
    val eclipseCard: Color,
    val accountGradientTop: Color,
    val accountGradientBottom: Color,
    val settingsBg: Color,
    val settingsCard: Color,
    val settingsDivider: Color,
    val onSurfacePrimary: Color,
    val onSurfaceSecondary: Color,
    val cardStroke: Color,
    val accentGradient: Brush,
    val accountSheetBrush: Brush,
)

/** Dark-mode Lunaria tokens (nocturnal eclipse indigo-obsidian). */
val LunariaDarkTokens = LunariaThemeTokens(
    isDark = true,
    isAmoled = false,
    moonGlow = LunariaColors.MoonGlow,
    silverPetal = LunariaColors.SilverPetal,
    petalLavender = LunariaColors.PetalLavender,
    lunarMist = LunariaColors.LunarMist,
    eclipseBase = LunariaColors.EclipseBase,
    eclipseSurface = LunariaColors.EclipseSurface,
    eclipseCard = LunariaColors.EclipseCard,
    accountGradientTop = LunariaColors.AccountGradientTop,
    accountGradientBottom = LunariaColors.AccountGradientBottom,
    settingsBg = LunariaColors.SettingsBg,
    settingsCard = LunariaColors.SettingsCard,
    settingsDivider = LunariaColors.SettingsDivider,
    onSurfacePrimary = LunariaColors.MoonGlow,
    onSurfaceSecondary = LunariaColors.SilverPetal.copy(alpha = 0.78f),
    cardStroke = LunariaColors.SilverPetal.copy(alpha = 0.12f),
    accentGradient = LunariaColors.AccentGradient,
    accountSheetBrush = LunariaColors.AccountSheetGradient,
)

/** AMOLED-mode Lunaria tokens (true `#000000` background with deep obsidian-lilac elevation). */
val LunariaAmoledTokens = LunariaDarkTokens.copy(
    isAmoled = true,
    eclipseBase = Color.Black,
    eclipseSurface = Color(0xFF0C0B12),
    eclipseCard = Color(0xFF151320),
    accountGradientTop = Color(0xFF12101D),
    accountGradientBottom = Color.Black,
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFF12101D), Color.Black),
    ),
)

/** Light-mode Lunaria tokens (mother-of-pearl alabaster with deep twilight-violet ink). */
val LunariaLightTokens = LunariaThemeTokens(
    isDark = false,
    isAmoled = false,
    moonGlow = LunariaColors.PearlInk,
    silverPetal = LunariaColors.PearlMutedInk,
    petalLavender = LunariaColors.PetalVioletDeep,
    lunarMist = Color(0xFF555FB8),
    eclipseBase = LunariaColors.PearlBase,
    eclipseSurface = LunariaColors.PearlSurface,
    eclipseCard = LunariaColors.PearlCard,
    accountGradientTop = Color(0xFFF3EFFB),
    accountGradientBottom = Color(0xFFE7E2F3),
    settingsBg = Color(0xFFF2F2F7),
    settingsCard = Color.White,
    settingsDivider = Color(0xFFE2E0EA),
    onSurfacePrimary = LunariaColors.PearlInk,
    onSurfaceSecondary = LunariaColors.PearlMutedInk,
    cardStroke = LunariaColors.PearlInk.copy(alpha = 0.08f),
    accentGradient = Brush.linearGradient(
        colors = listOf(LunariaColors.PetalVioletDeep, Color(0xFF555FB8)),
    ),
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFFF3EFFB), Color(0xFFE7E2F3)),
    ),
)

/** CompositionLocal supplying the active [LunariaThemeTokens]. */
val LocalLunariaColors = staticCompositionLocalOf { LunariaDarkTokens }

/** Material 3 Dark color scheme tuned to Lunaria tokens. */
val LunariaDarkColorScheme: ColorScheme = darkColorScheme(
    primary = LunariaColors.MoonGlow,
    onPrimary = LunariaColors.EclipseBase,
    secondary = LunariaColors.PetalLavender,
    onSecondary = LunariaColors.EclipseBase,
    tertiary = LunariaColors.LunarMist,
    onTertiary = LunariaColors.EclipseBase,
    background = LunariaColors.EclipseBase,
    onBackground = LunariaColors.MoonGlow,
    surface = LunariaColors.EclipseSurface,
    onSurface = LunariaColors.MoonGlow,
    surfaceVariant = LunariaColors.EclipseCard,
    onSurfaceVariant = LunariaColors.SilverPetal,
    outline = LunariaColors.SilverPetal.copy(alpha = 0.20f),
    outlineVariant = LunariaColors.SilverPetal.copy(alpha = 0.10f),
)

/** Material 3 AMOLED color scheme tuned to Lunaria tokens. */
val LunariaAmoledColorScheme: ColorScheme = darkColorScheme(
    primary = LunariaColors.MoonGlow,
    onPrimary = Color.Black,
    secondary = LunariaColors.PetalLavender,
    onSecondary = Color.Black,
    tertiary = LunariaColors.LunarMist,
    onTertiary = Color.Black,
    background = Color.Black,
    onBackground = LunariaColors.MoonGlow,
    surface = Color(0xFF0C0B12),
    onSurface = LunariaColors.MoonGlow,
    surfaceVariant = Color(0xFF151320),
    onSurfaceVariant = LunariaColors.SilverPetal,
    outline = LunariaColors.SilverPetal.copy(alpha = 0.18f),
    outlineVariant = LunariaColors.SilverPetal.copy(alpha = 0.09f),
)

/** Material 3 Light color scheme tuned to Lunaria tokens. */
val LunariaLightColorScheme: ColorScheme = lightColorScheme(
    primary = LunariaColors.PearlInk,
    onPrimary = Color.White,
    secondary = LunariaColors.PetalVioletDeep,
    onSecondary = Color.White,
    tertiary = Color(0xFF555FB8),
    onTertiary = Color.White,
    background = LunariaColors.PearlBase,
    onBackground = LunariaColors.PearlInk,
    surface = LunariaColors.PearlSurface,
    onSurface = LunariaColors.PearlInk,
    surfaceVariant = LunariaColors.PearlCard,
    onSurfaceVariant = LunariaColors.PearlMutedInk,
    outline = Color(0xFFD5CFE3),
    outlineVariant = Color(0xFFE6E1F0),
)

/** Returns the Material 3 [ColorScheme] for the requested theme mode. */
fun lunariaColorScheme(darkTheme: Boolean, amoled: Boolean = false): ColorScheme = when {
    !darkTheme -> LunariaLightColorScheme
    amoled -> LunariaAmoledColorScheme
    else -> LunariaDarkColorScheme
}

/** Returns the [LunariaThemeTokens] for the requested theme mode. */
fun lunariaTokens(darkTheme: Boolean, amoled: Boolean = false): LunariaThemeTokens = when {
    !darkTheme -> LunariaLightTokens
    amoled -> LunariaAmoledTokens
    else -> LunariaDarkTokens
}

// =============================================================================
// App-Wide Open-Source Font Family (Plus Jakarta Sans, SIL OFL 1.1)
// Note: Existing Lyrics display font in SonataDesignSystem is kept untouched.
// =============================================================================

/**
 * App-wide geometric-humanist sans-serif family backed by Plus Jakarta Sans
 * (SIL Open Font License 1.1). Maps W400..W800 cleanly to the bundled static cuts.
 */
val SonataAppFontFamily: FontFamily = FontFamily(
    Font(R.font.sonata_sans_regular, FontWeight.Normal),
    Font(R.font.sonata_sans_medium, FontWeight.Medium),
    Font(R.font.sonata_sans_semibold, FontWeight.SemiBold),
    Font(R.font.sonata_sans_bold, FontWeight.Bold),
    Font(R.font.sonata_sans_bold, FontWeight.ExtraBold),
)

/**
 * App-wide Material 3 [Typography] built on [SonataAppFontFamily] (Plus Jakarta Sans).
 * Distinct from the synced Lyrics rendering typography, which remains untouched.
 */
val SonataTypography: Typography = Typography(
    displayLarge = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.ExtraBold,
        fontSize = 34.sp,
        lineHeight = 40.sp,
        letterSpacing = (-0.5).sp,
    ),
    displayMedium = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 28.sp,
        lineHeight = 34.sp,
        letterSpacing = (-0.35).sp,
    ),
    displaySmall = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 24.sp,
        lineHeight = 30.sp,
        letterSpacing = (-0.25).sp,
    ),
    headlineLarge = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.ExtraBold,
        fontSize = 30.sp,
        lineHeight = 36.sp,
        letterSpacing = (-0.4).sp,
    ),
    headlineMedium = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 22.sp,
        lineHeight = 28.sp,
        letterSpacing = (-0.25).sp,
    ),
    headlineSmall = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 20.sp,
        lineHeight = 26.sp,
        letterSpacing = (-0.15).sp,
    ),
    titleLarge = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 20.sp,
        lineHeight = 25.sp,
        letterSpacing = (-0.2).sp,
    ),
    titleMedium = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 16.sp,
        lineHeight = 22.sp,
        letterSpacing = 0.sp,
    ),
    titleSmall = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.05.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        lineHeight = 23.sp,
        letterSpacing = 0.1.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.15.sp,
    ),
    bodySmall = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 12.sp,
        lineHeight = 16.sp,
        letterSpacing = 0.2.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.1.sp,
    ),
    labelMedium = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 12.sp,
        lineHeight = 16.sp,
        letterSpacing = 0.2.sp,
    ),
    labelSmall = TextStyle(
        fontFamily = SonataAppFontFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 11.sp,
        lineHeight = 14.sp,
        letterSpacing = 0.25.sp,
    ),
)
