/*
 * Liquid Glass integration layer on top of the vendored backdrop library
 * (dev.sonata.android.backdrop, Apache-2.0).
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.toRect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.backdrop.BackdropEffectScope
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
private const val BLUR_RADIUS_DP = 14f
private const val LENS_HEIGHT = 0.36f
private const val LENS_AMOUNT = 0.42f
private const val LENS_MAX_DP = 48f
private const val SURFACE_OPACITY = 0.4f

/** Resolution fraction the glass surface records and processes its backdrop at. */
private const val GLASS_RESOLUTION_SCALE = 0.25f

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
fun glassContentColor(): Color = LocalLunariaColors.current.onSurfacePrimary

/** Selected-tab indicator colour for a glass surface, tuned to the active theme. */
@Composable
fun glassIndicatorColor(): Color = LocalLunariaColors.current.pillIndicatorColor

/** The selected tab's pill in dark theme, a step whiter than the bar. */
private const val DARK_INDICATOR_OPACITY = 0.14f

/** Hoisted static glass rim/shadow providers so Modifier.equals succeeds across recompositions. */
private val DefaultGlassHighlight: () -> Highlight? = { Highlight.Default }
private val DefaultGlassShadow: () -> Shadow? = { Shadow.Default }

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
    val tokens = LocalLunariaColors.current
    val rimBrush = remember(tokens.glassRimTopLeft, tokens.glassRimBottomRight) {
        Brush.linearGradient(
            colorStops = arrayOf(
                0.0f to tokens.glassRimTopLeft,
                0.55f to tokens.glassRimBottomRight.copy(alpha = tokens.glassRimBottomRight.alpha * 0.45f),
                1.0f to tokens.glassRimBottomRight,
            ),
            start = Offset.Zero,
            end = Offset(Float.POSITIVE_INFINITY, Float.POSITIVE_INFINITY),
        )
    }
    val shapeBlock: () -> Shape = remember(shape) { { shape } }
    val shapeProvider = remember(shapeBlock) { ShapeProvider(shapeBlock) }

    return shadow(
        elevation = if (tokens.isViolet) 10.dp else 6.dp,
        shape = shape,
        clip = false,
        ambientColor = tokens.glassShadowColor,
        spotColor = tokens.glassShadowColor,
    )
        .clip(shape)
        .background(
            color = if (useGlass) tokens.glassTint else fallbackColor,
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
        .border(0.75.dp, rimBrush, shape)
}

/**
 * Just the rim of liquid glass — the directional [Highlight] that catches the
 * light along a glass edge — for a control that keeps its own fill.
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
 * Renders this composable as a liquid glass surface sampling [LocalAppBackdrop]:
 * vibrancy, blur and single-tap lens refraction, then a theme-adaptive surface tint
 * (light glass on light theme, dark on dark). Returns the receiver unchanged on devices
 * without RenderEffect support.
 */
@Composable
fun Modifier.liquidGlass(
    shape: CornerBasedShape,
    /** Drawn over the glass in place of the theme's default tint, when set. */
    surfaceTint: Color? = null,
    chromaticAberration: Boolean = true,
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
    val tokens = LocalLunariaColors.current
    val surfaceTintColor = surfaceTint ?: tokens.glassTint
    val rimBrush = remember(tokens.glassRimTopLeft, tokens.glassRimBottomRight) {
        Brush.linearGradient(
            colorStops = arrayOf(
                0.0f to tokens.glassRimTopLeft,
                0.52f to tokens.glassRimBottomRight.copy(alpha = tokens.glassRimBottomRight.alpha * 0.40f),
                1.0f to tokens.glassRimBottomRight,
            ),
            start = Offset.Zero,
            end = Offset(Float.POSITIVE_INFINITY, Float.POSITIVE_INFINITY),
        )
    }
    val themeShadowProvider: () -> Shadow? = remember(tokens.glassShadowColor, tokens.isViolet) {
        {
            Shadow(
                radius = if (tokens.isViolet) 18.dp else 14.dp,
                offset = DpOffset(0.dp, if (tokens.isViolet) 8.dp else 5.dp),
                color = tokens.glassShadowColor,
            )
        }
    }

    val shapeBlock: () -> Shape = remember(shape) { { shape } }
    val effectsBlock: BackdropEffectScope.() -> Unit = remember(
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
    val specularTop = Color.White.copy(alpha = if (tokens.isDark) 0.07f else 0.18f)
    val specularBottom = tokens.petalLavender.copy(alpha = if (tokens.isDark) 0.08f else 0.10f)
    val onDrawSurfaceBlock: DrawScope.() -> Unit = remember(
        surfaceTintColor,
        specularTop,
        specularBottom,
    ) {
        val sheenBrush = Brush.verticalGradient(
            0.0f to specularTop,
            0.5f to Color.Transparent,
            1.0f to specularBottom,
        )
        val drawBlock: DrawScope.() -> Unit = {
            drawRect(color = surfaceTintColor, size = size)
            drawRect(brush = sheenBrush, size = size)
        }
        drawBlock
    }

    return drawBackdrop(
        backdrop = backdrop,
        shape = shapeBlock,
        effects = effectsBlock,
        highlight = DefaultGlassHighlight,
        shadow = themeShadowProvider,
        onDrawSurface = onDrawSurfaceBlock,
        exportedBackdrop = exportedBackdrop,
        backdropScale = resolutionScale,
    ).border(0.75.dp, rimBrush, shape)
}

/**
 * Reusable theme-aware Liquid Glass button/bar modifier.
 *
 * Applies real [liquidGlass] refraction whenever [LocalLiquidGlassEnabled] is true and supported,
 * and falls back to a layered Figma-stack surface (drop shadow + fill + inner specular rim)
 * when Liquid Glass is turned off.
 */
@Composable
fun Modifier.sonataThemeGlassButton(
    shape: CornerBasedShape,
    surfaceTint: Color? = null,
    fallbackColor: Color = LocalLunariaColors.current.eclipseCard,
    chromaticAberration: Boolean = false,
    shadowElevation: Dp = 10.dp,
): Modifier {
    val tokens = LocalLunariaColors.current
    val glassOn = LocalLiquidGlassEnabled.current && isGlassSupported() && !LocalReduceDynamicBlur.current
    if (glassOn) {
        return this.liquidGlass(
            shape = shape,
            surfaceTint = surfaceTint ?: tokens.glassTint,
            chromaticAberration = chromaticAberration,
        )
    }
    val rimBrush = remember(tokens.glassRimTopLeft, tokens.glassRimBottomRight) {
        Brush.linearGradient(
            colors = listOf(tokens.glassRimTopLeft, tokens.glassRimBottomRight),
            start = Offset.Zero,
            end = Offset(Float.POSITIVE_INFINITY, Float.POSITIVE_INFINITY),
        )
    }
    return this
        .shadow(
            elevation = shadowElevation,
            shape = shape,
            clip = false,
            ambientColor = tokens.glassShadowColor,
            spotColor = tokens.glassShadowColor,
        )
        .clip(shape)
        .background(fallbackColor, shape)
        .border(0.75.dp, rimBrush, shape)
}

/**
 * Hardware-fast-path capsule shape for Liquid Glass pills.
 *
 * Returns [Outline.Rounded] with a 50% corner radius so Android HWUI uses
 * fast rounded-rect clipping instead of an expensive 128-segment Path mask.
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
