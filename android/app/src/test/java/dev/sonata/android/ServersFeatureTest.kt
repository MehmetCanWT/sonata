package dev.sonata.android

import dev.sonata.android.data.AppLanguage
import dev.sonata.android.ui.components.isSaveableExternalCollection
import dev.sonata.android.ui.i18n.EnglishServerStrings
import dev.sonata.android.ui.i18n.RussianServerStrings
import dev.sonata.android.ui.i18n.TurkishServerStrings
import dev.sonata.android.ui.i18n.resolveSonataStrings
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sonata_android.AndroidHeader
import uniffi.sonata_android.AndroidHomeCard
import uniffi.sonata_android.AndroidHomeCardKind
import uniffi.sonata_android.AndroidPlaylist
import uniffi.sonata_android.AndroidServerAlbum
import uniffi.sonata_android.AndroidServerKind
import uniffi.sonata_android.AndroidServerLibrary
import uniffi.sonata_android.AndroidServerProfile
import uniffi.sonata_android.AndroidServerSearchResult
import uniffi.sonata_android.AndroidServerStream
import uniffi.sonata_android.AndroidTrack
import uniffi.sonata_android.AndroidWebDavListing

class ServersFeatureTest {

    @Test
    fun `fake gateway supports server CRUD, connection test, browse, search, and stream headers`() = runTest {
        val gateway = FakeSonataEngineGateway()

        val initial = gateway.listServers()
        assertTrue(initial.isEmpty())

        val jf = AndroidServerProfile(
            id = "",
            name = "Home Jellyfin",
            kind = AndroidServerKind.JELLYFIN,
            baseUrl = "https://jf.internal",
            enabled = true,
            username = "alice",
            password = "secret",
            apiKey = null,
            accessToken = "tok-123",
            userId = "uid-1",
            bearerToken = null,
            libraryIds = emptyList(),
        )
        val afterAdd = gateway.upsertServer(jf)
        assertEquals(1, afterAdd.size)
        val jfId = afterAdd.first().id
        assertTrue(jfId.startsWith("jf-"))

        assertEquals("JELLYFIN OK", gateway.testServerConnection(afterAdd.first()))

        val disabled = gateway.setServerEnabled(jfId, false)
        assertFalse(disabled.first().enabled)

        gateway.serverLibrariesHandler = {
            listOf(AndroidServerLibrary(id = "lib-1", name = "Lossless Music", kind = "music"))
        }
        gateway.serverPlaylistsHandler = {
            listOf(AndroidPlaylist(id = "jellyfin:$jfId:pl-1", name = "Late Night", thumbnailUrl = null, isOwned = false))
        }
        gateway.serverAlbumHandler = { albId ->
            AndroidServerAlbum(
                id = albId,
                title = "Random Access Memories",
                subtitle = "Daft Punk",
                year = "2013",
                coverUrl = null,
                tracks = listOf(
                    AndroidTrack(
                        id = "jellyfin:$jfId:trk-1",
                        title = "Giorgio by Moroder",
                        artist = "Daft Punk",
                        album = "Random Access Memories",
                        durationSecs = 544u,
                        thumbnailUrl = null,
                        liked = false,
                        albumId = albId,
                        isExplicit = false,
                    ),
                ),
            )
        }
        gateway.allServersSearchHandler = { q, _ ->
            AndroidServerSearchResult(
                tracks = listOf(
                    AndroidTrack(
                        id = "subsonic:sub-1:s-1",
                        title = "Result for $q",
                        artist = "Artist",
                        album = "Album",
                        durationSecs = 180u,
                        thumbnailUrl = null,
                        liked = false,
                    ),
                ),
                albums = listOf(
                    AndroidHomeCard(
                        kind = AndroidHomeCardKind.ALBUM,
                        id = "jellyfin:$jfId:alb-1",
                        title = "Album $q",
                        subtitle = "Artist",
                        thumbnailUrl = null,
                        track = null,
                    ),
                ),
                artists = emptyList(),
                playlists = emptyList(),
            )
        }
        gateway.serverStreamHandler = { trk ->
            AndroidServerStream(
                url = "https://jf.internal/Audio/${trk.id}/stream?static=true",
                headers = listOf(
                    AndroidHeader(name = "X-Emby-Token", value = "tok-123"),
                    AndroidHeader(name = "Authorization", value = "MediaBrowser Token=\"tok-123\""),
                ),
                mimeType = "audio/flac",
                bitrateKbps = 1024u,
                codec = "flac",
                audioBadge = "FLAC · 1024kbps",
            )
        }
        gateway.webdavBrowseHandler = { _, _ ->
            AndroidWebDavListing(
                folders = listOf(AndroidServerLibrary(id = "webdav:dav-1:/Rock", name = "Rock", kind = "folder")),
                tracks = emptyList(),
            )
        }

        assertEquals(1, gateway.listServerLibraries(jfId).size)
        assertEquals(1, gateway.listServerPlaylists(jfId).size)
        val album = gateway.getServerAlbum("jellyfin:$jfId:alb-1")
        assertEquals("Random Access Memories", album.title)
        assertEquals(1, album.tracks.size)

        val searchRes = gateway.searchAllServers("Daft", 20u)
        assertEquals(1, searchRes.tracks.size)
        assertEquals(1, searchRes.albums.size)

        val stream = gateway.resolveServerStream(album.tracks.first())
        assertEquals("FLAC · 1024kbps", stream.audioBadge)
        assertTrue(stream.headers.any { it.name == "X-Emby-Token" && it.value == "tok-123" })

        val webdavRoot = gateway.browseWebdavFolder("dav-1", null)
        assertEquals("Rock", webdavRoot.folders.first().name)

        val afterDelete = gateway.deleteServer(jfId)
        assertTrue(afterDelete.isEmpty())
    }

    @Test
    fun `self-hosted track and collection IDs route cleanly and keep headers isolated`() {
        assertTrue(isSelfHostedId("jellyfin:srv1:item1"))
        assertTrue(isSelfHostedId("subsonic:srv2:item2"))
        assertTrue(isSelfHostedId("webdav:srv3:/Music/track.flac"))
        assertFalse(isSelfHostedId("dQw4w9WgXcQ"))
        assertFalse(isSelfHostedId("jiosaavn:abc"))
        assertFalse(isSelfHostedId("local:123"))
        assertFalse(isSelfHostedId(null))

        assertEquals("JFN", selfHostedKindTag("jellyfin:srv1:item1"))
        assertEquals("SUB", selfHostedKindTag("subsonic:srv2:item2"))
        assertEquals("DAV", selfHostedKindTag("webdav:srv3:/Music/track.flac"))
        assertNull(selfHostedKindTag("dQw4w9WgXcQ"))

        assertFalse(isSaveableExternalCollection("jellyfin:srv1:alb1", isOwned = false))
        assertFalse(isSaveableExternalCollection("subsonic:srv2:pl1", isOwned = false))
        assertFalse(isSaveableExternalCollection("webdav:srv3:/Music", isOwned = false))
        assertTrue(isSaveableExternalCollection("MPREb_12345", isOwned = false))

        PlaybackAudioBridge.clearStreamHeaders()
        PlaybackAudioBridge.registerStreamHeaders(
            url = "https://jf.internal/Audio/123/stream?static=true",
            headers = mapOf(
                "X-Emby-Token" to "secret-token",
                "Authorization" to "MediaBrowser Token=\"secret-token\"",
            ),
        )
        val resolvedHeaders = PlaybackAudioBridge.headersForStreamUrl(
            "https://jf.internal/Audio/123/stream?static=true",
        )
        assertEquals("secret-token", resolvedHeaders["X-Emby-Token"])
        assertEquals("MediaBrowser Token=\"secret-token\"", resolvedHeaders["Authorization"])
        assertTrue(PlaybackAudioBridge.headersForStreamUrl("https://other.example/audio").isEmpty())

        PlaybackAudioBridge.clearStreamHeaders()
        assertTrue(
            PlaybackAudioBridge.headersForStreamUrl("https://jf.internal/Audio/123/stream?static=true").isEmpty(),
        )
    }

    @Test
    fun `server localization strings are complete and distinct across en, tr, and ru`() {
        val en = resolveSonataStrings(AppLanguage.ENGLISH).servers
        val tr = resolveSonataStrings(AppLanguage.TURKISH).servers
        val ru = resolveSonataStrings(AppLanguage.RUSSIAN).servers

        assertEquals(EnglishServerStrings, en)
        assertEquals(TurkishServerStrings, tr)
        assertEquals(RussianServerStrings, ru)

        listOf(en, tr, ru).forEach { s ->
            assertTrue(s.selfHostedServers.isNotBlank())
            assertTrue(s.manageServers.isNotBlank())
            assertTrue(s.manageServersSummary.isNotBlank())
            assertTrue(s.addServer.isNotBlank())
            assertTrue(s.editServer.isNotBlank())
            assertTrue(s.deleteServer.isNotBlank())
            assertTrue(s.testConnection.isNotBlank())
            assertTrue(s.saveServer.isNotBlank())
            assertTrue(s.kindJellyfin.isNotBlank())
            assertTrue(s.kindSubsonic.isNotBlank())
            assertTrue(s.kindWebDav.isNotBlank())
            assertTrue(s.footerServers.isNotBlank())
            assertTrue(s.enabledCountSummary(1, 2).isNotBlank())
            assertTrue(s.connectionOk("OK").contains("OK"))
            assertTrue(s.connectionFailed("err").contains("err"))
            assertTrue(s.deleteServerConfirmBody("NAS").contains("NAS"))
            assertTrue(s.serverSaved("NAS").contains("NAS"))
            assertTrue(s.serverDeleted("NAS").contains("NAS"))
            assertTrue(s.serverTracksCount(3).contains("3"))
        }

        assertFalse(en.selfHostedServers == tr.selfHostedServers)
        assertFalse(en.selfHostedServers == ru.selfHostedServers)
        assertFalse(tr.selfHostedServers == ru.selfHostedServers)
    }
}
