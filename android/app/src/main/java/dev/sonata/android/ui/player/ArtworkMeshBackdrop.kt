package dev.sonata.android.ui.player

import android.graphics.Bitmap
import android.graphics.Color as AndroidColor
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt

/**
 * Artwork-derived color tokens for player surfaces and contrast scrims.
 */
data class SonataArtworkPalette(
    val background: Color,
    val wash: Color,
    val elevated: Color,
    val accent: Color,
    val onBackgroundVariant: Color,
    val topBandLuminance: Float,
)

private const val MESH_GRID = 6
private const val MESH_SIZE = 32
private const val MESH_CACHE_LIMIT = 32
private const val NEUTRAL_SATURATION_THRESHOLD = 0.12f

private val bakedMeshCache = object : LinkedHashMap<String, Bitmap>(MESH_CACHE_LIMIT, 0.75f, true) {
    override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Bitmap>): Boolean =
        size > MESH_CACHE_LIMIT
}

/**
 * Extracts a harmonious palette from [bitmap] in pure Kotlin.
 *
 * Includes a neutral-cover saturation guard (`sat < 0.12f` stays neutral so monochrome
 * artwork never shifts red/maroon) and computes the top-10% WCAG linear relative luminance
 * (`topBandLuminance`) for header contrast scrims.
 */
fun extractSonataArtworkPalette(
    bitmap: Bitmap,
    darkTheme: Boolean = true,
): SonataArtworkPalette {
    val sample = prepareSoftwareSample(bitmap, maxEdge = 64)
    val w = sample.width
    val h = sample.height
    val pixels = IntArray(w * h)
    sample.getPixels(pixels, 0, w, 0, 0, w, h)
    if (sample !== bitmap) sample.recycle()

    // 1. Top-10% WCAG linear relative luminance
    val topRows = max(1, (h * 0.10f).roundToInt())
    var lumSum = 0.0
    var lumCount = 0
    for (y in 0 until topRows) {
        val rowOffset = y * w
        for (x in 0 until w) {
            val c = pixels[rowOffset + x]
            lumSum += wcagRelativeLuminance(
                AndroidColor.red(c) / 255f,
                AndroidColor.green(c) / 255f,
                AndroidColor.blue(c) / 255f,
            )
            lumCount++
        }
    }
    val topBandLuminance = if (lumCount > 0) (lumSum / lumCount).toFloat().coerceIn(0f, 1f) else 0f

    // 2. Dominant hue & saturation analysis
    val hsv = FloatArray(3)
    var satSum = 0f
    var valSum = 0f
    var count = 0
    val hueBins = FloatArray(36)
    val hueSatWeight = FloatArray(36)
    var bestAccentScore = -1f
    var bestAccentH = 0f
    var bestAccentS = 0f
    var bestAccentV = 0.6f

    for (pixel in pixels) {
        AndroidColor.colorToHSV(pixel, hsv)
        val hue = hsv[0]
        val sat = hsv[1]
        val value = hsv[2]
        satSum += sat
        valSum += value
        count++

        if (sat >= NEUTRAL_SATURATION_THRESHOLD && value in 0.10f..0.95f) {
            val bin = ((hue / 10f).toInt()).coerceIn(0, 35)
            val weight = sat * (0.4f + 0.6f * value)
            hueBins[bin] += weight
            hueSatWeight[bin] += sat * weight
        }

        // Accent candidate prefers vivid mid-bright tones
        val vibrance = sat * 1.4f + (1f - abs(value - 0.68f)) * 0.6f
        if (sat >= NEUTRAL_SATURATION_THRESHOLD && vibrance > bestAccentScore) {
            bestAccentScore = vibrance
            bestAccentH = hue
            bestAccentS = sat
            bestAccentV = value
        }
    }

    val meanSat = if (count > 0) satSum / count else 0f
    var dominantBin = 0
    var dominantWeight = 0f
    for (i in hueBins.indices) {
        if (hueBins[i] > dominantWeight) {
            dominantWeight = hueBins[i]
            dominantBin = i
        }
    }

    val isNeutral = dominantWeight <= 0.001f || meanSat < NEUTRAL_SATURATION_THRESHOLD
    val domHue = dominantBin * 10f + 5f
    val domSat = if (isNeutral) 0f else (hueSatWeight[dominantBin] / dominantWeight).coerceIn(0.14f, 0.78f)

    return if (darkTheme) {
        if (isNeutral) {
            SonataArtworkPalette(
                background = Color(0xFF0E0E10),
                wash = Color(0xFF1C1C20),
                elevated = Color(0xFF26262B),
                accent = Color(0xFFDCDCE2),
                onBackgroundVariant = Color(0xFFB4B4BC),
                topBandLuminance = topBandLuminance,
            )
        } else {
            val bgColor = hsvToComposeColor(domHue, min(domSat * 0.55f, 0.42f), 0.11f)
            val washColor = hsvToComposeColor(domHue, min(domSat * 0.65f, 0.52f), 0.22f)
            val elevatedColor = hsvToComposeColor(domHue, min(domSat * 0.48f, 0.38f), 0.28f)
            val accentColor = hsvToComposeColor(
                bestAccentH,
                bestAccentS.coerceIn(0.42f, 0.85f),
                max(bestAccentV, 0.78f).coerceAtMost(0.96f),
            )
            val onBgVariant = hsvToComposeColor(domHue, min(domSat * 0.22f, 0.18f), 0.80f)
            SonataArtworkPalette(
                background = bgColor,
                wash = washColor,
                elevated = elevatedColor,
                accent = accentColor,
                onBackgroundVariant = onBgVariant,
                topBandLuminance = topBandLuminance,
            )
        }
    } else {
        if (isNeutral) {
            SonataArtworkPalette(
                background = Color(0xFFF4F4F6),
                wash = Color(0xFFE7E7EC),
                elevated = Color(0xFFFFFFFF),
                accent = Color(0xFF28282E),
                onBackgroundVariant = Color(0xFF56565F),
                topBandLuminance = topBandLuminance,
            )
        } else {
            val bgColor = hsvToComposeColor(domHue, min(domSat * 0.14f, 0.12f), 0.96f)
            val washColor = hsvToComposeColor(domHue, min(domSat * 0.22f, 0.18f), 0.90f)
            val elevatedColor = hsvToComposeColor(domHue, min(domSat * 0.10f, 0.08f), 0.99f)
            val accentColor = hsvToComposeColor(
                bestAccentH,
                bestAccentS.coerceIn(0.50f, 0.88f),
                min(bestAccentV, 0.58f).coerceAtLeast(0.38f),
            )
            val onBgVariant = hsvToComposeColor(domHue, min(domSat * 0.30f, 0.24f), 0.34f)
            SonataArtworkPalette(
                background = bgColor,
                wash = washColor,
                elevated = elevatedColor,
                accent = accentColor,
                onBackgroundVariant = onBgVariant,
                topBandLuminance = topBandLuminance,
            )
        }
    }
}

/**
 * Generates a CPU-baked 32x32 soft color mesh from [bitmap] and caches it in an LRU cache (32 entries).
 *
 * Pipeline:
 * 1. Downsample artwork to a 6x6 RGB grid.
 * 2. Keep row 0 sampled from the bottom edge of the artwork for top-seam continuity,
 *    and cyclically shift rows 1..5 deterministically from [seedKey] to avoid upside-down face mirroring.
 * 3. Smoothstep bilinear upsample from 6x6 to 32x32.
 * 4. Apply 3-pass separable box blur (horizontal + vertical) for ultra-smooth gradients with zero runtime blur cost.
 */
fun generateBakedArtworkMesh(bitmap: Bitmap, seedKey: String): Bitmap {
    synchronized(bakedMeshCache) {
        bakedMeshCache[seedKey]?.takeIf { !it.isRecycled }?.let { return it }
    }

    val src = prepareSoftwareSample(bitmap, maxEdge = 96)
    val sw = src.width
    val sh = src.height
    val srcPixels = IntArray(sw * sh)
    src.getPixels(srcPixels, 0, sw, 0, 0, sw, sh)
    if (src !== bitmap) src.recycle()

    // Step 1: Downsample into 6x6 RGB grid via area averaging
    val rawR = FloatArray(MESH_GRID * MESH_GRID)
    val rawG = FloatArray(MESH_GRID * MESH_GRID)
    val rawB = FloatArray(MESH_GRID * MESH_GRID)

    for (gy in 0 until MESH_GRID) {
        val y0 = (gy * sh) / MESH_GRID
        val y1 = max(y0 + 1, ((gy + 1) * sh) / MESH_GRID)
        for (gx in 0 until MESH_GRID) {
            val x0 = (gx * sw) / MESH_GRID
            val x1 = max(x0 + 1, ((gx + 1) * sw) / MESH_GRID)
            var rSum = 0f
            var gSum = 0f
            var bSum = 0f
            var n = 0
            for (py in y0 until y1) {
                val off = py * sw
                for (px in x0 until x1) {
                    val c = srcPixels[off + px]
                    rSum += AndroidColor.red(c)
                    gSum += AndroidColor.green(c)
                    bSum += AndroidColor.blue(c)
                    n++
                }
            }
            val idx = gy * MESH_GRID + gx
            val inv = if (n > 0) 1f / n else 0f
            rawR[idx] = rSum * inv
            rawG[idx] = gSum * inv
            rawB[idx] = bSum * inv
        }
    }

    // Step 2: Bottom edge at row 0, rows 1..5 cyclically shifted by deterministic seed
    val gridR = FloatArray(MESH_GRID * MESH_GRID)
    val gridG = FloatArray(MESH_GRID * MESH_GRID)
    val gridB = FloatArray(MESH_GRID * MESH_GRID)
    val seedHash = seedKey.hashCode() and 0x7FFF_FFFF
    val rowShift = 1 + (seedHash % (MESH_GRID - 1))

    for (gx in 0 until MESH_GRID) {
        val bottomIdx = (MESH_GRID - 1) * MESH_GRID + gx
        gridR[gx] = rawR[bottomIdx]
        gridG[gx] = rawG[bottomIdx]
        gridB[gx] = rawB[bottomIdx]
    }
    for (gy in 1 until MESH_GRID) {
        val srcRow = (gy - 1 + rowShift) % (MESH_GRID - 1)
        val colShift = ((seedHash ushr (gy * 3)) + gy) % MESH_GRID
        for (gx in 0 until MESH_GRID) {
            val srcCol = (gx + colShift) % MESH_GRID
            val dstIdx = gy * MESH_GRID + gx
            val srcIdx = srcRow * MESH_GRID + srcCol
            gridR[dstIdx] = rawR[srcIdx]
            gridG[dstIdx] = rawG[srcIdx]
            gridB[dstIdx] = rawB[srcIdx]
        }
    }

    // Step 3: Smoothstep bilinear upsample 6x6 -> 32x32
    val upR = FloatArray(MESH_SIZE * MESH_SIZE)
    val upG = FloatArray(MESH_SIZE * MESH_SIZE)
    val upB = FloatArray(MESH_SIZE * MESH_SIZE)

    for (y in 0 until MESH_SIZE) {
        val fy = (y.toFloat() / (MESH_SIZE - 1)) * (MESH_GRID - 1)
        val y0 = fy.toInt().coerceIn(0, MESH_GRID - 1)
        val y1 = min(y0 + 1, MESH_GRID - 1)
        val ty = smoothstep(fy - y0)
        for (x in 0 until MESH_SIZE) {
            val fx = (x.toFloat() / (MESH_SIZE - 1)) * (MESH_GRID - 1)
            val x0 = fx.toInt().coerceIn(0, MESH_GRID - 1)
            val x1 = min(x0 + 1, MESH_GRID - 1)
            val tx = smoothstep(fx - x0)

            val i00 = y0 * MESH_GRID + x0
            val i10 = y0 * MESH_GRID + x1
            val i01 = y1 * MESH_GRID + x0
            val i11 = y1 * MESH_GRID + x1

            val dst = y * MESH_SIZE + x
            upR[dst] = bilerp(gridR[i00], gridR[i10], gridR[i01], gridR[i11], tx, ty)
            upG[dst] = bilerp(gridG[i00], gridG[i10], gridG[i01], gridG[i11], tx, ty)
            upB[dst] = bilerp(gridB[i00], gridB[i10], gridB[i01], gridB[i11], tx, ty)
        }
    }

    // Step 4: 3-pass separable box blur (radius 2)
    val tmpR = FloatArray(MESH_SIZE * MESH_SIZE)
    val tmpG = FloatArray(MESH_SIZE * MESH_SIZE)
    val tmpB = FloatArray(MESH_SIZE * MESH_SIZE)
    repeat(3) {
        boxBlurHorizontal(upR, tmpR, MESH_SIZE, radius = 2)
        boxBlurVertical(tmpR, upR, MESH_SIZE, radius = 2)
        boxBlurHorizontal(upG, tmpG, MESH_SIZE, radius = 2)
        boxBlurVertical(tmpG, upG, MESH_SIZE, radius = 2)
        boxBlurHorizontal(upB, tmpB, MESH_SIZE, radius = 2)
        boxBlurVertical(tmpB, upB, MESH_SIZE, radius = 2)
    }

    // Slightly darken and preserve saturation for legible white player chrome
    val outPixels = IntArray(MESH_SIZE * MESH_SIZE)
    for (i in outPixels.indices) {
        val r = (upR[i] * 0.58f).roundToInt().coerceIn(0, 255)
        val g = (upG[i] * 0.58f).roundToInt().coerceIn(0, 255)
        val b = (upB[i] * 0.58f).roundToInt().coerceIn(0, 255)
        outPixels[i] = AndroidColor.rgb(r, g, b)
    }

    val result = Bitmap.createBitmap(outPixels, MESH_SIZE, MESH_SIZE, Bitmap.Config.ARGB_8888)
    synchronized(bakedMeshCache) {
        bakedMeshCache[seedKey] = result
    }
    return result
}

/**
 * Renders a CPU-baked 32x32 artwork color mesh with a smooth 700ms crossfade on track changes
 * and zero steady-state CPU/GPU blur overhead.
 */
@Composable
fun BakedArtworkMeshBackdrop(
    artworkUrl: String?,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    var previousMesh by remember { mutableStateOf<ImageBitmap?>(null) }
    var currentMesh by remember { mutableStateOf<ImageBitmap?>(null) }
    val transitionAlpha = remember { Animatable(1f) }

    LaunchedEffect(artworkUrl) {
        if (artworkUrl.isNullOrBlank()) return@LaunchedEffect
        val baked = withContext(Dispatchers.IO) {
            runCatching {
                val request = ImageRequest.Builder(context)
                    .data(artworkUrl)
                    .size(128, 128)
                    .allowHardware(false)
                    .build()
                val result = SingletonImageLoader.get(context).execute(request)
                val bmp = (result as? SuccessResult)?.image?.toBitmap() ?: return@runCatching null
                withContext(Dispatchers.Default) {
                    generateBakedArtworkMesh(bmp, seedKey = artworkUrl).asImageBitmap()
                }
            }.getOrNull()
        } ?: return@LaunchedEffect

        if (currentMesh == null) {
            currentMesh = baked
            transitionAlpha.snapTo(1f)
        } else {
            previousMesh = currentMesh
            currentMesh = baked
            transitionAlpha.snapTo(0f)
            transitionAlpha.animateTo(1f, animationSpec = tween(durationMillis = 700))
            previousMesh = null
        }
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(Color(0xFF0A0A0C)),
    ) {
        val prev = previousMesh
        val curr = currentMesh
        if (prev != null || curr != null) {
            Canvas(modifier = Modifier.fillMaxSize()) {
                val dstSize = IntSize(size.width.roundToInt().coerceAtLeast(1), size.height.roundToInt().coerceAtLeast(1))
                val alpha = transitionAlpha.value
                if (prev != null && alpha < 1f) {
                    drawImage(
                        image = prev,
                        dstOffset = IntOffset.Zero,
                        dstSize = dstSize,
                        alpha = 1f,
                        filterQuality = FilterQuality.Medium,
                    )
                }
                if (curr != null) {
                    drawImage(
                        image = curr,
                        dstOffset = IntOffset.Zero,
                        dstSize = dstSize,
                        alpha = alpha,
                        filterQuality = FilterQuality.Medium,
                    )
                }
            }
        }
    }
}

private fun prepareSoftwareSample(source: Bitmap, maxEdge: Int): Bitmap {
    val needsCopy = source.config == Bitmap.Config.HARDWARE || source.config != Bitmap.Config.ARGB_8888
    val maxDim = max(source.width, source.height)
    if (!needsCopy && maxDim <= maxEdge) return source

    val scale = if (maxDim > maxEdge) maxEdge.toFloat() / maxDim else 1f
    val targetW = max(1, (source.width * scale).roundToInt())
    val targetH = max(1, (source.height * scale).roundToInt())
    val software = if (needsCopy) source.copy(Bitmap.Config.ARGB_8888, false) else source
    if (software.width == targetW && software.height == targetH) return software
    val scaled = Bitmap.createScaledBitmap(software, targetW, targetH, true)
    if (software !== source && software !== scaled) software.recycle()
    return scaled
}

private fun wcagRelativeLuminance(r: Float, g: Float, b: Float): Double {
    fun linearize(c: Float): Double =
        if (c <= 0.04045f) c / 12.92 else ((c + 0.055) / 1.055).pow(2.4)
    return 0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

private fun hsvToComposeColor(hue: Float, sat: Float, value: Float): Color {
    val argb = AndroidColor.HSVToColor(floatArrayOf(hue, sat.coerceIn(0f, 1f), value.coerceIn(0f, 1f)))
    return Color(argb)
}

private fun smoothstep(t: Float): Float {
    val c = t.coerceIn(0f, 1f)
    return c * c * (3f - 2f * c)
}

private fun bilerp(v00: Float, v10: Float, v01: Float, v11: Float, tx: Float, ty: Float): Float {
    val top = v00 + (v10 - v00) * tx
    val bottom = v01 + (v11 - v01) * tx
    return top + (bottom - top) * ty
}

private fun boxBlurHorizontal(src: FloatArray, dst: FloatArray, size: Int, radius: Int) {
    val diameter = radius * 2 + 1
    for (y in 0 until size) {
        val row = y * size
        for (x in 0 until size) {
            var sum = 0f
            for (k in -radius..radius) {
                val sx = (x + k).coerceIn(0, size - 1)
                sum += src[row + sx]
            }
            dst[row + x] = sum / diameter
        }
    }
}

private fun boxBlurVertical(src: FloatArray, dst: FloatArray, size: Int, radius: Int) {
    val diameter = radius * 2 + 1
    for (y in 0 until size) {
        for (x in 0 until size) {
            var sum = 0f
            for (k in -radius..radius) {
                val sy = (y + k).coerceIn(0, size - 1)
                sum += src[sy * size + x]
            }
            dst[y * size + x] = sum / diameter
        }
    }
}
