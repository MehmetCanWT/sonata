/*
 * Basic Theme (Clean Monochrome Editorial Overhaul) for Sonata Android.
 *
 * Uses pure monochrome dark/light surfaces:
 * - Dark: #000000 canvas, #0D0D0F elevated surface, #1C1C1E grouped cards, #8E8E93 muted text, #2C2C2E hairlines, #FFFFFF primary
 * - Light: #FFFFFF canvas, #F7F7F9 surface, #F2F2F7 grouped cards, #6E6E73 muted text, #E5E5EA hairlines, #000000 primary
 * - Zero purple atmospheric orbs or colored backdrop washes; clean 12-14dp card geometry and signature #FA2D48 emphasis accent.
 */
package dev.sonata.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import dev.sonata.android.data.DesignTheme

/** Monochrome Basic palette & signature crimson emphasis accent. */
object BasicPalette {
    val AccentRed = Color(0xFFFA2D48)

    // Dark scheme
    val DarkBackground = Color(0xFF000000)
    val DarkSurface = Color(0xFF0D0D0F)
    val DarkSurfaceVariant = Color(0xFF1C1C1E)
    val DarkOnSurfaceVariant = Color(0xFF8E8E93)
    val DarkOutline = Color(0xFF2C2C2E)

    // Light scheme
    val LightBackground = Color(0xFFFFFFFF)
    val LightSurface = Color(0xFFF7F7F9)
    val LightSurfaceVariant = Color(0xFFF2F2F7)
    val LightOnSurfaceVariant = Color(0xFF6E6E73)
    val LightOutline = Color(0xFFE5E5EA)
}

val BasicDarkColorScheme: ColorScheme = darkColorScheme(
    primary = Color.White,
    onPrimary = Color.Black,
    primaryContainer = BasicPalette.DarkSurfaceVariant,
    onPrimaryContainer = Color.White,
    secondary = BasicPalette.DarkOnSurfaceVariant,
    onSecondary = Color.Black,
    secondaryContainer = BasicPalette.DarkSurfaceVariant,
    onSecondaryContainer = Color.White,
    tertiary = BasicPalette.AccentRed,
    onTertiary = Color.White,
    background = BasicPalette.DarkBackground,
    onBackground = Color.White,
    surface = BasicPalette.DarkSurface,
    onSurface = Color.White,
    surfaceVariant = BasicPalette.DarkSurfaceVariant,
    onSurfaceVariant = BasicPalette.DarkOnSurfaceVariant,
    outline = BasicPalette.DarkOutline,
    outlineVariant = BasicPalette.DarkOutline,
)

val BasicLightColorScheme: ColorScheme = lightColorScheme(
    primary = Color.Black,
    onPrimary = Color.White,
    primaryContainer = BasicPalette.LightSurfaceVariant,
    onPrimaryContainer = Color.Black,
    secondary = BasicPalette.LightOnSurfaceVariant,
    onSecondary = Color.White,
    secondaryContainer = BasicPalette.LightSurfaceVariant,
    onSecondaryContainer = Color.Black,
    tertiary = BasicPalette.AccentRed,
    onTertiary = Color.White,
    background = BasicPalette.LightBackground,
    onBackground = Color.Black,
    surface = BasicPalette.LightSurface,
    onSurface = Color.Black,
    surfaceVariant = BasicPalette.LightSurfaceVariant,
    onSurfaceVariant = BasicPalette.LightOnSurfaceVariant,
    outline = BasicPalette.LightOutline,
    outlineVariant = BasicPalette.LightOutline,
)

val BasicDarkTokens = LunariaThemeTokens(
    designTheme = DesignTheme.BASIC,
    isDark = true,
    isAmoled = true,
    moonGlow = Color.White,
    silverPetal = BasicPalette.DarkOnSurfaceVariant,
    petalLavender = Color.White,
    lunarMist = BasicPalette.DarkOnSurfaceVariant,
    eclipseBase = BasicPalette.DarkBackground,
    eclipseSurface = BasicPalette.DarkSurface,
    eclipseCard = BasicPalette.DarkSurfaceVariant,
    accountGradientTop = BasicPalette.DarkSurfaceVariant,
    accountGradientBottom = BasicPalette.DarkBackground,
    settingsBg = BasicPalette.DarkBackground,
    settingsCard = BasicPalette.DarkSurfaceVariant,
    settingsDivider = BasicPalette.DarkOutline,
    onSurfacePrimary = Color.White,
    onSurfaceSecondary = BasicPalette.DarkOnSurfaceVariant,
    cardStroke = BasicPalette.DarkOutline,
    accentGradient = Brush.linearGradient(listOf(Color.White, Color(0xFFD1D1D6))),
    accountSheetBrush = Brush.verticalGradient(listOf(BasicPalette.DarkSurface, BasicPalette.DarkBackground)),
    pageBackdropBrush = Brush.verticalGradient(listOf(BasicPalette.DarkBackground, BasicPalette.DarkBackground)),
    glassTint = Color(0xFF121212).copy(alpha = 0.40f),
    glassRimTopLeft = Color.White.copy(alpha = 0.22f),
    glassRimBottomRight = Color.White.copy(alpha = 0.08f),
    glassShadowColor = Color.Black.copy(alpha = 0.40f),
    pillIndicatorColor = Color.White.copy(alpha = 0.14f),
    pillIndicatorRimColor = Color.White.copy(alpha = 0.20f),
    heroCardScrimColor = Color.Black,
    cardCornerRadius = 12.dp,
    miniPlayerHeight = 56.dp,
    miniPlayerShape = RoundedCornerShape(percent = 50),
    navBarShape = RoundedCornerShape(percent = 50),
    tabPillShape = RoundedCornerShape(percent = 50),
    heroFullBleed = true,
    openLatestReleaseStack = false,
    showTrackDividers = true,
)

val BasicLightTokens = LunariaThemeTokens(
    designTheme = DesignTheme.BASIC,
    isDark = false,
    isAmoled = false,
    moonGlow = Color.Black,
    silverPetal = BasicPalette.LightOnSurfaceVariant,
    petalLavender = Color.Black,
    lunarMist = BasicPalette.LightOnSurfaceVariant,
    eclipseBase = BasicPalette.LightBackground,
    eclipseSurface = BasicPalette.LightSurface,
    eclipseCard = BasicPalette.LightSurfaceVariant,
    accountGradientTop = BasicPalette.LightSurfaceVariant,
    accountGradientBottom = BasicPalette.LightBackground,
    settingsBg = BasicPalette.LightBackground,
    settingsCard = BasicPalette.LightSurfaceVariant,
    settingsDivider = BasicPalette.LightOutline,
    onSurfacePrimary = Color.Black,
    onSurfaceSecondary = BasicPalette.LightOnSurfaceVariant,
    cardStroke = BasicPalette.LightOutline,
    accentGradient = Brush.linearGradient(listOf(Color.Black, Color(0xFF3A3A3C))),
    accountSheetBrush = Brush.verticalGradient(listOf(BasicPalette.LightSurface, BasicPalette.LightBackground)),
    pageBackdropBrush = Brush.verticalGradient(listOf(BasicPalette.LightBackground, BasicPalette.LightBackground)),
    glassTint = Color(0xFFFAFAFA).copy(alpha = 0.40f),
    glassRimTopLeft = Color.White.copy(alpha = 0.55f),
    glassRimBottomRight = Color.White.copy(alpha = 0.14f),
    glassShadowColor = Color.Black.copy(alpha = 0.14f),
    pillIndicatorColor = Color.White.copy(alpha = 0.50f),
    pillIndicatorRimColor = Color.White.copy(alpha = 0.65f),
    heroCardScrimColor = Color(0xFF1C1C1E),
    cardCornerRadius = 12.dp,
    miniPlayerHeight = 56.dp,
    miniPlayerShape = RoundedCornerShape(percent = 50),
    navBarShape = RoundedCornerShape(percent = 50),
    tabPillShape = RoundedCornerShape(percent = 50),
    heroFullBleed = true,
    openLatestReleaseStack = false,
    showTrackDividers = true,
)
