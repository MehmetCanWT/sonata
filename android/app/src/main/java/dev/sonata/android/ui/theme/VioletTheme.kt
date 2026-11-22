/*
 * Dedicated Violet Theme System for Sonata Android.
 *
 * Inspired by research/theme/Violet.png, violet artist.png, and effects.jpg:
 * radiant orchid/amethyst mist Layer-Blur atmospheric canvas, sculpted squircle
 * surfaces (14dp cards, 26-28dp rounded floating chrome), full-bleed artist
 * hero covers, and open double-sleeve Latest Release presentation across Light,
 * Dark, and AMOLED modes.
 */
package dev.sonata.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.sonata.android.data.DesignTheme

/** Structural layout & shape metrics unique to the Violet design language. */
object VioletDesignMetrics {
    val cardCornerRadius: Dp = 14.dp
    val miniPlayerHeight: Dp = 64.dp
    val miniPlayerCornerRadius: Dp = 26.dp
    val navBarCornerRadius: Dp = 28.dp
    val tabPillCornerRadius: Dp = 18.dp
    const val heroFullBleed: Boolean = true
    const val openLatestReleaseStack: Boolean = true
    const val showTrackDividers: Boolean = false

    val cardShape: RoundedCornerShape = RoundedCornerShape(cardCornerRadius)
    val miniPlayerShape: RoundedCornerShape = RoundedCornerShape(miniPlayerCornerRadius)
    val navBarShape: RoundedCornerShape = RoundedCornerShape(navBarCornerRadius)
    val tabPillShape: RoundedCornerShape = RoundedCornerShape(tabPillCornerRadius)
}

/** Draws the Violet signature Figma Layer-Blur radiant orchid glow orbs across the upper canvas. */
fun DrawScope.drawVioletAtmosphericBackdrop(
    isDark: Boolean,
    accent: Color,
    mist: Color,
) {
    val w = size.width
    val h = size.height
    if (w <= 0f || h <= 0f) return

    val primaryGlow = if (isDark) accent else Color(0xFF9B5DE5)
    val secondaryGlow = if (isDark) mist else Color(0xFFD59BF6)
    val magentaAura = if (isDark) Color(0xFF9146D4) else Color(0xFFE29AF7)
    val orbAlpha = if (isDark) 0.36f else 0.48f

    drawCircle(
        brush = Brush.radialGradient(
            colors = listOf(
                primaryGlow.copy(alpha = orbAlpha),
                secondaryGlow.copy(alpha = orbAlpha * 0.56f),
                Color.Transparent,
            ),
            center = Offset(w * 0.20f, h * 0.08f),
            radius = w * 0.86f,
        ),
        radius = w * 0.86f,
        center = Offset(w * 0.20f, h * 0.08f),
    )
    drawCircle(
        brush = Brush.radialGradient(
            colors = listOf(
                magentaAura.copy(alpha = orbAlpha * 0.78f),
                primaryGlow.copy(alpha = orbAlpha * 0.28f),
                Color.Transparent,
            ),
            center = Offset(w * 0.88f, h * 0.17f),
            radius = w * 0.68f,
        ),
        radius = w * 0.68f,
        center = Offset(w * 0.88f, h * 0.17f),
    )
    if (!isDark) {
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(
                    Color(0xFFC38EF5).copy(alpha = 0.26f),
                    Color.Transparent,
                ),
                center = Offset(w * 0.52f, h * 0.38f),
                radius = w * 0.72f,
            ),
            radius = w * 0.72f,
            center = Offset(w * 0.52f, h * 0.38f),
        )
    }
}

/** Violet Light tokens (`Violet.png`, `violet artist.png`, `effects.jpg`). */
val VioletLightTokens = LunariaThemeTokens(
    designTheme = DesignTheme.VIOLET,
    isDark = false,
    isAmoled = false,
    moonGlow = Color(0xFF211333),
    silverPetal = Color(0xFF6B5387),
    petalLavender = Color(0xFF7745B5),
    lunarMist = Color(0xFFA778E3),
    eclipseBase = Color(0xFFF3EBFD),
    eclipseSurface = Color(0xFFE7D9FA),
    eclipseCard = Color(0xFFDEC8F8),
    accountGradientTop = Color(0xFFE8D8FC),
    accountGradientBottom = Color(0xFFF7F1FD),
    settingsBg = Color(0xFFF6F1FC),
    settingsCard = Color.White,
    settingsDivider = Color(0xFFE7DCF5),
    onSurfacePrimary = Color(0xFF211333),
    onSurfaceSecondary = Color(0xFF6B5387),
    cardStroke = Color(0xFFAF82E8).copy(alpha = 0.45f),
    accentGradient = Brush.linearGradient(
        colors = listOf(Color(0xFF7745B5), Color(0xFFA778E3)),
    ),
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFFE8D8FC), Color(0xFFF7F1FD)),
    ),
    pageBackdropBrush = Brush.verticalGradient(
        0.0f to Color(0xFFD6B8F8),
        0.34f to Color(0xFFEBDDFC),
        0.72f to Color(0xFFF5EEFD),
        1.0f to Color(0xFFFAF6FE),
    ),
    glassTint = Color(0xFFF3E8FD).copy(alpha = 0.36f),
    glassRimTopLeft = Color.White.copy(alpha = 0.94f),
    glassRimBottomRight = Color(0xFFB58AE8).copy(alpha = 0.56f),
    glassShadowColor = Color(0xFF7D4CC2).copy(alpha = 0.28f),
    pillIndicatorColor = Color(0xFFE894DC).copy(alpha = 0.36f),
    pillIndicatorRimColor = Color.White.copy(alpha = 0.94f),
    heroCardScrimColor = Color(0xFF3B1E6D),
    cardCornerRadius = VioletDesignMetrics.cardCornerRadius,
    miniPlayerHeight = VioletDesignMetrics.miniPlayerHeight,
    miniPlayerShape = VioletDesignMetrics.miniPlayerShape,
    navBarShape = VioletDesignMetrics.navBarShape,
    tabPillShape = VioletDesignMetrics.tabPillShape,
    heroFullBleed = VioletDesignMetrics.heroFullBleed,
    openLatestReleaseStack = VioletDesignMetrics.openLatestReleaseStack,
    showTrackDividers = VioletDesignMetrics.showTrackDividers,
)

/** Material 3 Violet Light color scheme. */
val VioletLightColorScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF7B52AB),
    onPrimary = Color.White,
    secondary = Color(0xFF9F7AD1),
    onSecondary = Color(0xFF231633),
    tertiary = Color(0xFF6A4896),
    onTertiary = Color.White,
    background = Color(0xFFF6F0FC),
    onBackground = Color(0xFF231633),
    surface = Color(0xFFEEE5F8),
    onSurface = Color(0xFF231633),
    surfaceVariant = Color(0xFFE6DAF5),
    onSurfaceVariant = Color(0xFF76658B),
    outline = Color(0xFFC5A8EE),
    outlineVariant = Color(0xFFE3D5F6),
)

/** Violet Dark tokens (sculpted deep amethyst glass). */
val VioletDarkTokens = LunariaThemeTokens(
    designTheme = DesignTheme.VIOLET,
    isDark = true,
    isAmoled = false,
    moonGlow = Color(0xFFF5EEFF),
    silverPetal = Color(0xFFC6B3E2),
    petalLavender = Color(0xFFC9A7FF),
    lunarMist = Color(0xFF9B74E8),
    eclipseBase = Color(0xFF110A22),
    eclipseSurface = Color(0xFF1D1236),
    eclipseCard = Color(0xFF2A1A4C),
    accountGradientTop = Color(0xFF29184B),
    accountGradientBottom = Color(0xFF110A22),
    settingsBg = Color(0xFF0E081C),
    settingsCard = Color(0xFF1A1030),
    settingsDivider = Color(0xFF2F1F52),
    onSurfacePrimary = Color(0xFFF5EEFF),
    onSurfaceSecondary = Color(0xFFC6B3E2),
    cardStroke = Color(0xFFD2C3F6).copy(alpha = 0.22f),
    accentGradient = Brush.linearGradient(
        colors = listOf(Color(0xFFC9A7FF), Color(0xFF9B74E8)),
    ),
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFF29184B), Color(0xFF110A22)),
    ),
    pageBackdropBrush = Brush.verticalGradient(
        0.0f to Color(0xFF2D1B52),
        0.45f to Color(0xFF190F30),
        1.0f to Color(0xFF0E081C),
    ),
    glassTint = Color(0xFF241445).copy(alpha = 0.42f),
    glassRimTopLeft = Color(0xFFEADFFF).copy(alpha = 0.54f),
    glassRimBottomRight = Color(0xFF8C62D8).copy(alpha = 0.26f),
    glassShadowColor = Color(0xFF4A258C).copy(alpha = 0.42f),
    pillIndicatorColor = Color(0xFFDF78D5).copy(alpha = 0.32f),
    pillIndicatorRimColor = Color(0xFFFAD4F8).copy(alpha = 0.62f),
    heroCardScrimColor = Color(0xFF2E1B4F),
    cardCornerRadius = VioletDesignMetrics.cardCornerRadius,
    miniPlayerHeight = VioletDesignMetrics.miniPlayerHeight,
    miniPlayerShape = VioletDesignMetrics.miniPlayerShape,
    navBarShape = VioletDesignMetrics.navBarShape,
    tabPillShape = VioletDesignMetrics.tabPillShape,
    heroFullBleed = VioletDesignMetrics.heroFullBleed,
    openLatestReleaseStack = VioletDesignMetrics.openLatestReleaseStack,
    showTrackDividers = VioletDesignMetrics.showTrackDividers,
)

/** Material 3 Violet Dark color scheme. */
val VioletDarkColorScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFFC9A7FF),
    onPrimary = Color(0xFF1E1038),
    secondary = Color(0xFF36255C),
    onSecondary = Color(0xFFD2C3F6),
    tertiary = Color(0xFF9B74E8),
    onTertiary = Color(0xFF110A22),
    background = Color(0xFF110A22),
    onBackground = Color(0xFFF5EEFF),
    surface = Color(0xFF1D1236),
    onSurface = Color(0xFFF5EEFF),
    surfaceVariant = Color(0xFF2A1A4C),
    onSurfaceVariant = Color(0xFFC6B3E2),
    outline = Color(0xFFD2C3F6).copy(alpha = 0.28f),
    outlineVariant = Color(0xFFD2C3F6).copy(alpha = 0.14f),
)

/** Violet AMOLED tokens (true black background with sculpted amethyst glass). */
val VioletAmoledTokens = VioletDarkTokens.copy(
    isAmoled = true,
    eclipseBase = Color.Black,
    eclipseSurface = Color(0xFF120B22),
    eclipseCard = Color(0xFF1C1133),
    settingsBg = Color.Black,
    settingsCard = Color(0xFF130C21),
    accountGradientTop = Color(0xFF1B1033),
    accountGradientBottom = Color.Black,
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFF1B1033), Color.Black),
    ),
    pageBackdropBrush = Brush.verticalGradient(
        0.0f to Color(0xFF170D2C),
        0.40f to Color(0xFF0A0614),
        1.0f to Color.Black,
    ),
)

/** Material 3 Violet AMOLED color scheme. */
val VioletAmoledColorScheme: ColorScheme = VioletDarkColorScheme.copy(
    background = Color.Black,
    surface = Color(0xFF120B22),
    surfaceVariant = Color(0xFF1C1133),
)
