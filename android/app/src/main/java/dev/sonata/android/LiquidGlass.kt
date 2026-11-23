/*
 * Neutral Crystal-Clear Liquid Glass Surface System for Sonata Android.
 *
 * Uses Android 12+ RenderEffect backdrop blur, vibrancy, and Android 13+ AGSL lens refraction
 * with neutral light/dark glass tinting (no theme-colored pink or purple tint), directional
 * highlight rim, and clean flat fallback when Liquid Glass or dynamic blur is disabled.
 */
package dev.sonata.android

import android.os.Build
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.shape.CornerBasedShape
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.toRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.backdrop.backdrops.LayerBackdrop
import dev.sonata.android.backdrop.drawBackdrop
import dev.sonata.android.backdrop.effects.blur
import dev.sonata.android.backdrop.effects.colorControls
import dev.sonata.android.backdrop.effects.lens
import dev.sonata.android.backdrop.highlight.Highlight
import dev.sonata.android.backdrop.highlight.HighlightElement
import dev.sonata.android.backdrop.internal.ShapeProvider
import dev.sonata.android.backdrop.shadow.Shadow
import dev.sonata.android.ui.theme.LocalLunariaColors

/** Whether the Liquid Glass navigation bar and floating controls are turned on. */
val LocalLiquidGlassEnabled = staticCompositionLocalOf { false }

/** Whether Liquid Glass uses frosted-only performance mode (no AGSL lens refraction). */
val LocalGlassPerformance = staticCompositionLocalOf { false }

/** Whether dynamic backdrop blur is replaced with solid surfaces. */
val LocalReduceDynamicBlur = staticCompositionLocalOf { false }

/** Whether tab bar and pill spring animations are snapped immediately. */
val LocalReduceAnimation = staticCompositionLocalOf { false }

/** The backdrop content (app UI) that a Liquid Glass surface samples from. */
val LocalAppBackdrop = staticCompositionLocalOf<Backdrop> { error("No AppBackdrop provided") }

/**
 * Records the rendered bar glass surface so the travelling selection pill can refract
 * the glass surface directly above it.
 */
internal val LocalGlassExport = compositionLocalOf<LayerBackdrop?> { null }

/** Standard spring used by glass navigation transitions. */
internal val GlassSpring = spring<Float>(dampingRatio = 0.72f, stiffness = 320f)

/** Backdrop blur pipeline requires RenderEffect on RenderNode (Android 12 / API 31+). */
fun isGlassSupported(sdkInt: Int = Build.VERSION.SDK_INT): Boolean = sdkInt >= Build.VERSION_CODES.S

// Crystal-clear neutral Liquid Glass optical constants
private const val VIBRANCY = 1f
private const val BLUR_RADIUS_DP = 8f
private const val LENS_HEIGHT = 0.5f
private const val LENS_AMOUNT = 0.5f
private const val LENS_MAX_DP = 48f
private const val SURFACE_OPACITY = 0.4f
private const val GLASS_RESOLUTION_SCALE = 0.33f
private const val FROST_BLUR_RADIUS_DP = 16f
private const val FROST_RESOLUTION_SCALE = 0.25f

private val LightGlassTint = Color(0xFFFAFAFA)
private val DarkGlassTint = Color(0xFF121212)

/** Subtle neutral hairline along a bar's edge when Liquid Glass is off or lightweight. */
internal val GLASS_EDGE_WIDTH = 0.5.dp
internal val GLASS_EDGE_COLOR = Color.White.copy(alpha = 0.10f)

/** Neutral icon and label color for content sitting on a glass surface. */
@Composable
fun glassContentColor(): Color =
    if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) Color.Black else Color.White

/** Neutral selected-tab indicator color for a glass surface (pure white lift in both themes). */
@Composable
fun glassIndicatorColor(): Color =
    if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) {
        Color.White.copy(alpha = 0.5f)
    } else {
        Color.White.copy(alpha = DARK_INDICATOR_OPACITY)
    }

private const val DARK_INDICATOR_OPACITY = 0.14f

private val DefaultGlassHighlight: () -> Highlight? = { Highlight.Default }
private val DefaultGlassShadow: () -> Shadow? = { Shadow.Default }

@Composable
private fun neutralGlassTint(): Color =
    if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) LightGlassTint else DarkGlassTint

/**
 * Lightweight visual match for neutral Liquid Glass over a stable background without
 * backdrop capture, blur, or lens refraction. Falls back to [fallbackColor] when disabled.
 */
@Composable
fun Modifier.lightweightLiquidGlass(
    shape: CornerBasedShape,
    fallbackColor: Color,
): Modifier {
    val useGlass = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    val glassTint = neutralGlassTint()
    val shapeBlock: () -> Shape = remember(shape) { { shape } }
    val shapeProvider = remember(shapeBlock) { ShapeProvider(shapeBlock) }

    return clip(shape)
        .background(
            color = if (useGlass) glassTint.copy(alpha = SURFACE_OPACITY) else fallbackColor,
            shape = shape,
        )
        .then(
            if (useGlass) {
                HighlightElement(
                    shapeProvider = shapeProvider,
                    highlight = DefaultGlassHighlight,
                )
            } else {
                Modifier
            },
        )
        .border(GLASS_EDGE_WIDTH, GLASS_EDGE_COLOR, shape)
}

/**
 * Just the directional specular rim of Liquid Glass for controls that keep their own fill.
 */
@Composable
fun Modifier.liquidGlassRim(shape: CornerBasedShape): Modifier {
    if (!LocalLiquidGlassEnabled.current || !isGlassSupported()) return this
    val shapeBlock: () -> Shape = remember(shape) { { shape } }
    val shapeProvider = remember(shapeBlock) { ShapeProvider(shapeBlock) }
    return then(
        HighlightElement(
            shapeProvider = shapeProvider,
            highlight = DefaultGlassHighlight,
        ),
    )
}

/**
 * Renders this composable as a crystal-clear neutral Liquid Glass surface sampling
 * [LocalAppBackdrop]: vibrancy, blur, Android 13+ lens refraction with chromatic aberration,
 * directional highlight, contact shadow, and neutral light/dark surface tint.
 *
 * When Liquid Glass is turned off, unsupported, or dynamic blur is reduced, renders a crisp
 * solid surface with a subtle hairline edge.
 */
@Composable
fun Modifier.liquidGlass(
    shape: CornerBasedShape,
    /** Drawn over the glass in place of the neutral default tint when explicitly requested. */
    surfaceTint: Color? = null,
    chromaticAberration: Boolean = true,
): Modifier {
    val tokens = LocalLunariaColors.current
    if (!LocalLiquidGlassEnabled.current || !isGlassSupported()) {
        return clip(shape)
            .background(tokens.eclipseSurface, shape)
            .border(GLASS_EDGE_WIDTH, tokens.cardStroke, shape)
    }
    if (LocalReduceDynamicBlur.current) {
        return clip(shape)
            .background(MaterialTheme.colorScheme.surface, shape)
            .border(GLASS_EDGE_WIDTH, GLASS_EDGE_COLOR, shape)
    }
    val glassPerformance = LocalGlassPerformance.current
    val backdrop = LocalAppBackdrop.current
    val exportedBackdrop = LocalGlassExport.current
    val density = LocalDensity.current
    val resolutionScale = if (glassPerformance) FROST_RESOLUTION_SCALE else GLASS_RESOLUTION_SCALE
    val blurRadiusDp = if (glassPerformance) FROST_BLUR_RADIUS_DP else BLUR_RADIUS_DP
    val blurPx = with(density) { blurRadiusDp.dp.toPx() } * resolutionScale
    val lensHeightPx = with(density) { (LENS_HEIGHT * LENS_MAX_DP).dp.toPx() } * resolutionScale
    val lensAmountPx = with(density) { (LENS_AMOUNT * LENS_MAX_DP).dp.toPx() } * resolutionScale
    val neutralTint = neutralGlassTint().copy(alpha = SURFACE_OPACITY)
    val surfaceTintColor = surfaceTint ?: neutralTint

    val shapeBlock: () -> Shape = remember(shape) { { shape } }
    val effectsBlock: dev.sonata.android.backdrop.BackdropEffectScope.() -> Unit = remember(
        glassPerformance,
        blurPx,
        lensHeightPx,
        lensAmountPx,
        chromaticAberration,
    ) {
        {
            colorControls(saturation = 1f + 0.5f * VIBRANCY)
            blur(blurPx)
            if (!glassPerformance && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                lens(
                    refractionHeight = lensHeightPx,
                    refractionAmount = lensAmountPx,
                    depthEffect = true,
                    chromaticAberration = chromaticAberration,
                )
            }
        }
    }
    val onDrawSurfaceBlock: androidx.compose.ui.graphics.drawscope.DrawScope.() -> Unit = remember(surfaceTintColor) {
        {
            drawRect(color = surfaceTintColor, size = size)
        }
    }

    return drawBackdrop(
        backdrop = backdrop,
        shape = shapeBlock,
        effects = effectsBlock,
        highlight = DefaultGlassHighlight,
        shadow = DefaultGlassShadow,
        onDrawSurface = onDrawSurfaceBlock,
        exportedBackdrop = exportedBackdrop,
        backdropScale = resolutionScale,
    )
}

/**
 * Reusable Liquid Glass button/bar modifier using neutral crystal-clear glass when enabled
 * and falling back to a crisp flat solid surface when Liquid Glass is turned off.
 */
@Composable
fun Modifier.sonataThemeGlassButton(
    shape: CornerBasedShape,
    surfaceTint: Color? = null,
    fallbackColor: Color = LocalLunariaColors.current.eclipseSurface,
    chromaticAberration: Boolean = true,
    shadowElevation: Dp = 6.dp,
): Modifier {
    val tokens = LocalLunariaColors.current
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    if (glassOn) {
        return this.liquidGlass(
            shape = shape,
            surfaceTint = surfaceTint,
            chromaticAberration = chromaticAberration,
        )
    }
    return this
        .shadow(
            elevation = (shadowElevation * 0.5f).coerceAtMost(4.dp),
            shape = shape,
            clip = false,
            ambientColor = Color.Black.copy(alpha = if (tokens.isDark) 0.28f else 0.10f),
            spotColor = Color.Black.copy(alpha = if (tokens.isDark) 0.28f else 0.10f),
        )
        .clip(shape)
        .background(fallbackColor, shape)
        .border(GLASS_EDGE_WIDTH, tokens.cardStroke, shape)
}

/**
 * Hardware-fast-path capsule shape for Liquid Glass pills.
 */
val GlassCapsuleShape: Shape = object : Shape {
    override fun createOutline(
        size: Size,
        layoutDirection: LayoutDirection,
        density: Density,
    ): Outline {
        val radius = minOf(size.width, size.height) * 0.5f
        return Outline.Rounded(
            RoundRect(
                left = 0f,
                top = 0f,
                right = size.width,
                bottom = size.height,
                cornerRadius = CornerRadius(radius, radius),
            ),
        )
    }
}

/**
 * Hardware-fast-path pill shape for Liquid Glass capsules.
 *
 * Returns [Outline.Rounded] with a 50% corner radius so Android HWUI uses
 * fast rounded-rect clipping instead of an expensive Path mask.
 */
class SquirclePillShape(val exponent: Float = SQUIRCLE_PILL_EXPONENT) :
    CornerBasedShape(CornerSize(50), CornerSize(50), CornerSize(50), CornerSize(50)) {

    override fun copy(
        topStart: CornerSize,
        topEnd: CornerSize,
        bottomEnd: CornerSize,
        bottomStart: CornerSize,
    ): CornerBasedShape = SquirclePillShape(exponent)

    override fun createOutline(
        size: Size,
        topStart: Float,
        topEnd: Float,
        bottomEnd: Float,
        bottomStart: Float,
        layoutDirection: LayoutDirection,
    ): Outline {
        val r = size.minDimension / 2f
        if (r <= 0f) return Outline.Rectangle(size.toRect())
        return Outline.Rounded(RoundRect(size.toRect(), CornerRadius(r, r)))
    }

    override fun equals(other: Any?): Boolean = other is SquirclePillShape && other.exponent == exponent

    override fun hashCode(): Int = exponent.hashCode()

    override fun toString(): String = "SquirclePillShape(exponent = $exponent)"
}

/** Superellipse exponent identifier for liquid glass pills. */
const val SQUIRCLE_PILL_EXPONENT = 2.1f
