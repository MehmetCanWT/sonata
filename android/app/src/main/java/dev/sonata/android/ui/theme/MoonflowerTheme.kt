/*
 * Dedicated Moonflower Theme System for Sonata Android.
 *
 * Inspired by research/theme/moonflower.png & settings page.jpg:
 * flowing gossamer translucent silk petal veils across the upper-right canvas,
 * soft rounded 20dp editorial cards, 100% stadium capsule floating chrome,
 * framed floating hero cards, and subtle botanical hairline row dividers across
 * Light, Dark, and AMOLED modes.
 */
package dev.sonata.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.sonata.android.data.DesignTheme

/** Structural layout & shape metrics unique to the Moonflower design language. */
object MoonflowerDesignMetrics {
    val cardCornerRadius: Dp = 24.dp
    val miniPlayerHeight: Dp = 54.dp
    val capsuleCornerPercent: Int = 50
    const val heroFullBleed: Boolean = false
    const val openLatestReleaseStack: Boolean = false
    const val showTrackDividers: Boolean = true

    val cardShape: RoundedCornerShape = RoundedCornerShape(cardCornerRadius)
    val miniPlayerShape: RoundedCornerShape = RoundedCornerShape(percent = capsuleCornerPercent)
    val navBarShape: RoundedCornerShape = RoundedCornerShape(percent = capsuleCornerPercent)
    val tabPillShape: RoundedCornerShape = RoundedCornerShape(percent = capsuleCornerPercent)
}

private val EdgeOuterStroke = Stroke(width = 3.0f, cap = StrokeCap.Round)
private val EdgeInnerStroke = Stroke(width = 2.0f, cap = StrokeCap.Round)

private class MoonflowerBackdropCache(
    val width: Float,
    val height: Float,
    val isDark: Boolean,
    val accent: Color,
    val mist: Color,
    val veilBack: Path,
    val veilBackBrush: Brush,
    val veilPrimary: Path,
    val veilPrimaryBrush: Brush,
    val veilSecondary: Path,
    val veilSecondaryBrush: Brush,
    val veilFold: Path,
    val veilFoldBrush: Brush,
    val edgeCurveOuter: Path,
    val edgeCurveOuterColor: Color,
    val edgeCurveInner: Path,
    val edgeCurveInnerColor: Color,
)

@Volatile
private var cachedMoonflowerBackdrop: MoonflowerBackdropCache? = null

/** Draws the Moonflower signature flowing gossamer translucent silk petal veils and luminous rim curve. */
fun DrawScope.drawMoonflowerAtmosphericBackdrop(
    isDark: Boolean,
    accent: Color,
    mist: Color,
) {
    val w = size.width
    val h = size.height
    if (w <= 0f || h <= 0f) return

    val cached = cachedMoonflowerBackdrop?.takeIf {
        it.width == w && it.height == h && it.isDark == isDark && it.accent == accent && it.mist == mist
    } ?: run {
        val roseBlush = if (isDark) Color(0xFFC682AE) else Color(0xFFF0BDD8)
        val silkPeriwinkle = if (isDark) Color(0xFF7B7FC4) else Color(0xFFC6B7E6)
        val petalCrease = if (isDark) Color(0xFF453668) else Color(0xFF9E86C2)

        val veilBack = Path().apply {
            moveTo(-w * 0.04f, -h * 0.02f)
            cubicTo(w * 0.28f, h * 0.05f, w * 0.56f, h * 0.16f, w * 1.06f, h * 0.08f)
            lineTo(w * 1.06f, -h * 0.05f)
            close()
        }
        val veilPrimary = Path().apply {
            moveTo(w * 0.08f, -h * 0.04f)
            cubicTo(w * 0.42f, h * 0.08f, w * 0.68f, h * 0.23f, w * 1.06f, h * 0.16f)
            lineTo(w * 1.06f, -h * 0.05f)
            close()
        }
        val veilSecondary = Path().apply {
            moveTo(w * 0.22f, -h * 0.02f)
            cubicTo(w * 0.52f, h * 0.14f, w * 0.78f, h * 0.28f, w * 1.06f, h * 0.23f)
            lineTo(w * 1.06f, h * 0.09f)
            cubicTo(w * 0.78f, h * 0.14f, w * 0.52f, h * 0.04f, w * 0.22f, -h * 0.02f)
            close()
        }
        val veilFold = Path().apply {
            moveTo(w * 0.48f, -h * 0.03f)
            cubicTo(w * 0.68f, h * 0.11f, w * 0.84f, h * 0.20f, w * 1.06f, h * 0.18f)
            lineTo(w * 1.06f, -h * 0.03f)
            close()
        }

        val silkAlpha = if (isDark) 0.28f else 0.58f
        val veilBackBrush = Brush.linearGradient(
            colors = listOf(
                roseBlush.copy(alpha = silkAlpha * 0.45f),
                silkPeriwinkle.copy(alpha = silkAlpha * 0.65f),
                Color.Transparent,
            ),
            start = Offset(0f, 0f),
            end = Offset(w, h * 0.18f),
        )
        val veilPrimaryBrush = Brush.linearGradient(
            colors = listOf(
                roseBlush.copy(alpha = silkAlpha * 0.55f),
                silkPeriwinkle.copy(alpha = silkAlpha * 0.85f),
                Color.White.copy(alpha = if (isDark) 0.16f else 0.54f),
            ),
            start = Offset(w * 0.15f, 0f),
            end = Offset(w, h * 0.22f),
        )
        val veilSecondaryBrush = Brush.linearGradient(
            colors = listOf(
                Color.Transparent,
                petalCrease.copy(alpha = silkAlpha * 0.62f),
                roseBlush.copy(alpha = silkAlpha * 0.74f),
            ),
            start = Offset(w * 0.25f, 0f),
            end = Offset(w, h * 0.28f),
        )
        val veilFoldBrush = Brush.linearGradient(
            colors = listOf(
                Color.White.copy(alpha = if (isDark) 0.12f else 0.46f),
                mist.copy(alpha = silkAlpha * 0.48f),
                accent.copy(alpha = silkAlpha * 0.35f),
            ),
            start = Offset(w * 0.50f, 0f),
            end = Offset(w, h * 0.19f),
        )
        val edgeCurveOuter = Path().apply {
            moveTo(w * 0.20f, -h * 0.01f)
            cubicTo(w * 0.52f, h * 0.13f, w * 0.78f, h * 0.25f, w * 1.04f, h * 0.20f)
        }
        val edgeCurveInner = Path().apply {
            moveTo(w * 0.44f, -h * 0.02f)
            cubicTo(w * 0.66f, h * 0.10f, w * 0.84f, h * 0.19f, w * 1.04f, h * 0.16f)
        }

        MoonflowerBackdropCache(
            width = w,
            height = h,
            isDark = isDark,
            accent = accent,
            mist = mist,
            veilBack = veilBack,
            veilBackBrush = veilBackBrush,
            veilPrimary = veilPrimary,
            veilPrimaryBrush = veilPrimaryBrush,
            veilSecondary = veilSecondary,
            veilSecondaryBrush = veilSecondaryBrush,
            veilFold = veilFold,
            veilFoldBrush = veilFoldBrush,
            edgeCurveOuter = edgeCurveOuter,
            edgeCurveOuterColor = Color.White.copy(alpha = if (isDark) 0.22f else 0.78f),
            edgeCurveInner = edgeCurveInner,
            edgeCurveInnerColor = roseBlush.copy(alpha = if (isDark) 0.26f else 0.72f),
        ).also { cachedMoonflowerBackdrop = it }
    }

    drawPath(path = cached.veilBack, brush = cached.veilBackBrush)
    drawPath(path = cached.veilPrimary, brush = cached.veilPrimaryBrush)
    drawPath(path = cached.veilSecondary, brush = cached.veilSecondaryBrush)
    drawPath(path = cached.veilFold, brush = cached.veilFoldBrush)
    drawPath(path = cached.edgeCurveOuter, color = cached.edgeCurveOuterColor, style = EdgeOuterStroke)
    drawPath(path = cached.edgeCurveInner, color = cached.edgeCurveInnerColor, style = EdgeInnerStroke)
}

/** Moonflower Light tokens (`moonflower.png`, `settings page.jpg`). */
val MoonflowerLightTokens = LunariaThemeTokens(
    designTheme = DesignTheme.MOONFLOWER,
    isDark = false,
    isAmoled = false,
    moonGlow = Color(0xFF1A1421),
    silverPetal = Color(0xFF72647A),
    petalLavender = Color(0xFFB35D8E),
    lunarMist = Color(0xFF8C7DB8),
    eclipseBase = Color(0xFFFDF8FB),
    eclipseSurface = Color(0xFFF7ECF4),
    eclipseCard = Color(0xFFEFDFEB),
    accountGradientTop = Color(0xFFF6E6F2),
    accountGradientBottom = Color(0xFFFFFCFE),
    settingsBg = Color(0xFFFFFFFF),
    settingsCard = Color(0xFFF6F6F9),
    settingsDivider = Color(0xFFEAE9F0),
    onSurfacePrimary = Color(0xFF1A1421),
    onSurfaceSecondary = Color(0xFF72647A),
    cardStroke = Color(0xFFE5C4D9).copy(alpha = 0.55f),
    accentGradient = Brush.linearGradient(
        colors = listOf(Color(0xFFC46B9C), Color(0xFF8C7DB8)),
    ),
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFFF6E6F2), Color(0xFFFFFCFE)),
    ),
    pageBackdropBrush = Brush.verticalGradient(
        0.0f to Color(0xFFF6E6F2),
        0.42f to Color(0xFFFBF6FA),
        1.0f to Color(0xFFFFFCFE),
    ),
    glassTint = Color(0xFFFAFAFA).copy(alpha = 0.40f),
    glassRimTopLeft = Color.White.copy(alpha = 0.55f),
    glassRimBottomRight = Color.White.copy(alpha = 0.14f),
    glassShadowColor = Color.Black.copy(alpha = 0.14f),
    pillIndicatorColor = Color.White.copy(alpha = 0.50f),
    pillIndicatorRimColor = Color.White.copy(alpha = 0.65f),
    heroCardScrimColor = Color(0xFF6D4E7C),
    cardCornerRadius = MoonflowerDesignMetrics.cardCornerRadius,
    miniPlayerHeight = MoonflowerDesignMetrics.miniPlayerHeight,
    miniPlayerShape = MoonflowerDesignMetrics.miniPlayerShape,
    navBarShape = MoonflowerDesignMetrics.navBarShape,
    tabPillShape = MoonflowerDesignMetrics.tabPillShape,
    heroFullBleed = MoonflowerDesignMetrics.heroFullBleed,
    openLatestReleaseStack = MoonflowerDesignMetrics.openLatestReleaseStack,
    showTrackDividers = MoonflowerDesignMetrics.showTrackDividers,
)

/** Material 3 Moonflower Light color scheme. */
val MoonflowerLightColorScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF8C5F96),
    onPrimary = Color.White,
    secondary = Color(0xFFF2CBE0),
    onSecondary = Color(0xFF1A1523),
    tertiary = Color(0xFF7B74B8),
    onTertiary = Color.White,
    background = Color(0xFFF4EFFA),
    onBackground = Color(0xFF1A1523),
    surface = Color(0xFFEDE6F6),
    onSurface = Color(0xFF1A1523),
    surfaceVariant = Color(0xFFE4DCF0),
    onSurfaceVariant = Color(0xFF6D647A),
    outline = Color(0xFFD7C7E4),
    outlineVariant = Color(0xFFE7DDF0),
)

/** Moonflower Dark tokens (nocturnal moonflower silk & rose-lavender obsidian). */
val MoonflowerDarkTokens = LunariaThemeTokens(
    designTheme = DesignTheme.MOONFLOWER,
    isDark = true,
    isAmoled = false,
    moonGlow = Color(0xFFF4EFFB),
    silverPetal = Color(0xFFB9B0CC),
    petalLavender = Color(0xFFD49CE8),
    lunarMist = Color(0xFF8F96D9),
    eclipseBase = Color(0xFF0B0914),
    eclipseSurface = Color(0xFF151224),
    eclipseCard = Color(0xFF1F1A33),
    accountGradientTop = Color(0xFF1B142E),
    accountGradientBottom = Color(0xFF0B0914),
    settingsBg = Color(0xFF090710),
    settingsCard = Color(0xFF151222),
    settingsDivider = Color(0xFF26213A),
    onSurfacePrimary = Color(0xFFF4EFFB),
    onSurfaceSecondary = Color(0xFFB9B0CC),
    cardStroke = Color(0xFFC9C2DE).copy(alpha = 0.14f),
    accentGradient = Brush.linearGradient(
        colors = listOf(Color(0xFFD49CE8), Color(0xFF8F96D9)),
    ),
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFF1B142E), Color(0xFF0B0914)),
    ),
    pageBackdropBrush = Brush.verticalGradient(
        0.0f to Color(0xFF1B142E),
        0.48f to Color(0xFF100C1D),
        1.0f to Color(0xFF090711),
    ),
    glassTint = Color(0xFF121212).copy(alpha = 0.40f),
    glassRimTopLeft = Color.White.copy(alpha = 0.22f),
    glassRimBottomRight = Color.White.copy(alpha = 0.08f),
    glassShadowColor = Color.Black.copy(alpha = 0.40f),
    pillIndicatorColor = Color.White.copy(alpha = 0.14f),
    pillIndicatorRimColor = Color.White.copy(alpha = 0.20f),
    heroCardScrimColor = Color(0xFF392B50),
    cardCornerRadius = MoonflowerDesignMetrics.cardCornerRadius,
    miniPlayerHeight = MoonflowerDesignMetrics.miniPlayerHeight,
    miniPlayerShape = MoonflowerDesignMetrics.miniPlayerShape,
    navBarShape = MoonflowerDesignMetrics.navBarShape,
    tabPillShape = MoonflowerDesignMetrics.tabPillShape,
    heroFullBleed = MoonflowerDesignMetrics.heroFullBleed,
    openLatestReleaseStack = MoonflowerDesignMetrics.openLatestReleaseStack,
    showTrackDividers = MoonflowerDesignMetrics.showTrackDividers,
)

/** Material 3 Moonflower Dark color scheme. */
val MoonflowerDarkColorScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFFD49CE8),
    onPrimary = Color(0xFF1C0E28),
    secondary = Color(0xFF8F96D9),
    onSecondary = Color(0xFF0B0914),
    tertiary = Color(0xFFF2CBE0),
    onTertiary = Color(0xFF1A1523),
    background = Color(0xFF0B0914),
    onBackground = Color(0xFFF4EFFB),
    surface = Color(0xFF151224),
    onSurface = Color(0xFFF4EFFB),
    surfaceVariant = Color(0xFF1F1A33),
    onSurfaceVariant = Color(0xFFB9B0CC),
    outline = Color(0xFFC9C2DE).copy(alpha = 0.22f),
    outlineVariant = Color(0xFFC9C2DE).copy(alpha = 0.12f),
)

/** Moonflower AMOLED tokens (true black background with translucent silk capsule accents). */
val MoonflowerAmoledTokens = MoonflowerDarkTokens.copy(
    isAmoled = true,
    eclipseBase = Color.Black,
    eclipseSurface = Color(0xFF0C0B12),
    eclipseCard = Color(0xFF151320),
    settingsBg = Color.Black,
    settingsCard = Color(0xFF111018),
    accountGradientTop = Color(0xFF141022),
    accountGradientBottom = Color.Black,
    accountSheetBrush = Brush.verticalGradient(
        colors = listOf(Color(0xFF141022), Color.Black),
    ),
    pageBackdropBrush = Brush.verticalGradient(
        0.0f to Color(0xFF120D20),
        0.40f to Color(0xFF07050D),
        1.0f to Color.Black,
    ),
)

/** Material 3 Moonflower AMOLED color scheme. */
val MoonflowerAmoledColorScheme: ColorScheme = MoonflowerDarkColorScheme.copy(
    background = Color.Black,
    surface = Color(0xFF0C0B12),
    surfaceVariant = Color(0xFF151320),
)
