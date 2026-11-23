/*
 * Shared Theme Contract, Lunaria Color Constants, & App Typography for Sonata Android.
 *
 * Theme implementations live in dedicated files:
 * - VioletTheme.kt (Light, Dark, AMOLED tokens, color schemes & Layer-Blur canvas)
 * - MoonflowerTheme.kt (Light, Dark, AMOLED tokens, color schemes & silk petal canvas)
 */
package dev.sonata.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Typography
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.R
import dev.sonata.android.data.DesignTheme

/** Core shared botanical & moonlight color constants referenced across components. */
object LunariaColors {
    val MoonGlow = Color(0xFFEDE8F8)
    val SilverPetal = Color(0xFFC9C2DE)
    val PetalLavender = Color(0xFFB79CED)
    val LunarMist = Color(0xFF8F96D9)

    val EclipseBase = Color(0xFF0A0910)
    val EclipseSurface = Color(0xFF14121E)
    val EclipseCard = Color(0xFF1D1A2B)

    val AccountGradientTop = Color(0xFF1A162B)
    val AccountGradientBottom = Color(0xFF0B0A12)

    val SettingsBg = Color(0xFF000000)
    val SettingsCard = Color(0xFF161618)
    val SettingsDivider = Color(0xFF262629)

    val PearlBase = Color(0xFFFFFFFF)
    val PearlSurface = Color(0xFFF2F2F7)
    val PearlCard = Color(0xFFEBEBF0)
    val PearlInk = Color(0xFF1C1C1E)
    val PearlMutedInk = Color(0xFF6C6C70)
    val PetalVioletDeep = Color(0xFF6B4EC4)

    val AccentGradient: Brush = Brush.linearGradient(
        colors = listOf(PetalLavender, LunarMist),
    )
}

/**
 * Resolved palette & structural design tokens for the active theme variant
 * (Violet or Moonflower across Light, Dark, and AMOLED modes).
 */
@Immutable
data class LunariaThemeTokens(
    val designTheme: DesignTheme = DesignTheme.BASIC,
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
    val pageBackdropBrush: Brush = Brush.verticalGradient(
        colors = listOf(eclipseSurface, eclipseBase),
    ),
    val glassTint: Color = if (isDark) {
        Color(0xFF121212).copy(alpha = 0.40f)
    } else {
        Color(0xFFFAFAFA).copy(alpha = 0.40f)
    },
    val glassRimTopLeft: Color = Color.White.copy(alpha = if (isDark) 0.22f else 0.72f),
    val glassRimBottomRight: Color = Color.White.copy(alpha = if (isDark) 0.08f else 0.24f),
    val glassShadowColor: Color = Color.Black.copy(alpha = if (isDark) 0.40f else 0.14f),
    val pillIndicatorColor: Color = Color.White.copy(alpha = if (isDark) 0.14f else 0.50f),
    val pillIndicatorRimColor: Color = Color.White.copy(alpha = if (isDark) 0.20f else 0.65f),
    val heroCardScrimColor: Color = Color(0xFF4B356B),
    val cardCornerRadius: Dp = 20.dp,
    val miniPlayerHeight: Dp = 54.dp,
    val miniPlayerShape: RoundedCornerShape = RoundedCornerShape(percent = 50),
    val navBarShape: RoundedCornerShape = RoundedCornerShape(percent = 50),
    val tabPillShape: RoundedCornerShape = RoundedCornerShape(percent = 50),
    val heroFullBleed: Boolean = false,
    val openLatestReleaseStack: Boolean = false,
    val showTrackDividers: Boolean = true,
) {
    val isViolet: Boolean get() = designTheme == DesignTheme.VIOLET
    val isMoonflower: Boolean get() = designTheme == DesignTheme.MOONFLOWER
    val isBasic: Boolean get() = designTheme == DesignTheme.BASIC
    val cardShape: Shape get() = RoundedCornerShape(cardCornerRadius)
}

/** CompositionLocal supplying the active [LunariaThemeTokens]. */
val LocalLunariaColors = staticCompositionLocalOf { BasicDarkTokens }

/** Draws the active theme's atmospheric canvas backdrop (Violet orbs, Moonflower silk petals, or clean solid Basic). */
fun DrawScope.drawThemeAtmosphericBackdrop(
    designTheme: DesignTheme,
    isDark: Boolean,
    accent: Color,
    mist: Color,
) {
    when (designTheme) {
        DesignTheme.VIOLET -> drawVioletAtmosphericBackdrop(isDark = isDark, accent = accent, mist = mist)
        DesignTheme.MOONFLOWER -> drawMoonflowerAtmosphericBackdrop(isDark = isDark, accent = accent, mist = mist)
        DesignTheme.BASIC -> Unit
    }
}

/** Returns the Material 3 [ColorScheme] for the requested theme mode. */
fun lunariaColorScheme(
    darkTheme: Boolean,
    amoled: Boolean = false,
    designTheme: DesignTheme = DesignTheme.BASIC,
): ColorScheme = when (designTheme) {
    DesignTheme.VIOLET -> when {
        amoled -> VioletAmoledColorScheme
        darkTheme -> VioletDarkColorScheme
        else -> VioletLightColorScheme
    }
    DesignTheme.MOONFLOWER -> when {
        amoled -> MoonflowerAmoledColorScheme
        darkTheme -> MoonflowerDarkColorScheme
        else -> MoonflowerLightColorScheme
    }
    DesignTheme.BASIC -> if (darkTheme || amoled) BasicDarkColorScheme else BasicLightColorScheme
}

/** Returns the [LunariaThemeTokens] for the requested theme mode. */
fun lunariaTokens(
    darkTheme: Boolean,
    amoled: Boolean = false,
    designTheme: DesignTheme = DesignTheme.BASIC,
): LunariaThemeTokens = when (designTheme) {
    DesignTheme.VIOLET -> when {
        amoled -> VioletAmoledTokens
        darkTheme -> VioletDarkTokens
        else -> VioletLightTokens
    }
    DesignTheme.MOONFLOWER -> when {
        amoled -> MoonflowerAmoledTokens
        darkTheme -> MoonflowerDarkTokens
        else -> MoonflowerLightTokens
    }
    DesignTheme.BASIC -> if (darkTheme || amoled) BasicDarkTokens else BasicLightTokens
}

// =============================================================================
// App-Wide Open-Source Font Family (Plus Jakarta Sans, SIL OFL 1.1)
// =============================================================================

val SonataAppFontFamily: FontFamily = FontFamily(
    Font(R.font.sonata_sans_regular, FontWeight.Normal),
    Font(R.font.sonata_sans_medium, FontWeight.Medium),
    Font(R.font.sonata_sans_semibold, FontWeight.SemiBold),
    Font(R.font.sonata_sans_bold, FontWeight.Bold),
    Font(R.font.sonata_sans_bold, FontWeight.ExtraBold),
)

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
