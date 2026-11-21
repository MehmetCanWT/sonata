/*
 * Vendored from Kyant0/backdrop v2.0.0 (io.github.kyant0:backdrop)
 * https://github.com/Kyant0/backdrop — Copyright 2025 Kyant0, Apache License 2.0
 *
 * Vendored so the library ships as source with this app (binary AARs compiled
 * against older Compose broke at runtime) and to add a backdrop resolution
 * scale for cheaper effect rendering. KMP expect/actual declarations were
 * merged into this single Android source set. Package renamed accordingly.
 */
package dev.sonata.android.backdrop.internal

import android.graphics.BlurMaskFilter
import android.util.LruCache
import androidx.compose.ui.graphics.Paint
import dev.sonata.android.backdrop.RuntimeShader
import dev.sonata.android.backdrop.asAndroidRuntimeShader
import kotlin.math.roundToInt

private val BlurMaskFilterCache = LruCache<Int, BlurMaskFilter>(16)

internal fun Paint.blur(radius: Float) {
    val frameworkPaint = this.asFrameworkPaint()
    if (radius <= 0f) {
        frameworkPaint.maskFilter = null
        return
    }
    val key = (radius * 4f).roundToInt().coerceAtLeast(1)
    val filter = BlurMaskFilterCache.get(key) ?: BlurMaskFilter(key / 4f, BlurMaskFilter.Blur.NORMAL).also {
        BlurMaskFilterCache.put(key, it)
    }
    frameworkPaint.maskFilter = filter
}

internal fun Paint.setRuntimeShader(runtimeShader: RuntimeShader?) {
    this.asFrameworkPaint().shader = runtimeShader?.asAndroidRuntimeShader()
}
