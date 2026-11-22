/*
 * Full-screen Add to Playlist picker & New Playlist dialog for Sonata Android.
 *
 * Matches Sonata's clean #000000 settings/picker aesthetic with Lunaria Lavender
 * (#B79CED) multi-select rings, live search filtering, and real YouTube Music
 * playlist mutation + offline local playlist fallback.
 */
package dev.sonata.android.ui.components

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.SonataViewModel
import dev.sonata.android.ui.PAGE_GUTTER
import dev.sonata.android.ui.ROW_ART_PX
import dev.sonata.android.ui.SearchField
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.artworkAt
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.theme.LocalLunariaColors
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidTrack

@Composable
fun AddToPlaylistSheet(
    visible: Boolean,
    targets: List<AndroidTrack>,
    playlists: List<AndroidPlaylist>,
    overrideTitle: String? = null,
    overrideSubtitle: String? = null,
    overrideCoverUrl: String? = null,
    onDismiss: () -> Unit,
    onConfirmAdd: (playlistIds: List<String>, tracks: List<AndroidTrack>) -> Unit,
    onCreatePlaylist: (title: String, tracks: List<AndroidTrack>) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (visible) {
        BackHandler(onBack = onDismiss)
    }

    AnimatedVisibility(
        visible = visible,
        enter = slideInVertically(tween(240)) { it / 6 } + fadeIn(tween(180)),
        exit = slideOutVertically(tween(200)) { it / 6 } + fadeOut(tween(150)),
        modifier = modifier.fillMaxSize(),
    ) {
        var selectedIds by remember(targets) { mutableStateOf(emptySet<String>()) }
        var searchQuery by remember(targets) { mutableStateOf("") }
        var showCreateDialog by remember { mutableStateOf(false) }

        val editablePlaylists = remember(playlists, searchQuery) {
            val base = playlists.filterNot { it.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID }
            if (searchQuery.isBlank()) {
                base
            } else {
                base.filter { it.name.contains(searchQuery.trim(), ignoreCase = true) }
            }
        }

        val headerTitle = remember(targets, overrideTitle) {
            overrideTitle?.takeIf { it.isNotBlank() }
                ?: targets.firstOrNull()?.title
                ?: "Selected Tracks"
        }
        val headerSubtitle = remember(targets, overrideSubtitle) {
            overrideSubtitle?.takeIf { it.isNotBlank() }
                ?: if (targets.size > 1) {
                    "${targets.size} tracks"
                } else {
                    targets.firstOrNull()?.artist.orEmpty()
                }
        }
        val headerArtwork = remember(targets, overrideCoverUrl) {
            (overrideCoverUrl?.takeIf { it.isNotBlank() } ?: targets.firstOrNull()?.thumbnailUrl)
                .artworkAt(ROW_ART_PX)
        }
        val headerRequest = rememberArtworkRequest(headerArtwork, ROW_ART_PX, ROW_ART_PX)

        val themeTokens = LocalLunariaColors.current
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(themeTokens.pageBackdropBrush)
                .statusBarsPadding()
                .navigationBarsPadding(),
        ) {
            // Top bar: Close (X) left, Confirm (Check) right
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onDismiss) {
                    Icon(
                        imageVector = SonataIcons.Close,
                        contentDescription = "Close",
                        tint = themeTokens.onSurfacePrimary,
                        modifier = Modifier.size(22.dp),
                    )
                }
                IconButton(
                    enabled = selectedIds.isNotEmpty(),
                    onClick = {
                        onConfirmAdd(selectedIds.toList(), targets)
                        onDismiss()
                    },
                ) {
                    Icon(
                        imageVector = SonataIcons.Check,
                        contentDescription = "Save to playlists",
                        tint = if (selectedIds.isNotEmpty()) themeTokens.petalLavender else themeTokens.onSurfaceSecondary.copy(alpha = 0.40f),
                        modifier = Modifier.size(24.dp),
                    )
                }
            }

            // Target preview header
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .clip(RoundedCornerShape(8.dp))
                        .background(themeTokens.eclipseCard)
                        .border(0.5.dp, themeTokens.cardStroke, RoundedCornerShape(8.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    if (headerRequest != null) {
                        AsyncImage(
                            model = headerRequest,
                            contentDescription = headerTitle,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.MusicNote,
                            contentDescription = null,
                            tint = themeTokens.onSurfaceSecondary,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = headerTitle,
                        style = MaterialTheme.typography.titleMedium.copy(
                            fontSize = 15.5.sp,
                            fontWeight = FontWeight.SemiBold,
                        ),
                        color = themeTokens.onSurfacePrimary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (headerSubtitle.isNotBlank()) {
                        Text(
                            text = headerSubtitle,
                            style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
                            color = themeTokens.onSurfaceSecondary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }

            HorizontalDivider(color = themeTokens.cardStroke)

            Text(
                text = "ADD TO PLAYLIST",
                style = MaterialTheme.typography.labelSmall.copy(
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    letterSpacing = 0.8.sp,
                ),
                color = themeTokens.onSurfaceSecondary,
                modifier = Modifier.padding(horizontal = PAGE_GUTTER, vertical = 12.dp),
            )

            // + New Playlist row
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { showCreateDialog = true }
                    .padding(horizontal = PAGE_GUTTER, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Box(
                    modifier = Modifier.size(36.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = PlaylistPlusIcon,
                        contentDescription = "New Playlist",
                        tint = themeTokens.petalLavender,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Text(
                    text = "New Playlist",
                    style = MaterialTheme.typography.titleMedium.copy(
                        fontSize = 16.sp,
                        fontWeight = FontWeight.Medium,
                    ),
                    color = themeTokens.onSurfacePrimary,
                )
            }

            // Search bar
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = PAGE_GUTTER, vertical = 8.dp),
            ) {
                SearchField(
                    query = searchQuery,
                    onQueryChange = { searchQuery = it },
                    onSubmit = {},
                    placeholder = "Find a playlist",
                )
            }

            // Playlists list
            LazyColumn(
                modifier = Modifier.weight(1f).fillMaxWidth(),
                contentPadding = PaddingValues(
                    start = PAGE_GUTTER,
                    end = PAGE_GUTTER,
                    top = 6.dp,
                    bottom = 24.dp,
                ),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                items(
                    items = editablePlaylists,
                    key = { it.id },
                    contentType = { "playlist_picker_row" },
                ) { playlist ->
                    val isSelected = playlist.id in selectedIds
                    val coverUrl = remember(playlist.thumbnailUrl) {
                        playlist.thumbnailUrl.artworkAt(ROW_ART_PX)
                    }
                    val coverReq = rememberArtworkRequest(coverUrl, ROW_ART_PX, ROW_ART_PX)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .clickable {
                                selectedIds = if (isSelected) {
                                    selectedIds - playlist.id
                                } else {
                                    selectedIds + playlist.id
                                }
                            }
                            .padding(vertical = 8.dp, horizontal = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(themeTokens.eclipseCard)
                                .border(0.5.dp, themeTokens.cardStroke, RoundedCornerShape(8.dp)),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (coverReq != null) {
                                AsyncImage(
                                    model = coverReq,
                                    contentDescription = playlist.name,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.MusicNote,
                                    contentDescription = null,
                                    tint = themeTokens.onSurfaceSecondary,
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        }

                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = playlist.name,
                                style = MaterialTheme.typography.titleMedium.copy(
                                    fontSize = 15.5.sp,
                                    fontWeight = FontWeight.Medium,
                                ),
                                color = themeTokens.onSurfacePrimary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            val subtitleText = if (playlist.id.startsWith("local:")) {
                                "Offline playlist"
                            } else {
                                "Playlist"
                            }
                            Text(
                                text = subtitleText,
                                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
                                color = themeTokens.onSurfaceSecondary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }

                        Box(
                            modifier = Modifier
                                .size(24.dp)
                                .clip(CircleShape)
                                .then(
                                    if (isSelected) {
                                        Modifier.background(themeTokens.petalLavender, CircleShape)
                                    } else {
                                        Modifier.border(1.5.dp, themeTokens.onSurfaceSecondary.copy(alpha = 0.45f), CircleShape)
                                    },
                                ),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (isSelected) {
                                Icon(
                                    imageVector = SonataIcons.Check,
                                    contentDescription = "Selected",
                                    tint = Color.White,
                                    modifier = Modifier.size(15.dp),
                                )
                            }
                        }
                    }
                }
            }
        }

        if (showCreateDialog) {
            var newTitle by remember { mutableStateOf("") }
            AlertDialog(
                onDismissRequest = { showCreateDialog = false },
                containerColor = themeTokens.eclipseSurface,
                titleContentColor = themeTokens.onSurfacePrimary,
                textContentColor = themeTokens.onSurfaceSecondary,
                title = {
                    Text(
                        text = "New Playlist",
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                    )
                },
                text = {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(themeTokens.eclipseCard)
                            .border(0.5.dp, themeTokens.cardStroke, RoundedCornerShape(12.dp))
                            .padding(horizontal = 14.dp, vertical = 12.dp),
                    ) {
                        if (newTitle.isEmpty()) {
                            Text(
                                text = "Playlist title",
                                style = MaterialTheme.typography.bodyMedium,
                                color = themeTokens.onSurfaceSecondary.copy(alpha = 0.65f),
                            )
                        }
                        BasicTextField(
                            value = newTitle,
                            onValueChange = { newTitle = it },
                            singleLine = true,
                            textStyle = MaterialTheme.typography.bodyMedium.copy(color = themeTokens.onSurfacePrimary),
                            cursorBrush = SolidColor(themeTokens.petalLavender),
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                },
                confirmButton = {
                    TextButton(
                        enabled = newTitle.isNotBlank(),
                        onClick = {
                            val trimmed = newTitle.trim()
                            showCreateDialog = false
                            if (trimmed.isNotEmpty()) {
                                onCreatePlaylist(trimmed, targets)
                                onDismiss()
                            }
                        },
                    ) {
                        Text(
                            text = "Create",
                            color = if (newTitle.isNotBlank()) themeTokens.petalLavender else themeTokens.onSurfaceSecondary.copy(alpha = 0.45f),
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showCreateDialog = false }) {
                        Text(text = "Cancel", color = themeTokens.onSurfaceSecondary)
                    }
                },
            )
        }
    }
}

private val PlaylistPlusIcon: ImageVector by lazy(LazyThreadSafetyMode.NONE) {
    ImageVector.Builder(
        name = "PlaylistPlus",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
        path(
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 2.1f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        ) {
            moveTo(12f, 5f)
            lineTo(12f, 19f)
            moveTo(5f, 12f)
            lineTo(19f, 12f)
        }
    }.build()
}
