/*
 * Downloads & Offline Manager sheet/screen for Sonata Android.
 *
 * Displays offline storage usage, active downloads with live progress & cancel
 * controls, downloaded tracks list with offline playback & deletion, and quick
 * Wi-Fi / Export toggles.
 */
package dev.sonata.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.data.ActiveDownload
import dev.sonata.android.data.DownloadedTrackRecord
import dev.sonata.android.ui.CircleGlyph
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.SonataGlassCard
import dev.sonata.android.ui.theme.sonataGlassSurface

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DownloadsManagerSheet(
    downloads: List<DownloadedTrackRecord>,
    activeDownloads: Map<String, ActiveDownload>,
    totalBytes: Long,
    wifiOnly: Boolean,
    exportToMusic: Boolean,
    onToggleWifiOnly: () -> Unit,
    onToggleExportToMusic: () -> Unit,
    onPlayOfflineTrack: (DownloadedTrackRecord) -> Unit,
    onPlayAllOffline: () -> Unit,
    onCancelActiveDownload: (String) -> Unit,
    onRemoveDownload: (String) -> Unit,
    onClearAllDownloads: () -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val activeList = remember(activeDownloads) { activeDownloads.values.toList() }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = Color(0xFF0B0B0F),
        contentColor = Color.White,
        dragHandle = null,
    ) {
        LazyColumn(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(horizontal = PAGE_GUTTER),
            contentPadding = PaddingValues(top = 18.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            item(key = "dl_header") {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            text = "Downloads",
                            style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
                            color = Color.White,
                        )
                        Text(
                            text = "${downloads.size} tracks · ${formatBytes(totalBytes)} offline",
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.White.copy(alpha = 0.60f),
                        )
                    }
                    CircleGlyph(
                        icon = SonataIcons.Close,
                        contentDescription = "Close Downloads",
                        onClick = onDismiss,
                    )
                }
            }

            // Storage summary & quick toggles
            item(key = "dl_summary_card") {
                SonataGlassCard(modifier = Modifier.fillMaxWidth()) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            DownloadToggleChip(
                                label = if (wifiOnly) "Wi-Fi Only: On" else "Wi-Fi Only: Off",
                                active = wifiOnly,
                                onClick = onToggleWifiOnly,
                                modifier = Modifier.weight(1f),
                            )
                            DownloadToggleChip(
                                label = if (exportToMusic) "Music/Sonata" else "App Private",
                                active = exportToMusic,
                                onClick = onToggleExportToMusic,
                                modifier = Modifier.weight(1f),
                            )
                        }

                        if (downloads.isNotEmpty()) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                val pillShape = remember { RoundedCornerShape(percent = 50) }
                                Row(
                                    modifier = Modifier
                                        .weight(1f)
                                        .height(40.dp)
                                        .clip(pillShape)
                                        .background(SonataAccent)
                                        .clickable(onClick = onPlayAllOffline),
                                    horizontalArrangement = Arrangement.Center,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Icon(SonataIcons.Play, null, tint = Color.White, modifier = Modifier.size(15.dp))
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        text = "Play Offline",
                                        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
                                        color = Color.White,
                                    )
                                }
                                Box(
                                    modifier = Modifier
                                        .height(40.dp)
                                        .sonataGlassSurface(shape = pillShape)
                                        .clip(pillShape)
                                        .clickable(onClick = onClearAllDownloads)
                                        .padding(horizontal = 16.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = "Clear All",
                                        style = MaterialTheme.typography.labelMedium,
                                        color = Color.White.copy(alpha = 0.78f),
                                    )
                                }
                            }
                        }
                    }
                }
            }

            // Active downloads section
            if (activeList.isNotEmpty()) {
                item(key = "dl_active_header") {
                    Text(
                        text = "ACTIVE DOWNLOADS (${activeList.size})",
                        style = MaterialTheme.typography.labelSmall.copy(
                            letterSpacing = 0.8.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = Color.White.copy(alpha = 0.56f),
                    )
                }

                items(activeList, key = { "active_${it.videoId}" }) { item ->
                    SonataGlassCard(
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(16.dp),
                    ) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(12.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                Column(Modifier.weight(1f)) {
                                    Text(
                                        text = item.title,
                                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                                        color = Color.White,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                    Text(
                                        text = "${item.artist} · ${item.status}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = Color.White.copy(alpha = 0.6f),
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                                Box(
                                    modifier = Modifier
                                        .size(30.dp)
                                        .clip(CircleShape)
                                        .background(Color.White.copy(alpha = 0.1f))
                                        .clickable { onCancelActiveDownload(item.videoId) },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Icon(
                                        imageVector = SonataIcons.Close,
                                        contentDescription = "Cancel download",
                                        tint = Color.White,
                                        modifier = Modifier.size(14.dp),
                                    )
                                }
                            }
                            LinearProgressIndicator(
                                progress = { item.progress.coerceIn(0f, 1f) },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(4.dp)
                                    .clip(RoundedCornerShape(percent = 50)),
                                color = SonataAccent,
                                trackColor = Color.White.copy(alpha = 0.14f),
                            )
                        }
                    }
                }
            }

            // Downloaded tracks list
            item(key = "dl_saved_header") {
                Text(
                    text = "SAVED ON DEVICE",
                    style = MaterialTheme.typography.labelSmall.copy(
                        letterSpacing = 0.8.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    color = Color.White.copy(alpha = 0.56f),
                )
            }

            if (downloads.isEmpty()) {
                item(key = "dl_empty") {
                    SonataGlassCard(modifier = Modifier.fillMaxWidth()) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(28.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = SonataIcons.Download,
                                contentDescription = null,
                                tint = Color.White.copy(alpha = 0.4f),
                                modifier = Modifier.size(32.dp),
                            )
                            Text(
                                text = "No Offline Tracks Yet",
                                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                                color = Color.White,
                            )
                            Text(
                                text = "Tap Download on any song or album to listen without a connection.",
                                style = MaterialTheme.typography.bodySmall,
                                color = Color.White.copy(alpha = 0.58f),
                            )
                        }
                    }
                }
            } else {
                items(downloads, key = { it.videoId }) { rec ->
                    val rowShape = remember { RoundedCornerShape(16.dp) }
                    val artUrl = remember(rec.coverUrl) { rec.coverUrl.artworkAt(ROW_ART_PX) }
                    val artReq = rememberArtworkRequest(artUrl, ROW_ART_PX, ROW_ART_PX)

                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .sonataGlassSurface(shape = rowShape)
                            .clip(rowShape)
                            .clickable { onPlayOfflineTrack(rec) }
                            .padding(10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .background(Color.White.copy(alpha = 0.08f)),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (artReq != null) {
                                AsyncImage(
                                    model = artReq,
                                    contentDescription = rec.title,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(SonataIcons.MusicNote, null, tint = Color.White.copy(alpha = 0.5f))
                            }
                        }

                        Column(Modifier.weight(1f)) {
                            Text(
                                text = rec.title,
                                style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                                color = Color.White,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            val meta = buildString {
                                append(rec.artist)
                                append(" · ")
                                append(formatBytes(rec.fileSizeBytes))
                                if (!rec.codec.isNullOrBlank()) {
                                    append(" · ")
                                    append(rec.codec.uppercase())
                                }
                            }
                            Text(
                                text = meta,
                                style = MaterialTheme.typography.bodySmall,
                                color = Color.White.copy(alpha = 0.58f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }

                        Box(
                            modifier = Modifier
                                .size(32.dp)
                                .clip(CircleShape)
                                .background(Color.White.copy(alpha = 0.08f))
                                .clickable { onRemoveDownload(rec.videoId) },
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = SonataIcons.Close,
                                contentDescription = "Delete download",
                                tint = Color.White.copy(alpha = 0.72f),
                                modifier = Modifier.size(14.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DownloadToggleChip(
    label: String,
    active: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = remember { RoundedCornerShape(12.dp) }
    Box(
        modifier = modifier
            .height(36.dp)
            .clip(shape)
            .background(if (active) SonataAccent.copy(alpha = 0.24f) else Color.White.copy(alpha = 0.08f))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = if (active) FontWeight.Bold else FontWeight.Medium,
            ),
            color = if (active) Color.White else Color.White.copy(alpha = 0.68f),
            maxLines = 1,
        )
    }
}

private fun formatBytes(bytes: Long): String {
    if (bytes <= 0L) return "0 MB"
    val mb = bytes / (1024.0 * 1024.0)
    return if (mb >= 1024.0) {
        String.format("%.2f GB", mb / 1024.0)
    } else {
        String.format("%.1f MB", mb)
    }
}
