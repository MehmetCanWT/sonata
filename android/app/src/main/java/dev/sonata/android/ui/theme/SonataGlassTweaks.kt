/*
 * Clean-room Sonata glass & flat surface theme tokens and icon helpers.
 *
 * Keeps real Liquid Glass backdrop refraction strictly scoped to floating bars
 * and floating top chrome over scrollable content, while providing flat Lunaria
 * moon-tinted dark surfaces and neutral grouped settings surfaces elsewhere.
 */
package dev.sonata.android.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.shape.CornerBasedShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.sonata.android.LocalLiquidGlassEnabled
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.isGlassSupported
import dev.sonata.android.liquidGlass
import dev.sonata.android.ui.SonataAccent

/** Sonata's warm obsidian tint for dark frosted glass surfaces. */
val SonataObsidianGlass = Color(0xFF141318)

/** Sonata's soft alabaster tint for light frosted glass surfaces. */
val SonataPearlGlass = Color(0xFFF6F6FA)

/** Subtle crimson-tinted halo used sparingly on active Sonata pills. */
val SonataAccentHalo = SonataAccent.copy(alpha = 0.20f)

/** Standard uniform size for bottom navigation tab icons (equal across all 5 tabs). */
val SonataTabIconSize: Dp = 22.dp

/** Lunaria moon-tinted dark obsidian-indigo palette for flat modal menus (no Liquid Glass). */
object SonataLunariaTokens {
    val SheetTop = Color(0xFF141221)
    val SheetMid = Color(0xFF0E0D17)
    val SheetBottom = Color(0xFF09090D)
    val CardSurface = Color(0xFF1A1926)
    val CardSurfaceElevated = Color(0xFF211F30)
    val HairlineBorder = Color(0xFF2B293C)
    val MoonAccent = Color(0xFFC5BDF5)
    val TextPrimary = Color(0xFFF4F3FA)
    val TextSecondary = Color(0xFFA7A4BC)

    val SheetGradient: Brush
        get() = Brush.verticalGradient(
            colors = listOf(SheetTop, SheetMid, SheetBottom),
        )
}

/** Flat neutral gray palette for the full-screen Settings experience (zero Liquid Glass). */
object SonataSettingsTokens {
    val Background = Color(0xFF000000)
    val SearchBar = Color(0xFF161618)
    val Card = Color(0xFF161618)
    val CardPressed = Color(0xFF202024)
    val Hairline = Color(0xFF262629)
    val TextPrimary = Color(0xFFFFFFFF)
    val TextSecondary = Color(0xFF8E8E93)
    val ToggleActive = LunariaColors.PetalLavender
    val ToggleInactive = Color(0xFF39393D)
}

/**
 * Crisp Radio Waves `((•))` vector glyph matching the 24x24 optical grid used across
 * all five navigation tabs.
 */
val SonataRadioWavesIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "SonataRadioWaves",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        // Center broadcast dot
        path(
            fill = SolidColor(Color.Black),
            pathFillType = PathFillType.NonZero,
        ) {
            moveTo(12f, 9.75f)
            arcTo(2.25f, 2.25f, 0f, isMoreThanHalf = true, isPositiveArc = true, 11.99f, 9.75f)
            close()
        }
        // Inner left wave
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.9f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(8.46f, 8.46f)
            arcTo(5.0f, 5.0f, 0f, isMoreThanHalf = false, isPositiveArc = false, 8.46f, 15.54f)
        }
        // Inner right wave
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.9f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(15.54f, 8.46f)
            arcTo(5.0f, 5.0f, 0f, isMoreThanHalf = false, isPositiveArc = true, 15.54f, 15.54f)
        }
        // Outer left wave
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.9f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(5.64f, 5.64f)
            arcTo(9.0f, 9.0f, 0f, isMoreThanHalf = false, isPositiveArc = false, 5.64f, 18.36f)
        }
        // Outer right wave
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.9f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(18.36f, 5.64f)
            arcTo(9.0f, 9.0f, 0f, isMoreThanHalf = false, isPositiveArc = true, 18.36f, 18.36f)
        }
    }.build()
}

/**
 * Directional top-left specular rim highlight reserved for true floating glass surfaces.
 */
@Composable
fun Modifier.sonataSpecularRim(
    shape: Shape,
    strokeWidth: Dp = 0.75.dp,
    accentGlow: Boolean = false,
): Modifier {
    val isLight = MaterialTheme.colorScheme.surface.luminance() > 0.5f
    val brush = remember(isLight, accentGlow) {
        val topLeft = when {
            accentGlow -> SonataAccent.copy(alpha = 0.55f)
            isLight -> Color.White.copy(alpha = 0.72f)
            else -> Color.White.copy(alpha = 0.24f)
        }
        val mid = if (isLight) Color.Black.copy(alpha = 0.06f) else Color.White.copy(alpha = 0.07f)
        val bottomRight = if (isLight) Color.Black.copy(alpha = 0.12f) else Color.White.copy(alpha = 0.03f)
        Brush.linearGradient(
            colorStops = arrayOf(
                0f to topLeft,
                0.48f to mid,
                1f to bottomRight,
            ),
            start = Offset.Zero,
            end = Offset(Float.POSITIVE_INFINITY, Float.POSITIVE_INFINITY),
        )
    }
    return border(width = strokeWidth, brush = brush, shape = shape)
}

/**
 * Applies Liquid Glass refraction ONLY when [liveBackdrop] is true (floating bars/pills
 * where dynamic content scrolls behind). When [liveBackdrop] is false, renders a clean flat
 * surface with zero glass blur, zero fake sheen, and subtle hairline edge.
 */
@Composable
fun Modifier.sonataGlassSurface(
    shape: CornerBasedShape = RoundedCornerShape(20.dp),
    liveBackdrop: Boolean = false,
    tintOverride: Color? = null,
    accentGlow: Boolean = false,
): Modifier {
    val isLight = MaterialTheme.colorScheme.surface.luminance() > 0.5f
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported()
    val reduceBlur = LocalReduceDynamicBlur.current
    val baseTint = tintOverride ?: if (isLight) SonataPearlGlass else SonataObsidianGlass

    if (liveBackdrop && glassOn && !reduceBlur) {
        return liquidGlass(
            shape = shape,
            surfaceTint = baseTint.copy(alpha = if (isLight) 0.52f else 0.42f),
        ).sonataSpecularRim(
            shape = shape,
            strokeWidth = 0.75.dp,
            accentGlow = accentGlow,
        )
    }

    val flatColor = tintOverride ?: if (isLight) {
        Color(0xFFF2F2F7)
    } else {
        Color(0xFF161618)
    }
    val borderColor = when {
        accentGlow -> SonataAccent.copy(alpha = 0.45f)
        isLight -> Color.Black.copy(alpha = 0.08f)
        else -> Color.White.copy(alpha = 0.07f)
    }
    return clip(shape)
        .background(flatColor, shape)
        .border(0.5.dp, borderColor, shape)
}

/** Clean container applying [Modifier.sonataGlassSurface] (flat by default unless [liveBackdrop] = true). */
@Composable
fun SonataGlassCard(
    modifier: Modifier = Modifier,
    shape: CornerBasedShape = RoundedCornerShape(20.dp),
    liveBackdrop: Boolean = false,
    tintOverride: Color? = null,
    accentGlow: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier = modifier.sonataGlassSurface(
            shape = shape,
            liveBackdrop = liveBackdrop,
            tintOverride = tintOverride,
            accentGlow = accentGlow,
        ),
        content = content,
    )
}

/** Clean pill container applying [Modifier.sonataGlassSurface]. */
@Composable
fun SonataGlassPillBox(
    modifier: Modifier = Modifier,
    shape: CornerBasedShape = RoundedCornerShape(percent = 50),
    liveBackdrop: Boolean = false,
    tintOverride: Color? = null,
    accentGlow: Boolean = false,
    content: @Composable BoxScope.() -> Unit,
) {
    Box(
        modifier = modifier.sonataGlassSurface(
            shape = shape,
            liveBackdrop = liveBackdrop,
            tintOverride = tintOverride,
            accentGlow = accentGlow,
        ),
        content = content,
    )
}
