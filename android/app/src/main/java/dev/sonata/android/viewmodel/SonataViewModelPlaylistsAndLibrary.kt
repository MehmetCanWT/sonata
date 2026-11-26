/*
 * Personal/local playlists, pinned & saved collections, listening history,
 * artist enrichment, collection/album/mood navigation, and queue mutations for SonataViewModel.
 */
package dev.sonata.android.viewmodel

import android.os.SystemClock
import android.util.Log
import androidx.lifecycle.viewModelScope
import dev.sonata.android.PinnedCollectionEntry
import dev.sonata.android.PlaybackService
import dev.sonata.android.SavedExternalPlaylist
import dev.sonata.android.SonataViewModel
import dev.sonata.android.data.SmbClient
import dev.sonata.android.data.SonataEngineGateway
import dev.sonata.android.data.isLocalTrackId
import dev.sonata.android.data.isPersonalTrackId
import dev.sonata.android.data.resolveYtmFallbackCoverUrl
import dev.sonata.android.data.runSuspendCatching
import dev.sonata.android.isSelfHostedId
import dev.sonata.android.ui.components.PERSONAL_PLAYLIST_ID_PREFIX
import dev.sonata.android.ui.components.isEditableUserPlaylist
import dev.sonata.android.ui.components.normalizeCollectionId
import dev.sonata.android.ui.i18n.EnglishSonataStrings
import dev.sonata.android.ui.i18n.localizeMoodTitle
import dev.sonata.android.ui.i18n.sourcesStrings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import uniffi.sonata_android.AndroidArtistPage
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidHomeShelf
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidTrack
import kotlin.random.Random

internal fun trackToJson(t: AndroidTrack): JSONObject = JSONObject().apply {
    put("id", t.id)
    put("title", t.title)
    put("artist", t.artist)
    put("album", t.album)
    put("durationSecs", t.durationSecs?.toLong() ?: 0L)
    put("thumbnailUrl", t.thumbnailUrl ?: "")
    put("liked", t.liked)
    put("albumId", t.albumId ?: "")
    put("isExplicit", t.isExplicit)
}

internal fun jsonToTrack(obj: JSONObject?): AndroidTrack? {
    obj ?: return null
    val id = obj.optString("id").trim()
    val title = obj.optString("title").trim()
    if (id.isBlank() || title.isBlank()) return null
    return AndroidTrack(
        id = id,
        title = title,
        artist = obj.optString("artist"),
        album = obj.optString("album"),
        durationSecs = obj.optLong("durationSecs", 0L).takeIf { it > 0L }?.toULong(),
        thumbnailUrl = obj.optString("thumbnailUrl").takeIf { it.isNotBlank() },
        liked = obj.optBoolean("liked", false),
        albumId = obj.optString("albumId").takeIf { it.isNotBlank() },
        isExplicit = obj.optBoolean("isExplicit", false),
    )
}

internal fun SonataViewModel.loadLocalPlaylistsLockedImpl(): MutableList<SonataViewModel.LocalPlaylistEntry> {
    val raw = localPlaylistsPrefs.getString(SonataViewModel.LOCAL_PLAYLISTS_KEY, null) ?: return mutableListOf()
    return runCatching {
        val arr = JSONArray(raw)
        val out = mutableListOf<SonataViewModel.LocalPlaylistEntry>()
        for (i in 0 until arr.length()) {
            val obj = arr.optJSONObject(i) ?: continue
            val id = obj.optString("id").trim()
            val name = obj.optString("name").trim()
            if (id.isBlank() || name.isBlank()) continue
            val trackArr = obj.optJSONArray("tracks")
            val tracks = buildList {
                if (trackArr != null) {
                    for (j in 0 until trackArr.length()) {
                        jsonToTrack(trackArr.optJSONObject(j))?.let { add(it) }
                    }
                }
            }
            out.add(SonataViewModel.LocalPlaylistEntry(id = id, name = name, tracks = tracks))
        }
        out
    }.getOrDefault(mutableListOf())
}

internal fun SonataViewModel.saveLocalPlaylistsLockedImpl(entries: List<SonataViewModel.LocalPlaylistEntry>) {
    val arr = JSONArray()
    entries.forEach { entry ->
        arr.put(
            JSONObject().apply {
                put("id", entry.id)
                put("name", entry.name)
                put(
                    "tracks",
                    JSONArray().apply {
                        entry.tracks.forEach { put(trackToJson(it)) }
                    },
                )
            },
        )
    }
    localPlaylistsPrefs.edit().putString(SonataViewModel.LOCAL_PLAYLISTS_KEY, arr.toString()).apply()
}

internal fun SonataViewModel.loadLocalPlaylistsAsAndroidImpl(): List<AndroidPlaylist> = synchronized(localPlaylistsLock) {
    loadLocalPlaylistsLockedImpl().map { entry ->
        AndroidPlaylist(
            id = entry.id,
            name = entry.name,
            thumbnailUrl = entry.tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
        )
    }
}

internal fun SonataViewModel.getLocalPlaylistTracksImpl(playlistId: String): List<AndroidTrack> = synchronized(localPlaylistsLock) {
    loadLocalPlaylistsLockedImpl()
        .firstOrNull { it.id == playlistId }
        ?.tracks
        .orEmpty()
}

internal fun SonataViewModel.appendTracksToLocalPlaylistImpl(playlistId: String, tracks: List<AndroidTrack>): Boolean =
    synchronized(localPlaylistsLock) {
        val entries = loadLocalPlaylistsLockedImpl()
        val idx = entries.indexOfFirst { it.id == playlistId }
        if (idx < 0) return false
        val existing = entries[idx]
        val merged = (existing.tracks + tracks).distinctBy { it.id }
        entries[idx] = existing.copy(tracks = merged)
        saveLocalPlaylistsLockedImpl(entries)
        true
    }

internal fun SonataViewModel.isLikedMusicPlaylistIdImpl(id: String): Boolean =
    id.equals("VLLM", ignoreCase = true) || id.equals("LM", ignoreCase = true)

internal fun SonataViewModel.resolveTrackCoverUrlImpl(track: AndroidTrack): String? =
    track.thumbnailUrl?.takeIf { it.isNotBlank() }
        ?: resolveYtmFallbackCoverUrl(
            trackId = track.id,
            personalOnlyMode = preferences.state.value.personalOnlyMode,
        )

internal fun SonataViewModel.applyLatestLikedCoverImpl(playlists: List<AndroidPlaylist>): List<AndroidPlaylist> {
    val likedCover = latestLikedCoverUrl?.takeIf { it.isNotBlank() } ?: return playlists
    return playlists.map { pl ->
        if (isLikedMusicPlaylistIdImpl(pl.id)) pl.copy(thumbnailUrl = likedCover) else pl
    }
}

internal fun SonataViewModel.updateLatestLikedCoverImpl(coverUrl: String?, clearIfNull: Boolean = false) {
    val normalized = coverUrl?.takeIf { it.isNotBlank() }
    if (normalized != null) {
        latestLikedCoverUrl = normalized
        prefs.edit().putString("last_liked_cover_url", normalized).apply()
    } else if (clearIfNull) {
        latestLikedCoverUrl = null
        prefs.edit().remove("last_liked_cover_url").apply()
    } else {
        return
    }
    _uiState.update { state ->
        val updatedPlaylists = state.playlists.map { pl ->
            if (isLikedMusicPlaylistIdImpl(pl.id)) pl.copy(thumbnailUrl = normalized) else pl
        }
        val viewingLiked = state.collectionBrowseId?.let { isLikedMusicPlaylistIdImpl(it) } == true
        val updatedCollectionCover = if (viewingLiked) normalized else state.collectionCoverUrl
        if (updatedPlaylists == state.playlists && updatedCollectionCover == state.collectionCoverUrl) {
            state
        } else {
            state.copy(
                playlists = updatedPlaylists,
                collectionCoverUrl = updatedCollectionCover,
            )
        }
    }
    if (normalized != null) {
        syncCollectionMetadataToSavedAndPinnedImpl("VLLM", uiStrings().likedSongs, null, normalized)
    }
}

internal fun SonataViewModel.computeOwnedPlaylistIdsImpl(
    remote: List<AndroidPlaylist>,
    savedExternalIds: Set<String>,
): Set<String> {
    val owned = LinkedHashSet<String>()
    remote.forEach { pl ->
        if (pl.isOwned) {
            val norm = normalizeCollectionId(pl.id)
            if (norm.startsWith("PL") && norm !in savedExternalIds) {
                owned.add(norm)
            }
        }
    }
    return owned
}

internal fun SonataViewModel.mergePlaylistsWithLocalImpl(
    remote: List<AndroidPlaylist>,
    personalList: List<uniffi.sonata_android.AndroidPersonalPlaylist> = personalPlaylistsCache,
): List<AndroidPlaylist> {
    val personalAsAndroid = personalList.map { p ->
        AndroidPlaylist(
            id = p.id,
            name = p.name,
            thumbnailUrl = p.tracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
            isOwned = true,
        )
    }
    val local = loadLocalPlaylistsAsAndroidImpl()
    if (preferences.state.value.personalOnlyMode) {
        val combined = personalAsAndroid + local
        if (combined.isEmpty()) return emptyList()
        val seen = HashSet<String>(combined.size)
        return combined.filter { pl ->
            val key = normalizeCollectionId(pl.id).ifBlank { pl.id }
            seen.add(key)
        }
    }
    val decoratedRemote = applyLatestLikedCoverImpl(remote)
    val savedExternal = synchronized(pinnedSavedLock) { cachedSavedExternalPlaylists }
        .map { saved ->
            AndroidPlaylist(
                id = saved.id,
                name = saved.title,
                thumbnailUrl = saved.coverUrl,
                isOwned = false,
            )
        }
    val combined = decoratedRemote + savedExternal + personalAsAndroid + local
    if (combined.isEmpty()) return emptyList()
    val seen = HashSet<String>(combined.size)
    return combined.filter { pl ->
        val key = normalizeCollectionId(pl.id).ifBlank { pl.id }
        seen.add(key)
    }
}

internal fun SonataViewModel.loadPinnedCollectionsLockedImpl(): List<PinnedCollectionEntry> {
    val raw = prefs.getString("pinned_collections_v1", null) ?: return emptyList()
    return runCatching {
        val arr = JSONArray(raw)
        buildList {
            for (i in 0 until arr.length()) {
                val obj = arr.optJSONObject(i) ?: continue
                val id = obj.optString("id").trim()
                val title = obj.optString("title").trim()
                if (id.isBlank() || title.isBlank()) continue
                add(
                    PinnedCollectionEntry(
                        id = id,
                        title = title,
                        subtitle = obj.optString("subtitle"),
                        coverUrl = obj.optString("coverUrl").takeIf { it.isNotBlank() },
                        kindLabel = obj.optString("kindLabel", "Playlist").ifBlank { "Playlist" },
                        isAlbum = obj.optBoolean("isAlbum", false),
                        pinnedAtMs = obj.optLong("pinnedAtMs", 0L),
                    ),
                )
            }
        }
    }.getOrDefault(emptyList())
}

internal fun SonataViewModel.savePinnedCollectionsLockedImpl(entries: List<PinnedCollectionEntry>) {
    val arr = JSONArray()
    entries.forEach { entry ->
        arr.put(
            JSONObject().apply {
                put("id", entry.id)
                put("title", entry.title)
                put("subtitle", entry.subtitle)
                put("coverUrl", entry.coverUrl ?: "")
                put("kindLabel", entry.kindLabel)
                put("isAlbum", entry.isAlbum)
                put("pinnedAtMs", entry.pinnedAtMs)
            },
        )
    }
    prefs.edit().putString("pinned_collections_v1", arr.toString()).apply()
}

internal fun SonataViewModel.loadSavedExternalPlaylistsLockedImpl(): List<SavedExternalPlaylist> {
    val raw = prefs.getString("saved_external_playlists_v1", null) ?: return emptyList()
    return runCatching {
        val arr = JSONArray(raw)
        buildList {
            for (i in 0 until arr.length()) {
                val obj = arr.optJSONObject(i) ?: continue
                val id = obj.optString("id").trim()
                val title = obj.optString("title").trim()
                if (id.isBlank() || title.isBlank()) continue
                add(
                    SavedExternalPlaylist(
                        id = id,
                        title = title,
                        subtitle = obj.optString("subtitle"),
                        coverUrl = obj.optString("coverUrl").takeIf { it.isNotBlank() },
                        kindLabel = obj.optString("kindLabel", "Playlist").ifBlank { "Playlist" },
                        savedAtMs = obj.optLong("savedAtMs", 0L),
                    ),
                )
            }
        }
    }.getOrDefault(emptyList())
}

internal fun SonataViewModel.saveExternalPlaylistsLockedImpl(entries: List<SavedExternalPlaylist>) {
    val arr = JSONArray()
    entries.forEach { entry ->
        arr.put(
            JSONObject().apply {
                put("id", entry.id)
                put("title", entry.title)
                put("subtitle", entry.subtitle)
                put("coverUrl", entry.coverUrl ?: "")
                put("kindLabel", entry.kindLabel)
                put("savedAtMs", entry.savedAtMs)
            },
        )
    }
    prefs.edit().putString("saved_external_playlists_v1", arr.toString()).apply()
}

internal fun SonataViewModel.initPinnedAndSavedFromPrefsImpl() {
    val (pinned, saved) = synchronized(pinnedSavedLock) {
        cachedPinnedCollections = loadPinnedCollectionsLockedImpl()
        cachedSavedExternalPlaylists = loadSavedExternalPlaylistsLockedImpl()
        cachedPinnedCollections to cachedSavedExternalPlaylists
    }
    val savedIds = saved.mapTo(HashSet()) { normalizeCollectionId(it.id) }
    _uiState.update { state ->
        state.copy(
            pinnedCollections = pinned,
            savedExternalPlaylistIds = savedIds,
        )
    }
}

internal fun SonataViewModel.syncCollectionMetadataToSavedAndPinnedImpl(
    id: String,
    title: String,
    subtitle: String?,
    coverUrl: String?,
) {
    val norm = normalizeCollectionId(id)
    if (norm.isBlank()) return
    val cleanTitle = title.trim()
    val cleanCover = coverUrl?.takeIf { it.isNotBlank() }
    val cleanSubtitle = subtitle?.trim()
    var pinnedUpdated: List<PinnedCollectionEntry>? = null
    var savedUpdated: List<SavedExternalPlaylist>? = null
    synchronized(pinnedSavedLock) {
        val nextPinned = cachedPinnedCollections.map { entry ->
            if (normalizeCollectionId(entry.id) == norm) {
                entry.copy(
                    title = cleanTitle.ifBlank { entry.title },
                    subtitle = cleanSubtitle?.takeIf { it.isNotBlank() } ?: entry.subtitle,
                    coverUrl = cleanCover ?: entry.coverUrl,
                )
            } else {
                entry
            }
        }
        if (nextPinned != cachedPinnedCollections) {
            cachedPinnedCollections = nextPinned
            savePinnedCollectionsLockedImpl(nextPinned)
            pinnedUpdated = nextPinned
        }

        val nextSaved = cachedSavedExternalPlaylists.map { entry ->
            if (normalizeCollectionId(entry.id) == norm) {
                entry.copy(
                    title = cleanTitle.ifBlank { entry.title },
                    subtitle = cleanSubtitle?.takeIf { it.isNotBlank() } ?: entry.subtitle,
                    coverUrl = cleanCover ?: entry.coverUrl,
                )
            } else {
                entry
            }
        }
        if (nextSaved != cachedSavedExternalPlaylists) {
            cachedSavedExternalPlaylists = nextSaved
            saveExternalPlaylistsLockedImpl(nextSaved)
            savedUpdated = nextSaved
        }
    }
    if (pinnedUpdated != null || savedUpdated != null) {
        _uiState.update { state ->
            val updatedPlaylists = if (savedUpdated != null) {
                state.playlists.map { pl ->
                    if (normalizeCollectionId(pl.id) == norm) {
                        pl.copy(
                            name = cleanTitle.ifBlank { pl.name },
                            thumbnailUrl = cleanCover ?: pl.thumbnailUrl,
                        )
                    } else {
                        pl
                    }
                }
            } else {
                state.playlists
            }
            state.copy(
                pinnedCollections = pinnedUpdated ?: state.pinnedCollections,
                playlists = updatedPlaylists,
            )
        }
    }
}

internal fun SonataViewModel.getCachedArtistPageImpl(key: String): AndroidArtistPage? {
    val norm = key.trim().lowercase()
    if (norm.isBlank()) return null
    val entry = artistPageCache[norm] ?: return null
    if (SystemClock.elapsedRealtime() - entry.cachedAtMs >= SonataViewModel.ARTIST_CACHE_TTL_MS) {
        artistPageCache.remove(norm, entry)
        return null
    }
    return entry.page
}

internal fun SonataViewModel.putCachedArtistPageImpl(page: AndroidArtistPage, queryName: String = "") {
    val now = SystemClock.elapsedRealtime()
    val entry = SonataViewModel.CachedArtistPage(page = page, cachedAtMs = now)
    if (page.id.isNotBlank()) {
        artistPageCache[page.id.trim().lowercase()] = entry
    }
    if (page.name.isNotBlank()) {
        artistPageCache["name:${page.name.trim().lowercase()}"] = entry
    }
    if (queryName.isNotBlank()) {
        artistPageCache["name:${queryName.trim().lowercase()}"] = entry
    }
    if (artistPageCache.size > SonataViewModel.ARTIST_CACHE_MAX_ENTRIES * 2) {
        artistPageCache.entries
            .sortedBy { it.value.cachedAtMs }
            .take(artistPageCache.size - SonataViewModel.ARTIST_CACHE_MAX_ENTRIES * 2)
            .forEach { artistPageCache.remove(it.key, it.value) }
    }
    val primaryKey = page.name.trim().lowercase()
    if (primaryKey.isNotBlank() && page.id.startsWith("UC")) {
        val card = AndroidHomeCard(
            kind = AndroidHomeCardKind.ARTIST,
            id = page.id,
            title = page.name,
            subtitle = page.monthlyListeners ?: page.subscribers ?: uiStrings().artistDefault,
            thumbnailUrl = page.thumbnailUrl,
            track = null,
        )
        resolvedArtistCardCache[primaryKey] = card
        queryName.trim().lowercase().takeIf { it.isNotBlank() }?.let {
            resolvedArtistCardCache.putIfAbsent(it, card)
        }
    }
}

internal fun SonataViewModel.seedArtistCardsFromShelvesImpl(shelves: List<AndroidHomeShelf>) {
    shelves.asSequence()
        .flatMap { it.cards.asSequence() }
        .filter { it.kind == AndroidHomeCardKind.ARTIST && it.title.isNotBlank() }
        .forEach { card ->
            val key = card.title.trim().lowercase()
            val existing = resolvedArtistCardCache[key]
            if (existing == null || (!existing.id.startsWith("UC") && card.id.startsWith("UC"))) {
                resolvedArtistCardCache[key] = card
            }
        }
}

internal fun SonataViewModel.buildRecentArtistCardsImpl(
    historyTracks: List<AndroidTrack>,
    shelves: List<AndroidHomeShelf>,
): List<AndroidHomeCard> {
    seedArtistCardsFromShelvesImpl(shelves)
    val merged = linkedMapOf<String, AndroidHomeCard>()
    for (track in historyTracks) {
        val primary = SonataViewModel.extractPrimaryArtistName(track.artist)
        val key = primary.lowercase()
        if (key.isBlank() || merged.containsKey(key)) continue
        val cached = resolvedArtistCardCache[key]
        merged[key] = if (cached != null) {
            if (cached.thumbnailUrl.isNullOrBlank() && !track.thumbnailUrl.isNullOrBlank()) {
                cached.copy(thumbnailUrl = track.thumbnailUrl)
            } else {
                cached
            }
        } else {
            AndroidHomeCard(
                kind = AndroidHomeCardKind.ARTIST,
                id = "",
                title = primary,
                subtitle = uiStrings().recentlyPlayedSubtitle,
                thumbnailUrl = track.thumbnailUrl,
                track = null,
            )
        }
    }
    shelves.asSequence()
        .flatMap { it.cards.asSequence() }
        .filter { it.kind == AndroidHomeCardKind.ARTIST && it.title.isNotBlank() }
        .forEach { card ->
            val key = card.title.trim().lowercase()
            if (!merged.containsKey(key)) {
                merged[key] = card
            }
        }
    return merged.values.take(16)
}

internal fun SonataViewModel.ensureArtistCardEnrichedImpl(rawArtist: String) {
    if (preferences.state.value.personalOnlyMode) return
    val primary = SonataViewModel.extractPrimaryArtistName(rawArtist)
    val key = primary.lowercase()
    if (key.isBlank()) return
    val existing = resolvedArtistCardCache[key]
    if (existing != null && existing.id.startsWith("UC") && !existing.thumbnailUrl.isNullOrBlank()) {
        return
    }
    if (!inFlightArtistLookups.add(key)) return
    viewModelScope.launch(Dispatchers.IO) {
        try {
            val engine = requireEngine()
            val page = getCachedArtistPageImpl("name:$key")
                ?: runSuspendCatching { engine.fetchArtistByName(primary) }.getOrNull()
                    ?.also { putCachedArtistPageImpl(it, primary) }
            if (page != null && page.id.startsWith("UC")) {
                val refreshed = buildRecentArtistCardsImpl(_lastPlayedTracks.value, _uiState.value.shelves)
                _uiState.update { it.copy(recentArtistCards = refreshed) }
            }
        } finally {
            inFlightArtistLookups.remove(key)
        }
    }
}

internal fun SonataViewModel.activeHistoryPrefKey(signedInOverride: Boolean? = null): String {
    val isSignedIn = signedInOverride ?: _uiState.value.signedIn
    return if (isSignedIn) SonataViewModel.ACCOUNT_LAST_PLAYED_KEY else SonataViewModel.GUEST_LAST_PLAYED_KEY
}

internal fun SonataViewModel.loadLocalLastPlayedHistoryImpl(signedInOverride: Boolean? = null): List<AndroidTrack> = synchronized(historyLock) {
    val key = activeHistoryPrefKey(signedInOverride)
    if (localHistoryCacheKey == key) {
        localHistoryCache?.let { return it }
    }
    val raw = historyPrefs.getString(key, null)
    val loaded = if (raw == null) {
        emptyList()
    } else {
        runCatching {
            val arr = JSONArray(raw)
            val parsed = buildList {
                for (i in 0 until arr.length()) {
                    val obj = arr.optJSONObject(i) ?: continue
                    val id = obj.optString("id").trim()
                    val title = obj.optString("title").trim()
                    if (id.isBlank() || title.isBlank()) continue
                    add(
                        AndroidTrack(
                            id = id,
                            title = title,
                            artist = obj.optString("artist"),
                            album = obj.optString("album"),
                            durationSecs = obj.optLong("durationSecs", 0L).takeIf { it > 0L }?.toULong(),
                            thumbnailUrl = obj.optString("thumbnailUrl").takeIf { it.isNotBlank() },
                            liked = obj.optBoolean("liked", false),
                            albumId = obj.optString("albumId").takeIf { it.isNotBlank() },
                            isExplicit = obj.optBoolean("isExplicit", false),
                        )
                    )
                }
            }
            SonataViewModel.deduplicateTracks(parsed).take(SonataViewModel.MAX_LAST_PLAYED_HISTORY)
        }.getOrDefault(emptyList())
    }
    localHistoryCacheKey = key
    localHistoryCache = loaded
    loaded
}

internal fun SonataViewModel.saveLocalLastPlayedHistoryImpl(
    tracks: List<AndroidTrack>,
    signedInOverride: Boolean? = null,
) = synchronized(historyLock) {
    val key = activeHistoryPrefKey(signedInOverride)
    val clean = SonataViewModel.deduplicateTracks(tracks).take(SonataViewModel.MAX_LAST_PLAYED_HISTORY)
    localHistoryCacheKey = key
    localHistoryCache = clean
    val arr = JSONArray()
    clean.forEach { t ->
        arr.put(
            JSONObject().apply {
                put("id", t.id)
                put("title", t.title)
                put("artist", t.artist)
                put("album", t.album)
                put("durationSecs", t.durationSecs?.toLong() ?: 0L)
                put("thumbnailUrl", t.thumbnailUrl ?: "")
                put("liked", t.liked)
                put("albumId", t.albumId ?: "")
                put("isExplicit", t.isExplicit)
            }
        )
    }
    historyPrefs.edit().putString(key, arr.toString()).apply()
}

internal fun SonataViewModel.clearGuestLastPlayedHistoryImpl() = synchronized(historyLock) {
    if (localHistoryCacheKey != SonataViewModel.ACCOUNT_LAST_PLAYED_KEY) {
        localHistoryCacheKey = null
        localHistoryCache = null
    }
    historyPrefs.edit()
        .remove(SonataViewModel.GUEST_LAST_PLAYED_KEY)
        .remove(SonataViewModel.LAST_PLAYED_KEY)
        .apply()
}

internal fun SonataViewModel.clearAccountLastPlayedHistoryImpl() = synchronized(historyLock) {
    localHistoryCacheKey = null
    localHistoryCache = null
    historyPrefs.edit()
        .remove(SonataViewModel.ACCOUNT_LAST_PLAYED_KEY)
        .remove(SonataViewModel.DISMISSED_LAST_PLAYED_KEY)
        .remove(SonataViewModel.LAST_PLAYED_KEY)
        .apply()
}

private data class DismissedHistoryEntry(
    val id: String,
    val canonicalKey: String,
)

private fun parseDismissedEntries(raw: String?): List<DismissedHistoryEntry> {
    if (raw.isNullOrBlank()) return emptyList()
    return runCatching {
        val arr = JSONArray(raw)
        buildList {
            for (i in 0 until arr.length()) {
                val obj = arr.optJSONObject(i) ?: continue
                val id = obj.optString("id").trim()
                val key = obj.optString("key").trim()
                if (id.isNotBlank() || key.isNotBlank()) {
                    add(DismissedHistoryEntry(id = id, canonicalKey = key))
                }
            }
        }.takeLast(SonataViewModel.MAX_DISMISSED_LAST_PLAYED)
    }.getOrDefault(emptyList())
}

private fun encodeDismissedEntries(entries: List<DismissedHistoryEntry>): String {
    val arr = JSONArray()
    entries.takeLast(SonataViewModel.MAX_DISMISSED_LAST_PLAYED).forEach { entry ->
        arr.put(
            JSONObject().apply {
                put("id", entry.id)
                put("key", entry.canonicalKey)
            },
        )
    }
    return arr.toString()
}

internal fun isDismissedHistoryTrack(
    track: AndroidTrack,
    dismissed: Set<Pair<String, String>>,
    dismissedIds: Set<String>,
    dismissedKeys: Set<String>,
): Boolean {
    if (dismissed.isEmpty()) return false
    val key = SonataViewModel.canonicalTrackKey(track)
    return (track.id.isNotBlank() && track.id in dismissedIds) ||
        (key.isNotBlank() && key in dismissedKeys)
}

internal fun SonataViewModel.recordLastPlayedImpl(track: AndroidTrack) {
    if (track.id.isBlank() || track.title.isBlank()) return
    val trackKey = SonataViewModel.canonicalTrackKey(track)
    val merged = synchronized(historyLock) {
        val dismissed = parseDismissedEntries(historyPrefs.getString(SonataViewModel.DISMISSED_LAST_PLAYED_KEY, null))
        val remainingDismissed = dismissed.filterNot {
            it.id == track.id || (trackKey.isNotBlank() && it.canonicalKey == trackKey)
        }
        if (remainingDismissed.size != dismissed.size) {
            historyPrefs.edit()
                .putString(SonataViewModel.DISMISSED_LAST_PLAYED_KEY, encodeDismissedEntries(remainingDismissed))
                .apply()
        }
        val currentLocal = loadLocalLastPlayedHistoryImpl()
        val updatedLocal = SonataViewModel.deduplicateTracks(
            listOf(track) + currentLocal.filterNot {
                it.id == track.id || SonataViewModel.canonicalTrackKey(it) == trackKey
            }
        ).take(SonataViewModel.MAX_LAST_PLAYED_HISTORY)
        saveLocalLastPlayedHistoryImpl(updatedLocal)
        SonataViewModel.deduplicateTracks(
            updatedLocal + _lastPlayedTracks.value.filterNot { existing ->
                existing.id == track.id ||
                    SonataViewModel.canonicalTrackKey(existing) == trackKey ||
                    updatedLocal.any { it.id == existing.id || SonataViewModel.canonicalTrackKey(it) == SonataViewModel.canonicalTrackKey(existing) }
            }
        ).take(SonataViewModel.MAX_LAST_PLAYED_HISTORY)
    }
    _lastPlayedTracks.value = merged
    val updatedArtists = buildRecentArtistCardsImpl(merged, _uiState.value.shelves)
    _uiState.update { it.copy(lastPlayedTracks = merged, recentArtistCards = updatedArtists) }
    ensureArtistCardEnrichedImpl(track.artist)
}

internal fun SonataViewModel.removeFromLastPlayedImpl(track: AndroidTrack) {
    if (track.id.isBlank()) return
    val trackKey = SonataViewModel.canonicalTrackKey(track)
    val updated = synchronized(historyLock) {
        val currentDismissed = parseDismissedEntries(historyPrefs.getString(SonataViewModel.DISMISSED_LAST_PLAYED_KEY, null))
        val nextDismissed = (
            currentDismissed.filterNot {
                it.id == track.id || (trackKey.isNotBlank() && it.canonicalKey == trackKey)
            } + DismissedHistoryEntry(id = track.id, canonicalKey = trackKey)
        ).takeLast(SonataViewModel.MAX_DISMISSED_LAST_PLAYED)
        historyPrefs.edit()
            .putString(SonataViewModel.DISMISSED_LAST_PLAYED_KEY, encodeDismissedEntries(nextDismissed))
            .apply()

        val currentLocal = loadLocalLastPlayedHistoryImpl()
        val nextLocal = currentLocal.filterNot {
            it.id == track.id || SonataViewModel.canonicalTrackKey(it) == trackKey
        }
        saveLocalLastPlayedHistoryImpl(nextLocal)
        _lastPlayedTracks.value.filterNot {
            it.id == track.id || SonataViewModel.canonicalTrackKey(it) == trackKey
        }
    }
    _lastPlayedTracks.value = updated
    val updatedArtists = buildRecentArtistCardsImpl(updated, _uiState.value.shelves)
    _uiState.update { it.copy(lastPlayedTracks = updated, recentArtistCards = updatedArtists) }

    val stateSnapshot = _uiState.value
    if (
        stateSnapshot.signedIn &&
        !preferences.state.value.personalOnlyMode &&
        !isPersonalTrackId(track.id)
    ) {
        viewModelScope.launch(Dispatchers.IO) {
            runSuspendCatching { requireEngine().removeFromListeningHistory(track.id) }
                .onFailure { err ->
                    handleRateLimitIfNeeded(err)
                    Log.w("SonataLibrary", "Failed to remove track from YTM listening history: ${err.message}")
                }
        }
    }
}

internal fun SonataViewModel.mergeYtmListeningHistoryImpl(ytmHistory: List<AndroidTrack>) {
    if (ytmHistory.isEmpty()) return
    if (preferences.state.value.personalOnlyMode || !_uiState.value.signedIn) return
    val merged = synchronized(historyLock) {
        val dismissedEntries = parseDismissedEntries(historyPrefs.getString(SonataViewModel.DISMISSED_LAST_PLAYED_KEY, null))
        val dismissedPairs = dismissedEntries.mapTo(HashSet()) { it.id to it.canonicalKey }
        val dismissedIds = dismissedEntries.mapNotNullTo(HashSet()) { it.id.takeIf(String::isNotBlank) }
        val dismissedKeys = dismissedEntries.mapNotNullTo(HashSet()) { it.canonicalKey.takeIf(String::isNotBlank) }

        val filteredRemote = ytmHistory.filterNot {
            isDismissedHistoryTrack(it, dismissedPairs, dismissedIds, dismissedKeys)
        }
        val accountLocal = loadLocalLastPlayedHistoryImpl(signedInOverride = true).filterNot {
            isDismissedHistoryTrack(it, dismissedPairs, dismissedIds, dismissedKeys)
        }
        val combined = SonataViewModel.deduplicateTracks(filteredRemote + accountLocal)
            .take(SonataViewModel.MAX_LAST_PLAYED_HISTORY)
        saveLocalLastPlayedHistoryImpl(combined, signedInOverride = true)
        combined
    }
    _lastPlayedTracks.value = merged
    val updatedArtists = buildRecentArtistCardsImpl(merged, _uiState.value.shelves)
    _uiState.update { it.copy(lastPlayedTracks = merged, recentArtistCards = updatedArtists) }
}

internal fun SonataViewModel.refreshListeningHistoryImpl() {
    if (preferences.state.value.personalOnlyMode) return
    if (listeningHistoryJob?.isActive == true) return
    listeningHistoryJob = viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        if (!runCatching { engine.isSignedIn() }.getOrDefault(false)) return@launch
        runSuspendCatching { engine.fetchListeningHistory() }
            .onSuccess { history ->
                mergeYtmListeningHistoryImpl(history)
            }
            .onFailure { err ->
                handleRateLimitIfNeeded(err)
                Log.w("SonataLibrary", "Listening history refresh failed: ${err.message}")
            }
    }
}

internal fun shouldSyncLikeToYtm(trackId: String, personalOnlyMode: Boolean): Boolean {
    if (personalOnlyMode) return false
    val raw = trackId.trim()
    return raw.isNotBlank() &&
        !isPersonalTrackId(raw) &&
        !isSelfHostedId(raw) &&
        !isLocalTrackId(raw) &&
        !raw.startsWith("personal:", ignoreCase = true)
}

internal fun shouldCreatePersonalPlaylist(
    forcePersonal: Boolean,
    hasPersonalTrack: Boolean,
    signedIn: Boolean,
): Boolean = forcePersonal || hasPersonalTrack || !signedIn

internal fun SonataViewModel.toggleStarTrackImpl(trackId: String) {
    val nowLiked = trackId !in _uiState.value.likedTrackIds
    _uiState.update { state ->
        val updated = if (nowLiked) {
            state.likedTrackIds + trackId
        } else {
            state.likedTrackIds - trackId
        }
        state.copy(likedTrackIds = updated)
    }
    val snapshotTrack = _uiState.value.let { state ->
        listOfNotNull(state.activeTrack).firstOrNull { it.id == trackId }
            ?: state.queue.firstOrNull { it.id == trackId }
            ?: state.collectionTracks.firstOrNull { it.id == trackId }
            ?: state.tracks.firstOrNull { it.id == trackId }
            ?: state.lastPlayedTracks.firstOrNull { it.id == trackId }
            ?: (state.shelves + state.exploreShelves + state.radioMixShelves + state.moodCategoryShelves)
                .asSequence()
                .flatMap { shelf -> shelf.tracks.asSequence() + shelf.cards.asSequence().mapNotNull { it.track } }
                .firstOrNull { it.id == trackId }
    }
    if (nowLiked) {
        val likedCover = snapshotTrack?.let { resolveTrackCoverUrlImpl(it) }
            ?: resolveYtmFallbackCoverUrl(
                trackId = trackId,
                personalOnlyMode = preferences.state.value.personalOnlyMode,
            )
        updateLatestLikedCoverImpl(likedCover)
    }
    if (nowLiked &&
        settings.value.autoDownloadLiked &&
        snapshotTrack != null &&
        !isLocalTrackId(snapshotTrack.id) &&
        !downloadManager.isDownloaded(snapshotTrack.id)
    ) {
        downloadTrack(snapshotTrack)
    }
    val canSyncYtmLike = shouldSyncLikeToYtm(trackId, preferences.state.value.personalOnlyMode)
    if (canSyncYtmLike) {
        viewModelScope.launch(Dispatchers.IO) {
            if (preferences.state.value.personalOnlyMode) return@launch
            val engine = requireEngine()
            if (runCatching { engine.isSignedIn() }.getOrDefault(false)) {
                val ytmId: String = (if (snapshotTrack != null) {
                    resolveYtmVideoIdForTrack(engine, snapshotTrack)
                } else if (
                    !trackId.startsWith("jiosaavn:", ignoreCase = true) &&
                    !isPersonalTrackId(trackId) &&
                    !isSelfHostedId(trackId) &&
                    !isLocalTrackId(trackId) &&
                    !trackId.startsWith("personal:", ignoreCase = true)
                ) {
                    trackId
                } else {
                    null
                }) ?: return@launch
                runSuspendCatching { engine.setLikeStatus(ytmId, nowLiked) }
                    .onSuccess {
                        if (!nowLiked) {
                            runSuspendCatching { engine.fetchCollectionTracks("VLLM") }
                                .onSuccess { likedTracks ->
                                    val nextCover = likedTracks.firstNotNullOfOrNull { resolveTrackCoverUrlImpl(it) }
                                    updateLatestLikedCoverImpl(nextCover, clearIfNull = likedTracks.isEmpty())
                                }
                        }
                    }
                    .onFailure { err ->
                        Log.w("SonataLibrary", "Like status sync failed for '$ytmId': ${err.message}")
                    }
            }
        }
    }
}

internal fun SonataViewModel.openAddToPlaylistImpl(tracks: List<AndroidTrack>) {
    val unique = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
    if (unique.isEmpty()) return
    _uiState.update {
        it.copy(
            addToPlaylistTargets = unique,
            addToPlaylistStatus = null,
        )
    }
}

internal fun SonataViewModel.dismissAddToPlaylistImpl() {
    _uiState.update {
        it.copy(
            addToPlaylistTargets = emptyList(),
            addToPlaylistStatus = null,
        )
    }
}

internal fun SonataViewModel.clearAddToPlaylistStatusImpl() {
    _uiState.update {
        if (it.addToPlaylistStatus != null) it.copy(addToPlaylistStatus = null) else it
    }
}

internal fun SonataViewModel.addTracksToPlaylistsImpl(
    tracks: List<AndroidTrack>,
    playlistIds: List<String>,
    onComplete: (Boolean, String) -> Unit = { _, _ -> },
) {
    val s = uiStrings()
    val cleanTracks = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
    val cleanPlaylistIds = playlistIds.map { it.trim() }.filter { it.isNotBlank() }.distinct()
    if (cleanTracks.isEmpty() || cleanPlaylistIds.isEmpty()) {
        onComplete(false, s.selectAtLeastOnePlaylist)
        return
    }
    val hasPersonalTrack = cleanTracks.any { isPersonalTrackId(it.id) }
    val ytmOnlyTracks = cleanTracks.filterNot { isPersonalTrackId(it.id) }
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val personalOnly = preferences.state.value.personalOnlyMode
        val signedIn = !personalOnly && runCatching { engine.isSignedIn() }.getOrDefault(false)
        val resolvedVideoIds = if (signedIn && !hasPersonalTrack && ytmOnlyTracks.isNotEmpty() && cleanPlaylistIds.any { !it.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) && !it.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) }) {
            resolveYtmVideoIds(engine, ytmOnlyTracks)
        } else {
            emptyList()
        }
        _uiState.update { it.copy(addToPlaylistStatus = s.addingToPlaylistStatus) }

        var successCount = 0
        var firstError: String? = null

        val results = coroutineScope {
            cleanPlaylistIds.map { plId ->
                async {
                    when {
                        plId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) -> runSuspendCatching {
                            cleanTracks.forEach { tr ->
                                engine.addTrackToPersonalPlaylist(plId, tr)
                            }
                        }
                        plId.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) -> {
                            if (appendTracksToLocalPlaylistImpl(plId, cleanTracks)) {
                                Result.success(Unit)
                            } else {
                                Result.failure(IllegalStateException(s.failedUpdatePlaylist))
                            }
                        }
                        hasPersonalTrack -> {
                            Result.failure(IllegalStateException(sourcesStrings(preferences.state.value.appLanguage).addToPersonalPlaylist))
                        }
                        !signedIn -> {
                            Result.failure(IllegalStateException(s.signInToEditPlaylists))
                        }
                        resolvedVideoIds.isEmpty() -> {
                            Result.failure(IllegalStateException(s.noYtmMatchFound))
                        }
                        else -> {
                            runSuspendCatching { engine.addTracksToPlaylist(plId, resolvedVideoIds) }
                        }
                    }
                }
            }.awaitAll()
        }

        results.forEach { res ->
            res.onSuccess {
                successCount++
            }.onFailure { err ->
                if (firstError == null) firstError = err.message ?: s.failedUpdatePlaylist
            }
        }

        val updatedPersonal = runSuspendCatching { engine.listPersonalPlaylists() }
            .getOrNull()
            ?.also { personalPlaylistsCache = it }
        val updatedRemote = if (signedIn && successCount > 0 && cleanPlaylistIds.any { !it.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) && !it.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) }) {
            runSuspendCatching { engine.fetchLibraryPlaylists() }.getOrNull()
        } else {
            null
        }
        val ok = successCount == cleanPlaylistIds.size
        val message = when {
            ok -> if (cleanTracks.size == 1) {
                s.addedSingleTrackToPlaylists(cleanTracks.first().title, cleanPlaylistIds.size)
            } else {
                s.addedTracksToPlaylistsCount(cleanTracks.size, cleanPlaylistIds.size)
            }
            successCount > 0 -> s.addedToSomePlaylists(successCount, cleanPlaylistIds.size)
            else -> firstError ?: s.couldNotAddToPlaylist
        }

        _uiState.update { state ->
            val personalNow = updatedPersonal ?: state.personalPlaylists
            val baseRemote = updatedRemote ?: state.playlists.filterNot {
                it.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) || it.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)
            }
            val nextPlaylists = mergePlaylistsWithLocalImpl(baseRemote, personalNow)
            val updatedCollectionTracks = if (
                state.collectionBrowseId != null &&
                state.collectionBrowseId in cleanPlaylistIds &&
                state.collectionBrowseId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)
            ) {
                personalNow.firstOrNull { it.id == state.collectionBrowseId }?.tracks ?: state.collectionTracks
            } else {
                state.collectionTracks
            }
            state.copy(
                personalPlaylists = personalNow,
                playlists = nextPlaylists,
                collectionTracks = updatedCollectionTracks,
                addToPlaylistTargets = if (ok) emptyList() else state.addToPlaylistTargets,
                addToPlaylistStatus = message,
                status = message,
            )
        }
        withContext(Dispatchers.Main) {
            onComplete(ok, message)
        }
    }
}

internal fun SonataViewModel.createPlaylistWithTracksImpl(
    title: String,
    tracks: List<AndroidTrack>,
    onComplete: (Boolean, String) -> Unit = { _, _ -> },
    forcePersonal: Boolean = false,
) {
    val s = uiStrings()
    val srcStrings = sourcesStrings(preferences.state.value.appLanguage)
    val cleanTitle = title.trim()
    if (cleanTitle.isEmpty()) {
        onComplete(false, s.playlistNameEmpty)
        return
    }
    val cleanTracks = tracks.filter { it.id.isNotBlank() }.distinctBy { it.id }
    val hasPersonalTrack = cleanTracks.any { isPersonalTrackId(it.id) }
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val personalOnly = preferences.state.value.personalOnlyMode
        val signedIn = !personalOnly && runCatching { engine.isSignedIn() }.getOrDefault(false)
        _uiState.update { it.copy(addToPlaylistStatus = s.creatingPlaylistStatus(cleanTitle)) }

        if (shouldCreatePersonalPlaylist(forcePersonal, hasPersonalTrack, signedIn)) {
            val createRes = runSuspendCatching {
                var created = engine.createPersonalPlaylist(cleanTitle)
                cleanTracks.forEach { tr ->
                    created = engine.addTrackToPersonalPlaylist(created.id, tr)
                }
                created
            }
            if (createRes.isSuccess) {
                val updatedPersonal = runSuspendCatching { engine.listPersonalPlaylists() }
                    .getOrDefault(personalPlaylistsCache)
                    .also { personalPlaylistsCache = it }
                val message = srcStrings.personalPlaylistCreated(cleanTitle)
                _uiState.update { state ->
                    val baseRemote = state.playlists.filterNot {
                        it.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) || it.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)
                    }
                    state.copy(
                        personalPlaylists = updatedPersonal,
                        playlists = mergePlaylistsWithLocalImpl(baseRemote, updatedPersonal),
                        addToPlaylistTargets = emptyList(),
                        addToPlaylistStatus = message,
                        status = message,
                    )
                }
                withContext(Dispatchers.Main) { onComplete(true, message) }
            } else {
                val errMsg = createRes.exceptionOrNull()?.message ?: s.failedCreatePlaylist
                _uiState.update { it.copy(addToPlaylistStatus = errMsg, status = errMsg) }
                withContext(Dispatchers.Main) { onComplete(false, errMsg) }
            }
            return@launch
        }

        val resolvedVideoIds = resolveYtmVideoIds(engine, cleanTracks)
        val remoteResult = runSuspendCatching { engine.createPlaylist(cleanTitle, "", resolvedVideoIds) }
        if (remoteResult.isSuccess) {
            val createdId = remoteResult.getOrNull().orEmpty()
            val updatedRemote = runSuspendCatching { engine.fetchLibraryPlaylists() }.getOrNull()
            val createdThumb = cleanTracks.firstOrNull { !it.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
            val message = s.createdPlaylistStatus(cleanTitle)
            _uiState.update { state ->
                val base = updatedRemote ?: (listOf(AndroidPlaylist(createdId, cleanTitle, createdThumb, isOwned = true)) + state.playlists)
                val ownedNorm = normalizeCollectionId(createdId)
                val nextOwned = if (ownedNorm.startsWith("PL")) {
                    state.ownedPlaylistIds + ownedNorm
                } else {
                    computeOwnedPlaylistIdsImpl(base, state.savedExternalPlaylistIds)
                }
                state.copy(
                    playlists = mergePlaylistsWithLocalImpl(base),
                    ownedPlaylistIds = nextOwned,
                    addToPlaylistTargets = emptyList(),
                    addToPlaylistStatus = message,
                    status = message,
                )
            }
            withContext(Dispatchers.Main) { onComplete(true, message) }
        } else {
            val errMsg = remoteResult.exceptionOrNull()?.message ?: s.failedCreatePlaylist
            _uiState.update {
                it.copy(
                    addToPlaylistStatus = errMsg,
                    status = errMsg,
                )
            }
            withContext(Dispatchers.Main) { onComplete(false, errMsg) }
        }
    }
}

internal fun SonataViewModel.deletePersonalPlaylistImpl(playlistId: String) {
    if (!playlistId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)) return
    val srcStrings = sourcesStrings(preferences.state.value.appLanguage)
    val existingName = _uiState.value.personalPlaylists.firstOrNull { it.id == playlistId }?.name ?: playlistId
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.deletePersonalPlaylist(playlistId) }
            .onSuccess {
                val updatedPersonal = runSuspendCatching { engine.listPersonalPlaylists() }
                    .getOrDefault(emptyList())
                    .also { personalPlaylistsCache = it }
                val msg = srcStrings.personalPlaylistRemoved(existingName)
                _uiState.update { state ->
                    val baseRemote = state.playlists.filterNot {
                        it.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) || it.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)
                    }
                    state.copy(
                        personalPlaylists = updatedPersonal,
                        playlists = mergePlaylistsWithLocalImpl(baseRemote, updatedPersonal),
                        collectionBrowseId = if (state.collectionBrowseId == playlistId) null else state.collectionBrowseId,
                        collectionTitle = if (state.collectionBrowseId == playlistId) null else state.collectionTitle,
                        collectionTracks = if (state.collectionBrowseId == playlistId) emptyList() else state.collectionTracks,
                        status = msg,
                    )
                }
            }
    }
}

internal fun SonataViewModel.removeTrackFromPersonalPlaylistImpl(playlistId: String, trackId: String) {
    if (!playlistId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) || trackId.isBlank()) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.removeTrackFromPersonalPlaylist(playlistId, trackId) }
            .onSuccess { updatedPl ->
                val updatedPersonal = runSuspendCatching { engine.listPersonalPlaylists() }
                    .getOrDefault(personalPlaylistsCache)
                    .also { personalPlaylistsCache = it }
                _uiState.update { state ->
                    val baseRemote = state.playlists.filterNot {
                        it.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) || it.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)
                    }
                    state.copy(
                        personalPlaylists = updatedPersonal,
                        playlists = mergePlaylistsWithLocalImpl(baseRemote, updatedPersonal),
                        collectionTracks = if (state.collectionBrowseId == playlistId) updatedPl.tracks else state.collectionTracks,
                    )
                }
            }
    }
}

internal fun SonataViewModel.editPlaylistImpl(
    playlistId: String,
    title: String?,
    description: String?,
    privacy: String?,
    coverBytes: ByteArray?,
    mimeType: String?,
    onComplete: (Boolean, String) -> Unit = { _, _ -> },
) {
    val s = uiStrings()
    val cleanId = playlistId.trim()
    val cleanTitle = title?.trim()?.takeIf { it.isNotEmpty() }
    val cleanDesc = description?.trim()?.takeIf { it.isNotEmpty() }
    val cleanPrivacy = privacy?.trim()?.takeIf { it.isNotEmpty() }
    val hasMetadata = cleanTitle != null || cleanDesc != null || cleanPrivacy != null
    val hasCover = coverBytes != null && coverBytes.isNotEmpty()
    if (cleanId.isEmpty() || (!hasMetadata && !hasCover)) {
        onComplete(false, s.nothingToUpdate)
        return
    }

    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        if (cleanId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)) {
            if (cleanTitle == null) {
                withContext(Dispatchers.Main) { onComplete(false, s.nothingToUpdate) }
                return@launch
            }
            val renameRes = runSuspendCatching { engine.renamePersonalPlaylist(cleanId, cleanTitle) }
            if (renameRes.isSuccess) {
                val updatedPersonal = runSuspendCatching { engine.listPersonalPlaylists() }
                    .getOrDefault(personalPlaylistsCache)
                    .also { personalPlaylistsCache = it }
                _uiState.update { state ->
                    val baseRemote = state.playlists.filterNot {
                        it.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) || it.id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)
                    }
                    state.copy(
                        personalPlaylists = updatedPersonal,
                        playlists = mergePlaylistsWithLocalImpl(baseRemote, updatedPersonal),
                        collectionTitle = if (state.collectionBrowseId == cleanId) cleanTitle else state.collectionTitle,
                        status = s.updatedPlaylist,
                    )
                }
                withContext(Dispatchers.Main) { onComplete(true, s.updatedPlaylist) }
            } else {
                val err = renameRes.exceptionOrNull()?.message ?: s.failedUpdatePlaylist
                _uiState.update { it.copy(status = err) }
                withContext(Dispatchers.Main) { onComplete(false, err) }
            }
            return@launch
        }
        if (!runCatching { engine.isSignedIn() }.getOrDefault(false)) {
            val msg = s.signInToEditPlaylists
            withContext(Dispatchers.Main) { onComplete(false, msg) }
            return@launch
        }

        if (hasMetadata) {
            val metaRes = runSuspendCatching {
                engine.editPlaylistMetadata(cleanId, cleanTitle, cleanDesc, cleanPrivacy)
            }
            if (metaRes.isFailure) {
                val err = metaRes.exceptionOrNull()?.message ?: s.failedUpdatePlaylist
                _uiState.update { it.copy(status = err) }
                withContext(Dispatchers.Main) { onComplete(false, err) }
                return@launch
            }
        }

        if (hasCover) {
            val bytes = coverBytes!!
            val mime = mimeType?.takeIf { it.isNotBlank() } ?: "image/jpeg"
            val coverRes = runSuspendCatching {
                engine.uploadPlaylistCover(cleanId, bytes, mime)
            }
            if (coverRes.isFailure) {
                val err = coverRes.exceptionOrNull()?.message ?: s.failedUploadCover
                _uiState.update { it.copy(status = err) }
                withContext(Dispatchers.Main) { onComplete(false, err) }
                return@launch
            }
        }

        val refreshedRemote = runSuspendCatching { engine.fetchLibraryPlaylists() }.getOrNull()
        val normalizedId = cleanId.removePrefix("VL")
        val matchedRemote = refreshedRemote?.firstOrNull {
            it.id == cleanId || it.id.removePrefix("VL") == normalizedId
        }

        val updatedTitle = cleanTitle ?: matchedRemote?.name
        val statusMsg = if (hasCover && cleanTitle != null) {
            s.updatedPlaylistAndCover
        } else if (hasCover) {
            s.updatedPlaylistCover
        } else {
            s.updatedPlaylist
        }

        _uiState.update { state ->
            val isCurrentDetail = state.collectionBrowseId?.let {
                it == cleanId || it.removePrefix("VL") == normalizedId
            } == true
            val basePlaylists = refreshedRemote ?: state.playlists.map { pl ->
                if (pl.id == cleanId || pl.id.removePrefix("VL") == normalizedId) {
                    pl.copy(name = updatedTitle ?: pl.name)
                } else {
                    pl
                }
            }
            state.copy(
                playlists = mergePlaylistsWithLocalImpl(basePlaylists),
                collectionTitle = if (isCurrentDetail && updatedTitle != null) updatedTitle else state.collectionTitle,
                collectionCoverUrl = if (isCurrentDetail && matchedRemote?.thumbnailUrl != null) {
                    matchedRemote.thumbnailUrl
                } else {
                    state.collectionCoverUrl
                },
                status = statusMsg,
            )
        }
        withContext(Dispatchers.Main) { onComplete(true, statusMsg) }
    }
}

internal fun SonataViewModel.removeTrackFromCacheImpl(videoId: String) {
    if (videoId.isBlank()) return
    viewModelScope.launch(Dispatchers.IO) {
        val cached = streamCache.remove(videoId)
        PlaybackService.evictCacheKey(videoId)
        val rawCacheKey = activeCacheKeysByTrackId.remove(videoId)
        if (rawCacheKey != null) {
            PlaybackService.evictCacheKey(rawCacheKey)
        }
        if (cached != null && cached.stream.badge.isNotBlank()) {
            PlaybackService.evictCacheKey("$videoId:${cached.stream.badge}")
        }
        val activeBadge = _uiState.value.takeIf { it.activeTrack?.id == videoId }?.streamBadge
        if (!activeBadge.isNullOrBlank()) {
            PlaybackService.evictCacheKey("$videoId:$activeBadge")
        }
        downloadManager.removeDownload(videoId)
        _uiState.update { it.copy(status = uiStrings().removedFromCacheStatus) }
    }
}

internal suspend fun SonataViewModel.fetchTracksForCollectionActionImpl(
    engine: SonataEngineGateway,
    browseId: String,
): List<AndroidTrack> {
    return when {
        browseId == SonataViewModel.DOWNLOADS_PLAYLIST_ID -> downloads.value.map { downloadedRecordToAndroidTrack(it) }
        browseId.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) -> {
            _uiState.value.personalPlaylists.firstOrNull { it.id == browseId }?.tracks
                ?: personalPlaylistsCache.firstOrNull { it.id == browseId }?.tracks
                ?: runSuspendCatching {
                    engine.listPersonalPlaylists().firstOrNull { it.id == browseId }?.tracks
                }.getOrNull().orEmpty()
        }
        browseId.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) -> getLocalPlaylistTracksImpl(browseId)
        browseId.startsWith("local:album:") -> {
            val library = _uiState.value.localMusicLibrary
            library.albums.firstOrNull { it.id == browseId }?.tracks?.map { it.toAndroidTrack() }
                ?: library.tracks
                    .filter { it.toAndroidTrack().albumId == browseId }
                    .map { it.toAndroidTrack() }
        }
        isLocalTrackId(browseId) -> emptyList()
        browseId.startsWith("smb:") -> {
            val srvId = browseId.removePrefix("smb:").substringBefore(':')
            val srv = _uiState.value.servers.firstOrNull { it.id == srvId }
            if (srv != null) {
                runSuspendCatching { SmbClient.getAlbumTracks(srv, browseId) }.getOrDefault(emptyList())
            } else {
                runSuspendCatching { engine.getServerPlaylistTracks(browseId) }.getOrDefault(emptyList())
            }
        }
        isSelfHostedId(browseId) -> {
            runSuspendCatching { engine.getServerPlaylistTracks(browseId) }.getOrDefault(emptyList())
        }
        preferences.state.value.personalOnlyMode -> emptyList()
        else -> runSuspendCatching { engine.fetchCollectionTracks(browseId) }.getOrDefault(emptyList())
    }
}

internal fun SonataViewModel.playCollectionShuffledImpl(
    browseId: String,
    title: String,
    subtitle: String = "",
    coverUrl: String? = null,
) {
    if (browseId.isBlank()) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val tracks = fetchTracksForCollectionActionImpl(engine, browseId)
        if (tracks.isEmpty()) {
            _uiState.update { it.copy(status = uiStrings().noTracksInCollection(title)) }
            return@launch
        }
        val backfilled = SonataViewModel.backfillCollectionTracks(
            rawTracks = tracks,
            id = browseId,
            title = title.ifBlank { uiStrings().playlistLabel },
            subtitle = subtitle.takeIf { it.isNotBlank() },
            coverUrl = coverUrl,
        )
        toggleShuffleAndPlayImpl(backfilled)
    }
}

internal fun SonataViewModel.enqueueCollectionNextImpl(browseId: String) {
    if (browseId.isBlank()) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val tracks = fetchTracksForCollectionActionImpl(engine, browseId)
        if (tracks.isEmpty()) return@launch
        val current = _uiState.value
        val existingQueue = current.queue
        if (existingQueue.isEmpty() || current.activeTrack == null) {
            playTrack(tracks.first(), tracks)
            return@launch
        }
        val insertIndex = (current.queueIndex + 1).coerceIn(0, existingQueue.size)
        val mutable = existingQueue.toMutableList()
        mutable.addAll(insertIndex, tracks)
        _uiState.update {
            it.copy(queue = mutable, status = uiStrings().addedToPlayNextStatus(tracks.size))
        }
        syncRemoteQueueIfConnectedImpl(
            updatedQueue = mutable,
            queueIndex = current.queueIndex,
            playWhenReady = current.isPlaying,
        )
    }
}

internal fun SonataViewModel.enqueueCollectionLaterImpl(browseId: String) {
    if (browseId.isBlank()) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val tracks = fetchTracksForCollectionActionImpl(engine, browseId)
        if (tracks.isEmpty()) return@launch
        val current = _uiState.value
        val existingQueue = current.queue
        if (existingQueue.isEmpty() || current.activeTrack == null) {
            playTrack(tracks.first(), tracks)
            return@launch
        }
        val updated = existingQueue + tracks
        _uiState.update {
            it.copy(
                queue = updated,
                status = uiStrings().addedToQueueStatus(tracks.size),
            )
        }
        syncRemoteQueueIfConnectedImpl(
            updatedQueue = updated,
            queueIndex = current.queueIndex,
            playWhenReady = current.isPlaying,
        )
    }
}

internal fun SonataViewModel.downloadCollectionByIdImpl(browseId: String) {
    if (browseId.isBlank()) return
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        val tracks = fetchTracksForCollectionActionImpl(engine, browseId)
        if (tracks.isNotEmpty()) {
            downloadCollection(tracks, browseId)
        }
    }
}

internal fun SonataViewModel.playNextImpl(track: AndroidTrack) {
    val current = _uiState.value
    val queue = current.queue
    if (queue.isEmpty() || current.activeTrack == null) {
        playTrack(track, listOf(track))
        return
    }
    val insertIndex = (current.queueIndex + 1).coerceIn(0, queue.size)
    val mutable = queue.toMutableList()
    mutable.add(insertIndex, track)
    _uiState.update { it.copy(queue = mutable, status = uiStrings().playingNextStatus(track.title)) }
    syncRemoteQueueIfConnectedImpl(
        updatedQueue = mutable,
        queueIndex = current.queueIndex,
        playWhenReady = current.isPlaying,
    )
}

internal fun SonataViewModel.addToQueueImpl(track: AndroidTrack) {
    val current = _uiState.value
    val queue = current.queue
    if (queue.isEmpty() || current.activeTrack == null) {
        playTrack(track, listOf(track))
        return
    }
    val updated = queue + track
    _uiState.update {
        it.copy(queue = updated, status = uiStrings().addedTrackToQueueStatus(track.title))
    }
    syncRemoteQueueIfConnectedImpl(
        updatedQueue = updated,
        queueIndex = current.queueIndex,
        playWhenReady = current.isPlaying,
    )
}

internal fun SonataViewModel.toggleShuffleAndPlayImpl(tracks: List<AndroidTrack>) {
    if (tracks.isEmpty()) return
    preferences.update { it.copy(shuffleEnabled = true) }
    _uiState.update { it.copy(shuffleEnabled = true) }
    val start = tracks[Random.nextInt(tracks.size)]
    playTrack(start, tracks)
}

internal fun SonataViewModel.isCollectionPinnedImpl(browseId: String?): Boolean {
    val norm = normalizeCollectionId(browseId)
    if (norm.isBlank()) return false
    return _uiState.value.pinnedCollections.any { normalizeCollectionId(it.id) == norm }
}

internal fun SonataViewModel.isPlaylistSavedToLibraryImpl(browseId: String?): Boolean {
    val norm = normalizeCollectionId(browseId)
    if (norm.isBlank()) return false
    val state = _uiState.value
    if (norm in state.savedExternalPlaylistIds) return true
    if (norm in state.ownedPlaylistIds) return false
    return state.playlists.any { normalizeCollectionId(it.id) == norm }
}

internal fun SonataViewModel.isOwnedEditablePlaylistImpl(browseId: String?): Boolean {
    if (browseId?.startsWith(PERSONAL_PLAYLIST_ID_PREFIX) == true) return true
    val state = _uiState.value
    return isEditableUserPlaylist(
        browseId = browseId,
        signedIn = state.signedIn,
        ownedPlaylistIds = state.ownedPlaylistIds,
        savedExternalPlaylistIds = state.savedExternalPlaylistIds,
    )
}

internal fun SonataViewModel.togglePinCollectionImpl(
    id: String,
    title: String,
    subtitle: String = "",
    coverUrl: String? = null,
    kindLabel: String = "",
    isAlbum: Boolean = false,
) {
    val norm = normalizeCollectionId(id)
    if (norm.isBlank()) return
    val s = uiStrings()
    val cleanTitle = title.trim().ifBlank { s.playlistLabel }
    viewModelScope.launch(Dispatchers.IO) {
        var nowPinned = false
        val updatedList = synchronized(pinnedSavedLock) {
            val existingIdx = cachedPinnedCollections.indexOfFirst { normalizeCollectionId(it.id) == norm }
            val next = if (existingIdx >= 0) {
                nowPinned = false
                cachedPinnedCollections.filterNot { normalizeCollectionId(it.id) == norm }
            } else {
                nowPinned = true
                val resolvedCover = coverUrl?.takeIf { it.isNotBlank() }
                    ?: _uiState.value.playlists.firstOrNull { normalizeCollectionId(it.id) == norm }?.thumbnailUrl
                val entry = PinnedCollectionEntry(
                    id = id.trim(),
                    title = cleanTitle,
                    subtitle = subtitle.trim().ifBlank {
                        if (isAlbum) s.albumLabel else s.playlistLabel
                    },
                    coverUrl = resolvedCover,
                    kindLabel = kindLabel.ifBlank { if (isAlbum) s.albumLabel else s.playlistLabel },
                    isAlbum = isAlbum,
                    pinnedAtMs = System.currentTimeMillis(),
                )
                listOf(entry) + cachedPinnedCollections
            }
            cachedPinnedCollections = next
            savePinnedCollectionsLockedImpl(next)
            next
        }
        val statusMsg = if (nowPinned) s.pinnedStatus(cleanTitle) else s.unpinnedStatus(cleanTitle)
        _uiState.update { state ->
            state.copy(
                pinnedCollections = updatedList,
                status = statusMsg,
            )
        }
    }
}

internal fun SonataViewModel.toggleSaveExternalPlaylistImpl(
    id: String,
    title: String,
    subtitle: String = "",
    coverUrl: String? = null,
    kindLabel: String = "",
) {
    val norm = normalizeCollectionId(id)
    if (norm.isBlank() || isOwnedEditablePlaylistImpl(id)) return
    val s = uiStrings()
    val cleanTitle = title.trim().ifBlank { s.playlistLabel }
    viewModelScope.launch(Dispatchers.IO) {
        var nowSaved = false
        val updatedSaved = synchronized(pinnedSavedLock) {
            val existsInSaved = cachedSavedExternalPlaylists.any { normalizeCollectionId(it.id) == norm }
            val inPlaylists = _uiState.value.playlists.any { normalizeCollectionId(it.id) == norm }
            val shouldRemove = existsInSaved || inPlaylists
            val next = if (shouldRemove) {
                nowSaved = false
                cachedSavedExternalPlaylists.filterNot { normalizeCollectionId(it.id) == norm }
            } else {
                nowSaved = true
                val entry = SavedExternalPlaylist(
                    id = id.trim(),
                    title = cleanTitle,
                    subtitle = subtitle.trim().ifBlank { s.playlistLabel },
                    coverUrl = coverUrl?.takeIf { it.isNotBlank() },
                    kindLabel = kindLabel.ifBlank { s.playlistLabel },
                    savedAtMs = System.currentTimeMillis(),
                )
                listOf(entry) + cachedSavedExternalPlaylists
            }
            cachedSavedExternalPlaylists = next
            saveExternalPlaylistsLockedImpl(next)
            next
        }
        val savedIdSet = updatedSaved.mapTo(HashSet()) { normalizeCollectionId(it.id) }
        val statusMsg = if (nowSaved) s.addedToLibraryStatus(cleanTitle) else s.removedFromLibraryStatus(cleanTitle)
        _uiState.update { state ->
            val remoteClean = state.playlists.filter { pl ->
                val plNorm = normalizeCollectionId(pl.id)
                !pl.id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX) &&
                    plNorm !in state.savedExternalPlaylistIds &&
                    (nowSaved || plNorm != norm)
            }
            state.copy(
                savedExternalPlaylistIds = savedIdSet,
                playlists = mergePlaylistsWithLocalImpl(remoteClean),
                status = statusMsg,
            )
        }
        val engine = requireEngine()
        if (runCatching { engine.isSignedIn() }.getOrDefault(false)) {
            runSuspendCatching { engine.setPlaylistSaved(id, nowSaved) }
                .onFailure { err ->
                    Log.w("SonataLibrary", "setPlaylistSaved($id, $nowSaved) remote sync failed: ${err.message}")
                }
        }
    }
}

internal fun SonataViewModel.openPinnedCollectionImpl(entry: PinnedCollectionEntry) {
    if (entry.id == SonataViewModel.DOWNLOADS_PLAYLIST_ID) {
        openDownloadsPlaylistImpl()
    } else {
        openAlbumOrPlaylistImpl(
            id = entry.id,
            title = entry.title,
            coverUrl = entry.coverUrl,
            subtitle = entry.subtitle,
            kindLabel = entry.kindLabel,
        )
    }
}

internal fun SonataViewModel.openPlaylistImpl(playlist: AndroidPlaylist) {
    val s = uiStrings()
    openCollectionByIdImpl(
        id = playlist.id,
        title = playlist.name,
        coverUrl = playlist.thumbnailUrl,
        subtitle = if (playlist.isOwned) (_uiState.value.accountProfile?.name ?: s.playlistLabel) else s.playlistLabel,
        kindLabel = s.playlistLabel,
    )
}

internal fun SonataViewModel.openDownloadsPlaylistImpl() {
    val s = uiStrings()
    val latestCover = downloads.value.maxByOrNull { it.downloadedAtMs }?.coverUrl?.takeIf { it.isNotBlank() }
        ?: downloads.value.firstOrNull { !it.coverUrl.isNullOrBlank() }?.coverUrl
    openCollectionByIdImpl(
        id = SonataViewModel.DOWNLOADS_PLAYLIST_ID,
        title = s.downloads,
        coverUrl = latestCover,
        subtitle = s.offlineSongsCount(downloads.value.size),
        kindLabel = s.offlineLibrary,
    )
}

internal fun SonataViewModel.openAlbumOrPlaylistImpl(
    id: String,
    title: String,
    coverUrl: String? = null,
    subtitle: String? = null,
    kindLabel: String? = null,
    year: String? = null,
) {
    val isAlbumKind = kindLabel.equals("Album", ignoreCase = true) ||
        kindLabel.equals(uiStrings().albumLabel, ignoreCase = true)
    if (isSelfHostedId(id)) {
        if (isAlbumKind) {
            openServerAlbumById(
                albumId = id,
                albumTitle = title,
                artistName = subtitle.orEmpty(),
                coverUrl = coverUrl,
                year = year,
            )
        } else {
            openCollectionByIdImpl(
                id = id,
                title = title,
                coverUrl = coverUrl,
                subtitle = subtitle,
                kindLabel = kindLabel,
                year = year,
            )
        }
        return
    }
    if (isAlbumKind &&
        !id.startsWith("MPRE") &&
        !id.startsWith("OLAK") &&
        !id.startsWith("VL") &&
        !id.startsWith("PL")
    ) {
        openAlbumImpl(
            albumId = id,
            albumTitle = title,
            artistName = subtitle.orEmpty(),
            coverUrl = coverUrl,
            year = year,
        )
        return
    }
    openCollectionByIdImpl(
        id = id,
        title = title,
        coverUrl = coverUrl,
        subtitle = subtitle,
        kindLabel = kindLabel,
        year = year,
    )
}

internal fun SonataViewModel.openAlbumImpl(
    albumId: String,
    albumTitle: String,
    artistName: String = "",
    coverUrl: String? = null,
    year: String? = null,
) {
    val s = uiStrings()
    val cleanId = albumId.trim()
    val cleanTitle = albumTitle.trim()
    if (isSelfHostedId(cleanId)) {
        openServerAlbumById(
            albumId = cleanId,
            albumTitle = cleanTitle.ifBlank { s.albumLabel },
            artistName = artistName,
            coverUrl = coverUrl,
            year = year,
        )
        return
    }
    if (cleanId.startsWith("MPRE") ||
        cleanId.startsWith("OLAK") ||
        cleanId.startsWith("VL") ||
        cleanId.startsWith("PL")
    ) {
        openCollectionByIdImpl(
            id = cleanId,
            title = cleanTitle.ifBlank { s.albumLabel },
            coverUrl = coverUrl,
            subtitle = artistName.takeIf { it.isNotBlank() },
            kindLabel = "Album",
            year = year,
        )
        return
    }
    if (cleanTitle.isBlank()) {
        if (artistName.isNotBlank()) openArtistImpl("", artistName)
        return
    }
    speculativePrefetchJob?.cancel()
    viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        _uiState.update {
            it.copy(
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                collectionTitle = cleanTitle,
                collectionCoverUrl = coverUrl,
                collectionSubtitle = artistName.takeIf { sub -> sub.isNotBlank() },
                collectionKindLabel = "Album",
                collectionYear = year,
                collectionTracks = emptyList(),
                loading = true,
                status = s.openingNamedStatus(cleanTitle),
            )
        }
        val resolvedId = runSuspendCatching {
            engine.resolveAlbumBrowseId(cleanTitle, artistName)
        }.getOrNull()
        if (!resolvedId.isNullOrBlank()) {
            openCollectionByIdImpl(
                id = resolvedId,
                title = cleanTitle,
                coverUrl = coverUrl,
                subtitle = artistName.takeIf { sub -> sub.isNotBlank() },
                kindLabel = "Album",
                year = year,
            )
        } else {
            _uiState.update {
                it.copy(
                    loading = false,
                    collectionTitle = null,
                    collectionTracks = emptyList(),
                )
            }
            search(listOf(cleanTitle, artistName).filter { part -> part.isNotBlank() }.joinToString(" "))
        }
    }
}

internal fun SonataViewModel.openArtistImpl(browseId: String, name: String = "") {
    val cleanBrowseId = browseId.trim()
    val rawName = name.trim().ifBlank {
        cleanBrowseId.substringAfter(":artist:", "").trim()
    }
    if (cleanBrowseId.isBlank() && rawName.isBlank()) return
    speculativePrefetchJob?.cancel()

    val isPersonalOnly = preferences.state.value.personalOnlyMode
    val isPersonalArtistRequest = isPersonalOnly ||
        isPersonalTrackId(cleanBrowseId) ||
        cleanBrowseId.startsWith("personal:artist:")
    val preferPersonalFirst = isPersonalArtistRequest ||
        (cleanBrowseId.isBlank() && _uiState.value.activeTrack?.id?.let { isPersonalTrackId(it) } == true)

    val cached = if (!isPersonalArtistRequest) {
        (if (cleanBrowseId.startsWith("UC")) getCachedArtistPageImpl(cleanBrowseId) else null)
            ?: rawName.takeIf { it.isNotBlank() }?.let { getCachedArtistPageImpl("name:$it") }
    } else {
        null
    }
    if (cached != null) {
        _uiState.update {
            it.copy(
                collectionBrowseId = null,
                collectionTitle = null,
                collectionTracks = emptyList(),
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                artistLoading = false,
                artistPage = cached,
                status = cached.name,
            )
        }
        prefetchStreams(cached.popularTracks.take(2), delayMs = 1_000L)
        return
    }

    val s = uiStrings()
    artistJob?.cancel()
    artistJob = viewModelScope.launch(Dispatchers.IO) {
        _uiState.update {
            it.copy(
                collectionBrowseId = null,
                collectionTitle = null,
                collectionTracks = emptyList(),
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                artistLoading = true,
                artistPage = AndroidArtistPage(
                    id = cleanBrowseId,
                    name = rawName.ifBlank { s.artistDefault },
                    subtitle = "",
                    description = null,
                    subscribers = null,
                    monthlyListeners = null,
                    thumbnailUrl = null,
                    shufflePlaylistId = null,
                    radioPlaylistId = null,
                    latestRelease = null,
                    popularTracks = emptyList(),
                    releases = emptyList(),
                    relatedArtists = emptyList(),
                ),
                status = s.loadingNamedStatus(rawName.ifBlank { s.artistDefault }),
            )
        }

        if (preferPersonalFirst) {
            val personalPage = runSuspendCatching {
                buildPersonalArtistPageImpl(cleanBrowseId, rawName)
            }.getOrNull()
            if (personalPage != null) {
                _uiState.update {
                    it.copy(
                        artistLoading = false,
                        artistPage = personalPage,
                        status = personalPage.name,
                    )
                }
                return@launch
            }
            if (isPersonalOnly || isPersonalArtistRequest) {
                _uiState.update {
                    it.copy(
                        artistLoading = false,
                        artistPage = null,
                    )
                }
                return@launch
            }
        }

        val engine = requireEngine()
        val result = runSuspendCatching {
            if (cleanBrowseId.startsWith("UC")) {
                engine.fetchArtistPage(cleanBrowseId)
            } else {
                engine.fetchArtistByName(rawName)
            }
        }
        result
            .onSuccess { page ->
                putCachedArtistPageImpl(page, rawName)
                _uiState.update {
                    it.copy(
                        artistLoading = false,
                        artistPage = page,
                        status = page.name,
                    )
                }
                prefetchStreams(page.popularTracks.take(2), delayMs = 1_200L)
            }
            .onFailure { err ->
                val personalFallback = runSuspendCatching {
                    buildPersonalArtistPageImpl(cleanBrowseId, rawName)
                }.getOrNull()
                if (personalFallback != null) {
                    _uiState.update {
                        it.copy(
                            artistLoading = false,
                            artistPage = personalFallback,
                            status = personalFallback.name,
                        )
                    }
                } else if (!cleanBrowseId.startsWith("UC") && rawName.isNotBlank()) {
                    _uiState.update {
                        it.copy(
                            artistLoading = false,
                            artistPage = null,
                        )
                    }
                    search(rawName)
                } else {
                    _uiState.update {
                        it.copy(
                            artistLoading = false,
                            status = s.artistErrorStatus(err.message.orEmpty()),
                        )
                    }
                }
            }
    }
}

internal suspend fun SonataViewModel.buildPersonalArtistPageImpl(
    cleanBrowseId: String,
    cleanName: String,
): AndroidArtistPage? {
    val targetName = cleanName.ifBlank { cleanBrowseId.substringAfter(":artist:", "") }.trim()
    val preferredSrvId = if (cleanBrowseId.startsWith("smb:") ||
        cleanBrowseId.startsWith("jellyfin:") ||
        cleanBrowseId.startsWith("subsonic:") ||
        cleanBrowseId.startsWith("webdav:")
    ) {
        cleanBrowseId.substringAfter(':').substringBefore(':')
    } else {
        null
    }
    if (targetName.isBlank() && preferredSrvId.isNullOrBlank()) return null

    val stateSnapshot = _uiState.value
    val srcStrings = sourcesStrings(preferences.state.value.appLanguage)
    val enabledServers = stateSnapshot.servers
        .filter { it.enabled }
        .sortedByDescending { it.id == preferredSrvId }

    var resolvedName = targetName
    var metaAvatar: String? = null
    var metaBio: String? = null
    var metaSubtitle: String? = null
    val collectedTracks = ArrayList<AndroidTrack>()
    val collectedAlbums = ArrayList<AndroidHomeCard>()
    val collectedRelated = ArrayList<AndroidHomeCard>()

    // 1. SMB servers (rich artist folder image + artist.json/artist.nfo + albums/tracks)
    for (srv in enabledServers.filter { it.kind == AndroidServerKind.SMB }) {
        val profile = runSuspendCatching { SmbClient.fetchArtistProfile(srv, resolvedName) }.getOrNull() ?: continue
        if (resolvedName.isBlank()) resolvedName = profile.name
        if (metaAvatar == null) metaAvatar = profile.avatarUrl
        if (metaBio == null) metaBio = profile.bio
        if (metaSubtitle == null) metaSubtitle = profile.subtitle
        collectedAlbums.addAll(profile.albums)
        collectedTracks.addAll(profile.tracks)
        collectedRelated.addAll(profile.relatedArtists)
    }

    // 2. Non-SMB self-hosted servers (Jellyfin, Subsonic, WebDAV)
    val otherServers = enabledServers.filter { it.kind != AndroidServerKind.SMB }
    if (otherServers.isNotEmpty()) {
        val engine = runCatching { requireEngine() }.getOrNull()
        if (engine != null && cleanBrowseId.startsWith("webdav:")) {
            val webdavArtist = runSuspendCatching { engine.fetchArtistPage(cleanBrowseId) }.getOrNull()
            if (webdavArtist != null) {
                if (resolvedName.isBlank()) resolvedName = webdavArtist.name
                if (metaAvatar == null) metaAvatar = webdavArtist.thumbnailUrl
                if (metaBio == null) metaBio = webdavArtist.description
                if (metaSubtitle == null && webdavArtist.subtitle.isNotBlank()) metaSubtitle = webdavArtist.subtitle
                collectedAlbums.addAll(webdavArtist.releases)
                collectedTracks.addAll(webdavArtist.popularTracks)
                collectedRelated.addAll(webdavArtist.relatedArtists)
            }
        }
        for (srv in otherServers) {
            if (engine != null && srv.kind == AndroidServerKind.WEB_DAV && resolvedName.isNotBlank()) {
                val webdavId = "webdav:${srv.id}:artist:$resolvedName"
                if (webdavId != cleanBrowseId) {
                    val webdavArtist = runSuspendCatching { engine.fetchArtistPage(webdavId) }.getOrNull()
                    if (webdavArtist != null) {
                        if (metaAvatar == null) metaAvatar = webdavArtist.thumbnailUrl
                        if (metaBio == null) metaBio = webdavArtist.description
                        if (metaSubtitle == null && webdavArtist.subtitle.isNotBlank()) metaSubtitle = webdavArtist.subtitle
                        collectedAlbums.addAll(webdavArtist.releases)
                        collectedTracks.addAll(webdavArtist.popularTracks)
                        collectedRelated.addAll(webdavArtist.relatedArtists)
                        continue
                    }
                }
            }
            val snap = stateSnapshot.serverSourceSnapshots[srv.id]
            if (snap != null) {
                collectedTracks.addAll(snap.tracks.filter { it.artist.equals(resolvedName, ignoreCase = true) })
                collectedAlbums.addAll(
                    snap.albums.filter {
                        it.subtitle.equals(resolvedName, ignoreCase = true) ||
                            it.title.equals(resolvedName, ignoreCase = true)
                    },
                )
                val matchedArtistCard = snap.artists.firstOrNull {
                    it.title.equals(resolvedName, ignoreCase = true) || it.id == cleanBrowseId
                }
                if (metaAvatar == null && !matchedArtistCard?.thumbnailUrl.isNullOrBlank()) {
                    metaAvatar = matchedArtistCard?.thumbnailUrl
                }
                collectedRelated.addAll(snap.artists.filterNot { it.title.equals(resolvedName, ignoreCase = true) })
            }
            if (engine != null && srv.id == preferredSrvId && cleanBrowseId.isNotBlank()) {
                val directAlbums = runSuspendCatching {
                    engine.listServerAlbums(srv.id, null, cleanBrowseId)
                }.getOrDefault(emptyList())
                collectedAlbums.addAll(directAlbums)
            }
        }
    }

    // 3. Local device library
    if (preferences.state.value.localMusicEnabled) {
        val localTracks = stateSnapshot.localMusicLibrary.tracks
            .filter { it.artist.equals(resolvedName, ignoreCase = true) }
            .map { it.toAndroidTrack() }
        collectedTracks.addAll(localTracks)
        stateSnapshot.localMusicLibrary.albums
            .filter {
                it.artist.equals(resolvedName, ignoreCase = true) ||
                    it.tracks.any { tr -> tr.artist.equals(resolvedName, ignoreCase = true) }
            }
            .forEach { alb ->
                collectedAlbums += AndroidHomeCard(
                    kind = AndroidHomeCardKind.ALBUM,
                    id = alb.id,
                    title = alb.title,
                    subtitle = alb.artist,
                    thumbnailUrl = alb.albumArtUri,
                    track = alb.tracks.firstOrNull()?.toAndroidTrack(),
                )
            }
    }

    val distinctTracks = collectedTracks.distinctBy { it.id }
    val distinctAlbums = collectedAlbums.distinctBy { it.id }
    val distinctRelated = collectedRelated.distinctBy { it.title.lowercase() }.take(12)

    if (distinctTracks.isEmpty() && distinctAlbums.isEmpty() && metaAvatar == null && metaBio == null) {
        return null
    }

    val displayName = resolvedName.ifBlank { uiStrings().artistDefault }
    val fallbackSubtitle = listOfNotNull(
        distinctAlbums.size.takeIf { it > 0 }?.let { uiStrings().servers.serverAlbumsCount(it) },
        distinctTracks.size.takeIf { it > 0 }?.let { srcStrings.localTracksCount(it) },
    ).joinToString(" · ")
    val heroArt = metaAvatar
        ?: distinctAlbums.firstNotNullOfOrNull { it.thumbnailUrl }
        ?: distinctTracks.firstNotNullOfOrNull { it.thumbnailUrl }

    return AndroidArtistPage(
        id = cleanBrowseId.ifBlank { "personal:artist:$displayName" },
        name = displayName,
        subtitle = metaSubtitle?.takeIf { it.isNotBlank() } ?: fallbackSubtitle,
        description = metaBio,
        subscribers = null,
        monthlyListeners = null,
        thumbnailUrl = heroArt,
        shufflePlaylistId = null,
        radioPlaylistId = null,
        latestRelease = distinctAlbums.firstOrNull(),
        popularTracks = distinctTracks,
        releases = distinctAlbums,
        relatedArtists = distinctRelated,
    )
}

internal fun SonataViewModel.selectHomeMoodChipImpl(params: String?) {
    if (preferences.state.value.personalOnlyMode) return
    val currentSelected = _uiState.value.selectedHomeMoodParams
    val nextParams = when {
        params.isNullOrBlank() -> null
        currentSelected == params -> null
        else -> params
    }
    homeMoodChipJob?.cancel()
    if (nextParams == null) {
        val cachedFull = fullHomeShelves
        if (cachedFull.isNotEmpty()) {
            val updatedArtists = buildRecentArtistCardsImpl(_lastPlayedTracks.value, cachedFull)
            _uiState.update {
                it.copy(
                    selectedHomeMoodParams = null,
                    shelves = cachedFull,
                    recentArtistCards = updatedArtists,
                    refreshing = false,
                    status = uiStrings().personalizedShelvesCount(cachedFull.size),
                )
            }
            return
        }
        _uiState.update { it.copy(selectedHomeMoodParams = null, refreshing = true) }
        homeMoodChipJob?.cancel()
        homeMoodChipJob = viewModelScope.launch(Dispatchers.IO) {
            val engine = requireEngine()
            runSuspendCatching { engine.fetchHomeMoodFeed(null) }
                .onSuccess { rawShelves ->
                    if (!isActive || _uiState.value.selectedHomeMoodParams != null) return@onSuccess
                    val cleaned = SonataViewModel.normalizeAndMergeHomeShelves(rawShelves)
                    if (cleaned.isNotEmpty()) {
                        fullHomeShelves = cleaned
                    }
                    val updatedArtists = buildRecentArtistCardsImpl(_lastPlayedTracks.value, cleaned)
                    _uiState.update { state ->
                        state.copy(
                            shelves = cleaned.ifEmpty { state.shelves },
                            recentArtistCards = updatedArtists,
                            refreshing = false,
                            status = uiStrings().personalizedShelvesCount(cleaned.size),
                        )
                    }
                }
                .onFailure { err ->
                    handleRateLimitIfNeeded(err)
                    _uiState.update {
                        it.copy(
                            refreshing = false,
                            status = uiStrings().homeErrorStatus(err.message.orEmpty()),
                        )
                    }
                }
        }
        return
    }

    _uiState.update { it.copy(selectedHomeMoodParams = nextParams, refreshing = true) }
    homeMoodChipJob?.cancel()
    homeMoodChipJob = viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        runSuspendCatching { engine.fetchHomeMoodFeed(nextParams) }
            .onSuccess { rawShelves ->
                if (!isActive || _uiState.value.selectedHomeMoodParams != nextParams) return@onSuccess
                val cleaned = SonataViewModel.normalizeAndMergeHomeShelves(rawShelves)
                val updatedArtists = buildRecentArtistCardsImpl(_lastPlayedTracks.value, cleaned)
                _uiState.update { state ->
                    state.copy(
                        shelves = cleaned.ifEmpty { state.shelves },
                        recentArtistCards = updatedArtists,
                        refreshing = false,
                        status = uiStrings().personalizedShelvesCount(cleaned.size),
                    )
                }
            }
            .onFailure { err ->
                handleRateLimitIfNeeded(err)
                _uiState.update { state ->
                    val fallback = if (fullHomeShelves.isNotEmpty()) fullHomeShelves else state.shelves
                    state.copy(
                        selectedHomeMoodParams = null,
                        shelves = fallback,
                        refreshing = false,
                        status = uiStrings().homeErrorStatus(err.message.orEmpty()),
                    )
                }
            }
    }
}

internal fun SonataViewModel.openMoodCategoryImpl(title: String, browseId: String = "", params: String = "") {
    if (preferences.state.value.personalOnlyMode) return
    val cleanTitle = title.trim()
    if (cleanTitle.isBlank() && browseId.isBlank()) return
    speculativePrefetchJob?.cancel()
    moodCategoryJob?.cancel()
    val s = uiStrings()
    val displayTitle = localizeMoodTitle(cleanTitle, s).ifBlank { cleanTitle }
    _uiState.update {
        it.copy(
            collectionBrowseId = null,
            collectionTitle = null,
            collectionTracks = emptyList(),
            artistPage = null,
            artistLoading = false,
            showLyrics = false,
            moodCategoryTitle = displayTitle,
            moodCategoryShelves = emptyList(),
            moodCategoryLoading = true,
            status = s.loadingNamedStatus(displayTitle),
        )
    }
    moodCategoryJob?.cancel()
    moodCategoryJob = viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        var resolvedBrowseId = browseId.trim()
        var resolvedParams = params.trim()

        if (resolvedBrowseId.isBlank() || resolvedParams.isBlank()) {
            var categories = _uiState.value.moodCategories
            if (categories.isEmpty()) {
                runSuspendCatching { engine.fetchExploreFeed() }
                    .onSuccess { feed ->
                        categories = feed.moods
                        val cleanedExplore = SonataViewModel.normalizeAndMergeHomeShelves(feed.shelves)
                        _uiState.update { state ->
                            state.copy(
                                exploreShelves = cleanedExplore.ifEmpty { state.exploreShelves },
                                moodCategories = feed.moods.ifEmpty { state.moodCategories },
                            )
                        }
                    }
                    .onFailure { handleRateLimitIfNeeded(it) }
            }
            val match = categories.firstOrNull { cat ->
                val catRaw = cat.title.trim()
                val catLoc = localizeMoodTitle(catRaw, s)
                val enLoc = localizeMoodTitle(catRaw, EnglishSonataStrings)
                catRaw.equals(cleanTitle, ignoreCase = true) ||
                    catRaw.equals(displayTitle, ignoreCase = true) ||
                    catLoc.equals(cleanTitle, ignoreCase = true) ||
                    catLoc.equals(displayTitle, ignoreCase = true) ||
                    enLoc.equals(cleanTitle, ignoreCase = true) ||
                    (resolvedBrowseId.isNotBlank() && cat.browseId == resolvedBrowseId)
            }
            if (match != null) {
                if (resolvedBrowseId.isBlank()) resolvedBrowseId = match.browseId
                if (resolvedParams.isBlank()) resolvedParams = match.params
            }
        }

        if (resolvedBrowseId.isNotBlank()) {
            val rawShelves = runSuspendCatching {
                engine.fetchMoodCategory(resolvedBrowseId, resolvedParams)
            }.onFailure { err ->
                handleRateLimitIfNeeded(err)
            }.getOrDefault(emptyList())
            if (!isActive) return@launch
            val cleaned = SonataViewModel.normalizeAndMergeHomeShelves(rawShelves)
            if (cleaned.isNotEmpty()) {
                _uiState.update {
                    it.copy(
                        moodCategoryTitle = displayTitle,
                        moodCategoryShelves = cleaned,
                        moodCategoryLoading = false,
                        status = displayTitle,
                    )
                }
                return@launch
            }
        }

        if (!isActive) return@launch
        _uiState.update {
            it.copy(
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
            )
        }
        search("$cleanTitle music")
    }
}

internal fun SonataViewModel.closeMoodCategoryImpl() {
    moodCategoryJob?.cancel()
    _uiState.update {
        it.copy(
            moodCategoryTitle = null,
            moodCategoryShelves = emptyList(),
            moodCategoryLoading = false,
        )
    }
}

internal fun SonataViewModel.openCardImpl(card: AndroidHomeCard, shelfTracks: List<AndroidTrack> = emptyList()) {
    when (card.kind) {
        AndroidHomeCardKind.SONG, AndroidHomeCardKind.VIDEO -> {
            card.track?.let { track ->
                val queueSource = shelfTracks.ifEmpty { listOf(track) }
                playTrack(track, queueSource)
            }
        }
        AndroidHomeCardKind.MOOD -> openMoodCategoryImpl(card.title, card.id)
        AndroidHomeCardKind.PLAYLIST, AndroidHomeCardKind.ALBUM -> {
            val kind = if (card.kind == AndroidHomeCardKind.ALBUM) "Album" else "Playlist"
            val year = SonataViewModel.YEAR_REGEX.find(card.subtitle)?.value
            openCollectionByIdImpl(
                id = card.id,
                title = card.title,
                coverUrl = card.thumbnailUrl,
                subtitle = card.subtitle.ifBlank { null },
                kindLabel = kind,
                year = year,
            )
        }
        AndroidHomeCardKind.ARTIST -> openArtistImpl(card.id, card.title)
    }
}

internal fun SonataViewModel.openCollectionByIdImpl(
    id: String,
    title: String,
    coverUrl: String? = null,
    subtitle: String? = null,
    kindLabel: String? = null,
    year: String? = null,
) {
    val s = uiStrings()
    speculativePrefetchJob?.cancel()
    if (id == SonataViewModel.DOWNLOADS_PLAYLIST_ID) {
        val latestDownloadCover = downloads.value.maxByOrNull { it.downloadedAtMs }?.coverUrl?.takeIf { it.isNotBlank() }
        val offlineTracks = downloads.value.map { downloadedRecordToAndroidTrack(it) }
        val resolvedCover = coverUrl ?: latestDownloadCover ?: offlineTracks.firstOrNull { t -> !t.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl
        syncCollectionMetadataToSavedAndPinnedImpl(
            id = id,
            title = s.downloads,
            subtitle = s.offlineSongsCount(offlineTracks.size),
            coverUrl = resolvedCover,
        )
        _uiState.update {
            it.copy(
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                collectionBrowseId = id,
                collectionTitle = s.downloads,
                collectionCoverUrl = resolvedCover,
                collectionSubtitle = s.offlineSongsCount(offlineTracks.size),
                collectionKindLabel = s.offlineLibrary,
                collectionYear = null,
                collectionTracks = offlineTracks,
                loading = false,
                status = s.downloadedSongsCount(offlineTracks.size),
            )
        }
        return
    }
    if (id.startsWith(PERSONAL_PLAYLIST_ID_PREFIX)) {
        val cachedPl = _uiState.value.personalPlaylists.firstOrNull { it.id == id }
        if (cachedPl != null) {
            val pTracks = cachedPl.tracks
            val srcLabel = sourcesStrings(preferences.state.value.appLanguage).personalPlaylistsTitle
            _uiState.update {
                it.copy(
                    artistPage = null,
                    artistLoading = false,
                    moodCategoryTitle = null,
                    moodCategoryShelves = emptyList(),
                    moodCategoryLoading = false,
                    collectionBrowseId = id,
                    collectionTitle = cachedPl.name,
                    collectionCoverUrl = coverUrl ?: pTracks.firstOrNull { t -> !t.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
                    collectionSubtitle = s.offlinePlaylistSongsCount(pTracks.size),
                    collectionKindLabel = kindLabel ?: srcLabel,
                    collectionYear = year,
                    collectionTracks = pTracks,
                    loading = false,
                    status = s.tracksInCollectionStatus(pTracks.size, cachedPl.name),
                )
            }
            return
        }
    }
    if (id.startsWith(SonataViewModel.LOCAL_PLAYLIST_PREFIX)) {
        val localTracks = getLocalPlaylistTracksImpl(id)
        _uiState.update {
            it.copy(
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                collectionBrowseId = id,
                collectionTitle = title,
                collectionCoverUrl = coverUrl ?: localTracks.firstOrNull { t -> !t.thumbnailUrl.isNullOrBlank() }?.thumbnailUrl,
                collectionSubtitle = s.offlinePlaylistSongsCount(localTracks.size),
                collectionKindLabel = kindLabel ?: s.offlinePlaylist,
                collectionYear = year,
                collectionTracks = localTracks,
                loading = false,
                status = s.tracksInCollectionStatus(localTracks.size, title),
            )
        }
        return
    }
    if (id.startsWith("local:album:")) {
        val alb = _uiState.value.localMusicLibrary.albums.firstOrNull { it.id == id }
        val albTracks = alb?.tracks?.map { it.toAndroidTrack() }
            ?: _uiState.value.localMusicLibrary.tracks
                .filter { it.toAndroidTrack().albumId == id }
                .map { it.toAndroidTrack() }
        val resolvedTitle = alb?.title ?: title
        val resolvedCover = coverUrl ?: alb?.albumArtUri ?: albTracks.firstOrNull()?.thumbnailUrl
        _uiState.update {
            it.copy(
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                collectionBrowseId = id,
                collectionTitle = resolvedTitle,
                collectionCoverUrl = resolvedCover,
                collectionSubtitle = subtitle ?: alb?.artist ?: s.albumLabel,
                collectionKindLabel = kindLabel ?: s.albumLabel,
                collectionYear = year,
                collectionTracks = albTracks,
                loading = false,
                status = s.tracksInCollectionStatus(albTracks.size, resolvedTitle),
            )
        }
        return
    }
    collectionJob?.cancel()
    collectionJob = viewModelScope.launch(Dispatchers.IO) {
        val engine = requireEngine()
        _uiState.update {
            it.copy(
                artistPage = null,
                artistLoading = false,
                moodCategoryTitle = null,
                moodCategoryShelves = emptyList(),
                moodCategoryLoading = false,
                collectionBrowseId = id,
                collectionTitle = title,
                collectionCoverUrl = coverUrl,
                collectionSubtitle = subtitle,
                collectionKindLabel = kindLabel,
                collectionYear = year,
                collectionTracks = emptyList(),
                loading = true,
                status = s.loadingNamedStatus(title)
            )
        }
        runSuspendCatching {
            if (id.startsWith("smb:")) {
                val srvId = id.removePrefix("smb:").substringBefore(':')
                val srv = _uiState.value.servers.firstOrNull { it.id == srvId }
                if (srv != null) dev.sonata.android.data.SmbClient.getAlbumTracks(srv, id)
                else engine.getServerPlaylistTracks(id)
            } else if (isSelfHostedId(id)) {
                engine.getServerPlaylistTracks(id)
            } else {
                engine.fetchCollectionTracksFresh(id)
            }
        }
            .onSuccess { rawTracks ->
                val firstLiveTrackCover = rawTracks.firstNotNullOfOrNull { resolveTrackCoverUrlImpl(it) }
                val fallbackCover = if (isLikedMusicPlaylistIdImpl(id)) {
                    firstLiveTrackCover ?: coverUrl
                } else {
                    coverUrl ?: firstLiveTrackCover
                }
                val tracks = SonataViewModel.backfillCollectionTracks(
                    rawTracks = rawTracks,
                    id = id,
                    title = title,
                    subtitle = subtitle,
                    coverUrl = fallbackCover,
                )
                if (isLikedMusicPlaylistIdImpl(id)) {
                    likedTracksCache = tracks
                    val likedIds = tracks.mapNotNull { t -> t.id.takeIf { it.isNotBlank() } }
                    if (likedIds.isNotEmpty()) {
                        _uiState.update { state ->
                            val mergedLiked = state.likedTrackIds + likedIds
                            if (mergedLiked == state.likedTrackIds) state else state.copy(likedTrackIds = mergedLiked)
                        }
                    }
                    updateLatestLikedCoverImpl(fallbackCover, clearIfNull = tracks.isEmpty())
                }
                autoPlaylistTracksCache[id] = tracks
                if (trackCollectionContext.size > 512) {
                    trackCollectionContext.clear()
                }
                tracks.forEach { t ->
                    if (t.id.isNotBlank()) trackCollectionContext[t.id] = id
                }
                val resolvedHeaderCover = if (isLikedMusicPlaylistIdImpl(id)) {
                    fallbackCover ?: _uiState.value.collectionCoverUrl
                } else {
                    _uiState.value.collectionCoverUrl ?: fallbackCover
                }
                syncCollectionMetadataToSavedAndPinnedImpl(
                    id = id,
                    title = title,
                    subtitle = subtitle,
                    coverUrl = resolvedHeaderCover,
                )
                _uiState.update { state ->
                    state.copy(
                        loading = false,
                        collectionBrowseId = id,
                        collectionTitle = title,
                        collectionCoverUrl = resolvedHeaderCover,
                        collectionTracks = tracks,
                        status = s.tracksInCollectionStatus(tracks.size, title)
                    )
                }
                prefetchStreams(tracks.take(2), delayMs = 1_500L)
            }
            .onFailure { err ->
                _uiState.update {
                    it.copy(loading = false, status = s.collectionErrorStatus(err.message.orEmpty()))
                }
            }
    }
}
