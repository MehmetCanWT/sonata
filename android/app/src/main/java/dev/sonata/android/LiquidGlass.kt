/*
 * Liquid Glass integration glue on top of the vendored backdrop library
 * (dev.sonata.android.backdrop, Apache-2.0) and glass effect config inspired by
 * Echo-Music (GPL-3.0).
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
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.toRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalDensity
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
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.sin

/** Whether the liquid glass nav bar and floating glass controls are turned on. */
val LocalLiquidGlassEnabled = staticCompositionLocalOf { false }

/** Whether dynamic blur sampling is reduced to a flat surface fill. */
val LocalReduceDynamicBlur = staticCompositionLocalOf { false }

/** Whether lens refraction is skipped in favor of frosted blur only. */
val LocalGlassPerformance = staticCompositionLocalOf { false }

/** Whether tab bar and pill spring animations are snapped immediately. */
val LocalReduceAnimation = staticCompositionLocalOf { false }

/** The backdrop content (app UI) that a liquid glass surface samples from. */
val LocalAppBackdrop = staticCompositionLocalOf<Backdrop> { error("No AppBackdrop provided") }

/**
 * Where a [liquidGlass] surface composed under it also records the glass it
 * draws — the sampled, blurred and tinted backdrop, without its rim or shadow —
 * so a lens above it can refract that surface rather than the page behind it.
 * The tab bar's travelling selection pill is the one reader; see
 * [dev.sonata.android.floatingtabbar.GlassSelectionPill].
 */
internal val LocalGlassExport = compositionLocalOf<LayerBackdrop?> { null }

/** Standard spring used by glass navigation transitions. */
internal val GlassSpring = spring<Float>(dampingRatio = 0.72f, stiffness = 320f)

/**
 * The backdrop blur pipeline requires [android.graphics.RenderEffect] on a
 * [android.graphics.RenderNode], available from Android 12 (API 31).
 */
fun isGlassSupported(sdkInt: Int = Build.VERSION.SDK_INT): Boolean = sdkInt >= Build.VERSION_CODES.S

private const val VIBRANCY = 1f
private const val BLUR_RADIUS_DP = 8f
private const val LENS_HEIGHT = 0.5f
private const val LENS_AMOUNT = 0.5f
private const val LENS_MAX_DP = 48f
private const val SURFACE_OPACITY = 0.4f

/** Resolution fraction the glass surface records and processes its backdrop at. */
private const val GLASS_RESOLUTION_SCALE = 0.33f

/** Blur radius and resolution under [LocalGlassPerformance] (no lens refraction). */
private const val FROST_BLUR_RADIUS_DP = 16f
private const val FROST_RESOLUTION_SCALE = 0.25f

/**
 * The hairline along a bar's edge, and what stands in for the glass rim
 * wherever the glass itself is not drawn.
 */
internal val GLASS_EDGE_WIDTH = 0.5.dp
internal val GLASS_EDGE_COLOR = Color.White.copy(alpha = 0.10f)

/** Icon and label colour for content sitting on a glass surface. */
@Composable
fun glassContentColor(): Color =
    if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) Color.Black else Color.White

/** Selected-tab indicator colour for a glass surface: white in both themes. */
@Composable
fun glassIndicatorColor(): Color =
    if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) {
        Color.White.copy(alpha = 0.5f)
    } else {
        Color.White.copy(alpha = DARK_INDICATOR_OPACITY)
    }

/** The selected tab's pill in dark theme, a step whiter than the bar. */
private const val DARK_INDICATOR_OPACITY = 0.14f

/**
 * A lightweight visual match for liquid glass over a stable background.
 *
 * Keeps the same translucent tint, directional highlight and hairline as
 * [liquidGlass], without backdrop capture, blur, lens refraction or shadow
 * rendering. When Liquid Glass is disabled or unsupported, [fallbackColor]
 * preserves the control's filled appearance.
 */
@Composable
fun Modifier.lightweightLiquidGlass(
    shape: CornerBasedShape,
    fallbackColor: Color,
): Modifier {
    val useGlass = LocalLiquidGlassEnabled.current && isGlassSupported()
    val glassTint = if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) {
        Color(0xFFFAFAFA)
    } else {
        Color(0xFF121212)
    }
    val shapeProvider = remember(shape) { ShapeProvider { shape } }

    return clip(shape)
        .background(
            color = if (useGlass) glassTint.copy(alpha = SURFACE_OPACITY) else fallbackColor,
            shape = shape,
        )
        .then(
            if (useGlass) {
                HighlightElement(
                    shapeProvider = shapeProvider,
                    highlight = { Highlight.Default },
                )
            } else {
                Modifier
            },
        )
        .border(GLASS_EDGE_WIDTH, GLASS_EDGE_COLOR, shape)
}

/**
 * Just the rim of liquid glass — the directional [Highlight] that catches the
 * light along a glass edge — for a control that keeps its own fill.
 */
@Composable
fun Modifier.liquidGlassRim(shape: CornerBasedShape): Modifier {
    if (!LocalLiquidGlassEnabled.current || !isGlassSupported()) return this
    val shapeProvider = remember(shape) { ShapeProvider { shape } }
    return then(
        HighlightElement(
            shapeProvider = shapeProvider,
            highlight = { Highlight.Default },
        ),
    )
}

/**
 * Renders this composable as a liquid glass surface sampling [LocalAppBackdrop]:
 * vibrancy, blur and lens refraction, then a theme-adaptive surface tint (light
 * glass on light theme, dark on dark). Returns the receiver unchanged on devices
 * without RenderEffect support.
 */
@Composable
fun Modifier.liquidGlass(
    shape: CornerBasedShape,
    /** Drawn over the glass in place of the theme's default tint, when set. */
    surfaceTint: Color? = null,
): Modifier {
    if (!isGlassSupported()) return this
    if (LocalReduceDynamicBlur.current) {
        return background(MaterialTheme.colorScheme.surface, shape)
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
    val themeTint = if (MaterialTheme.colorScheme.surface.luminance() > 0.5f) {
        Color(0xFFFAFAFA)
    } else {
        Color(0xFF121212)
    }
    val surfaceTintColor = surfaceTint ?: themeTint.copy(alpha = SURFACE_OPACITY)

    return drawBackdrop(
        backdrop = backdrop,
        shape = { shape },
        effects = {
            colorControls(saturation = 1f + 0.5f * VIBRANCY)
            blur(blurPx)
            if (!glassPerformance && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                lens(
                    refractionHeight = lensHeightPx,
                    refractionAmount = lensAmountPx,
                    depthEffect = true,
                    chromaticAberration = true,
                )
            }
        },
        highlight = { Highlight.Default },
        shadow = { Shadow.Default },
        onDrawSurface = {
            drawRect(color = surfaceTintColor, size = size)
        },
        exportedBackdrop = exportedBackdrop,
        backdropScale = resolutionScale,
    )
}

/**
 * A pill cut as a fully rounded squircle, the way Liquid Glass draws its capsules.
 *
 * Each quarter of an end is a superellipse, `|x|^n + |y|^n = 1`, where
 * [exponent] controls shoulder fullness. Implemented as a [CornerBasedShape]
 * of 50% corners so the backdrop's lens treats it as a capsule.
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
        val w = size.width
        val h = size.height
        val path = Path()
        path.moveTo(r, 0f)
        path.lineTo(w - r, 0f)
        path.quarter(w - r, r, 1f, -1f, r, towardEdge = false)
        path.lineTo(w, h - r)
        path.quarter(w - r, h - r, 1f, 1f, r, towardEdge = true)
        path.lineTo(r, h)
        path.quarter(r, h - r, -1f, 1f, r, towardEdge = false)
        path.lineTo(0f, r)
        path.quarter(r, r, -1f, -1f, r, towardEdge = true)
        path.close()
        return Outline.Generic(path)
    }

    private fun Path.quarter(
        cx: Float,
        cy: Float,
        sx: Float,
        sy: Float,
        r: Float,
        towardEdge: Boolean,
    ) {
        val power = 2f / exponent
        for (i in 1..QUARTER_STEPS) {
            val step = if (towardEdge) i else QUARTER_STEPS - i
            val t = step * (Math.PI / 2) / QUARTER_STEPS
            val x = cos(t).toFloat().coerceAtLeast(0f).pow(power)
            val y = sin(t).toFloat().coerceAtLeast(0f).pow(power)
            lineTo(cx + sx * r * x, cy + sy * r * y)
        }
    }

    override fun equals(other: Any?): Boolean = other is SquirclePillShape && other.exponent == exponent

    override fun hashCode(): Int = exponent.hashCode()

    override fun toString(): String = "SquirclePillShape(exponent = $exponent)"
}

/** Superellipse exponent for liquid glass pills. */
const val SQUIRCLE_PILL_EXPONENT = 2.1f

/** Segments in each quarter of an end. */
private const val QUARTER_STEPS = 32
