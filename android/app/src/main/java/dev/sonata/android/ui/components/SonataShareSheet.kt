/*
 * Custom visual Share modal & high-resolution PNG card renderer for Track, Album, Playlist, and Artist items.
 */
package dev.sonata.android.ui.components

import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapShader
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path as AndroidPath
import android.graphics.RadialGradient
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.Typeface
import android.net.Uri
import android.os.Environment
import android.provider.MediaStore
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import android.widget.Toast
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.FileProvider
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.size.Precision
import coil3.toBitmap
import dev.sonata.android.DetailShareIcon
import dev.sonata.android.R
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.buildShareUrlForCollection
import dev.sonata.android.encodeUrlQuery
import dev.sonata.android.shareYouTubeMusicUrl
import dev.sonata.android.ui.ArtworkPalette
import dev.sonata.android.ui.HEADER_ART_PX
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.LunariaThemeTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sonata_android.AndroidTrack
import java.io.File
import java.io.FileOutputStream
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

sealed interface SonataShareTarget {
    val shareTitle: String
    val shareSubtitle: String
    val eyebrowLabel: String
    val artworkUrl: String?
    val shareUrl: String

    data class Track(val track: AndroidTrack) : SonataShareTarget {
        override val shareTitle: String get() = track.title
        override val shareSubtitle: String
            get() = listOf(track.artist, track.album.takeIf { it.isNotBlank() })
                .filterNotNull()
                .filter { it.isNotBlank() }
                .joinToString(" · ")
        override val eyebrowLabel: String get() = "SONG"
        override val artworkUrl: String? get() = track.thumbnailUrl
        override val shareUrl: String
            get() = if (track.id.isNotBlank() &&
                !track.id.startsWith("jiosaavn:") &&
                !track.id.startsWith("local:")
            ) {
                "https://music.youtube.com/watch?v=${track.id}"
            } else {
                val query = listOf(track.title, track.artist).filter { it.isNotBlank() }.joinToString(" ")
                "https://music.youtube.com/search?q=${encodeUrlQuery(query)}"
            }
    }

    data class Collection(
        val browseId: String,
        val collectionTitle: String,
        val collectionSubtitle: String,
        val coverUrl: String?,
        val isAlbum: Boolean,
        val trackCount: Int? = null,
        val year: String? = null,
    ) : SonataShareTarget {
        override val shareTitle: String get() = collectionTitle
        override val shareSubtitle: String get() = collectionSubtitle
        override val eyebrowLabel: String get() = if (isAlbum) "ALBUM" else "PLAYLIST"
        override val artworkUrl: String? get() = coverUrl
        override val shareUrl: String
            get() = buildShareUrlForCollection(browseId, collectionTitle)

        val badgeMeta: String?
            get() = listOfNotNull(
                trackCount?.takeIf { it > 0 }?.let { "$it ${if (it == 1) "TRACK" else "TRACKS"}" },
                year?.takeIf { it.isNotBlank() },
            ).takeIf { it.isNotEmpty() }?.joinToString(" • ")

        fun localizedBadgeMeta(strings: dev.sonata.android.ui.i18n.SonataStrings): String? = listOfNotNull(
            trackCount?.takeIf { it > 0 }?.let { strings.shareTrackCountBadge(it) },
            year?.takeIf { it.isNotBlank() },
        ).takeIf { it.isNotEmpty() }?.joinToString(" • ")
    }

    data class Artist(
        val artistId: String,
        val artistName: String,
        val subscribers: String? = null,
        val avatarUrl: String?,
    ) : SonataShareTarget {
        override val shareTitle: String get() = artistName
        override val shareSubtitle: String get() = subscribers.orEmpty()
        override val eyebrowLabel: String get() = "ARTIST"
        override val artworkUrl: String? get() = avatarUrl
        override val shareUrl: String
            get() = if (artistId.startsWith("UC")) {
                "https://music.youtube.com/channel/$artistId"
            } else {
                "https://music.youtube.com/search?q=${encodeUrlQuery(artistName)}"
            }
    }
}

private data class ShareCardVisualPalette(
    val topColor: Color,
    val midColor: Color,
    val bottomColor: Color,
    val glowPrimary: Color,
    val glowSecondary: Color,
    val strokeColor: Color,
    val chipFill: Color,
)

private fun buildShareCardPalette(
    artPalette: ArtworkPalette,
    theme: LunariaThemeTokens,
): ShareCardVisualPalette {
    val artPrimary = artPalette.meshColors.getOrNull(0) ?: artPalette.accent
    val artSecondary = artPalette.meshColors.getOrNull(1) ?: artPalette.wash
    val artDeep = artPalette.meshColors.getOrNull(3) ?: artPalette.background

    val themeAnchorTop = Color(0xFF1C1C1E)
    val themeAnchorBot = Color(0xFF0A0A0C)
    val themeGlow = Color(0xFFE5E5EA)
    val themeAccent = Color(0xFFD1D1D6)

    return ShareCardVisualPalette(
        topColor = lerpColor(artDeep, themeAnchorTop, 0.44f),
        midColor = lerpColor(artSecondary, lerpColor(themeAnchorTop, themeAccent, 0.25f), 0.36f),
        bottomColor = lerpColor(artDeep, themeAnchorBot, 0.56f),
        glowPrimary = lerpColor(artPrimary, themeGlow, 0.34f),
        glowSecondary = lerpColor(artSecondary, theme.moonGlow, 0.38f),
        strokeColor = Color.White.copy(alpha = 0.22f),
        chipFill = Color.White.copy(alpha = 0.14f),
    )
}

private fun lerpColor(a: Color, b: Color, t: Float): Color {
    val clamped = t.coerceIn(0f, 1f)
    return Color(
        red = a.red + (b.red - a.red) * clamped,
        green = a.green + (b.green - a.green) * clamped,
        blue = a.blue + (b.blue - a.blue) * clamped,
        alpha = a.alpha + (b.alpha - a.alpha) * clamped,
    )
}

@Composable
internal fun SonataShareSheet(
    target: SonataShareTarget?,
    @Suppress("UNUSED_PARAMETER") backdrop: Backdrop?,
    onDismiss: () -> Unit,
    onStatusMessage: (String) -> Unit = {},
) {
    if (target == null) return

    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val theme = LocalLunariaColors.current
    val highResUrl = remember(target.artworkUrl) { target.artworkUrl.artworkAt(HEADER_ART_PX) }
    val artPalette = rememberArtworkPalette(highResUrl, dark = true, artPx = HEADER_ART_PX, animated = false)
    val cardPalette = remember(artPalette, theme) { buildShareCardPalette(artPalette, theme) }

    var loadedBitmap by remember(highResUrl) { mutableStateOf<Bitmap?>(null) }
    var busyAction by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(highResUrl) {
        loadedBitmap = null
        if (highResUrl.isNullOrBlank()) return@LaunchedEffect
        val req = ImageRequest.Builder(context)
            .data(highResUrl)
            .size(720)
            .precision(Precision.INEXACT)
            .allowHardware(false)
            .build()
        val result = SingletonImageLoader.get(context).execute(req)
        val bmp = (result as? SuccessResult)?.image?.toBitmap()
        if (bmp != null) {
            loadedBitmap = bmp.copy(Bitmap.Config.ARGB_8888, false) ?: bmp
        }
    }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
        ),
    ) {
        val scrimColor = if (theme.isBasic) {
            Color.Black.copy(alpha = 0.78f)
        } else {
            Color(0xFF090612).copy(alpha = 0.72f)
        }
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(scrimColor)
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                    onClick = onDismiss,
                )
                .statusBarsPadding()
                .navigationBarsPadding(),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                modifier = Modifier
                    .widthIn(max = 460.dp)
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp, vertical = 16.dp)
                    .verticalScroll(rememberScrollState())
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        onClick = {},
                    ),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val strings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = strings.shareCardTitle,
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                        color = Color.White,
                        modifier = Modifier.weight(1f),
                    )
                    Box(
                        modifier = Modifier
                            .size(36.dp)
                            .clip(CircleShape)
                            .background(Color.White.copy(alpha = 0.14f))
                            .border(0.75.dp, Color.White.copy(alpha = 0.25f), CircleShape)
                            .clickable(onClick = onDismiss),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = SonataIcons.Close,
                            contentDescription = strings.close,
                            tint = Color.White,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }

                // Live visual share card preview
                ShareCardPreview(
                    target = target,
                    bitmap = loadedBitmap,
                    palette = cardPalette,
                    theme = theme,
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(Modifier.height(18.dp))

                // 3 Action Buttons: Share Link, Share Image, Save Image
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    ShareActionPill(
                        label = strings.copyLink,
                        icon = LinkShareIcon,
                        busy = false,
                        theme = theme,
                        modifier = Modifier.weight(1f),
                        onClick = {
                            shareYouTubeMusicUrl(context, target.shareUrl, target.shareTitle)
                        },
                    )
                    ShareActionPill(
                        label = strings.shareMore,
                        icon = DetailShareIcon,
                        busy = busyAction == "share",
                        theme = theme,
                        modifier = Modifier.weight(1f),
                        onClick = {
                            if (busyAction != null) return@ShareActionPill
                            busyAction = "share"
                            scope.launch {
                                val uri = withContext(Dispatchers.IO) {
                                    val rendered = renderShareCardPngBitmap(
                                        target = target,
                                        artwork = loadedBitmap,
                                        palette = cardPalette,
                                        theme = theme,
                                        strings = strings,
                                    )
                                    writeShareCachePng(context, rendered, target.shareTitle)
                                }
                                busyAction = null
                                if (uri != null) {
                                    val send = Intent(Intent.ACTION_SEND).apply {
                                        type = "image/png"
                                        putExtra(Intent.EXTRA_STREAM, uri)
                                        putExtra(Intent.EXTRA_TEXT, "${target.shareTitle}\n${target.shareUrl}")
                                        putExtra(Intent.EXTRA_SUBJECT, target.shareTitle)
                                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                    }
                                    runCatching {
                                        context.startActivity(Intent.createChooser(send, strings.share))
                                    }
                                }
                            }
                        },
                    )
                    ShareActionPill(
                        label = strings.save,
                        icon = SonataIcons.Download,
                        busy = busyAction == "save",
                        theme = theme,
                        modifier = Modifier.weight(1f),
                        onClick = {
                            if (busyAction != null) return@ShareActionPill
                            busyAction = "save"
                            scope.launch {
                                val savedOk = withContext(Dispatchers.IO) {
                                    val rendered = renderShareCardPngBitmap(
                                        target = target,
                                        artwork = loadedBitmap,
                                        palette = cardPalette,
                                        theme = theme,
                                        strings = strings,
                                    )
                                    saveSharePngToGallery(context, rendered, target.shareTitle)
                                }
                                busyAction = null
                                val msg = if (savedOk) {
                                    strings.savedImageToGallery
                                } else {
                                    strings.failedToSaveImage
                                }
                                onStatusMessage(msg)
                                Toast.makeText(context, msg, Toast.LENGTH_SHORT).show()
                            }
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun ShareCardPreview(
    target: SonataShareTarget,
    bitmap: Bitmap?,
    palette: ShareCardVisualPalette,
    theme: LunariaThemeTokens,
    modifier: Modifier = Modifier,
) {
    val cardShape = RoundedCornerShape(16.dp)
    Box(
        modifier = modifier
            .aspectRatio(0.82f)
            .shadow(22.dp, cardShape, clip = false, ambientColor = palette.glowPrimary, spotColor = palette.glowPrimary)
            .clip(cardShape)
            .background(
                Brush.verticalGradient(
                    listOf(palette.topColor, palette.midColor, palette.bottomColor),
                ),
            )
            .border(1.dp, palette.strokeColor, cardShape),
    ) {
        // Monochrome Basic aura background
        Canvas(modifier = Modifier.fillMaxSize()) {
            val w = size.width
            val h = size.height
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(palette.glowPrimary.copy(alpha = 0.56f), palette.glowPrimary.copy(alpha = 0f)),
                    center = Offset(w * 0.5f, h * 0.34f),
                    radius = w * 0.68f,
                ),
                radius = w * 0.68f,
                center = Offset(w * 0.5f, h * 0.34f),
            )
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(palette.glowSecondary.copy(alpha = 0.38f), palette.glowSecondary.copy(alpha = 0f)),
                    center = Offset(w * 0.84f, h * 0.78f),
                    radius = w * 0.52f,
                ),
                radius = w * 0.52f,
                center = Offset(w * 0.84f, h * 0.78f),
            )
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 24.dp, vertical = 22.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(Modifier.height(6.dp))

            // Type Pill ("SONG" / "ALBUM" / "PLAYLIST" / "ARTIST")
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(palette.chipFill)
                        .border(0.75.dp, Color.White.copy(alpha = 0.24f), RoundedCornerShape(percent = 50))
                        .padding(horizontal = 11.dp, vertical = 4.dp),
                ) {
                    val shareStrings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
                    Text(
                        text = target.localizedEyebrowLabel(shareStrings),
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                            letterSpacing = 1.2.sp,
                        ),
                        color = Color.White.copy(alpha = 0.92f),
                    )
                }
                if (target is SonataShareTarget.Track && target.track.isExplicit) {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(6.dp))
                            .background(Color.White.copy(alpha = 0.20f))
                            .padding(horizontal = 6.dp, vertical = 2.dp),
                    ) {
                        Text(
                            text = "E",
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontSize = 9.5.sp,
                                fontWeight = FontWeight.ExtraBold,
                            ),
                            color = Color.White,
                        )
                    }
                }
            }

            Spacer(Modifier.weight(0.65f))

            // Visual Artwork tailored to Track / Album / Playlist / Artist
            ShareTargetArtworkVisual(
                target = target,
                bitmap = bitmap,
                theme = theme,
                palette = palette,
            )

            Spacer(Modifier.weight(0.7f))

            // Title & Subtitle
            Text(
                text = target.shareTitle,
                style = MaterialTheme.typography.headlineSmall.copy(
                    fontSize = 22.sp,
                    lineHeight = 27.sp,
                    fontWeight = FontWeight.ExtraBold,
                ),
                color = Color.White,
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )

            if (target.shareSubtitle.isNotBlank()) {
                Spacer(Modifier.height(5.dp))
                Text(
                    text = target.shareSubtitle,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Medium,
                    ),
                    color = Color.White.copy(alpha = 0.78f),
                    textAlign = TextAlign.Center,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            val shareMetaStrings = dev.sonata.android.ui.i18n.LocalSonataStrings.current
            val localizedMeta = (target as? SonataShareTarget.Collection)?.localizedBadgeMeta(shareMetaStrings)
            if (!localizedMeta.isNullOrBlank()) {
                Spacer(Modifier.height(10.dp))
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(percent = 50))
                        .background(Color.White.copy(alpha = 0.12f))
                        .padding(horizontal = 10.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = localizedMeta,
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold,
                            letterSpacing = 0.8.sp,
                        ),
                        color = Color.White.copy(alpha = 0.84f),
                    )
                }
            } else if (target is SonataShareTarget.Track) {
                Spacer(Modifier.height(12.dp))
                MiniWaveformAccent(
                    accentColor = palette.glowPrimary,
                    secondaryColor = Color.White.copy(alpha = 0.78f),
                )
            }

            Spacer(Modifier.weight(0.65f))
        }

        // Unobtrusive small Sonata placeholder watermark in the bottom-right corner
        Row(
            modifier = Modifier
                .align(Alignment.BottomEnd)
                .padding(end = 15.dp, bottom = 13.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_sonata_moon),
                contentDescription = null,
                tint = Color.White.copy(alpha = 0.54f),
                modifier = Modifier.size(11.dp),
            )
            Text(
                text = "Sonata",
                style = MaterialTheme.typography.labelSmall.copy(
                    fontSize = 10.5.sp,
                    fontWeight = FontWeight.SemiBold,
                    letterSpacing = 0.5.sp,
                ),
                color = Color.White.copy(alpha = 0.54f),
            )
        }
    }
}

@Composable
private fun ShareTargetArtworkVisual(
    target: SonataShareTarget,
    bitmap: Bitmap?,
    theme: LunariaThemeTokens,
    palette: ShareCardVisualPalette,
) {
    val artSize = 166.dp
    when (target) {
        is SonataShareTarget.Artist -> {
            Box(
                modifier = Modifier.size(artSize + 18.dp),
                contentAlignment = Alignment.Center,
            ) {
                Canvas(modifier = Modifier.fillMaxSize()) {
                    drawCircle(
                        brush = Brush.sweepGradient(
                            listOf(
                                palette.glowPrimary.copy(alpha = 0.65f),
                                palette.glowSecondary.copy(alpha = 0.20f),
                                Color.White.copy(alpha = 0.62f),
                                palette.glowPrimary.copy(alpha = 0.65f),
                            ),
                        ),
                        radius = size.minDimension * 0.49f,
                        style = Stroke(width = 2.dp.toPx()),
                    )
                }
                Box(
                    modifier = Modifier
                        .size(artSize)
                        .shadow(16.dp, CircleShape, clip = false)
                        .clip(CircleShape)
                        .background(palette.midColor)
                        .border(1.5.dp, Color.White.copy(alpha = 0.42f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    if (bitmap != null) {
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.Person,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.65f),
                            modifier = Modifier.size(56.dp),
                        )
                    }
                }
            }
        }
        is SonataShareTarget.Collection -> {
            val coverShape = RoundedCornerShape(
                when {
                    theme.isMoonflower -> 20.dp
                    theme.isBasic -> 12.dp
                    else -> 14.dp
                },
            )
            Box(
                modifier = Modifier.size(width = artSize + 36.dp, height = artSize + 14.dp),
                contentAlignment = Alignment.Center,
            ) {
                if (target.isAlbum) {
                    // Subtle vinyl record peeking out on the right side for Albums
                    Box(
                        modifier = Modifier
                            .offset(x = 22.dp)
                            .size(artSize * 0.88f)
                            .clip(CircleShape)
                            .background(Color(0xFF100E16))
                            .border(1.dp, Color.White.copy(alpha = 0.22f), CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        Box(
                            modifier = Modifier
                                .size(artSize * 0.28f)
                                .clip(CircleShape)
                                .background(palette.glowPrimary.copy(alpha = 0.85f))
                                .border(2.dp, Color(0xFF0A090F), CircleShape),
                        )
                    }
                } else {
                    // Stacked glass sleeves behind Playlist cover
                    Box(
                        modifier = Modifier
                            .offset(y = (-8).dp)
                            .size(artSize * 0.88f)
                            .clip(coverShape)
                            .background(Color.White.copy(alpha = 0.14f))
                            .border(0.75.dp, Color.White.copy(alpha = 0.22f), coverShape),
                    )
                }
                Box(
                    modifier = Modifier
                        .offset(x = if (target.isAlbum) (-12).dp else 0.dp)
                        .size(artSize)
                        .shadow(18.dp, coverShape, clip = false)
                        .clip(coverShape)
                        .background(palette.midColor)
                        .border(1.2.dp, Color.White.copy(alpha = 0.38f), coverShape),
                    contentAlignment = Alignment.Center,
                ) {
                    if (bitmap != null) {
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = Color.White.copy(alpha = 0.6f),
                            modifier = Modifier.size(54.dp),
                        )
                    }
                }
            }
        }
        is SonataShareTarget.Track -> {
            val coverShape = RoundedCornerShape(
                when {
                    theme.isMoonflower -> 22.dp
                    theme.isBasic -> 14.dp
                    else -> 16.dp
                },
            )
            Box(
                modifier = Modifier
                    .size(artSize)
                    .shadow(20.dp, coverShape, clip = false, ambientColor = palette.glowPrimary, spotColor = palette.glowPrimary)
                    .clip(coverShape)
                    .background(palette.midColor)
                    .border(1.2.dp, Color.White.copy(alpha = 0.42f), coverShape),
                contentAlignment = Alignment.Center,
            ) {
                if (bitmap != null) {
                    Image(
                        bitmap = bitmap.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Icon(
                        imageVector = SonataIcons.MusicNote,
                        contentDescription = null,
                        tint = Color.White.copy(alpha = 0.6f),
                        modifier = Modifier.size(54.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun MiniWaveformAccent(
    accentColor: Color,
    secondaryColor: Color,
) {
    val bars = remember { listOf(0.32f, 0.56f, 0.85f, 0.48f, 1.0f, 0.72f, 0.40f, 0.90f, 0.62f, 0.35f, 0.78f, 0.46f) }
    Row(
        horizontalArrangement = Arrangement.spacedBy(3.5.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.height(18.dp),
    ) {
        bars.forEachIndexed { idx, amp ->
            Box(
                modifier = Modifier
                    .width(2.5.dp)
                    .height((16 * amp).dp)
                    .clip(RoundedCornerShape(percent = 50))
                    .background(if (idx % 2 == 0) secondaryColor else accentColor.copy(alpha = 0.92f)),
            )
        }
    }
}

@Composable
private fun ShareActionPill(
    label: String,
    icon: ImageVector,
    busy: Boolean,
    theme: LunariaThemeTokens,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    val shape = RoundedCornerShape(14.dp)
    val tintTop = Color(0xFF222226).copy(alpha = 0.90f)
    val tintBot = Color(0xFF141416).copy(alpha = 0.95f)
    val rimBrush = Brush.linearGradient(
        listOf(
            Color.White.copy(alpha = 0.24f),
            Color.White.copy(alpha = 0.10f),
        ),
    )
    Column(
        modifier = modifier
            .clip(shape)
            .background(Brush.verticalGradient(listOf(tintTop, tintBot)), shape)
            .border(0.9.dp, rimBrush, shape)
            .clickable(enabled = !busy, onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (busy) {
            CircularProgressIndicator(
                color = Color.White,
                strokeWidth = 2.dp,
                modifier = Modifier.size(18.dp),
            )
        } else {
            Icon(
                imageVector = icon,
                contentDescription = label,
                tint = Color.White,
                modifier = Modifier.size(18.dp),
            )
        }
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall.copy(
                fontSize = 11.5.sp,
                fontWeight = FontWeight.SemiBold,
            ),
            color = Color.White,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

private fun SonataShareTarget.localizedEyebrowLabel(
    strings: dev.sonata.android.ui.i18n.SonataStrings,
): String = when (this) {
    is SonataShareTarget.Track -> strings.shareEyebrowSong
    is SonataShareTarget.Collection -> if (isAlbum) strings.shareEyebrowAlbum else strings.shareEyebrowPlaylist
    is SonataShareTarget.Artist -> strings.shareEyebrowArtist
}

private fun renderShareCardPngBitmap(
    target: SonataShareTarget,
    artwork: Bitmap?,
    palette: ShareCardVisualPalette,
    theme: LunariaThemeTokens,
    strings: dev.sonata.android.ui.i18n.SonataStrings = dev.sonata.android.ui.i18n.EnglishSonataStrings,
): Bitmap {
    val w = 1080
    val h = 1320
    val out = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(out)

    // 1. Background gradient
    val bgPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        shader = LinearGradient(
            0f,
            0f,
            0f,
            h.toFloat(),
            intArrayOf(
                palette.topColor.toArgb(),
                palette.midColor.toArgb(),
                palette.bottomColor.toArgb(),
            ),
            floatArrayOf(0f, 0.45f, 1f),
            Shader.TileMode.CLAMP,
        )
    }
    canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), bgPaint)

    // 2. Ambient radial blooms
    val glow1 = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        shader = RadialGradient(
            w * 0.5f,
            h * 0.35f,
            w * 0.65f,
            intArrayOf(palette.glowPrimary.copy(alpha = 0.58f).toArgb(), palette.glowPrimary.copy(alpha = 0f).toArgb()),
            null,
            Shader.TileMode.CLAMP,
        )
    }
    canvas.drawCircle(w * 0.5f, h * 0.35f, w * 0.65f, glow1)

    val glow2 = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        shader = RadialGradient(
            w * 0.85f,
            h * 0.78f,
            w * 0.50f,
            intArrayOf(palette.glowSecondary.copy(alpha = 0.40f).toArgb(), palette.glowSecondary.copy(alpha = 0f).toArgb()),
            null,
            Shader.TileMode.CLAMP,
        )
    }
    canvas.drawCircle(w * 0.85f, h * 0.78f, w * 0.50f, glow2)

    // Subtle theme ring accent (omitted in Basic monochrome)
    if (!theme.isBasic) {
        val ringPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = 2.5f
            color = Color.White.copy(alpha = if (theme.isMoonflower) 0.12f else 0.14f).toArgb()
        }
        canvas.drawCircle(w * 0.5f, h * 0.38f, w * 0.39f, ringPaint)
    }

    // 3. Eyebrow capsule pill
    val eyebrowText = target.localizedEyebrowLabel(strings)
    val chipTextPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.White.copy(alpha = 0.94f).toArgb()
        textSize = 27f
        letterSpacing = 0.12f
        typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
    }
    val chipWidth = chipTextPaint.measureText(eyebrowText) + 56f
    val chipHeight = 52f
    val chipRect = RectF(
        (w - chipWidth) / 2f,
        92f,
        (w + chipWidth) / 2f,
        92f + chipHeight,
    )
    canvas.drawRoundRect(
        chipRect,
        chipHeight / 2f,
        chipHeight / 2f,
        Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.White.copy(alpha = 0.16f).toArgb() },
    )
    canvas.drawRoundRect(
        chipRect,
        chipHeight / 2f,
        chipHeight / 2f,
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = 2f
            color = Color.White.copy(alpha = 0.28f).toArgb()
        },
    )
    val chipFm = chipTextPaint.fontMetrics
    canvas.drawText(
        eyebrowText,
        (w - chipTextPaint.measureText(eyebrowText)) / 2f,
        chipRect.centerY() - (chipFm.ascent + chipFm.descent) / 2f,
        chipTextPaint,
    )

    // 4. Center Artwork tailored to Track / Album / Playlist / Artist
    val artSize = 480f
    val centerY = 472f
    when (target) {
        is SonataShareTarget.Artist -> {
            val r = artSize / 2f
            val cx = w / 2f
            canvas.drawCircle(
                cx,
                centerY,
                r + 22f,
                Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    style = Paint.Style.STROKE
                    strokeWidth = 5f
                    color = palette.glowPrimary.copy(alpha = 0.72f).toArgb()
                },
            )
            drawBitmapClipped(canvas, artwork, RectF(cx - r, centerY - r, cx + r, centerY + r), r)
            canvas.drawCircle(
                cx,
                centerY,
                r,
                Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    style = Paint.Style.STROKE
                    strokeWidth = 4f
                    color = Color.White.copy(alpha = 0.44f).toArgb()
                },
            )
        }
        is SonataShareTarget.Collection -> {
            val coverCorner = when {
                theme.isMoonflower -> 54f
                theme.isBasic -> 34f
                else -> 40f
            }
            val leftOffset = if (target.isAlbum) -34f else 0f
            if (target.isAlbum) {
                val discR = artSize * 0.44f
                val discCx = w / 2f + 80f
                canvas.drawCircle(
                    discCx,
                    centerY,
                    discR,
                    Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color(0xFF100E16).toArgb() },
                )
                canvas.drawCircle(
                    discCx,
                    centerY,
                    discR * 0.34f,
                    Paint(Paint.ANTI_ALIAS_FLAG).apply { color = palette.glowPrimary.copy(alpha = 0.85f).toArgb() },
                )
            } else {
                val backSize = artSize * 0.88f
                val backRect = RectF(
                    w / 2f - backSize / 2f,
                    centerY - backSize / 2f - 24f,
                    w / 2f + backSize / 2f,
                    centerY + backSize / 2f - 24f,
                )
                canvas.drawRoundRect(
                    backRect,
                    coverCorner,
                    coverCorner,
                    Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.White.copy(alpha = 0.14f).toArgb() },
                )
            }
            val rect = RectF(
                w / 2f - artSize / 2f + leftOffset,
                centerY - artSize / 2f,
                w / 2f + artSize / 2f + leftOffset,
                centerY + artSize / 2f,
            )
            drawBitmapClipped(canvas, artwork, rect, coverCorner)
            canvas.drawRoundRect(
                rect,
                coverCorner,
                coverCorner,
                Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    style = Paint.Style.STROKE
                    strokeWidth = 3.5f
                    color = Color.White.copy(alpha = 0.42f).toArgb()
                },
            )
        }
        is SonataShareTarget.Track -> {
            val coverCorner = when {
                theme.isMoonflower -> 58f
                theme.isBasic -> 36f
                else -> 42f
            }
            val rect = RectF(
                w / 2f - artSize / 2f,
                centerY - artSize / 2f,
                w / 2f + artSize / 2f,
                centerY + artSize / 2f,
            )
            drawBitmapClipped(canvas, artwork, rect, coverCorner)
            canvas.drawRoundRect(
                rect,
                coverCorner,
                coverCorner,
                Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    style = Paint.Style.STROKE
                    strokeWidth = 3.5f
                    color = Color.White.copy(alpha = 0.42f).toArgb()
                },
            )
        }
    }

    // 5. Title & Subtitle text blocks
    val textWidth = (w - 150).coerceAtLeast(300)
    val titlePaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.White.toArgb()
        textSize = 62f
        typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
    }
    val titleLayout = StaticLayout.Builder
        .obtain(target.shareTitle, 0, target.shareTitle.length, titlePaint, textWidth)
        .setAlignment(Layout.Alignment.ALIGN_CENTER)
        .setMaxLines(2)
        .build()

    var cursorY = 795f
    canvas.save()
    canvas.translate((w - textWidth) / 2f, cursorY)
    titleLayout.draw(canvas)
    canvas.restore()
    cursorY += titleLayout.height + 20f

    if (target.shareSubtitle.isNotBlank()) {
        val subPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.White.copy(alpha = 0.80f).toArgb()
            textSize = 38f
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.NORMAL)
        }
        val subLayout = StaticLayout.Builder
            .obtain(target.shareSubtitle, 0, target.shareSubtitle.length, subPaint, textWidth)
            .setAlignment(Layout.Alignment.ALIGN_CENTER)
            .setMaxLines(2)
            .build()
        canvas.save()
        canvas.translate((w - textWidth) / 2f, cursorY)
        subLayout.draw(canvas)
        canvas.restore()
        cursorY += subLayout.height + 36f
    }

    // Track waveform / Album metadata footer
    if (target is SonataShareTarget.Track) {
        val bars = listOf(0.32f, 0.56f, 0.85f, 0.48f, 1.0f, 0.72f, 0.40f, 0.90f, 0.62f, 0.35f, 0.78f, 0.46f)
        val barWidth = 8f
        val barGap = 10f
        val totalBarW = bars.size * barWidth + (bars.size - 1) * barGap
        var bx = (w - totalBarW) / 2f
        val barY = (cursorY + 26f).coerceAtMost(h - 140f)
        bars.forEachIndexed { idx, amp ->
            val bh = 46f * amp
            val p = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = if (idx % 2 == 0) {
                    Color.White.copy(alpha = 0.82f).toArgb()
                } else {
                    palette.glowPrimary.copy(alpha = 0.94f).toArgb()
                }
            }
            canvas.drawRoundRect(
                RectF(bx, barY - bh / 2f, bx + barWidth, barY + bh / 2f),
                barWidth / 2f,
                barWidth / 2f,
                p,
            )
            bx += barWidth + barGap
        }
    } else if (target is SonataShareTarget.Collection && !target.localizedBadgeMeta(strings).isNullOrBlank()) {
        val metaText = target.localizedBadgeMeta(strings).orEmpty()
        val metaPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.White.copy(alpha = 0.86f).toArgb()
            textSize = 27f
            letterSpacing = 0.08f
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
        }
        val mw = metaPaint.measureText(metaText) + 52f
        val mh = 48f
        val my = (cursorY + 6f).coerceAtMost(h - 160f)
        canvas.drawRoundRect(
            RectF((w - mw) / 2f, my, (w + mw) / 2f, my + mh),
            mh / 2f,
            mh / 2f,
            Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.White.copy(alpha = 0.14f).toArgb() },
        )
        val fm = metaPaint.fontMetrics
        canvas.drawText(
            metaText,
            (w - metaPaint.measureText(metaText)) / 2f,
            my + mh / 2f - (fm.ascent + fm.descent) / 2f,
            metaPaint,
        )
    }

    // 6. Small, unobtrusive Sonata moon + text watermark bottom-right
    val markPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.White.copy(alpha = 0.58f).toArgb()
        textSize = 29f
        letterSpacing = 0.04f
        typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
    }
    val word = "Sonata"
    val wordW = markPaint.measureText(word)
    val wordX = w - 46f - wordW
    val wordY = h - 42f
    canvas.drawText(word, wordX, wordY, markPaint)

    // Tiny crescent moon beside "Sonata" matching ic_sonata_moon silhouette
    val moonSize = 26f
    val moonLeft = wordX - moonSize - 10f
    val moonCenterY = wordY - 10f
    val moonPath = AndroidPath().apply {
        val r = moonSize / 2f
        addCircle(moonLeft + r, moonCenterY, r, AndroidPath.Direction.CW)
        val cutout = AndroidPath().apply {
            addCircle(moonLeft + r * 1.42f, moonCenterY - r * 0.24f, r * 0.82f, AndroidPath.Direction.CW)
        }
        op(cutout, AndroidPath.Op.DIFFERENCE)
    }
    canvas.drawPath(
        moonPath,
        Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.White.copy(alpha = 0.58f).toArgb() },
    )

    return out
}

private fun drawBitmapClipped(
    canvas: Canvas,
    source: Bitmap?,
    dest: RectF,
    cornerRadius: Float,
) {
    if (source == null) {
        canvas.drawRoundRect(
            dest,
            cornerRadius,
            cornerRadius,
            Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.White.copy(alpha = 0.14f).toArgb() },
        )
        return
    }
    val scaled = centerCropBitmap(source, dest.width().toInt().coerceAtLeast(1), dest.height().toInt().coerceAtLeast(1))
    val paint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.FILTER_BITMAP_FLAG).apply {
        shader = BitmapShader(scaled, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP).apply {
            setLocalMatrix(android.graphics.Matrix().apply { setTranslate(dest.left, dest.top) })
        }
    }
    canvas.drawRoundRect(dest, cornerRadius, cornerRadius, paint)
}

private fun centerCropBitmap(src: Bitmap, targetW: Int, targetH: Int): Bitmap {
    val scale = maxOf(targetW.toFloat() / src.width, targetH.toFloat() / src.height)
    val scaledW = (src.width * scale).toInt().coerceAtLeast(1)
    val scaledH = (src.height * scale).toInt().coerceAtLeast(1)
    val resized = Bitmap.createScaledBitmap(src, scaledW, scaledH, true)
    val x = ((scaledW - targetW) / 2).coerceAtLeast(0)
    val y = ((scaledH - targetH) / 2).coerceAtLeast(0)
    return Bitmap.createBitmap(
        resized,
        x,
        y,
        targetW.coerceAtMost(resized.width - x),
        targetH.coerceAtMost(resized.height - y),
    )
}

private fun writeShareCachePng(context: Context, bitmap: Bitmap, title: String): Uri? = runCatching {
    val dir = File(context.cacheDir, "share_cards").apply { mkdirs() }
    val safeName = title.replace(Regex("[^A-Za-z0-9._-]"), "_").take(40).ifBlank { "sonata_share" }
    val outFile = File(dir, "Sonata_$safeName.png")
    FileOutputStream(outFile).use { fos ->
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, fos)
    }
    FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", outFile)
}.getOrNull()

private fun saveSharePngToGallery(context: Context, bitmap: Bitmap, title: String): Boolean = runCatching {
    val safeName = title.replace(Regex("[^A-Za-z0-9._-]"), "_").take(40).ifBlank { "card" }
    val displayName = "Sonata_${safeName}_${System.currentTimeMillis() / 1000L}.png"
    val resolver = context.contentResolver
    val values = ContentValues().apply {
        put(MediaStore.Images.Media.DISPLAY_NAME, displayName)
        put(MediaStore.Images.Media.MIME_TYPE, "image/png")
        put(MediaStore.Images.Media.RELATIVE_PATH, "${Environment.DIRECTORY_PICTURES}/Sonata")
        put(MediaStore.Images.Media.IS_PENDING, 1)
    }
    val uri = resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values) ?: return false
    resolver.openOutputStream(uri)?.use { out ->
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
    } ?: return false
    values.clear()
    values.put(MediaStore.Images.Media.IS_PENDING, 0)
    resolver.update(uri, values, null, null)
    true
}.getOrDefault(false)

private val LinkShareIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "SonataShareLink",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        path(
            fill = SolidColor(Color.Transparent),
            stroke = SolidColor(Color.White),
            strokeLineWidth = 2f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(10f, 13f)
            arcToRelative(5f, 5f, 0f, false, false, 7.54f, 0.54f)
            lineToRelative(3f, -3f)
            arcToRelative(5f, 5f, 0f, false, false, -7.07f, -7.07f)
            lineToRelative(-1.72f, 1.71f)
            moveTo(14f, 11f)
            arcToRelative(5f, 5f, 0f, false, false, -7.54f, -0.54f)
            lineToRelative(-3f, 3f)
            arcToRelative(5f, 5f, 0f, false, false, 7.07f, 7.07f)
            lineToRelative(1.71f, -1.71f)
        }
    }.build()
}
