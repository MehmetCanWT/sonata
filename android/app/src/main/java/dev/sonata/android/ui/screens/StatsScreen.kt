/*
 * On-device Listening Statistics screen for Sonata Android.
 *
 * Displays private SQLite WAL playback insights across Week, Month, Year, and All Time
 * windows: total plays, listening minutes, unique tracks/artists, listening streak,
 * top artists/tracks/albums, and per-source breakdown (YTM, LOC, JFN, SUB, SMB, DAV, JIO).
 */
package dev.sonata.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.data.AppLanguage
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.ecosystemStrings
import dev.sonata.android.ui.rememberSafeTopInset
import dev.sonata.android.ui.sonataThemeAccent
import dev.sonata.android.ui.theme.BasicPalette
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidStatsSummary
import uniffi.sonata_android.AndroidStatsWindow

@Composable
fun StatsScreen(
    window: AndroidStatsWindow,
    summary: AndroidStatsSummary?,
    loading: Boolean,
    appLanguage: AppLanguage,
    onSelectWindow: (AndroidStatsWindow) -> Unit,
    onRefresh: () -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val accent = sonataThemeAccent()
    val statusTop = rememberSafeTopInset()
    val eco = remember(appLanguage) { ecosystemStrings(appLanguage) }
    val pageBg = if (tokens.isDark) BasicPalette.DarkBackground else BasicPalette.LightBackground
    val cardBg = if (tokens.isDark) BasicPalette.DarkSurfaceVariant else BasicPalette.LightSurfaceVariant
    val dividerColor = if (tokens.isDark) BasicPalette.DarkOutline else BasicPalette.LightOutline
    val primaryText = if (tokens.isDark) Color.White else Color.Black
    val secondaryText = if (tokens.isDark) BasicPalette.DarkOnSurfaceVariant else BasicPalette.LightOnSurfaceVariant
    val cardShape = RoundedCornerShape(16.dp)

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(pageBg)
            .padding(top = statusTop)
            .navigationBarsPadding(),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 680.dp)
                .fillMaxSize(),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(
                    modifier = Modifier
                        .size(40.dp)
                        .clip(CircleShape)
                        .background(cardBg)
                        .clickable(onClick = onBack),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.ChevronLeft,
                        contentDescription = null,
                        tint = primaryText,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        text = eco.listeningStatsTitle,
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                        color = primaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = eco.listeningStatsSubtitle,
                        style = MaterialTheme.typography.labelSmall,
                        color = secondaryText,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Box(
                    modifier = Modifier
                        .size(38.dp)
                        .clip(CircleShape)
                        .background(cardBg)
                        .clickable(onClick = onRefresh),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = SonataIcons.History,
                        contentDescription = null,
                        tint = primaryText,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }

            // Window selector pills
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val windows = listOf(
                    AndroidStatsWindow.WEEK to eco.statsWindowWeek,
                    AndroidStatsWindow.MONTH to eco.statsWindowMonth,
                    AndroidStatsWindow.YEAR to eco.statsWindowYear,
                    AndroidStatsWindow.ALL_TIME to eco.statsWindowAllTime,
                )
                windows.forEach { (w, label) ->
                    val selected = window == w
                    Box(
                        modifier = Modifier
                            .clip(CircleShape)
                            .background(if (selected) accent else cardBg)
                            .border(0.75.dp, if (selected) accent else dividerColor, CircleShape)
                            .clickable { onSelectWindow(w) }
                            .padding(horizontal = 14.dp, vertical = 7.dp),
                    ) {
                        Text(
                            text = label,
                            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                            color = if (selected) {
                                if (tokens.isBasic && tokens.isDark) Color.Black else Color.White
                            } else {
                                primaryText
                            },
                        )
                    }
                }
            }

            if (loading && summary == null) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(color = accent)
                }
            } else {
                val report = summary ?: AndroidStatsSummary(
                    totalPlays = 0uL,
                    totalMinutes = 0uL,
                    uniqueTracks = 0uL,
                    uniqueArtists = 0uL,
                    streakDays = 0uL,
                    topArtists = emptyList(),
                    topTracks = emptyList(),
                    topAlbums = emptyList(),
                    sourceBreakdown = emptyList(),
                )
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    item(key = "stats_overview") {
                        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                StatMetricCard(
                                    label = eco.statsTotalPlays,
                                    value = report.totalPlays.toString(),
                                    cardBg = cardBg,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    modifier = Modifier.weight(1f),
                                )
                                StatMetricCard(
                                    label = eco.statsTotalMinutes,
                                    value = report.totalMinutes.toString(),
                                    cardBg = cardBg,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    modifier = Modifier.weight(1f),
                                )
                            }
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                StatMetricCard(
                                    label = eco.statsUniqueTracks,
                                    value = report.uniqueTracks.toString(),
                                    cardBg = cardBg,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    modifier = Modifier.weight(1f),
                                )
                                StatMetricCard(
                                    label = eco.statsUniqueArtists,
                                    value = report.uniqueArtists.toString(),
                                    cardBg = cardBg,
                                    primaryText = primaryText,
                                    secondaryText = secondaryText,
                                    modifier = Modifier.weight(1f),
                                )
                            }
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clip(cardShape)
                                    .background(cardBg)
                                    .padding(horizontal = 16.dp, vertical = 12.dp),
                            ) {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                ) {
                                    Text(
                                        text = eco.statsStreakDays(report.streakDays.toInt()),
                                        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                                        color = primaryText,
                                    )
                                    Icon(
                                        imageVector = SonataIcons.Sparkles,
                                        contentDescription = null,
                                        tint = accent,
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                        }
                    }

                    if (report.sourceBreakdown.isNotEmpty()) {
                        item(key = "stats_sources") {
                            val totalSourcePlays = report.sourceBreakdown.sumOf { it.plays }.coerceAtLeast(1uL)
                            StatsSectionCard(
                                title = eco.statsSourceBreakdown,
                                cardBg = cardBg,
                                primaryText = primaryText,
                            ) {
                                report.sourceBreakdown.forEachIndexed { idx, src ->
                                    if (idx > 0) {
                                        HorizontalDivider(thickness = 0.5.dp, color = dividerColor)
                                    }
                                    val fraction = (src.plays.toFloat() / totalSourcePlays.toFloat()).coerceIn(0f, 1f)
                                    Column(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .padding(vertical = 8.dp),
                                        verticalArrangement = Arrangement.spacedBy(5.dp),
                                    ) {
                                        Row(
                                            modifier = Modifier.fillMaxWidth(),
                                            horizontalArrangement = Arrangement.SpaceBetween,
                                            verticalAlignment = Alignment.CenterVertically,
                                        ) {
                                            Text(
                                                text = src.sourceTag,
                                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Bold),
                                                color = primaryText,
                                            )
                                            Text(
                                                text = "${src.plays} · ${src.minutes}m",
                                                style = MaterialTheme.typography.bodySmall,
                                                color = secondaryText,
                                            )
                                        }
                                        LinearProgressIndicator(
                                            progress = { fraction },
                                            color = accent,
                                            trackColor = dividerColor,
                                            modifier = Modifier
                                                .fillMaxWidth()
                                                .height(4.dp)
                                                .clip(CircleShape),
                                        )
                                    }
                                }
                            }
                        }
                    }

                    if (report.topArtists.isNotEmpty()) {
                        item(key = "stats_artists") {
                            StatsSectionCard(
                                title = eco.statsTopArtists,
                                cardBg = cardBg,
                                primaryText = primaryText,
                            ) {
                                report.topArtists.take(10).forEachIndexed { idx, item ->
                                    if (idx > 0) {
                                        HorizontalDivider(thickness = 0.5.dp, color = dividerColor)
                                    }
                                    StatsRankRow(
                                        rank = idx + 1,
                                        title = item.artist,
                                        subtitle = null,
                                        trailing = "${item.plays} · ${item.minutes}m",
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                    )
                                }
                            }
                        }
                    }

                    if (report.topTracks.isNotEmpty()) {
                        item(key = "stats_tracks") {
                            StatsSectionCard(
                                title = eco.statsTopTracks,
                                cardBg = cardBg,
                                primaryText = primaryText,
                            ) {
                                report.topTracks.take(10).forEachIndexed { idx, item ->
                                    if (idx > 0) {
                                        HorizontalDivider(thickness = 0.5.dp, color = dividerColor)
                                    }
                                    StatsRankRow(
                                        rank = idx + 1,
                                        title = item.title,
                                        subtitle = "${item.artist} · ${item.sourceTag}",
                                        trailing = "${item.plays} · ${item.minutes}m",
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                    )
                                }
                            }
                        }
                    }

                    if (report.topAlbums.isNotEmpty()) {
                        item(key = "stats_albums") {
                            StatsSectionCard(
                                title = eco.statsTopAlbums,
                                cardBg = cardBg,
                                primaryText = primaryText,
                            ) {
                                report.topAlbums.take(10).forEachIndexed { idx, item ->
                                    if (idx > 0) {
                                        HorizontalDivider(thickness = 0.5.dp, color = dividerColor)
                                    }
                                    StatsRankRow(
                                        rank = idx + 1,
                                        title = item.album,
                                        subtitle = item.artist,
                                        trailing = "${item.plays} · ${item.minutes}m",
                                        primaryText = primaryText,
                                        secondaryText = secondaryText,
                                    )
                                }
                            }
                        }
                    }

                    item(key = "stats_privacy_footer") {
                        Text(
                            text = eco.statsPrivacyNote,
                            style = MaterialTheme.typography.bodySmall,
                            color = secondaryText,
                            modifier = Modifier.padding(horizontal = 4.dp, vertical = 6.dp),
                        )
                        Spacer(Modifier.height(24.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun StatMetricCard(
    label: String,
    value: String,
    cardBg: Color,
    primaryText: Color,
    secondaryText: Color,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(16.dp))
            .background(cardBg)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = value,
            style = MaterialTheme.typography.headlineSmall.copy(
                fontWeight = FontWeight.Bold,
                fontSize = 24.sp,
            ),
            color = primaryText,
            maxLines = 1,
        )
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = secondaryText,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun StatsSectionCard(
    title: String,
    cardBg: Color,
    primaryText: Color,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(cardBg)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
            color = primaryText,
            modifier = Modifier.padding(bottom = 4.dp),
        )
        content()
    }
}

@Composable
private fun StatsRankRow(
    rank: Int,
    title: String,
    subtitle: String?,
    trailing: String,
    primaryText: Color,
    secondaryText: Color,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(
            text = "$rank.",
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
            color = secondaryText,
            modifier = Modifier.width(24.dp),
        )
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                color = primaryText,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (!subtitle.isNullOrBlank()) {
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.labelSmall,
                    color = secondaryText,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Text(
            text = trailing,
            style = MaterialTheme.typography.labelMedium,
            color = secondaryText,
        )
    }
}
