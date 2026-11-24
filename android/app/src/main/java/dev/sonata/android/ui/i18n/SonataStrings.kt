package dev.sonata.android.ui.i18n

import androidx.compose.runtime.staticCompositionLocalOf
import dev.sonata.android.data.AppLanguage
import java.util.Locale

data class CommonStrings(
    val tabHome: String,
    val tabExplore: String,
    val tabRadio: String,
    val tabLibrary: String,
    val tabSearch: String,
    val play: String,
    val pause: String,
    val shuffle: String,
    val radio: String,
    val save: String,
    val saved: String,
    val download: String,
    val downloaded: String,
    val share: String,
    val close: String,
    val cancel: String,
    val back: String,
    val clear: String,
    val done: String,
    val retry: String,
    val refresh: String,
    val seeAll: String,
    val signIn: String,
    val signOut: String,
    val account: String,
    val remove: String,
    val create: String,
    val ok: String,
    val reset: String,
    val off: String,
    val homeHeading: String,
    val exploreHeading: String,
    val radioHeading: String,
    val libraryHeading: String,
    val searchPlaceholder: String,
    val yourLibrary: String,
    val signedInStatus: String,
    val notSignedInStatus: String,
    val signInToSyncLibrary: String,
    val noHistoryYet: String,
    val playSongsToBuildHistory: String,
    val searchEmptyTitle: String,
    val searchEmptySubtitle: String,
    val noResultsFound: String,
    val allTab: String,
    val noDownloadsYet: String,
    val noDownloadsHint: String,
    val playAll: String,
    val downloadAll: String,
    val removeDownload: String,
    val removeAllDownloads: String,
    val topSongs: String,
    val releases: String,
    val fansMightLike: String,
    val aboutArtist: String,
    val subscribe: String,
    val subscribed: String,
    val addedArtistToast: (String) -> String,
    val removedArtistToast: (String) -> String,
    val emptyCollection: String,
    val localFolderEmpty: String,
    val newPlaylist: String,
    val createPlaylist: String,
    val createNewPlaylistHint: String,
    val noPlaylistsYet: String,
    val addingTracksToPlaylist: (Int) -> String,
    val addedToPlaylistStatus: (String) -> String,
    val editPlaylist: String,
    val playlistName: String,
    val playlistDescription: String,
    val playlistPrivacy: String,
    val privacyKeep: String,
    val privacyPrivate: String,
    val privacyUnlisted: String,
    val privacyPublic: String,
    val changeCover: String,
    val coverSelected: String,
    val saving: String,
    val editPlaylistHint: String,
    val updateAvailable: String,
    val downloadNow: String,
    val installNow: String,
    val later: String,
    val downloadingApk: String,
    val offlineLibrary: String,
    val offlinePlaylist: String,
    val artistDefault: String,
    val nothingToUpdate: String,
    val signInToEditPlaylists: String,
    val selectAtLeastOnePlaylist: String,
    val playlistNameEmpty: String,
    val noYtmMatchFound: String,
    val failedUpdatePlaylist: String,
    val failedCreatePlaylist: String,
    val failedUploadCover: String,
    val updatedPlaylistAndCover: String,
    val updatedPlaylistCover: String,
    val updatedPlaylist: String,
    val addingToPlaylistStatus: String,
    val creatingPlaylistStatus: (String) -> String,
    val createdPlaylistStatus: (String) -> String,
    val createdOfflinePlaylistStatus: (String) -> String,
    val addedSingleTrackToPlaylists: (String, Int) -> String,
    val addedTracksToPlaylistsCount: (Int, Int) -> String,
    val addedToSomePlaylists: (Int, Int) -> String,
    val couldNotAddToPlaylist: String,
    val noTracksInCollection: (String) -> String,
    val loadingNamedStatus: (String) -> String,
    val openingNamedStatus: (String) -> String,
    val tracksInCollectionStatus: (Int, String) -> String,
    val offlineSongsCount: (Int) -> String,
    val offlinePlaylistSongsCount: (Int) -> String,
    val downloadedSongsCount: (Int) -> String,
    val pinned: String,
    val pinToLibrary: String,
    val unpinFromLibrary: String,
    val addToLibrary: String,
    val removeFromLibrary: String,
    val savedInLibrary: String,
    val pinnedStatus: (String) -> String,
    val unpinnedStatus: (String) -> String,
    val addedToLibraryStatus: (String) -> String,
    val removedFromLibraryStatus: (String) -> String,
)

data class FeedStrings(
    val topPicksForYou: String,
    val lastPlayed: String,
    val quickPicks: String,
    val recentSearches: String,
    val listeningHistory: String,
    val downloads: String,
    val playlists: String,
    val likedSongs: String,
    val albums: String,
    val artists: String,
    val songs: String,
    val samples: String,
    val videos: String,
    val signInBannerTitle: String,
    val signInBannerSubtitle: String,
    val recentSearchesSubtitle: String,
    val quickControls: String,
    val refreshHomeMixes: String,
    val curatedForYou: String,
    val discoveryStation: String,
    val myStation: String,
    val userStationSuffix: (String) -> String,
    val artistMixDefaultTitle: String,
    val artistMixFeaturing: (String) -> String,
    val artistMixDefaultSubtitle: String,
    val newReleaseEyebrow: String,
    val featuredPlaylistEyebrow: String,
    val featuredChartEyebrow: String,
    val updatedPlaylistEyebrow: String,
    val findYourMood: String,
    val yourPlaylists: String,
    val playlistLabel: String,
    val albumLabel: String,
    val singleLabel: String,
    val artistLabel: String,
    val trackLabel: String,
    val radioMixEyebrow: String,
    val featuredMixes: String,
    val featuredStation: String,
    val artistAndTrackStations: String,
    val moodAndGenreStations: String,
    val artistRadioTitle: (String) -> String,
    val basedOnTrack: (String) -> String,
    val trendingViralHits: String,
    val playlistsAndAlbums: String,
    val exploreMore: String,
    val recentlyPlayedFallback: String,
    val allMoodsAndGenres: String,
    val forgottenFavorites: String,
    val recommendedAlbums: String,
    val newReleases: String,
    val trending: String,
    val charts: String,
    val similarArtists: String,
    val fromYourLibrary: String,
    val mixedForYou: String,
    val musicVideos: String,
    val moodRelax: String,
    val moodWorkout: String,
    val moodEnergize: String,
    val moodFocus: String,
    val moodCommute: String,
    val moodParty: String,
    val moodSleep: String,
    val moodFeelGood: String,
    val moodRomance: String,
    val moodSad: String,
    val moodChill: String,
    val moodPodcasts: String,
    val featuredPlaylistsForYou: String,
    val fromTheCommunity: String,
    val singlesAndEps: String,
    val singles: String,
    val newReleasesForYou: String,
    val coversAndRemixes: String,
    val livePerformances: String,
    val featuredOn: String,
    val latestRelease: String,
    val albumsAndSingles: String,
    val similarToPrefix: (String) -> String,
    val moreFromPrefix: (String) -> String,
    val becauseYouListenedToPrefix: (String) -> String,
    val recentlyPlayedSubtitle: String,
    val personalizedShelvesCount: (Int) -> String,
    val searchingQueryStatus: (String) -> String,
    val searchResultsHeader: (String) -> String,
    val songsFoundCount: (Int) -> String,
    val songsFoundYtmJioCount: (Int) -> String,
    val songsFoundJioCount: (Int) -> String,
    val searchErrorStatus: (String) -> String,
    val homeErrorStatus: (String) -> String,
    val artistErrorStatus: (String) -> String,
    val collectionErrorStatus: (String) -> String,
)

data class PlayerStrings(
    val audioPipeline: String,
    val audioPipelineSubtitle: String,
    val autoplay: String,
    val autoplayOn: String,
    val autoplayOff: String,
    val shuffleOn: String,
    val shuffleOff: String,
    val repeatOff: String,
    val repeatAll: String,
    val repeatOne: String,
    val loadingYoutubeMusic: String,
    val startingRadio: String,
    val radioUnavailable: String,
    val startingPersonalMix: String,
    val startingDiscoveryMix: String,
    val sleepTimerPaused: String,
    val resolving: String,
    val offlineLabel: String,
    val liveStream: String,
    val noTrackPlaying: String,
    val playingTrackStatus: (String) -> String,
    val playingTrackOfflineStatus: (String) -> String,
    val resolvingTrackStatus: (String) -> String,
    val addedToPlayNextStatus: (Int) -> String,
    val addedToQueueStatus: (Int) -> String,
    val playingNextStatus: (String) -> String,
    val addedTrackToQueueStatus: (String) -> String,
    val mediaCacheClearedStatus: String,
    val imageCacheClearedStatus: String,
    val updateCheckFailed: String,
    val offlinePlaybackError: (String) -> String,
    val lyrics: String,
    val queue: String,
    val upNext: String,
    val loadingLyrics: String,
    val noLyricsAvailable: String,
    val lyricsTranslation: String,
    val favorite: String,
    val options: String,
    val previous: String,
    val next: String,
    val tracksCount: (Int) -> String,
    val savedCount: (Int) -> String,
    val wordSynced: String,
    val syncedLabel: String,
    val unavailableLabel: String,
    val lyricsDisplay: String,
    val lyricsTiming: String,
    val syncOffset: String,
    val translateLyrics: String,
    val romanizeLyrics: String,
    val sourceLabel: (String) -> String,
    val playNext: String,
    val addToQueue: String,
    val addToPlaylist: String,
    val startRadio: String,
    val goToAlbum: String,
    val goToArtist: String,
    val songInfo: String,
    val like: String,
    val unlike: String,
    val dislike: String,
    val removeFromCache: String,
    val playCollection: String,
    val shufflePlay: String,
    val openAlbum: String,
    val openPlaylist: String,
    val sleepTimer: String,
    val sleepOff: String,
    val sleepEndOfTrack: String,
    val sleepMinutesRemaining: (Int) -> String,
    val audioControls: String,
    val playbackSpeed: String,
    val skipSilence: String,
    val shareCardTitle: String,
    val copyLink: String,
    val shareMore: String,
    val linkCopied: String,
    val audioPipelineCodec: String,
    val audioPipelineBitrate: String,
    val audioPipelineSampleRate: String,
    val audioPipelineOutput: String,
    val audioPipelineSource: String,
    val audioPipelineMode: String,
    val audioPipelineBitTransparent: String,
    val openSignInPrompt: String,
    val wifiOnlyDownloadBlocked: String,
    val alreadyDownloadedNamed: (String) -> String,
    val alreadyDownloadedShort: String,
    val downloadingNamed: (String) -> String,
    val downloadFailedStreamUnavailable: String,
    val downloadFailedShort: String,
    val downloadFailedNamed: (String) -> String,
    val downloadedNamed: (String) -> String,
    val allTracksAlreadyDownloaded: (Int) -> String,
    val allTracksAlreadyDownloadedShort: String,
    val downloadingTracksCount: (Int) -> String,
    val downloadedCountOfTotal: (Int, Int) -> String,
    val downloadsClearedStatus: String,
    val removedFromCacheStatus: String,
    val signingInYtmStatus: String,
    val signedInAsStatus: (String) -> String,
    val signedInYtmStatus: String,
    val signInFailedStatus: (String) -> String,
    val signedOutStatus: String,
    val engineInitErrorStatus: (String) -> String,
    val recoveringStreamStatus: String,
    val playbackStoppedStatus: (String) -> String,
    val playbackErrorStatus: (String) -> String,
    val playFailedStatus: (String) -> String,
    val streamBadgeError: String,
    val updateOpeningInstaller: String,
    val updatePermissionRequired: String,
    val updateNoApkAsset: String,
    val updateStartingDownload: String,
    val updateInstallingVersion: (String) -> String,
    val updateApkMissingRetry: String,
    val musicMode: String,
    val videoMode: String,
    val videoQualityWarningTitle: String,
    val videoQualityWarningBody: String,
    val doNotShowAgain: String,
    val switchToVideo: String,
    val videoQuality: String,
    val videoQualityAuto: String,
    val fullscreen: String,
    val exitFullscreen: String,
    val loadingVideo: String,
    val videoUnavailableStatus: String,
)

data class SettingsStrings(
    val settingsTitle: String,
    val settingsSubtitle: String,
    val searchSettingsPlaceholder: String,
    val searchNoMatch: (String) -> String,
    val sectionAccount: String,
    val sectionAudioQuality: String,
    val sectionTheme: String,
    val sectionLanguage: String,
    val sectionGlassMotion: String,
    val sectionPlayerCovers: String,
    val catDownloads: String,
    val catPlayback: String,
    val catContent: String,
    val catAppearance: String,
    val catPerformance: String,
    val catStorage: String,
    val catYourData: String,
    val catMisc: String,
    val catAccountUpdates: String,
    val streamingQuality: String,
    val downloadQuality: String,
    val jioSaavnFallback: String,
    val jioSaavnFallbackDesc: String,
    val playbackSummary: String,
    val contentSummary: String,
    val performanceSummary: String,
    val storageSummary: String,
    val yourDataSummary: String,
    val miscSummary: String,
    val accountUpdatesSummary: String,
    val designTheme: String,
    val colorAppearance: String,
    val appLanguage: String,
    val appLanguageDesc: String,
    val langSystem: String,
    val langEnglish: String,
    val langTurkish: String,
    val langRussian: String,
    val langSystemDesc: String,
    val langEnglishDesc: String,
    val langTurkishDesc: String,
    val langRussianDesc: String,
    val themeModeSystem: String,
    val themeModeLight: String,
    val themeModeDark: String,
    val themeModeAmoled: String,
    val liquidGlass: String,
    val liquidGlassDesc: String,
    val liquidGlassApiHint: String,
    val glassPerformanceMode: String,
    val glassPerformanceDesc: String,
    val reduceDynamicBlur: String,
    val reduceDynamicBlurDesc: String,
    val reduceAnimation: String,
    val reduceAnimationDesc: String,
    val fullScreenCoverArt: String,
    val fullScreenCoverArtDesc: String,
    val animatedArtwork: String,
    val animatedArtworkDesc: String,
    val animatedArtworkCellular: String,
    val animatedArtworkCellularDesc: String,
    val artworkTintedPages: String,
    val artworkTintedPagesDesc: String,
    val fullScreenDetailCover: String,
    val fullScreenDetailCoverDesc: String,
    val showTotalDuration: String,
    val showTotalDurationDesc: String,
    val playerBackdrop: String,
    val hdrQualitySources: String,
    val footerQualitySources: String,
    val includeJioSaavnDownloads: String,
    val includeJioSaavnDownloadsDesc: String,
    val wifiOnlyDownloads: String,
    val wifiOnlyDownloadsDesc: String,
    val autoDownloadLiked: String,
    val autoDownloadLikedDesc: String,
    val hdrVideoDownloads: String,
    val footerVideoDownloads: String,
    val downloadVideosIfAvailable: String,
    val downloadVideosIfAvailableDesc: String,
    val videoDownloadQuality: String,
    val videoDl1080Title: String,
    val videoDl1080Desc: String,
    val videoDl1080Size: String,
    val videoDl720Title: String,
    val videoDl720Desc: String,
    val videoDl720Size: String,
    val videoDl480Title: String,
    val videoDl480Desc: String,
    val videoDl480Size: String,
    val videoDl360Title: String,
    val videoDl360Desc: String,
    val videoDl360Size: String,
    val downloadingVideoStatus: String,
    val hdrStorageLocation: String,
    val footerStorageLocation: String,
    val downloadLocation: String,
    val storageLocationInternal: String,
    val storageLocationExternal: String,
    val storageInternalDesc: String,
    val storageExternalDesc: String,
    val storageInternalLabel: String,
    val storageExternalLabel: String,
    val downloadedStorage: String,
    val removeAllDownloadsConfirmTitle: String,
    val removeAllDownloadsConfirmBody: (Int, String) -> String,
    val deleteAllOfflineSongs: (Int) -> String,
    val hdrQueueTransitions: String,
    val autoplayDesc: String,
    val crossfade: String,
    val crossfadeSecondsOverlap: (Int) -> String,
    val durationLabel: String,
    val skipSilenceDesc: String,
    val sleepTimerFadeOut: String,
    val sleepTimerFadeOutDesc: String,
    val hdrSpeedAndTiming: String,
    val hdrHomeFeed: String,
    val footerHomeFeed: String,
    val hideRecents: String,
    val hideRecentsDesc: String,
    val hideTopPicks: String,
    val hideTopPicksDesc: String,
    val hideHomeMoods: String,
    val hideHomeMoodsDesc: String,
    val hideHomeVideos: String,
    val hideHomeVideosDesc: String,
    val squareVideoThumbnails: String,
    val squareVideoThumbnailsDesc: String,
    val hideLyricsPreview: String,
    val hideLyricsPreviewDesc: String,
    val blurUnfocusedLyrics: String,
    val blurUnfocusedLyricsDesc: String,
    val wordByWordSync: String,
    val wordByWordSyncDesc: String,
    val translateLyricsDesc: String,
    val translationLanguage: String,
    val romanizeLyricsDesc: String,
    val globalLyricOffset: String,
    val footerAppearanceTheme: String,
    val hdrDisplayRendering: String,
    val footerDisplayRendering: String,
    val refreshRateHigh: String,
    val refreshRateDesc: String,
    val hdrStorageCache: String,
    val downloadedSongs: String,
    val clearAudioCache: String,
    val clearAudioCacheDesc: String,
    val clearImageCache: String,
    val clearImageCacheDesc: String,
    val hdrBackupRestore: String,
    val footerBackupRestore: String,
    val exportBackup: String,
    val exportBackupDesc: String,
    val importBackup: String,
    val importBackupDesc: String,
    val hdrPlayerSystem: String,
    val hideStatusBarPlayer: String,
    val hideStatusBarPlayerDesc: String,
    val hideQualityBadge: String,
    val hideQualityBadgeDesc: String,
    val stopMusicOnClose: String,
    val stopMusicOnCloseDesc: String,
    val showNerdStats: String,
    val showNerdStatsDesc: String,
    val hdrYtmAccount: String,
    val connectedStatus: String,
    val ytmSessionActive: String,
    val signInYtmSettingsSubtitle: String,
    val signOutDesc: String,
    val signInYtmTitle: String,
    val signInYtmDesc: String,
    val hdrAboutUpdates: String,
    val footerAboutUpdates: String,
    val sonataVersion: String,
    val downloadUpdateVersion: (String) -> String,
    val openReleaseApk: String,
    val checkForUpdates: String,
    val checkingForUpdates: String,
    val compareGithubReleases: String,
    val footerStreamingQuality: String,
    val hdrOrderedHighestToSmallest: String,
    val footerDownloadQuality: String,
    val qualityLossless: String,
    val qualityHigh: String,
    val qualityMedium: String,
    val qualityLow: String,
    val qualityLosslessDetail: String,
    val qualityHighDetail: String,
    val qualityMediumDetail: String,
    val qualityLowDetail: String,
    val qualityLosslessDesc: String,
    val qualityHighDesc: String,
    val qualityMediumDesc: String,
    val qualityLowDesc: String,
    val dlHighest: String,
    val dlHigh: String,
    val dlStandard: String,
    val dlLow: String,
    val themeVioletDesc: String,
    val themeMoonflowerDesc: String,
    val themeBasicDesc: String,
    val backdropMesh: String,
    val backdropWaves: String,
    val backdropBlurred: String,
    val backdropMeshDesc: String,
    val backdropWavesDesc: String,
    val backdropBlurredDesc: String,
    val loudnessNormalization: String,
    val loudnessNormalizationDesc: String,
    val equalizerTitle: String,
    val eqPresetFlat: String,
    val eqPresetBassBoost: String,
    val eqPresetVocal: String,
    val eqPresetAcoustic: String,
    val eqPresetElectronic: String,
    val eqPresetLateNight: String,
    val eqPresetCustom: String,
    val storageFreeOf: (String, String) -> String,
)

data class DiscordStrings(
    val catDiscord: String,
    val discordSummary: String,
    val hdrDiscordAccount: String,
    val hdrDiscordPresence: String,
    val hdrDiscordAppId: String,
    val footerDiscordAppId: String,
    val discordSignInTitle: String,
    val discordSignInSubtitle: String,
    val discordSignInCompleting: String,
    val discordSignInAction: String,
    val discordSignInActionDesc: String,
    val discordSignOutAction: String,
    val discordSignOutDesc: String,
    val discordConsentTitle: String,
    val discordConsentBody: String,
    val discordConsentContinue: String,
    val discordEnableRpc: String,
    val discordEnableRpcDesc: String,
    val discordActivityType: String,
    val discordActivityListening: String,
    val discordActivityPlaying: String,
    val discordActivityWatching: String,
    val discordActivityCompeting: String,
    val discordShowTimestamps: String,
    val discordShowTimestampsDesc: String,
    val discordShowArtwork: String,
    val discordShowArtworkDesc: String,
    val discordShowButtons: String,
    val discordShowButtonsDesc: String,
    val discordAppIdLabel: String,
    val discordAppIdPlaceholder: String,
    val discordAppIdApply: String,
    val discordAppIdReset: String,
    val discordAppIdInvalid: String,
    val discordStatusLabel: String,
    val discordStatusConnected: String,
    val discordStatusConnectedAs: (String) -> String,
    val discordStatusConnecting: String,
    val discordStatusReconnecting: String,
    val discordStatusDisconnected: String,
    val discordStatusAuthFailed: String,
    val discordStatusError: (String) -> String,
    val discordReconnect: String,
)

/**
 * UI string table shared across all Sonata Android screens, menus, dialogs, and settings.
 *
 * Split across five sub-records to stay well below the JVM 255-parameter constructor limit
 * while keeping flat property accessors for every UI caller.
 */
data class SonataStrings(
    val common: CommonStrings,
    val feed: FeedStrings,
    val player: PlayerStrings,
    val settings: SettingsStrings,
    val discord: DiscordStrings,
    val servers: ServerStrings = EnglishServerStrings,
) {
    val tabHome: String get() = common.tabHome
    val tabExplore: String get() = common.tabExplore
    val tabRadio: String get() = common.tabRadio
    val tabLibrary: String get() = common.tabLibrary
    val tabSearch: String get() = common.tabSearch
    val play: String get() = common.play
    val pause: String get() = common.pause
    val shuffle: String get() = common.shuffle
    val radio: String get() = common.radio
    val save: String get() = common.save
    val saved: String get() = common.saved
    val download: String get() = common.download
    val downloaded: String get() = common.downloaded
    val share: String get() = common.share
    val close: String get() = common.close
    val cancel: String get() = common.cancel
    val back: String get() = common.back
    val clear: String get() = common.clear
    val done: String get() = common.done
    val retry: String get() = common.retry
    val refresh: String get() = common.refresh
    val seeAll: String get() = common.seeAll
    val signIn: String get() = common.signIn
    val signOut: String get() = common.signOut
    val account: String get() = common.account
    val remove: String get() = common.remove
    val create: String get() = common.create
    val ok: String get() = common.ok
    val reset: String get() = common.reset
    val off: String get() = common.off
    val homeHeading: String get() = common.homeHeading
    val exploreHeading: String get() = common.exploreHeading
    val radioHeading: String get() = common.radioHeading
    val libraryHeading: String get() = common.libraryHeading
    val searchPlaceholder: String get() = common.searchPlaceholder
    val yourLibrary: String get() = common.yourLibrary
    val signedInStatus: String get() = common.signedInStatus
    val notSignedInStatus: String get() = common.notSignedInStatus
    val signInToSyncLibrary: String get() = common.signInToSyncLibrary
    val noHistoryYet: String get() = common.noHistoryYet
    val playSongsToBuildHistory: String get() = common.playSongsToBuildHistory
    val searchEmptyTitle: String get() = common.searchEmptyTitle
    val searchEmptySubtitle: String get() = common.searchEmptySubtitle
    val noResultsFound: String get() = common.noResultsFound
    val allTab: String get() = common.allTab
    val noDownloadsYet: String get() = common.noDownloadsYet
    val noDownloadsHint: String get() = common.noDownloadsHint
    val playAll: String get() = common.playAll
    val downloadAll: String get() = common.downloadAll
    val removeDownload: String get() = common.removeDownload
    val removeAllDownloads: String get() = common.removeAllDownloads
    val topSongs: String get() = common.topSongs
    val releases: String get() = common.releases
    val fansMightLike: String get() = common.fansMightLike
    val aboutArtist: String get() = common.aboutArtist
    val subscribe: String get() = common.subscribe
    val subscribed: String get() = common.subscribed
    val addedArtistToast: (String) -> String get() = common.addedArtistToast
    val removedArtistToast: (String) -> String get() = common.removedArtistToast
    val emptyCollection: String get() = common.emptyCollection
    val localFolderEmpty: String get() = common.localFolderEmpty
    val newPlaylist: String get() = common.newPlaylist
    val createPlaylist: String get() = common.createPlaylist
    val createNewPlaylistHint: String get() = common.createNewPlaylistHint
    val noPlaylistsYet: String get() = common.noPlaylistsYet
    val addingTracksToPlaylist: (Int) -> String get() = common.addingTracksToPlaylist
    val addedToPlaylistStatus: (String) -> String get() = common.addedToPlaylistStatus
    val editPlaylist: String get() = common.editPlaylist
    val playlistName: String get() = common.playlistName
    val playlistDescription: String get() = common.playlistDescription
    val playlistPrivacy: String get() = common.playlistPrivacy
    val privacyKeep: String get() = common.privacyKeep
    val privacyPrivate: String get() = common.privacyPrivate
    val privacyUnlisted: String get() = common.privacyUnlisted
    val privacyPublic: String get() = common.privacyPublic
    val changeCover: String get() = common.changeCover
    val coverSelected: String get() = common.coverSelected
    val saving: String get() = common.saving
    val editPlaylistHint: String get() = common.editPlaylistHint
    val updateAvailable: String get() = common.updateAvailable
    val downloadNow: String get() = common.downloadNow
    val installNow: String get() = common.installNow
    val later: String get() = common.later
    val downloadingApk: String get() = common.downloadingApk
    val offlineLibrary: String get() = common.offlineLibrary
    val offlinePlaylist: String get() = common.offlinePlaylist
    val artistDefault: String get() = common.artistDefault
    val nothingToUpdate: String get() = common.nothingToUpdate
    val signInToEditPlaylists: String get() = common.signInToEditPlaylists
    val selectAtLeastOnePlaylist: String get() = common.selectAtLeastOnePlaylist
    val playlistNameEmpty: String get() = common.playlistNameEmpty
    val noYtmMatchFound: String get() = common.noYtmMatchFound
    val failedUpdatePlaylist: String get() = common.failedUpdatePlaylist
    val failedCreatePlaylist: String get() = common.failedCreatePlaylist
    val failedUploadCover: String get() = common.failedUploadCover
    val updatedPlaylistAndCover: String get() = common.updatedPlaylistAndCover
    val updatedPlaylistCover: String get() = common.updatedPlaylistCover
    val updatedPlaylist: String get() = common.updatedPlaylist
    val addingToPlaylistStatus: String get() = common.addingToPlaylistStatus
    val creatingPlaylistStatus: (String) -> String get() = common.creatingPlaylistStatus
    val createdPlaylistStatus: (String) -> String get() = common.createdPlaylistStatus
    val createdOfflinePlaylistStatus: (String) -> String get() = common.createdOfflinePlaylistStatus
    val addedSingleTrackToPlaylists: (String, Int) -> String get() = common.addedSingleTrackToPlaylists
    val addedTracksToPlaylistsCount: (Int, Int) -> String get() = common.addedTracksToPlaylistsCount
    val addedToSomePlaylists: (Int, Int) -> String get() = common.addedToSomePlaylists
    val couldNotAddToPlaylist: String get() = common.couldNotAddToPlaylist
    val noTracksInCollection: (String) -> String get() = common.noTracksInCollection
    val loadingNamedStatus: (String) -> String get() = common.loadingNamedStatus
    val openingNamedStatus: (String) -> String get() = common.openingNamedStatus
    val tracksInCollectionStatus: (Int, String) -> String get() = common.tracksInCollectionStatus
    val offlineSongsCount: (Int) -> String get() = common.offlineSongsCount
    val offlinePlaylistSongsCount: (Int) -> String get() = common.offlinePlaylistSongsCount
    val downloadedSongsCount: (Int) -> String get() = common.downloadedSongsCount
    val pinned: String get() = common.pinned
    val pinToLibrary: String get() = common.pinToLibrary
    val unpinFromLibrary: String get() = common.unpinFromLibrary
    val addToLibrary: String get() = common.addToLibrary
    val removeFromLibrary: String get() = common.removeFromLibrary
    val savedInLibrary: String get() = common.savedInLibrary
    val pinnedStatus: (String) -> String get() = common.pinnedStatus
    val unpinnedStatus: (String) -> String get() = common.unpinnedStatus
    val addedToLibraryStatus: (String) -> String get() = common.addedToLibraryStatus
    val removedFromLibraryStatus: (String) -> String get() = common.removedFromLibraryStatus

    val topPicksForYou: String get() = feed.topPicksForYou
    val lastPlayed: String get() = feed.lastPlayed
    val quickPicks: String get() = feed.quickPicks
    val recentSearches: String get() = feed.recentSearches
    val listeningHistory: String get() = feed.listeningHistory
    val downloads: String get() = feed.downloads
    val playlists: String get() = feed.playlists
    val likedSongs: String get() = feed.likedSongs
    val albums: String get() = feed.albums
    val artists: String get() = feed.artists
    val songs: String get() = feed.songs
    val samples: String get() = feed.samples
    val videos: String get() = feed.videos
    val signInBannerTitle: String get() = feed.signInBannerTitle
    val signInBannerSubtitle: String get() = feed.signInBannerSubtitle
    val recentSearchesSubtitle: String get() = feed.recentSearchesSubtitle
    val quickControls: String get() = feed.quickControls
    val refreshHomeMixes: String get() = feed.refreshHomeMixes
    val curatedForYou: String get() = feed.curatedForYou
    val discoveryStation: String get() = feed.discoveryStation
    val myStation: String get() = feed.myStation
    val userStationSuffix: (String) -> String get() = feed.userStationSuffix
    val artistMixDefaultTitle: String get() = feed.artistMixDefaultTitle
    val artistMixFeaturing: (String) -> String get() = feed.artistMixFeaturing
    val artistMixDefaultSubtitle: String get() = feed.artistMixDefaultSubtitle
    val newReleaseEyebrow: String get() = feed.newReleaseEyebrow
    val featuredPlaylistEyebrow: String get() = feed.featuredPlaylistEyebrow
    val featuredChartEyebrow: String get() = feed.featuredChartEyebrow
    val updatedPlaylistEyebrow: String get() = feed.updatedPlaylistEyebrow
    val findYourMood: String get() = feed.findYourMood
    val yourPlaylists: String get() = feed.yourPlaylists
    val playlistLabel: String get() = feed.playlistLabel
    val albumLabel: String get() = feed.albumLabel
    val singleLabel: String get() = feed.singleLabel
    val artistLabel: String get() = feed.artistLabel
    val trackLabel: String get() = feed.trackLabel
    val radioMixEyebrow: String get() = feed.radioMixEyebrow
    val featuredMixes: String get() = feed.featuredMixes
    val featuredStation: String get() = feed.featuredStation
    val artistAndTrackStations: String get() = feed.artistAndTrackStations
    val moodAndGenreStations: String get() = feed.moodAndGenreStations
    val artistRadioTitle: (String) -> String get() = feed.artistRadioTitle
    val basedOnTrack: (String) -> String get() = feed.basedOnTrack
    val trendingViralHits: String get() = feed.trendingViralHits
    val playlistsAndAlbums: String get() = feed.playlistsAndAlbums
    val exploreMore: String get() = feed.exploreMore
    val recentlyPlayedFallback: String get() = feed.recentlyPlayedFallback
    val allMoodsAndGenres: String get() = feed.allMoodsAndGenres
    val forgottenFavorites: String get() = feed.forgottenFavorites
    val recommendedAlbums: String get() = feed.recommendedAlbums
    val newReleases: String get() = feed.newReleases
    val trending: String get() = feed.trending
    val charts: String get() = feed.charts
    val similarArtists: String get() = feed.similarArtists
    val fromYourLibrary: String get() = feed.fromYourLibrary
    val mixedForYou: String get() = feed.mixedForYou
    val musicVideos: String get() = feed.musicVideos
    val moodRelax: String get() = feed.moodRelax
    val moodWorkout: String get() = feed.moodWorkout
    val moodEnergize: String get() = feed.moodEnergize
    val moodFocus: String get() = feed.moodFocus
    val moodCommute: String get() = feed.moodCommute
    val moodParty: String get() = feed.moodParty
    val moodSleep: String get() = feed.moodSleep
    val moodFeelGood: String get() = feed.moodFeelGood
    val moodRomance: String get() = feed.moodRomance
    val moodSad: String get() = feed.moodSad
    val moodChill: String get() = feed.moodChill
    val moodPodcasts: String get() = feed.moodPodcasts
    val featuredPlaylistsForYou: String get() = feed.featuredPlaylistsForYou
    val fromTheCommunity: String get() = feed.fromTheCommunity
    val singlesAndEps: String get() = feed.singlesAndEps
    val singles: String get() = feed.singles
    val newReleasesForYou: String get() = feed.newReleasesForYou
    val coversAndRemixes: String get() = feed.coversAndRemixes
    val livePerformances: String get() = feed.livePerformances
    val featuredOn: String get() = feed.featuredOn
    val latestRelease: String get() = feed.latestRelease
    val albumsAndSingles: String get() = feed.albumsAndSingles
    val similarToPrefix: (String) -> String get() = feed.similarToPrefix
    val moreFromPrefix: (String) -> String get() = feed.moreFromPrefix
    val becauseYouListenedToPrefix: (String) -> String get() = feed.becauseYouListenedToPrefix
    val recentlyPlayedSubtitle: String get() = feed.recentlyPlayedSubtitle
    val personalizedShelvesCount: (Int) -> String get() = feed.personalizedShelvesCount
    val searchingQueryStatus: (String) -> String get() = feed.searchingQueryStatus
    val searchResultsHeader: (String) -> String get() = feed.searchResultsHeader
    val songsFoundCount: (Int) -> String get() = feed.songsFoundCount
    val songsFoundYtmJioCount: (Int) -> String get() = feed.songsFoundYtmJioCount
    val songsFoundJioCount: (Int) -> String get() = feed.songsFoundJioCount
    val searchErrorStatus: (String) -> String get() = feed.searchErrorStatus
    val homeErrorStatus: (String) -> String get() = feed.homeErrorStatus
    val artistErrorStatus: (String) -> String get() = feed.artistErrorStatus
    val collectionErrorStatus: (String) -> String get() = feed.collectionErrorStatus

    val audioPipeline: String get() = player.audioPipeline
    val audioPipelineSubtitle: String get() = player.audioPipelineSubtitle
    val autoplay: String get() = player.autoplay
    val autoplayOn: String get() = player.autoplayOn
    val autoplayOff: String get() = player.autoplayOff
    val shuffleOn: String get() = player.shuffleOn
    val shuffleOff: String get() = player.shuffleOff
    val repeatOff: String get() = player.repeatOff
    val repeatAll: String get() = player.repeatAll
    val repeatOne: String get() = player.repeatOne
    val loadingYoutubeMusic: String get() = player.loadingYoutubeMusic
    val startingRadio: String get() = player.startingRadio
    val radioUnavailable: String get() = player.radioUnavailable
    val startingPersonalMix: String get() = player.startingPersonalMix
    val startingDiscoveryMix: String get() = player.startingDiscoveryMix
    val sleepTimerPaused: String get() = player.sleepTimerPaused
    val resolving: String get() = player.resolving
    val offlineLabel: String get() = player.offlineLabel
    val liveStream: String get() = player.liveStream
    val noTrackPlaying: String get() = player.noTrackPlaying
    val playingTrackStatus: (String) -> String get() = player.playingTrackStatus
    val playingTrackOfflineStatus: (String) -> String get() = player.playingTrackOfflineStatus
    val resolvingTrackStatus: (String) -> String get() = player.resolvingTrackStatus
    val addedToPlayNextStatus: (Int) -> String get() = player.addedToPlayNextStatus
    val addedToQueueStatus: (Int) -> String get() = player.addedToQueueStatus
    val playingNextStatus: (String) -> String get() = player.playingNextStatus
    val addedTrackToQueueStatus: (String) -> String get() = player.addedTrackToQueueStatus
    val mediaCacheClearedStatus: String get() = player.mediaCacheClearedStatus
    val imageCacheClearedStatus: String get() = player.imageCacheClearedStatus
    val updateCheckFailed: String get() = player.updateCheckFailed
    val offlinePlaybackError: (String) -> String get() = player.offlinePlaybackError
    val lyrics: String get() = player.lyrics
    val queue: String get() = player.queue
    val upNext: String get() = player.upNext
    val loadingLyrics: String get() = player.loadingLyrics
    val noLyricsAvailable: String get() = player.noLyricsAvailable
    val lyricsTranslation: String get() = player.lyricsTranslation
    val favorite: String get() = player.favorite
    val options: String get() = player.options
    val previous: String get() = player.previous
    val next: String get() = player.next
    val tracksCount: (Int) -> String get() = player.tracksCount
    val savedCount: (Int) -> String get() = player.savedCount
    val wordSynced: String get() = player.wordSynced
    val syncedLabel: String get() = player.syncedLabel
    val unavailableLabel: String get() = player.unavailableLabel
    val lyricsDisplay: String get() = player.lyricsDisplay
    val lyricsTiming: String get() = player.lyricsTiming
    val syncOffset: String get() = player.syncOffset
    val translateLyrics: String get() = player.translateLyrics
    val romanizeLyrics: String get() = player.romanizeLyrics
    val sourceLabel: (String) -> String get() = player.sourceLabel
    val playNext: String get() = player.playNext
    val addToQueue: String get() = player.addToQueue
    val addToPlaylist: String get() = player.addToPlaylist
    val startRadio: String get() = player.startRadio
    val goToAlbum: String get() = player.goToAlbum
    val goToArtist: String get() = player.goToArtist
    val songInfo: String get() = player.songInfo
    val like: String get() = player.like
    val unlike: String get() = player.unlike
    val dislike: String get() = player.dislike
    val removeFromCache: String get() = player.removeFromCache
    val playCollection: String get() = player.playCollection
    val shufflePlay: String get() = player.shufflePlay
    val openAlbum: String get() = player.openAlbum
    val openPlaylist: String get() = player.openPlaylist
    val sleepTimer: String get() = player.sleepTimer
    val sleepOff: String get() = player.sleepOff
    val sleepEndOfTrack: String get() = player.sleepEndOfTrack
    val sleepMinutesRemaining: (Int) -> String get() = player.sleepMinutesRemaining
    val audioControls: String get() = player.audioControls
    val playbackSpeed: String get() = player.playbackSpeed
    val skipSilence: String get() = player.skipSilence
    val shareCardTitle: String get() = player.shareCardTitle
    val copyLink: String get() = player.copyLink
    val shareMore: String get() = player.shareMore
    val linkCopied: String get() = player.linkCopied
    val audioPipelineCodec: String get() = player.audioPipelineCodec
    val audioPipelineBitrate: String get() = player.audioPipelineBitrate
    val audioPipelineSampleRate: String get() = player.audioPipelineSampleRate
    val audioPipelineOutput: String get() = player.audioPipelineOutput
    val audioPipelineSource: String get() = player.audioPipelineSource
    val audioPipelineMode: String get() = player.audioPipelineMode
    val audioPipelineBitTransparent: String get() = player.audioPipelineBitTransparent
    val openSignInPrompt: String get() = player.openSignInPrompt
    val wifiOnlyDownloadBlocked: String get() = player.wifiOnlyDownloadBlocked
    val alreadyDownloadedNamed: (String) -> String get() = player.alreadyDownloadedNamed
    val alreadyDownloadedShort: String get() = player.alreadyDownloadedShort
    val downloadingNamed: (String) -> String get() = player.downloadingNamed
    val downloadFailedStreamUnavailable: String get() = player.downloadFailedStreamUnavailable
    val downloadFailedShort: String get() = player.downloadFailedShort
    val downloadFailedNamed: (String) -> String get() = player.downloadFailedNamed
    val downloadedNamed: (String) -> String get() = player.downloadedNamed
    val allTracksAlreadyDownloaded: (Int) -> String get() = player.allTracksAlreadyDownloaded
    val allTracksAlreadyDownloadedShort: String get() = player.allTracksAlreadyDownloadedShort
    val downloadingTracksCount: (Int) -> String get() = player.downloadingTracksCount
    val downloadedCountOfTotal: (Int, Int) -> String get() = player.downloadedCountOfTotal
    val downloadsClearedStatus: String get() = player.downloadsClearedStatus
    val removedFromCacheStatus: String get() = player.removedFromCacheStatus
    val signingInYtmStatus: String get() = player.signingInYtmStatus
    val signedInAsStatus: (String) -> String get() = player.signedInAsStatus
    val signedInYtmStatus: String get() = player.signedInYtmStatus
    val signInFailedStatus: (String) -> String get() = player.signInFailedStatus
    val signedOutStatus: String get() = player.signedOutStatus
    val engineInitErrorStatus: (String) -> String get() = player.engineInitErrorStatus
    val recoveringStreamStatus: String get() = player.recoveringStreamStatus
    val playbackStoppedStatus: (String) -> String get() = player.playbackStoppedStatus
    val playbackErrorStatus: (String) -> String get() = player.playbackErrorStatus
    val playFailedStatus: (String) -> String get() = player.playFailedStatus
    val streamBadgeError: String get() = player.streamBadgeError
    val updateOpeningInstaller: String get() = player.updateOpeningInstaller
    val updatePermissionRequired: String get() = player.updatePermissionRequired
    val updateNoApkAsset: String get() = player.updateNoApkAsset
    val updateStartingDownload: String get() = player.updateStartingDownload
    val updateInstallingVersion: (String) -> String get() = player.updateInstallingVersion
    val updateApkMissingRetry: String get() = player.updateApkMissingRetry
    val musicMode: String get() = player.musicMode
    val videoMode: String get() = player.videoMode
    val videoQualityWarningTitle: String get() = player.videoQualityWarningTitle
    val videoQualityWarningBody: String get() = player.videoQualityWarningBody
    val doNotShowAgain: String get() = player.doNotShowAgain
    val switchToVideo: String get() = player.switchToVideo
    val videoQuality: String get() = player.videoQuality
    val videoQualityAuto: String get() = player.videoQualityAuto
    val fullscreen: String get() = player.fullscreen
    val exitFullscreen: String get() = player.exitFullscreen
    val loadingVideo: String get() = player.loadingVideo
    val videoUnavailableStatus: String get() = player.videoUnavailableStatus

    val settingsTitle: String get() = settings.settingsTitle
    val settingsSubtitle: String get() = settings.settingsSubtitle
    val searchSettingsPlaceholder: String get() = settings.searchSettingsPlaceholder
    val searchNoMatch: (String) -> String get() = settings.searchNoMatch
    val sectionAccount: String get() = settings.sectionAccount
    val sectionAudioQuality: String get() = settings.sectionAudioQuality
    val sectionTheme: String get() = settings.sectionTheme
    val sectionLanguage: String get() = settings.sectionLanguage
    val sectionGlassMotion: String get() = settings.sectionGlassMotion
    val sectionPlayerCovers: String get() = settings.sectionPlayerCovers
    val catDownloads: String get() = settings.catDownloads
    val catPlayback: String get() = settings.catPlayback
    val catContent: String get() = settings.catContent
    val catAppearance: String get() = settings.catAppearance
    val catPerformance: String get() = settings.catPerformance
    val catStorage: String get() = settings.catStorage
    val catYourData: String get() = settings.catYourData
    val catMisc: String get() = settings.catMisc
    val catAccountUpdates: String get() = settings.catAccountUpdates
    val streamingQuality: String get() = settings.streamingQuality
    val downloadQuality: String get() = settings.downloadQuality
    val jioSaavnFallback: String get() = settings.jioSaavnFallback
    val jioSaavnFallbackDesc: String get() = settings.jioSaavnFallbackDesc
    val playbackSummary: String get() = settings.playbackSummary
    val contentSummary: String get() = settings.contentSummary
    val performanceSummary: String get() = settings.performanceSummary
    val storageSummary: String get() = settings.storageSummary
    val yourDataSummary: String get() = settings.yourDataSummary
    val miscSummary: String get() = settings.miscSummary
    val accountUpdatesSummary: String get() = settings.accountUpdatesSummary
    val designTheme: String get() = settings.designTheme
    val colorAppearance: String get() = settings.colorAppearance
    val appLanguage: String get() = settings.appLanguage
    val appLanguageDesc: String get() = settings.appLanguageDesc
    val langSystem: String get() = settings.langSystem
    val langEnglish: String get() = settings.langEnglish
    val langTurkish: String get() = settings.langTurkish
    val langRussian: String get() = settings.langRussian
    val langSystemDesc: String get() = settings.langSystemDesc
    val langEnglishDesc: String get() = settings.langEnglishDesc
    val langTurkishDesc: String get() = settings.langTurkishDesc
    val langRussianDesc: String get() = settings.langRussianDesc
    val themeModeSystem: String get() = settings.themeModeSystem
    val themeModeLight: String get() = settings.themeModeLight
    val themeModeDark: String get() = settings.themeModeDark
    val themeModeAmoled: String get() = settings.themeModeAmoled
    val liquidGlass: String get() = settings.liquidGlass
    val liquidGlassDesc: String get() = settings.liquidGlassDesc
    val liquidGlassApiHint: String get() = settings.liquidGlassApiHint
    val glassPerformanceMode: String get() = settings.glassPerformanceMode
    val glassPerformanceDesc: String get() = settings.glassPerformanceDesc
    val reduceDynamicBlur: String get() = settings.reduceDynamicBlur
    val reduceDynamicBlurDesc: String get() = settings.reduceDynamicBlurDesc
    val reduceAnimation: String get() = settings.reduceAnimation
    val reduceAnimationDesc: String get() = settings.reduceAnimationDesc
    val fullScreenCoverArt: String get() = settings.fullScreenCoverArt
    val fullScreenCoverArtDesc: String get() = settings.fullScreenCoverArtDesc
    val animatedArtwork: String get() = settings.animatedArtwork
    val animatedArtworkDesc: String get() = settings.animatedArtworkDesc
    val animatedArtworkCellular: String get() = settings.animatedArtworkCellular
    val animatedArtworkCellularDesc: String get() = settings.animatedArtworkCellularDesc
    val artworkTintedPages: String get() = settings.artworkTintedPages
    val artworkTintedPagesDesc: String get() = settings.artworkTintedPagesDesc
    val fullScreenDetailCover: String get() = settings.fullScreenDetailCover
    val fullScreenDetailCoverDesc: String get() = settings.fullScreenDetailCoverDesc
    val showTotalDuration: String get() = settings.showTotalDuration
    val showTotalDurationDesc: String get() = settings.showTotalDurationDesc
    val playerBackdrop: String get() = settings.playerBackdrop
    val hdrQualitySources: String get() = settings.hdrQualitySources
    val footerQualitySources: String get() = settings.footerQualitySources
    val includeJioSaavnDownloads: String get() = settings.includeJioSaavnDownloads
    val includeJioSaavnDownloadsDesc: String get() = settings.includeJioSaavnDownloadsDesc
    val wifiOnlyDownloads: String get() = settings.wifiOnlyDownloads
    val wifiOnlyDownloadsDesc: String get() = settings.wifiOnlyDownloadsDesc
    val autoDownloadLiked: String get() = settings.autoDownloadLiked
    val autoDownloadLikedDesc: String get() = settings.autoDownloadLikedDesc
    val hdrVideoDownloads: String get() = settings.hdrVideoDownloads
    val footerVideoDownloads: String get() = settings.footerVideoDownloads
    val downloadVideosIfAvailable: String get() = settings.downloadVideosIfAvailable
    val downloadVideosIfAvailableDesc: String get() = settings.downloadVideosIfAvailableDesc
    val videoDownloadQuality: String get() = settings.videoDownloadQuality
    val videoDl1080Title: String get() = settings.videoDl1080Title
    val videoDl1080Desc: String get() = settings.videoDl1080Desc
    val videoDl1080Size: String get() = settings.videoDl1080Size
    val videoDl720Title: String get() = settings.videoDl720Title
    val videoDl720Desc: String get() = settings.videoDl720Desc
    val videoDl720Size: String get() = settings.videoDl720Size
    val videoDl480Title: String get() = settings.videoDl480Title
    val videoDl480Desc: String get() = settings.videoDl480Desc
    val videoDl480Size: String get() = settings.videoDl480Size
    val videoDl360Title: String get() = settings.videoDl360Title
    val videoDl360Desc: String get() = settings.videoDl360Desc
    val videoDl360Size: String get() = settings.videoDl360Size
    val downloadingVideoStatus: String get() = settings.downloadingVideoStatus
    val hdrStorageLocation: String get() = settings.hdrStorageLocation
    val footerStorageLocation: String get() = settings.footerStorageLocation
    val downloadLocation: String get() = settings.downloadLocation
    val storageLocationInternal: String get() = settings.storageLocationInternal
    val storageLocationExternal: String get() = settings.storageLocationExternal
    val storageInternalDesc: String get() = settings.storageInternalDesc
    val storageExternalDesc: String get() = settings.storageExternalDesc
    val storageInternalLabel: String get() = settings.storageInternalLabel
    val storageExternalLabel: String get() = settings.storageExternalLabel
    val downloadedStorage: String get() = settings.downloadedStorage
    val removeAllDownloadsConfirmTitle: String get() = settings.removeAllDownloadsConfirmTitle
    val removeAllDownloadsConfirmBody: (Int, String) -> String get() = settings.removeAllDownloadsConfirmBody
    val deleteAllOfflineSongs: (Int) -> String get() = settings.deleteAllOfflineSongs
    val hdrQueueTransitions: String get() = settings.hdrQueueTransitions
    val autoplayDesc: String get() = settings.autoplayDesc
    val crossfade: String get() = settings.crossfade
    val crossfadeSecondsOverlap: (Int) -> String get() = settings.crossfadeSecondsOverlap
    val durationLabel: String get() = settings.durationLabel
    val skipSilenceDesc: String get() = settings.skipSilenceDesc
    val sleepTimerFadeOut: String get() = settings.sleepTimerFadeOut
    val sleepTimerFadeOutDesc: String get() = settings.sleepTimerFadeOutDesc
    val hdrSpeedAndTiming: String get() = settings.hdrSpeedAndTiming
    val hdrHomeFeed: String get() = settings.hdrHomeFeed
    val footerHomeFeed: String get() = settings.footerHomeFeed
    val hideRecents: String get() = settings.hideRecents
    val hideRecentsDesc: String get() = settings.hideRecentsDesc
    val hideTopPicks: String get() = settings.hideTopPicks
    val hideTopPicksDesc: String get() = settings.hideTopPicksDesc
    val hideHomeMoods: String get() = settings.hideHomeMoods
    val hideHomeMoodsDesc: String get() = settings.hideHomeMoodsDesc
    val hideHomeVideos: String get() = settings.hideHomeVideos
    val hideHomeVideosDesc: String get() = settings.hideHomeVideosDesc
    val squareVideoThumbnails: String get() = settings.squareVideoThumbnails
    val squareVideoThumbnailsDesc: String get() = settings.squareVideoThumbnailsDesc
    val hideLyricsPreview: String get() = settings.hideLyricsPreview
    val hideLyricsPreviewDesc: String get() = settings.hideLyricsPreviewDesc
    val blurUnfocusedLyrics: String get() = settings.blurUnfocusedLyrics
    val blurUnfocusedLyricsDesc: String get() = settings.blurUnfocusedLyricsDesc
    val wordByWordSync: String get() = settings.wordByWordSync
    val wordByWordSyncDesc: String get() = settings.wordByWordSyncDesc
    val translateLyricsDesc: String get() = settings.translateLyricsDesc
    val translationLanguage: String get() = settings.translationLanguage
    val romanizeLyricsDesc: String get() = settings.romanizeLyricsDesc
    val globalLyricOffset: String get() = settings.globalLyricOffset
    val footerAppearanceTheme: String get() = settings.footerAppearanceTheme
    val hdrDisplayRendering: String get() = settings.hdrDisplayRendering
    val footerDisplayRendering: String get() = settings.footerDisplayRendering
    val refreshRateHigh: String get() = settings.refreshRateHigh
    val refreshRateDesc: String get() = settings.refreshRateDesc
    val hdrStorageCache: String get() = settings.hdrStorageCache
    val downloadedSongs: String get() = settings.downloadedSongs
    val clearAudioCache: String get() = settings.clearAudioCache
    val clearAudioCacheDesc: String get() = settings.clearAudioCacheDesc
    val clearImageCache: String get() = settings.clearImageCache
    val clearImageCacheDesc: String get() = settings.clearImageCacheDesc
    val hdrBackupRestore: String get() = settings.hdrBackupRestore
    val footerBackupRestore: String get() = settings.footerBackupRestore
    val exportBackup: String get() = settings.exportBackup
    val exportBackupDesc: String get() = settings.exportBackupDesc
    val importBackup: String get() = settings.importBackup
    val importBackupDesc: String get() = settings.importBackupDesc
    val hdrPlayerSystem: String get() = settings.hdrPlayerSystem
    val hideStatusBarPlayer: String get() = settings.hideStatusBarPlayer
    val hideStatusBarPlayerDesc: String get() = settings.hideStatusBarPlayerDesc
    val hideQualityBadge: String get() = settings.hideQualityBadge
    val hideQualityBadgeDesc: String get() = settings.hideQualityBadgeDesc
    val stopMusicOnClose: String get() = settings.stopMusicOnClose
    val stopMusicOnCloseDesc: String get() = settings.stopMusicOnCloseDesc
    val showNerdStats: String get() = settings.showNerdStats
    val showNerdStatsDesc: String get() = settings.showNerdStatsDesc
    val hdrYtmAccount: String get() = settings.hdrYtmAccount
    val connectedStatus: String get() = settings.connectedStatus
    val ytmSessionActive: String get() = settings.ytmSessionActive
    val signInYtmSettingsSubtitle: String get() = settings.signInYtmSettingsSubtitle
    val signOutDesc: String get() = settings.signOutDesc
    val signInYtmTitle: String get() = settings.signInYtmTitle
    val signInYtmDesc: String get() = settings.signInYtmDesc
    val hdrAboutUpdates: String get() = settings.hdrAboutUpdates
    val footerAboutUpdates: String get() = settings.footerAboutUpdates
    val sonataVersion: String get() = settings.sonataVersion
    val downloadUpdateVersion: (String) -> String get() = settings.downloadUpdateVersion
    val openReleaseApk: String get() = settings.openReleaseApk
    val checkForUpdates: String get() = settings.checkForUpdates
    val checkingForUpdates: String get() = settings.checkingForUpdates
    val compareGithubReleases: String get() = settings.compareGithubReleases
    val footerStreamingQuality: String get() = settings.footerStreamingQuality
    val hdrOrderedHighestToSmallest: String get() = settings.hdrOrderedHighestToSmallest
    val footerDownloadQuality: String get() = settings.footerDownloadQuality
    val qualityLossless: String get() = settings.qualityLossless
    val qualityHigh: String get() = settings.qualityHigh
    val qualityMedium: String get() = settings.qualityMedium
    val qualityLow: String get() = settings.qualityLow
    val qualityLosslessDetail: String get() = settings.qualityLosslessDetail
    val qualityHighDetail: String get() = settings.qualityHighDetail
    val qualityMediumDetail: String get() = settings.qualityMediumDetail
    val qualityLowDetail: String get() = settings.qualityLowDetail
    val qualityLosslessDesc: String get() = settings.qualityLosslessDesc
    val qualityHighDesc: String get() = settings.qualityHighDesc
    val qualityMediumDesc: String get() = settings.qualityMediumDesc
    val qualityLowDesc: String get() = settings.qualityLowDesc
    val dlHighest: String get() = settings.dlHighest
    val dlHigh: String get() = settings.dlHigh
    val dlStandard: String get() = settings.dlStandard
    val dlLow: String get() = settings.dlLow
    val themeVioletDesc: String get() = settings.themeVioletDesc
    val themeMoonflowerDesc: String get() = settings.themeMoonflowerDesc
    val themeBasicDesc: String get() = settings.themeBasicDesc
    val backdropMesh: String get() = settings.backdropMesh
    val backdropWaves: String get() = settings.backdropWaves
    val backdropBlurred: String get() = settings.backdropBlurred
    val backdropMeshDesc: String get() = settings.backdropMeshDesc
    val backdropWavesDesc: String get() = settings.backdropWavesDesc
    val backdropBlurredDesc: String get() = settings.backdropBlurredDesc
    val loudnessNormalization: String get() = settings.loudnessNormalization
    val loudnessNormalizationDesc: String get() = settings.loudnessNormalizationDesc
    val equalizerTitle: String get() = settings.equalizerTitle
    val eqPresetFlat: String get() = settings.eqPresetFlat
    val eqPresetBassBoost: String get() = settings.eqPresetBassBoost
    val eqPresetVocal: String get() = settings.eqPresetVocal
    val eqPresetAcoustic: String get() = settings.eqPresetAcoustic
    val eqPresetElectronic: String get() = settings.eqPresetElectronic
    val eqPresetLateNight: String get() = settings.eqPresetLateNight
    val eqPresetCustom: String get() = settings.eqPresetCustom
    val storageFreeOf: (String, String) -> String get() = settings.storageFreeOf
    val catDiscord: String get() = discord.catDiscord
    val discordSummary: String get() = discord.discordSummary
    val hdrDiscordAccount: String get() = discord.hdrDiscordAccount
    val hdrDiscordPresence: String get() = discord.hdrDiscordPresence
    val hdrDiscordAppId: String get() = discord.hdrDiscordAppId
    val footerDiscordAppId: String get() = discord.footerDiscordAppId
    val discordSignInTitle: String get() = discord.discordSignInTitle
    val discordSignInSubtitle: String get() = discord.discordSignInSubtitle
    val discordSignInCompleting: String get() = discord.discordSignInCompleting
    val discordSignInAction: String get() = discord.discordSignInAction
    val discordSignInActionDesc: String get() = discord.discordSignInActionDesc
    val discordSignOutAction: String get() = discord.discordSignOutAction
    val discordSignOutDesc: String get() = discord.discordSignOutDesc
    val discordConsentTitle: String get() = discord.discordConsentTitle
    val discordConsentBody: String get() = discord.discordConsentBody
    val discordConsentContinue: String get() = discord.discordConsentContinue
    val discordEnableRpc: String get() = discord.discordEnableRpc
    val discordEnableRpcDesc: String get() = discord.discordEnableRpcDesc
    val discordActivityType: String get() = discord.discordActivityType
    val discordActivityListening: String get() = discord.discordActivityListening
    val discordActivityPlaying: String get() = discord.discordActivityPlaying
    val discordActivityWatching: String get() = discord.discordActivityWatching
    val discordActivityCompeting: String get() = discord.discordActivityCompeting
    val discordShowTimestamps: String get() = discord.discordShowTimestamps
    val discordShowTimestampsDesc: String get() = discord.discordShowTimestampsDesc
    val discordShowArtwork: String get() = discord.discordShowArtwork
    val discordShowArtworkDesc: String get() = discord.discordShowArtworkDesc
    val discordShowButtons: String get() = discord.discordShowButtons
    val discordShowButtonsDesc: String get() = discord.discordShowButtonsDesc
    val discordAppIdLabel: String get() = discord.discordAppIdLabel
    val discordAppIdPlaceholder: String get() = discord.discordAppIdPlaceholder
    val discordAppIdApply: String get() = discord.discordAppIdApply
    val discordAppIdReset: String get() = discord.discordAppIdReset
    val discordAppIdInvalid: String get() = discord.discordAppIdInvalid
    val discordStatusLabel: String get() = discord.discordStatusLabel
    val discordStatusConnected: String get() = discord.discordStatusConnected
    val discordStatusConnectedAs: (String) -> String get() = discord.discordStatusConnectedAs
    val discordStatusConnecting: String get() = discord.discordStatusConnecting
    val discordStatusReconnecting: String get() = discord.discordStatusReconnecting
    val discordStatusDisconnected: String get() = discord.discordStatusDisconnected
    val discordStatusAuthFailed: String get() = discord.discordStatusAuthFailed
    val discordStatusError: (String) -> String get() = discord.discordStatusError
    val discordReconnect: String get() = discord.discordReconnect
}

val EnglishSonataStrings = SonataStrings(
    common = CommonStrings(
        tabHome = "Home",
        tabExplore = "Explore",
        tabRadio = "Radio",
        tabLibrary = "Library",
        tabSearch = "Search",
        play = "Play",
        pause = "Pause",
        shuffle = "Shuffle",
        radio = "Radio",
        save = "Save",
        saved = "Saved",
        download = "Download",
        downloaded = "Downloaded",
        share = "Share",
        close = "Close",
        cancel = "Cancel",
        back = "Back",
        clear = "Clear",
        done = "Done",
        retry = "Retry",
        refresh = "Refresh",
        seeAll = "See all",
        signIn = "Sign In",
        signOut = "Sign Out",
        account = "Account",
        remove = "Remove",
        create = "Create",
        ok = "OK",
        reset = "Reset",
        off = "Off",
        homeHeading = "Home",
        exploreHeading = "Explore",
        radioHeading = "Radio",
        libraryHeading = "Library",
        searchPlaceholder = "Search songs, artists, albums...",
        yourLibrary = "Your Library",
        signedInStatus = "Signed In",
        notSignedInStatus = "Not Signed In",
        signInToSyncLibrary = "Sign in to YouTube Music to sync playlists and likes",
        noHistoryYet = "No listening history yet",
        playSongsToBuildHistory = "Songs you play will appear here",
        searchEmptyTitle = "Find Music",
        searchEmptySubtitle = "Search songs, albums, artists, and playlists",
        noResultsFound = "No results found",
        allTab = "All",
        noDownloadsYet = "No offline downloads yet",
        noDownloadsHint = "Download songs or playlists to listen offline",
        playAll = "Play All",
        downloadAll = "Download All",
        removeDownload = "Remove Download",
        removeAllDownloads = "Remove All Downloads",
        topSongs = "Top Songs",
        releases = "Releases",
        fansMightLike = "Fans Might Also Like",
        aboutArtist = "About",
        subscribe = "Subscribe",
        subscribed = "Subscribed",
        addedArtistToast = { name -> "Subscribed to $name" },
        removedArtistToast = { name -> "Removed $name" },
        emptyCollection = "No tracks found",
        localFolderEmpty = "Local folder is empty",
        newPlaylist = "New Playlist",
        createPlaylist = "Create Playlist",
        createNewPlaylistHint = "Create a new YouTube Music playlist",
        noPlaylistsYet = "No playlists yet",
        addingTracksToPlaylist = { n -> "Adding $n tracks to playlist" },
        addedToPlaylistStatus = { name -> "Added to $name" },
        editPlaylist = "Edit Playlist",
        playlistName = "Playlist name",
        playlistDescription = "Description (optional)",
        playlistPrivacy = "Privacy",
        privacyKeep = "Keep current",
        privacyPrivate = "Private",
        privacyUnlisted = "Unlisted",
        privacyPublic = "Public",
        changeCover = "Change Cover",
        coverSelected = "New cover selected",
        saving = "Saving...",
        editPlaylistHint = "Changes sync directly to your YouTube Music account.",
        updateAvailable = "Update Available",
        downloadNow = "Download Now",
        installNow = "Install Now",
        later = "Later",
        downloadingApk = "Downloading APK...",
        offlineLibrary = "Offline Library",
        offlinePlaylist = "Offline Playlist",
        artistDefault = "Artist",
        nothingToUpdate = "Nothing to update",
        signInToEditPlaylists = "Sign in to YouTube Music to edit playlists",
        selectAtLeastOnePlaylist = "Select at least one playlist",
        playlistNameEmpty = "Playlist name cannot be empty",
        noYtmMatchFound = "No YouTube Music track match found",
        failedUpdatePlaylist = "Failed to update playlist",
        failedCreatePlaylist = "Failed to create playlist",
        failedUploadCover = "Failed to upload playlist cover",
        updatedPlaylistAndCover = "Updated playlist & cover",
        updatedPlaylistCover = "Updated playlist cover",
        updatedPlaylist = "Updated playlist",
        addingToPlaylistStatus = "Adding to playlist...",
        creatingPlaylistStatus = { name -> "Creating $name..." },
        createdPlaylistStatus = { name -> "Created \"$name\"" },
        createdOfflinePlaylistStatus = { name -> "Created offline playlist \"$name\"" },
        addedSingleTrackToPlaylists = { title, count -> "Added $title to $count playlist(s)" },
        addedTracksToPlaylistsCount = { tracks, playlists -> "Added $tracks tracks to $playlists playlist(s)" },
        addedToSomePlaylists = { ok, total -> "Added to $ok of $total playlists" },
        couldNotAddToPlaylist = "Could not add to playlist",
        noTracksInCollection = { title -> "No tracks found in $title" },
        loadingNamedStatus = { title -> "Loading $title..." },
        openingNamedStatus = { title -> "Opening $title..." },
        tracksInCollectionStatus = { count, title -> "$count tracks in $title" },
        offlineSongsCount = { count -> "$count offline songs" },
        offlinePlaylistSongsCount = { count -> "$count offline playlist songs" },
        downloadedSongsCount = { count -> "$count downloaded songs" },
        pinned = "Pinned",
        pinToLibrary = "Pin",
        unpinFromLibrary = "Unpin",
        addToLibrary = "Add to Library",
        removeFromLibrary = "Remove from Library",
        savedInLibrary = "In Library",
        pinnedStatus = { title -> "Pinned \"$title\"" },
        unpinnedStatus = { title -> "Unpinned \"$title\"" },
        addedToLibraryStatus = { title -> "Added \"$title\" to Library" },
        removedFromLibraryStatus = { title -> "Removed \"$title\" from Library" },
    ),
    feed = FeedStrings(
        topPicksForYou = "Top Picks for You",
        lastPlayed = "Last Played",
        quickPicks = "Quick picks",
        recentSearches = "Recent searches",
        listeningHistory = "History",
        downloads = "Downloads",
        playlists = "Playlists",
        likedSongs = "Liked Songs",
        albums = "Albums",
        artists = "Artists",
        songs = "Songs",
        samples = "Samples",
        videos = "Videos",
        signInBannerTitle = "Sign in to YouTube Music",
        signInBannerSubtitle = "Sync playlists, likes, and Extreme audio quality",
        recentSearchesSubtitle = "Recent searches",
        quickControls = "Quick Controls",
        refreshHomeMixes = "Refresh Home & Mixes",
        curatedForYou = "CURATED FOR YOU",
        discoveryStation = "Discovery Station",
        myStation = "My Station",
        userStationSuffix = { name -> "$name's Station" },
        artistMixDefaultTitle = "Favorites Mix",
        artistMixFeaturing = { artist -> "Featuring $artist" },
        artistMixDefaultSubtitle = "Personalized artist mix",
        newReleaseEyebrow = "NEW RELEASE",
        featuredPlaylistEyebrow = "FEATURED PLAYLIST",
        featuredChartEyebrow = "FEATURED CHART",
        updatedPlaylistEyebrow = "UPDATED PLAYLIST",
        findYourMood = "Find Your Mood",
        yourPlaylists = "Your Playlists",
        playlistLabel = "Playlist",
        albumLabel = "Album",
        singleLabel = "Single",
        artistLabel = "Artist",
        trackLabel = "Track",
        radioMixEyebrow = "RADIO MIX",
        featuredMixes = "Featured Mixes",
        featuredStation = "Featured Station",
        artistAndTrackStations = "Artist & Track Stations",
        moodAndGenreStations = "Mood & Genre Stations",
        artistRadioTitle = { artist -> "$artist Radio" },
        basedOnTrack = { title -> "Based on $title" },
        trendingViralHits = "Trending & Viral Hits",
        playlistsAndAlbums = "Playlists & Albums",
        exploreMore = "Explore More",
        recentlyPlayedFallback = "Recently Played",
        allMoodsAndGenres = "Moods & Genres",
        forgottenFavorites = "Forgotten Favorites",
        recommendedAlbums = "Recommended Albums",
        newReleases = "New Releases",
        trending = "Trending",
        charts = "Charts",
        similarArtists = "Similar Artists",
        fromYourLibrary = "From Your Library",
        mixedForYou = "Mixed for You",
        musicVideos = "Music Videos",
        moodRelax = "Relax",
        moodWorkout = "Workout",
        moodEnergize = "Energize",
        moodFocus = "Focus",
        moodCommute = "Commute",
        moodParty = "Party",
        moodSleep = "Sleep",
        moodFeelGood = "Feel Good",
        moodRomance = "Romance",
        moodSad = "Sad",
        moodChill = "Chill",
        moodPodcasts = "Podcasts",
        featuredPlaylistsForYou = "Featured playlists for you",
        fromTheCommunity = "From the community",
        singlesAndEps = "Singles & EPs",
        singles = "Singles",
        newReleasesForYou = "New releases for you",
        coversAndRemixes = "Covers & remixes",
        livePerformances = "Live performances",
        featuredOn = "Featured on",
        latestRelease = "Latest Release",
        albumsAndSingles = "Albums & Singles",
        similarToPrefix = { x -> "Similar to $x" },
        moreFromPrefix = { x -> "More from $x" },
        becauseYouListenedToPrefix = { x -> "Because you listened to $x" },
        recentlyPlayedSubtitle = "Recently played",
        personalizedShelvesCount = { n -> "$n personalized shelves" },
        searchingQueryStatus = { q -> "Searching \"$q\"..." },
        searchResultsHeader = { q -> "Search: $q" },
        songsFoundCount = { n -> "$n songs found" },
        songsFoundYtmJioCount = { n -> "$n songs found (YTM + JioSaavn)" },
        songsFoundJioCount = { n -> "$n songs found (JioSaavn)" },
        searchErrorStatus = { msg -> "Search error: $msg" },
        homeErrorStatus = { msg -> "Home error: $msg" },
        artistErrorStatus = { msg -> "Artist error: $msg" },
        collectionErrorStatus = { msg -> "Collection error: $msg" },
    ),
    player = PlayerStrings(
        audioPipeline = "Audio Pipeline",
        audioPipelineSubtitle = "Codec & bit depth",
        autoplay = "Autoplay",
        autoplayOn = "Endless playback enabled",
        autoplayOff = "Endless playback disabled",
        shuffleOn = "Shuffle on",
        shuffleOff = "Shuffle off",
        repeatOff = "Repeat off",
        repeatAll = "Repeat all",
        repeatOne = "Repeat one",
        loadingYoutubeMusic = "Loading YouTube Music...",
        startingRadio = "Starting endless radio...",
        radioUnavailable = "Could not start radio right now",
        startingPersonalMix = "Starting personal mix...",
        startingDiscoveryMix = "Starting discovery mix...",
        sleepTimerPaused = "Sleep timer paused playback",
        resolving = "Resolving...",
        offlineLabel = "Offline",
        liveStream = "Live Stream",
        noTrackPlaying = "Nothing Playing",
        playingTrackStatus = { title -> "Playing $title" },
        playingTrackOfflineStatus = { title -> "Playing $title (Offline)" },
        resolvingTrackStatus = { title -> "Resolving $title..." },
        addedToPlayNextStatus = { n -> "Added $n tracks to Play Next" },
        addedToQueueStatus = { n -> "Added $n tracks to Queue" },
        playingNextStatus = { title -> "Playing next: $title" },
        addedTrackToQueueStatus = { title -> "Added $title to queue" },
        mediaCacheClearedStatus = "Media & lyrics cache cleared",
        imageCacheClearedStatus = "Image cache cleared",
        updateCheckFailed = "Update check failed",
        offlinePlaybackError = { msg -> "Offline playback error: $msg" },
        lyrics = "Lyrics",
        queue = "Queue",
        upNext = "Up Next",
        loadingLyrics = "Loading lyrics...",
        noLyricsAvailable = "No lyrics available",
        lyricsTranslation = "Lyrics Translation",
        favorite = "Favorite",
        options = "Options",
        previous = "Previous",
        next = "Next",
        tracksCount = { n -> "$n tracks" },
        savedCount = { n -> "$n saved" },
        wordSynced = "Word-Synced",
        syncedLabel = "Synced",
        unavailableLabel = "Unavailable",
        lyricsDisplay = "Lyrics Display",
        lyricsTiming = "Timing Offset",
        syncOffset = "Sync Offset",
        translateLyrics = "Translate Lyrics",
        romanizeLyrics = "Romanize Lyrics",
        sourceLabel = { src -> "Source: $src" },
        playNext = "Play Next",
        addToQueue = "Add to Queue",
        addToPlaylist = "Add to Playlist",
        startRadio = "Start Radio",
        goToAlbum = "Go to Album",
        goToArtist = "Go to Artist",
        songInfo = "Song Info",
        like = "Like",
        unlike = "Unlike",
        dislike = "Dislike",
        removeFromCache = "Remove Download",
        playCollection = "Play",
        shufflePlay = "Shuffle",
        openAlbum = "Open Album",
        openPlaylist = "Open Playlist",
        sleepTimer = "Sleep Timer",
        sleepOff = "Off",
        sleepEndOfTrack = "End of Track",
        sleepMinutesRemaining = { m -> "${m}m left" },
        audioControls = "Audio Controls",
        playbackSpeed = "Playback Speed",
        skipSilence = "Skip Silence",
        shareCardTitle = "Share",
        copyLink = "Copy Link",
        shareMore = "More",
        linkCopied = "Link copied",
        audioPipelineCodec = "Codec",
        audioPipelineBitrate = "Bitrate",
        audioPipelineSampleRate = "Sample Rate",
        audioPipelineOutput = "Output Path",
        audioPipelineSource = "Source",
        audioPipelineMode = "Audio Mode",
        audioPipelineBitTransparent = "Bit-transparent output (no resampling/DSP)",
        openSignInPrompt = "Sign in to YouTube Music",
        wifiOnlyDownloadBlocked = "Wi-Fi required for downloads (Wi-Fi only enabled)",
        alreadyDownloadedNamed = { title -> "Already downloaded: $title" },
        alreadyDownloadedShort = "Already downloaded",
        downloadingNamed = { title -> "Downloading $title..." },
        downloadFailedStreamUnavailable = "Download failed: stream unavailable",
        downloadFailedShort = "Download failed",
        downloadFailedNamed = { msg -> "Download failed: $msg" },
        downloadedNamed = { title -> "Downloaded $title" },
        allTracksAlreadyDownloaded = { count -> "All $count tracks already downloaded" },
        allTracksAlreadyDownloadedShort = "All tracks already downloaded",
        downloadingTracksCount = { count -> "Downloading $count tracks..." },
        downloadedCountOfTotal = { saved, total -> "Downloaded $saved of $total tracks" },
        downloadsClearedStatus = "Downloads cleared",
        removedFromCacheStatus = "Removed from cache",
        signingInYtmStatus = "Signing in to YouTube Music...",
        signedInAsStatus = { name -> "Signed in as $name" },
        signedInYtmStatus = "Signed in to YouTube Music",
        signInFailedStatus = { msg -> "Sign-in failed: $msg" },
        signedOutStatus = "Signed out",
        engineInitErrorStatus = { msg -> "Engine init error: $msg" },
        recoveringStreamStatus = "Recovering stream...",
        playbackStoppedStatus = { msg -> "Playback stopped: $msg" },
        playbackErrorStatus = { msg -> "Playback error: $msg" },
        playFailedStatus = { msg -> "Play failed: $msg" },
        streamBadgeError = "Error",
        updateOpeningInstaller = "Opening installer...",
        updatePermissionRequired = "Allow 'Install unknown apps' for Sonata, then tap Install.",
        updateNoApkAsset = "No APK asset found in this release.",
        updateStartingDownload = "Starting download...",
        updateInstallingVersion = { ver -> "Installing Sonata v$ver..." },
        updateApkMissingRetry = "Downloaded APK missing; please retry.",
        musicMode = "Music",
        videoMode = "Video",
        videoQualityWarningTitle = "Video Mode Audio Quality",
        videoQualityWarningBody = "Switching to Video mode plays the music video's audio stream, which may lower audio quality compared to Music mode.",
        doNotShowAgain = "Do not show again",
        switchToVideo = "Switch to Video",
        videoQuality = "Video Quality",
        videoQualityAuto = "Auto",
        fullscreen = "Fullscreen",
        exitFullscreen = "Exit Fullscreen",
        loadingVideo = "Loading video...",
        videoUnavailableStatus = "Music video stream unavailable",
    ),
    settings = SettingsStrings(
        settingsTitle = "Settings",
        settingsSubtitle = "Audio, theme, cache",
        searchSettingsPlaceholder = "Search settings...",
        searchNoMatch = { q -> "No settings match \"$q\"" },
        sectionAccount = "ACCOUNT",
        sectionAudioQuality = "AUDIO QUALITY",
        sectionTheme = "THEME",
        sectionLanguage = "LANGUAGE",
        sectionGlassMotion = "GLASS & MOTION",
        sectionPlayerCovers = "PLAYER & COVERS",
        catDownloads = "Downloads",
        catPlayback = "Playback",
        catContent = "Content Preferences",
        catAppearance = "Appearance",
        catPerformance = "Performance",
        catStorage = "Storage",
        catYourData = "Your Data",
        catMisc = "Miscellaneous",
        catAccountUpdates = "Account & Updates",
        streamingQuality = "Streaming Quality",
        downloadQuality = "Download Quality",
        jioSaavnFallback = "JioSaavn 320 kbps Fallback",
        jioSaavnFallbackDesc = "Resolve higher-bitrate AAC streams when available",
        playbackSummary = "Autoplay, speed, skip silence, sleep timer",
        contentSummary = "Home shelves, recents, moods, videos",
        performanceSummary = "120 Hz · Glass & motion",
        storageSummary = "Cache limits, clear images",
        yourDataSummary = "Backup & restore settings",
        miscSummary = "Status bar, player behavior, stats",
        accountUpdatesSummary = "Session, release checks, version",
        designTheme = "Design Theme",
        colorAppearance = "Color Appearance",
        appLanguage = "Language",
        appLanguageDesc = "Follow Android system language or pick English / Türkçe / Русский",
        langSystem = "System Default",
        langEnglish = "English",
        langTurkish = "Türkçe",
        langRussian = "Русский",
        langSystemDesc = "Matches your Android device language automatically",
        langEnglishDesc = "Use English across the app",
        langTurkishDesc = "Uygulama genelinde Türkçe kullan",
        langRussianDesc = "Использовать русский язык в приложении",
        themeModeSystem = "System",
        themeModeLight = "Light",
        themeModeDark = "Dark",
        themeModeAmoled = "AMOLED",
        liquidGlass = "Liquid Glass",
        liquidGlassDesc = "Crystal-clear refracted navigation bar and floating chrome",
        liquidGlassApiHint = "Requires Android 13+ (API 33)",
        glassPerformanceMode = "Glass Performance Mode",
        glassPerformanceDesc = "Lighter single-pass glass rendering for smoother scrolling",
        reduceDynamicBlur = "Reduce Dynamic Blur",
        reduceDynamicBlurDesc = "Use crisp static frosted surfaces instead of live lens sampling",
        reduceAnimation = "Reduce Animation",
        reduceAnimationDesc = "Minimize spring transitions and ambient wave motion",
        fullScreenCoverArt = "Full-Screen Cover Art",
        fullScreenCoverArtDesc = "Edge-to-edge dissolving hero cover in portrait Now Playing",
        animatedArtwork = "Animated Cover Art",
        animatedArtworkDesc = "Play looping motion covers where available",
        animatedArtworkCellular = "Animated Art on Cellular",
        animatedArtworkCellularDesc = "Allow motion cover downloads on mobile data",
        artworkTintedPages = "Artwork-Tinted Pages",
        artworkTintedPagesDesc = "Blend subtle album palette tints into detail views",
        fullScreenDetailCover = "Full-Screen Detail Cover",
        fullScreenDetailCoverDesc = "Stretch album and playlist hero covers behind the top bar",
        showTotalDuration = "Show Queue Duration",
        showTotalDurationDesc = "Display remaining time alongside total track duration",
        playerBackdrop = "Player Backdrop",
        hdrQualitySources = "Quality & Sources",
        footerQualitySources = "Average song size assumes a ~3.5 minute track. Enabling Include JioSaavn lets offline downloads pick 320 kbps AAC when higher than YouTube Music.",
        includeJioSaavnDownloads = "Include JioSaavn",
        includeJioSaavnDownloadsDesc = "Search JioSaavn 320 kbps AAC when resolving highest-quality downloads",
        wifiOnlyDownloads = "Download over Wi-Fi only",
        wifiOnlyDownloadsDesc = "Avoid mobile data usage for offline tracks",
        autoDownloadLiked = "Auto-download liked songs",
        autoDownloadLikedDesc = "Save tracks offline automatically when starred",
        hdrVideoDownloads = "Music Video Downloads",
        footerVideoDownloads = "When enabled, tracks with an official YouTube Music video also download the video stream for offline Video mode playback.",
        downloadVideosIfAvailable = "Download videos if available",
        downloadVideosIfAvailableDesc = "Save the companion music video offline alongside downloaded tracks",
        videoDownloadQuality = "Video Download Quality",
        videoDl1080Title = "Full HD (1080p)",
        videoDl1080Desc = "Crispest resolution for large screens",
        videoDl1080Size = "~45 MB / video",
        videoDl720Title = "HD (720p)",
        videoDl720Desc = "Sharp detail with balanced storage use",
        videoDl720Size = "~22 MB / video",
        videoDl480Title = "Standard (480p)",
        videoDl480Desc = "Lightweight video for phones",
        videoDl480Size = "~12 MB / video",
        videoDl360Title = "Data Saver (360p)",
        videoDl360Desc = "Smallest file size for offline videos",
        videoDl360Size = "~7 MB / video",
        downloadingVideoStatus = "Downloading music video…",
        hdrStorageLocation = "Storage Location & Space",
        footerStorageLocation = "Existing downloads remain playable from their original folder. New downloads save to the selected storage location.",
        downloadLocation = "Download Location",
        storageLocationInternal = "Internal",
        storageLocationExternal = "External",
        storageInternalDesc = "Private app sandbox; removed on uninstall",
        storageExternalDesc = "Music/Sonata public folder; visible to other players",
        storageInternalLabel = "Internal · Private App Storage",
        storageExternalLabel = "External · Music/Sonata",
        downloadedStorage = "Downloaded Storage",
        removeAllDownloadsConfirmTitle = "Remove all downloads?",
        removeAllDownloadsConfirmBody = { count, bytes -> "This deletes $count downloaded songs ($bytes) from offline storage." },
        deleteAllOfflineSongs = { count -> "Delete all $count offline songs" },
        hdrQueueTransitions = "Queue & Playback",
        autoplayDesc = "Continue playing personalized related tracks when queue finishes",
        crossfade = "Crossfade",
        crossfadeSecondsOverlap = { sec -> "${sec}s overlap" },
        durationLabel = "Duration",
        skipSilenceDesc = "Shorten silent gaps during playback",
        sleepTimerFadeOut = "Sleep Timer Fade Out",
        sleepTimerFadeOutDesc = "Gently lower volume over the final seconds before pausing",
        hdrSpeedAndTiming = "Playback Speed",
        hdrHomeFeed = "Home Feed",
        footerHomeFeed = "Customize which shelves appear on the Home tab and how video cards are framed.",
        hideRecents = "Hide Recents",
        hideRecentsDesc = "Hide Last Played and Recently Played shelves on Home",
        hideTopPicks = "Hide Top Picks",
        hideTopPicksDesc = "Hide the Top Picks for You shelf on Home",
        hideHomeMoods = "Hide Find Your Mood",
        hideHomeMoodsDesc = "Hide the mood carousel from Home",
        hideHomeVideos = "Hide Home Videos",
        hideHomeVideosDesc = "Filter out video shelves from the Home feed",
        squareVideoThumbnails = "Square Video Thumbnails",
        squareVideoThumbnailsDesc = "Crop 16:9 music video cards into clean 1:1 album sleeves",
        hideLyricsPreview = "Hide Lyrics Preview",
        hideLyricsPreviewDesc = "Hide the inline synced lyric line above the scrubber",
        blurUnfocusedLyrics = "Blur Unfocused Lyrics",
        blurUnfocusedLyricsDesc = "Softly defocus upcoming and past lyric lines",
        wordByWordSync = "Word-by-Word Sync",
        wordByWordSyncDesc = "Prefer syllable-timed karaoke lyrics when available",
        translateLyricsDesc = "Show translated line beneath original lyrics",
        translationLanguage = "Translation Language",
        romanizeLyricsDesc = "Transliterate Japanese, Korean, and Cyrillic scripts",
        globalLyricOffset = "Global Lyric Offset",
        footerAppearanceTheme = "Basic activates the monochrome editorial UI overhaul on Home, Explore, Album, and Artist pages. Violet and Moonflower keep their full distinct visual styles.",
        hdrDisplayRendering = "Display & Rendering",
        footerDisplayRendering = "Lowering frame rate or enabling Glass Performance Mode reduces GPU usage and improves battery life.",
        refreshRateHigh = "120 Hz High Refresh Rate",
        refreshRateDesc = "Smooth high-refresh scrolling and animations",
        hdrStorageCache = "Storage & Cache",
        downloadedSongs = "Downloaded Songs",
        clearAudioCache = "Clear Audio & Lyrics Cache",
        clearAudioCacheDesc = "Free cached audio segments and synced lyrics",
        clearImageCache = "Clear Image Cache",
        clearImageCacheDesc = "Clear cached album covers and thumbnails from memory and disk",
        hdrBackupRestore = "Backup & Restore",
        footerBackupRestore = "Settings backup files are plain JSON and strictly exclude session cookies and account credentials.",
        exportBackup = "Export Settings Backup",
        exportBackupDesc = "Save preferences and per-track lyric offsets as JSON",
        importBackup = "Import Settings Backup",
        importBackupDesc = "Restore preferences from a previously exported JSON file",
        hdrPlayerSystem = "Player & System",
        hideStatusBarPlayer = "Hide Status Bar in Player",
        hideStatusBarPlayerDesc = "Use immersive full-height layout while Now Playing is open",
        hideQualityBadge = "Hide Quality Badge",
        hideQualityBadgeDesc = "Hide the audio format pill below the playback scrubber",
        stopMusicOnClose = "Stop Music on Close",
        stopMusicOnCloseDesc = "Stop playback and close media notification when swiping the app away",
        showNerdStats = "Show Nerd Stats",
        showNerdStatsDesc = "Display live codec, bitrate, sample rate, and cache source in Now Playing",
        hdrYtmAccount = "YouTube Music Account",
        connectedStatus = "Connected",
        ytmSessionActive = "YouTube Music session active",
        signInYtmSettingsSubtitle = "Sign in for personalized Home, playlists, and likes",
        signOutDesc = "Clear local YouTube Music session cookies",
        signInYtmTitle = "Sign In to YouTube Music",
        signInYtmDesc = "Open secure in-app browser sign-in",
        hdrAboutUpdates = "About & Updates",
        footerAboutUpdates = "Sonata Android is 100% open-source with zero trackers or proprietary analytics SDKs.",
        sonataVersion = "Sonata Version",
        downloadUpdateVersion = { ver -> "Download Update ($ver)" },
        openReleaseApk = "Open release APK download",
        checkForUpdates = "Check for Updates",
        checkingForUpdates = "Checking for Updates...",
        compareGithubReleases = "Compare installed build with GitHub Releases",
        footerStreamingQuality = "Higher streaming tiers prioritize Opus 251 / AAC 320 kbps when available.",
        hdrOrderedHighestToSmallest = "Ordered from Highest to Smallest",
        footerDownloadQuality = "Average per-song size assumes a standard ~3.5 minute track. Actual file size varies by song duration and codec.",
        qualityLossless = "Lossless",
        qualityHigh = "High",
        qualityMedium = "Medium",
        qualityLow = "Low",
        qualityLosslessDetail = "Lossless / Maximum",
        qualityHighDetail = "High (256–320 kbps)",
        qualityMediumDetail = "Medium (128 kbps)",
        qualityLowDetail = "Data Saver (64 kbps)",
        qualityLosslessDesc = "Highest bitrate stream available",
        qualityHighDesc = "Best balance of fidelity and fast startup",
        qualityMediumDesc = "Standard streaming bitrate",
        qualityLowDesc = "Minimal mobile data usage",
        dlHighest = "Highest",
        dlHigh = "High",
        dlStandard = "Standard",
        dlLow = "Data Saver",
        themeVioletDesc = "Sculpted deep amethyst glass",
        themeMoonflowerDesc = "Botanical silk & floral aura",
        themeBasicDesc = "Monochrome editorial layout overhaul",
        backdropMesh = "Artwork Mesh",
        backdropWaves = "Ambient Waves",
        backdropBlurred = "Blurred Cover",
        backdropMeshDesc = "Rich color mesh baked from album artwork",
        backdropWavesDesc = "Smooth animated ambient palette gradient",
        backdropBlurredDesc = "Full-screen softly blurred cover art",
        loudnessNormalization = "Loudness Normalization",
        loudnessNormalizationDesc = "Keep perceived volume balanced across tracks",
        equalizerTitle = "5-Band Equalizer",
        eqPresetFlat = "Flat",
        eqPresetBassBoost = "Bass Boost",
        eqPresetVocal = "Vocal",
        eqPresetAcoustic = "Acoustic",
        eqPresetElectronic = "Electronic",
        eqPresetLateNight = "Late Night",
        eqPresetCustom = "Custom",
        storageFreeOf = { free, total -> "$free free of $total" },
    ),
    discord = DiscordStrings(
        catDiscord = "Discord Rich Presence",
        discordSummary = "Mobile Discord RPC, status, app ID",
        hdrDiscordAccount = "Discord Account",
        hdrDiscordPresence = "Presence Display",
        hdrDiscordAppId = "Discord Application ID",
        footerDiscordAppId = "Use Sonata's default Application ID or paste your own Discord Developer Portal Application ID.",
        discordSignInTitle = "Sign In to Discord",
        discordSignInSubtitle = "Authenticate in Discord to share Listening status",
        discordSignInCompleting = "Completing sign-in...",
        discordSignInAction = "Sign In to Discord",
        discordSignInActionDesc = "Connect your Discord account for mobile Rich Presence",
        discordSignOutAction = "Sign Out of Discord",
        discordSignOutDesc = "Clear local Discord session and stop Rich Presence",
        discordConsentTitle = "Discord Rich Presence Notice",
        discordConsentBody = "Sonata connects directly to Discord's Gateway using your Discord account session so your Listening activity appears on mobile without needing a PC.\n\nYour Discord session credential is stored locally on this device, excluded from settings backups, and never logged or sent anywhere except Discord.",
        discordConsentContinue = "I Understand, Continue",
        discordEnableRpc = "Enable Discord Rich Presence",
        discordEnableRpcDesc = "Broadcast currently playing tracks to your Discord profile",
        discordActivityType = "Activity Type",
        discordActivityListening = "Listening",
        discordActivityPlaying = "Playing",
        discordActivityWatching = "Watching",
        discordActivityCompeting = "Competing",
        discordShowTimestamps = "Show Progress Timestamps",
        discordShowTimestampsDesc = "Display playback progress adjusted for playback speed",
        discordShowArtwork = "Show Track Artwork",
        discordShowArtworkDesc = "Proxy album covers through Discord external assets",
        discordShowButtons = "Show Listen Button",
        discordShowButtonsDesc = "Include a YouTube Music link button on your activity card",
        discordAppIdLabel = "Application ID",
        discordAppIdPlaceholder = "Enter Discord Application ID",
        discordAppIdApply = "Save",
        discordAppIdReset = "Reset Default",
        discordAppIdInvalid = "Enter a valid numeric Discord Application ID (15–22 digits)",
        discordStatusLabel = "Connection Status",
        discordStatusConnected = "Connected",
        discordStatusConnectedAs = { user -> "Connected as $user" },
        discordStatusConnecting = "Connecting to Discord Gateway...",
        discordStatusReconnecting = "Reconnecting to Discord...",
        discordStatusDisconnected = "Disconnected",
        discordStatusAuthFailed = "Session expired or rejected — sign in again",
        discordStatusError = { err -> "Connection error: $err" },
        discordReconnect = "Reconnect Gateway",
    ),
    servers = EnglishServerStrings,
)

val TurkishSonataStrings = SonataStrings(
    common = CommonStrings(
        tabHome = "Ana Sayfa",
        tabExplore = "Keşfet",
        tabRadio = "Radyo",
        tabLibrary = "Kitaplık",
        tabSearch = "Ara",
        play = "Oynat",
        pause = "Duraklat",
        shuffle = "Karıştır",
        radio = "Radyo",
        save = "Kaydet",
        saved = "Kaydedildi",
        download = "İndir",
        downloaded = "İndirildi",
        share = "Paylaş",
        close = "Kapat",
        cancel = "İptal",
        back = "Geri",
        clear = "Temizle",
        done = "Bitti",
        retry = "Tekrar Dene",
        refresh = "Yenile",
        seeAll = "Tümünü gör",
        signIn = "Giriş Yap",
        signOut = "Çıkış Yap",
        account = "Hesap",
        remove = "Kaldır",
        create = "Oluştur",
        ok = "Tamam",
        reset = "Sıfırla",
        off = "Kapalı",
        homeHeading = "Ana Sayfa",
        exploreHeading = "Keşfet",
        radioHeading = "Radyo",
        libraryHeading = "Kitaplık",
        searchPlaceholder = "Şarkı, sanatçı, albüm ara...",
        yourLibrary = "Kitaplığın",
        signedInStatus = "Giriş Yapıldı",
        notSignedInStatus = "Giriş Yapılmadı",
        signInToSyncLibrary = "Çalma listelerini ve beğenileri eşitlemek için giriş yap",
        noHistoryYet = "Henüz dinleme geçmişi yok",
        playSongsToBuildHistory = "Çaldığın şarkılar burada görünecek",
        searchEmptyTitle = "Müzik Bul",
        searchEmptySubtitle = "Şarkı, albüm, sanatçı ve çalma listesi ara",
        noResultsFound = "Sonuç bulunamadı",
        allTab = "Tümü",
        noDownloadsYet = "Henüz indirilen şarkı yok",
        noDownloadsHint = "Çevrimdışı dinlemek için şarkı veya liste indir",
        playAll = "Tümünü Oynat",
        downloadAll = "Tümünü İndir",
        removeDownload = "İndirmeyi Kaldır",
        removeAllDownloads = "Tüm İndirmeleri Sil",
        topSongs = "Popüler Şarkılar",
        releases = "Albümler ve Tekliler",
        fansMightLike = "Hayranların Sevebilecekleri",
        aboutArtist = "Hakkında",
        subscribe = "Abone Ol",
        subscribed = "Abone Olundu",
        addedArtistToast = { name -> "$name takip edildi" },
        removedArtistToast = { name -> "$name kaldırıldı" },
        emptyCollection = "Parça bulunamadı",
        localFolderEmpty = "Yerel klasör boş",
        newPlaylist = "Yeni Liste",
        createPlaylist = "Çalma Listesi Oluştur",
        createNewPlaylistHint = "Yeni bir YouTube Music çalma listesi oluştur",
        noPlaylistsYet = "Henüz çalma listesi yok",
        addingTracksToPlaylist = { n -> "Listeye $n parça ekleniyor" },
        addedToPlaylistStatus = { name -> "$name listesine eklendi" },
        editPlaylist = "Çalma Listesini Düzenle",
        playlistName = "Liste adı",
        playlistDescription = "Açıklama (isteğe bağlı)",
        playlistPrivacy = "Gizlilik",
        privacyKeep = "Mevcut kalsın",
        privacyPrivate = "Gizli",
        privacyUnlisted = "Liste dışı",
        privacyPublic = "Herkese açık",
        changeCover = "Kapak Değiştir",
        coverSelected = "Yeni kapak seçildi",
        saving = "Kaydediliyor...",
        editPlaylistHint = "Değişiklikler doğrudan YouTube Music hesabına uygulanır.",
        updateAvailable = "Güncelleme Mevcut",
        downloadNow = "Şimdi İndir",
        installNow = "Şimdi Kur",
        later = "Sonra",
        downloadingApk = "APK indiriliyor...",
        offlineLibrary = "Çevrimdışı Kitaplık",
        offlinePlaylist = "Çevrimdışı Çalma Listesi",
        artistDefault = "Sanatçı",
        nothingToUpdate = "Güncellenecek bir şey yok",
        signInToEditPlaylists = "Çalma listelerini düzenlemek için YouTube Music'e giriş yap",
        selectAtLeastOnePlaylist = "En az bir çalma listesi seç",
        playlistNameEmpty = "Çalma listesi adı boş olamaz",
        noYtmMatchFound = "Eşleşen YouTube Music parçası bulunamadı",
        failedUpdatePlaylist = "Çalma listesi güncellenemedi",
        failedCreatePlaylist = "Çalma listesi oluşturulamadı",
        failedUploadCover = "Çalma listesi kapağı yüklenemedi",
        updatedPlaylistAndCover = "Çalma listesi ve kapak güncellendi",
        updatedPlaylistCover = "Çalma listesi kapağı güncellendi",
        updatedPlaylist = "Çalma listesi güncellendi",
        addingToPlaylistStatus = "Çalma listesine ekleniyor...",
        creatingPlaylistStatus = { name -> "$name oluşturuluyor..." },
        createdPlaylistStatus = { name -> "\"$name\" oluşturuldu" },
        createdOfflinePlaylistStatus = { name -> "Çevrimdışı liste \"$name\" oluşturuldu" },
        addedSingleTrackToPlaylists = { title, count -> "$title, $count çalma listesine eklendi" },
        addedTracksToPlaylistsCount = { tracks, playlists -> "$tracks parça $playlists çalma listesine eklendi" },
        addedToSomePlaylists = { ok, total -> "$ok / $total çalma listesine eklendi" },
        couldNotAddToPlaylist = "Çalma listesine eklenemedi",
        noTracksInCollection = { title -> "$title içinde parça bulunamadı" },
        loadingNamedStatus = { title -> "$title yükleniyor..." },
        openingNamedStatus = { title -> "$title açılıyor..." },
        tracksInCollectionStatus = { count, title -> "$title içinde $count parça" },
        offlineSongsCount = { count -> "$count çevrimdışı şarkı" },
        offlinePlaylistSongsCount = { count -> "$count çevrimdışı liste şarkısı" },
        downloadedSongsCount = { count -> "$count indirilen şarkı" },
        pinned = "Sabitlenenler",
        pinToLibrary = "Sabitle",
        unpinFromLibrary = "Sabitlemeyi Kaldır",
        addToLibrary = "Kitaplığa Ekle",
        removeFromLibrary = "Kitaplıktan Kaldır",
        savedInLibrary = "Kitaplıkta",
        pinnedStatus = { title -> "\"$title\" sabitlendi" },
        unpinnedStatus = { title -> "\"$title\" sabitlemesi kaldırıldı" },
        addedToLibraryStatus = { title -> "\"$title\" kitaplığa eklendi" },
        removedFromLibraryStatus = { title -> "\"$title\" kitaplıktan kaldırıldı" },
    ),
    feed = FeedStrings(
        topPicksForYou = "Senin İçin Seçilenler",
        lastPlayed = "Son Çalınanlar",
        quickPicks = "Hızlı seçimler",
        recentSearches = "Son aramalar",
        listeningHistory = "Geçmiş",
        downloads = "İndirilenler",
        playlists = "Çalma Listeleri",
        likedSongs = "Beğenilen Şarkılar",
        albums = "Albümler",
        artists = "Sanatçılar",
        songs = "Şarkılar",
        samples = "Örnekler",
        videos = "Videolar",
        signInBannerTitle = "YouTube Music ile giriş yap",
        signInBannerSubtitle = "Çalma listelerini, beğenileri ve Extreme ses kalitesini eşitle",
        recentSearchesSubtitle = "Son aramalar",
        quickControls = "Hızlı Kontroller",
        refreshHomeMixes = "Ana Sayfa ve Miksleri Yenile",
        curatedForYou = "SENİN İÇİN SEÇİLDİ",
        discoveryStation = "Keşif İstasyonu",
        myStation = "Kişisel İstasyonum",
        userStationSuffix = { name -> "$name İstasyonu" },
        artistMixDefaultTitle = "Favori Miks",
        artistMixFeaturing = { artist -> "$artist ve benzerleri" },
        artistMixDefaultSubtitle = "Kişiselleştirilmiş sanatçı miksi",
        newReleaseEyebrow = "YENİ ÇIKAN",
        featuredPlaylistEyebrow = "ÖNE ÇIKAN LİSTE",
        featuredChartEyebrow = "ÖNE ÇIKAN LİSTELER",
        updatedPlaylistEyebrow = "GÜNCELLENEN LİSTE",
        findYourMood = "Ruh Halini Seç",
        yourPlaylists = "Çalma Listelerin",
        playlistLabel = "Çalma Listesi",
        albumLabel = "Albüm",
        singleLabel = "Tekli",
        artistLabel = "Sanatçı",
        trackLabel = "Parça",
        radioMixEyebrow = "RADYO MİKSİ",
        featuredMixes = "Öne Çıkan Miksler",
        featuredStation = "Öne Çıkan İstasyon",
        artistAndTrackStations = "Sanatçı ve Parça İstasyonları",
        moodAndGenreStations = "Ruh Hali ve Tür İstasyonları",
        artistRadioTitle = { artist -> "$artist Radyosu" },
        basedOnTrack = { title -> "$title temel alınarak" },
        trendingViralHits = "Trendler ve Popülerler",
        playlistsAndAlbums = "Çalma Listeleri ve Albümler",
        exploreMore = "Daha Fazlasını Keşfet",
        recentlyPlayedFallback = "Son Çalınanlar",
        allMoodsAndGenres = "Ruh Halleri ve Türler",
        forgottenFavorites = "Unutulan Favoriler",
        recommendedAlbums = "Önerilen Albümler",
        newReleases = "Yeni Çıkanlar",
        trending = "Trendler",
        charts = "Listeler",
        similarArtists = "Benzer Sanatçılar",
        fromYourLibrary = "Kitaplığından",
        mixedForYou = "Senin İçin Mikslendi",
        musicVideos = "Müzik Videoları",
        moodRelax = "Rahatla",
        moodWorkout = "Antrenman",
        moodEnergize = "Enerji",
        moodFocus = "Odaklan",
        moodCommute = "Yolculuk",
        moodParty = "Parti",
        moodSleep = "Uyku",
        moodFeelGood = "İyi Hisset",
        moodRomance = "Romantik",
        moodSad = "Hüzünlü",
        moodChill = "Sakin",
        moodPodcasts = "Podcast'ler",
        featuredPlaylistsForYou = "Senin için öne çıkan listeler",
        fromTheCommunity = "Topluluktan",
        singlesAndEps = "Tekliler ve EP'ler",
        singles = "Tekliler",
        newReleasesForYou = "Senin için yeni çıkanlar",
        coversAndRemixes = "Cover ve remiksler",
        livePerformances = "Canlı performanslar",
        featuredOn = "Yer aldığı listeler",
        latestRelease = "Son Çıkan",
        albumsAndSingles = "Albümler ve Tekliler",
        similarToPrefix = { x -> "$x benzerleri" },
        moreFromPrefix = { x -> "$x sanatçısından daha fazlası" },
        becauseYouListenedToPrefix = { x -> "$x dinlediğin için" },
        recentlyPlayedSubtitle = "Son çalınan",
        personalizedShelvesCount = { n -> "$n kişiselleştirilmiş raf" },
        searchingQueryStatus = { q -> "\"$q\" aranıyor..." },
        searchResultsHeader = { q -> "Arama: $q" },
        songsFoundCount = { n -> "$n şarkı bulundu" },
        songsFoundYtmJioCount = { n -> "$n şarkı bulundu (YTM + JioSaavn)" },
        songsFoundJioCount = { n -> "$n şarkı bulundu (JioSaavn)" },
        searchErrorStatus = { msg -> "Arama hatası: $msg" },
        homeErrorStatus = { msg -> "Ana sayfa hatası: $msg" },
        artistErrorStatus = { msg -> "Sanatçı hatası: $msg" },
        collectionErrorStatus = { msg -> "Koleksiyon hatası: $msg" },
    ),
    player = PlayerStrings(
        audioPipeline = "Ses Hattı",
        audioPipelineSubtitle = "Kodek ve bit derinliği",
        autoplay = "Sonsuz Çalma",
        autoplayOn = "Sonsuz çalma açıldı",
        autoplayOff = "Sonsuz çalma kapatıldı",
        shuffleOn = "Karışık çalma açık",
        shuffleOff = "Karışık çalma kapalı",
        repeatOff = "Tekrarlama kapalı",
        repeatAll = "Tümünü tekrarla",
        repeatOne = "Tek parçayı tekrarla",
        loadingYoutubeMusic = "YouTube Music yükleniyor...",
        startingRadio = "Sonsuz radyo başlatılıyor...",
        radioUnavailable = "Radyo şu anda başlatılamadı",
        startingPersonalMix = "Kişisel miks başlatılıyor...",
        startingDiscoveryMix = "Keşif miksi başlatılıyor...",
        sleepTimerPaused = "Uyku zamanlayıcısı oynatmayı duraklattı",
        resolving = "Çözümleniyor...",
        offlineLabel = "Çevrimdışı",
        liveStream = "Canlı Yayın",
        noTrackPlaying = "Hiçbir şey çalmıyor",
        playingTrackStatus = { title -> "Çalınıyor: $title" },
        playingTrackOfflineStatus = { title -> "Çalınıyor: $title (Çevrimdışı)" },
        resolvingTrackStatus = { title -> "Yükleniyor: $title..." },
        addedToPlayNextStatus = { n -> "Sonra çal sırasına $n parça eklendi" },
        addedToQueueStatus = { n -> "Sıraya $n parça eklendi" },
        playingNextStatus = { title -> "Sıradaki parça: $title" },
        addedTrackToQueueStatus = { title -> "Sıraya eklendi: $title" },
        mediaCacheClearedStatus = "Medya ve şarkı sözü önbelleği temizlendi",
        imageCacheClearedStatus = "Görsel önbelleği temizlendi",
        updateCheckFailed = "Güncelleme denetimi başarısız oldu",
        offlinePlaybackError = { msg -> "Çevrimdışı oynatma hatası: $msg" },
        lyrics = "Şarkı Sözleri",
        queue = "Sıra",
        upNext = "Sıradakiler",
        loadingLyrics = "Şarkı sözleri yükleniyor...",
        noLyricsAvailable = "Şarkı sözü bulunamadı",
        lyricsTranslation = "Söz Çevirisi",
        favorite = "Favori",
        options = "Seçenekler",
        previous = "Önceki",
        next = "Sonraki",
        tracksCount = { n -> "$n parça" },
        savedCount = { n -> "$n kayıtlı" },
        wordSynced = "Kelime Eşzamanlı",
        syncedLabel = "Eşzamanlı",
        unavailableLabel = "Mevcut Değil",
        lyricsDisplay = "Söz Görünümü",
        lyricsTiming = "Zamanlama Kaydırma",
        syncOffset = "Senkron Kayması",
        translateLyrics = "Sözleri Çevir",
        romanizeLyrics = "Okunuşu Göster",
        sourceLabel = { src -> "Kaynak: $src" },
        playNext = "Sonra Oynat",
        addToQueue = "Sıraya Ekle",
        addToPlaylist = "Çalma Listesine Ekle",
        startRadio = "Radyoyu Başlat",
        goToAlbum = "Albüme Git",
        goToArtist = "Sanatçıya Git",
        songInfo = "Parça Bilgisi",
        like = "Beğen",
        unlike = "Beğeniyi Kaldır",
        dislike = "Beğenme",
        removeFromCache = "İndirmeyi Kaldır",
        playCollection = "Oynat",
        shufflePlay = "Karıştır",
        openAlbum = "Albümü Aç",
        openPlaylist = "Listeyi Aç",
        sleepTimer = "Uyku Zamanlayıcısı",
        sleepOff = "Kapalı",
        sleepEndOfTrack = "Parça Sonunda",
        sleepMinutesRemaining = { m -> "$m dk kaldı" },
        audioControls = "Ses Kontrolleri",
        playbackSpeed = "Oynatma Hızı",
        skipSilence = "Sessizliği Atla",
        shareCardTitle = "Paylaş",
        copyLink = "Bağlantıyı Kopyala",
        shareMore = "Diğer",
        linkCopied = "Bağlantı kopyalandı",
        audioPipelineCodec = "Kodek",
        audioPipelineBitrate = "Bit Hızı",
        audioPipelineSampleRate = "Örnekleme Hızı",
        audioPipelineOutput = "Çıkış Yolu",
        audioPipelineSource = "Kaynak",
        audioPipelineMode = "Ses Modu",
        audioPipelineBitTransparent = "Bit-birebir çıkış (yeniden örnekleme/DSP yok)",
        openSignInPrompt = "YouTube Music ile giriş yap",
        wifiOnlyDownloadBlocked = "İndirmeler için Wi-Fi gerekli (Yalnızca Wi-Fi açık)",
        alreadyDownloadedNamed = { title -> "Zaten indirildi: $title" },
        alreadyDownloadedShort = "Zaten indirildi",
        downloadingNamed = { title -> "$title indiriliyor..." },
        downloadFailedStreamUnavailable = "İndirme başarısız: akış bulunamadı",
        downloadFailedShort = "İndirme başarısız",
        downloadFailedNamed = { msg -> "İndirme başarısız: $msg" },
        downloadedNamed = { title -> "$title indirildi" },
        allTracksAlreadyDownloaded = { count -> "$count parçanın tümü zaten indirildi" },
        allTracksAlreadyDownloadedShort = "Tüm parçalar zaten indirildi",
        downloadingTracksCount = { count -> "$count parça indiriliyor..." },
        downloadedCountOfTotal = { saved, total -> "$total parçadan $saved tanesi indirildi" },
        downloadsClearedStatus = "İndirilenler temizlendi",
        removedFromCacheStatus = "Önbellekten kaldırıldı",
        signingInYtmStatus = "YouTube Music'e giriş yapılıyor...",
        signedInAsStatus = { name -> "$name olarak giriş yapıldı" },
        signedInYtmStatus = "YouTube Music'e giriş yapıldı",
        signInFailedStatus = { msg -> "Giriş başarısız: $msg" },
        signedOutStatus = "Çıkış yapıldı",
        engineInitErrorStatus = { msg -> "Motor başlatma hatası: $msg" },
        recoveringStreamStatus = "Akış kurtarılıyor...",
        playbackStoppedStatus = { msg -> "Oynatma durdu: $msg" },
        playbackErrorStatus = { msg -> "Oynatma hatası: $msg" },
        playFailedStatus = { msg -> "Oynatma başarısız: $msg" },
        streamBadgeError = "Hata",
        updateOpeningInstaller = "Yükleyici açılıyor...",
        updatePermissionRequired = "Sonata için 'Bilinmeyen uygulamalara izin ver'i açıp Kur'a dokunun.",
        updateNoApkAsset = "Bu sürümde APK dosyası bulunamadı.",
        updateStartingDownload = "İndirme başlatılıyor...",
        updateInstallingVersion = { ver -> "Sonata v$ver kuruluyor..." },
        updateApkMissingRetry = "İndirilen APK bulunamadı; lütfen tekrar deneyin.",
        musicMode = "Müzik",
        videoMode = "Video",
        videoQualityWarningTitle = "Video Modunda Ses Kalitesi",
        videoQualityWarningBody = "Video moduna geçtiğinde müzik videosunun ses akışı oynatılır; ses kalitesi Müzik moduna göre düşebilir.",
        doNotShowAgain = "Bir daha gösterme",
        switchToVideo = "Video Moduna Geç",
        videoQuality = "Video Kalitesi",
        videoQualityAuto = "Otomatik",
        fullscreen = "Tam Ekran",
        exitFullscreen = "Tam Ekrandan Çık",
        loadingVideo = "Video yükleniyor...",
        videoUnavailableStatus = "Müzik videosu oynatılamadı",
    ),
    settings = SettingsStrings(
        settingsTitle = "Ayarlar",
        settingsSubtitle = "Ses, tema, önbellek",
        searchSettingsPlaceholder = "Ayarlarda ara...",
        searchNoMatch = { q -> "\"$q\" ile eşleşen ayar yok" },
        sectionAccount = "HESAP",
        sectionAudioQuality = "SES KALİTESİ",
        sectionTheme = "TEMA",
        sectionLanguage = "DİL",
        sectionGlassMotion = "CAM VE HAREKET",
        sectionPlayerCovers = "OYNATICI VE KAPAKLAR",
        catDownloads = "İndirilenler",
        catPlayback = "Oynatma",
        catContent = "İçerik Tercihleri",
        catAppearance = "Görünüm",
        catPerformance = "Performans",
        catStorage = "Depolama",
        catYourData = "Verilerin",
        catMisc = "Diğer",
        catAccountUpdates = "Hesap ve Güncellemeler",
        streamingQuality = "Yayın Kalitesi",
        downloadQuality = "İndirme Kalitesi",
        jioSaavnFallback = "JioSaavn 320 kbps Yedeği",
        jioSaavnFallbackDesc = "Mümkün olduğunda daha yüksek bit hızlı AAC akışlarını seç",
        playbackSummary = "Sonsuz çalma, hız, sessizliği atlama, zamanlayıcı",
        contentSummary = "Ana sayfa rafları, son çalınanlar, ruh halleri",
        performanceSummary = "120 Hz · Cam ve hareket",
        storageSummary = "Önbellek sınırları, görselleri temizle",
        yourDataSummary = "Ayarları yedekle ve geri yükle",
        miscSummary = "Durum çubuğu, oynatıcı davranışı, istatistik",
        accountUpdatesSummary = "Oturum, sürüm denetimi, versiyon",
        designTheme = "Tasarım Teması",
        colorAppearance = "Renk Görünümü",
        appLanguage = "Dil",
        appLanguageDesc = "Android sistem dilini izle veya English / Türkçe / Русский seç",
        langSystem = "Sistem Dili",
        langEnglish = "English",
        langTurkish = "Türkçe",
        langRussian = "Русский",
        langSystemDesc = "Android cihaz dilini otomatik izler",
        langEnglishDesc = "Uygulama genelinde İngilizce kullan",
        langTurkishDesc = "Uygulama genelinde Türkçe kullan",
        langRussianDesc = "Uygulama genelinde Rusça kullan",
        themeModeSystem = "Sistem",
        themeModeLight = "Açık",
        themeModeDark = "Koyu",
        themeModeAmoled = "AMOLED",
        liquidGlass = "Liquid Glass",
        liquidGlassDesc = "Kristal berraklığında kırılımlı gezinme çubuğu ve cam katmanlar",
        liquidGlassApiHint = "Android 13+ (API 33) gerektirir",
        glassPerformanceMode = "Cam Performans Modu",
        glassPerformanceDesc = "Daha akıcı kaydırma için hafif tek geçişli cam çizimi",
        reduceDynamicBlur = "Dinamik Bulanıklığı Azalt",
        reduceDynamicBlurDesc = "Canlı lens örneklemesi yerine keskin sabit buzlu yüzey kullan",
        reduceAnimation = "Animasyonu Azalt",
        reduceAnimationDesc = "Yay geçişlerini ve arka plan dalga hareketlerini azalt",
        fullScreenCoverArt = "Tam Ekran Kapak Görseli",
        fullScreenCoverArtDesc = "Dikey Oynatıcı ekranında kenardan kenara kapak",
        animatedArtwork = "Hareketli Kapak Görseli",
        animatedArtworkDesc = "Uygun parçalarda hareketli kapakları oynat",
        animatedArtworkCellular = "Hücresel Veride Hareketli Kapak",
        animatedArtworkCellularDesc = "Mobil veride hareketli kapak indirmeye izin ver",
        artworkTintedPages = "Kapağa Göre Renklenen Sayfalar",
        artworkTintedPagesDesc = "Detay görünümlerine albüm renk tonlarını uygula",
        fullScreenDetailCover = "Tam Ekran Detay Kapağı",
        fullScreenDetailCoverDesc = "Albüm ve liste kapaklarını üst çubuğun arkasına yay",
        showTotalDuration = "Toplam Süreyi Göster",
        showTotalDurationDesc = "İlerleme çubuğunda kalan süre yerine toplam süreyi göster",
        playerBackdrop = "Oynatıcı Arka Planı",
        hdrQualitySources = "Kalite ve Kaynaklar",
        footerQualitySources = "Ortalama boyut ~3,5 dakikalık parça varsayar. JioSaavn Dahil Et açıldığında indirmelerde uygunsa 320 kbps AAC seçilir.",
        includeJioSaavnDownloads = "JioSaavn Dahil Et",
        includeJioSaavnDownloadsDesc = "En yüksek kaliteli indirmelerde JioSaavn 320 kbps AAC ara",
        wifiOnlyDownloads = "Yalnızca Wi-Fi ile indir",
        wifiOnlyDownloadsDesc = "Çevrimdışı parçalar için mobil veri kullanımını önle",
        autoDownloadLiked = "Beğenilenleri otomatik indir",
        autoDownloadLikedDesc = "Beğenilen parçaları otomatik olarak çevrimdışı kaydet",
        hdrVideoDownloads = "Müzik Videosu İndirmeleri",
        footerVideoDownloads = "Açık olduğunda resmi YouTube Music videosu bulunan parçalar çevrimdışı Video modu için video akışını da indirir.",
        downloadVideosIfAvailable = "Varsa videoları da indir",
        downloadVideosIfAvailableDesc = "İndirilen parçaların yanında müzik videosunu da çevrimdışı kaydet",
        videoDownloadQuality = "Video İndirme Kalitesi",
        videoDl1080Title = "Full HD (1080p)",
        videoDl1080Desc = "Büyük ekranlar için en net çözünürlük",
        videoDl1080Size = "~45 MB / video",
        videoDl720Title = "HD (720p)",
        videoDl720Desc = "Dengeli depolama ile keskin görüntü",
        videoDl720Size = "~22 MB / video",
        videoDl480Title = "Standart (480p)",
        videoDl480Desc = "Telefonlar için hafif video",
        videoDl480Size = "~12 MB / video",
        videoDl360Title = "Veri Tasarrufu (360p)",
        videoDl360Desc = "Çevrimdışı videolar için en küçük dosya boyutu",
        videoDl360Size = "~7 MB / video",
        downloadingVideoStatus = "Müzik videosu indiriliyor…",
        hdrStorageLocation = "Depolama Konumu ve Alan",
        footerStorageLocation = "Mevcut indirmeler özgün klasöründen çalmaya devam eder. Yeni indirmeler seçilen konuma kaydedilir.",
        downloadLocation = "İndirme Konumu",
        storageLocationInternal = "Dahili",
        storageLocationExternal = "Harici",
        storageInternalDesc = "Özel uygulama alanı; uygulama silinince silinir",
        storageExternalDesc = "Music/Sonata ortak klasörü; diğer oynatıcılar görür",
        storageInternalLabel = "Dahili · Özel Uygulama Depolaması",
        storageExternalLabel = "Harici · Music/Sonata",
        downloadedStorage = "İndirilenler Depolaması",
        removeAllDownloadsConfirmTitle = "Tüm indirmeler silinsin mi?",
        removeAllDownloadsConfirmBody = { count, bytes -> "Bu işlem çevrimdışı depolamadan $count indirilen şarkıyı ($bytes) kalıcı olarak siler." },
        deleteAllOfflineSongs = { count -> "Tüm $count çevrimdışı şarkıyı sil" },
        hdrQueueTransitions = "Sıra ve Oynatma",
        autoplayDesc = "Liste veya sıra bittiğinde kişiselleştirilmiş benzer şarkılarla devam et",
        crossfade = "Geçiş (Crossfade)",
        crossfadeSecondsOverlap = { sec -> "${sec} sn geçiş" },
        durationLabel = "Süre",
        skipSilenceDesc = "Oynatma sırasındaki sessiz boşlukları atla",
        sleepTimerFadeOut = "Uyku Zamanlayıcısı Kısılması",
        sleepTimerFadeOutDesc = "Duraklatmadan önceki son saniyelerde sesi yavaşça kıs",
        hdrSpeedAndTiming = "Oynatma Hızı",
        hdrHomeFeed = "Ana Sayfa Akışı",
        footerHomeFeed = "Ana Sayfada hangi rafların görüneceğini ve video kartlarının çerçevesini özelleştir.",
        hideRecents = "Son Çalınanları Gizle",
        hideRecentsDesc = "Ana Sayfadaki Son Çalınanlar raflarını gizle",
        hideTopPicks = "Senin İçin Seçilenleri Gizle",
        hideTopPicksDesc = "Ana Sayfadaki Senin İçin Seçilenler rafını gizle",
        hideHomeMoods = "Ruh Halini Seç'i Gizle",
        hideHomeMoodsDesc = "Ana Sayfadaki ruh hali kartlarını gizle",
        hideHomeVideos = "Ana Sayfa Videolarını Gizle",
        hideHomeVideosDesc = "Ana Sayfa akışından video raflarını gizle",
        squareVideoThumbnails = "Kare Video Kapakları",
        squareVideoThumbnailsDesc = "16:9 video kartlarını 1:1 kare kapak olarak kırp",
        hideLyricsPreview = "Söz Önizlemesini Gizle",
        hideLyricsPreviewDesc = "İlerleme çubuğu üzerindeki tek satır şarkı sözünü gizle",
        blurUnfocusedLyrics = "Odak Dışı Sözleri Bulanıklaştır",
        blurUnfocusedLyricsDesc = "Geçmiş ve gelecek şarkı sözü satırlarını yumuşakça bulanıklaştır",
        wordByWordSync = "Kelime Kelime Eşzamanlama",
        wordByWordSyncDesc = "Uygun olduğunda hece/kelime zamanlı karaoke sözlerini tercih et",
        translateLyricsDesc = "Özgün sözlerin altında çevrilmiş satırı göster",
        translationLanguage = "Çeviri Dili",
        romanizeLyricsDesc = "Japonca, Korece ve Kiril alfabesini Latin harflerine dönüştür",
        globalLyricOffset = "Genel Söz Kaydırma",
        footerAppearanceTheme = "Basic; Ana Sayfa, Keşfet, Albüm ve Sanatçı sayfalarında monokrom editoryal görünümü açar. Violet ve Moonflower kendi stillerini korur.",
        hdrDisplayRendering = "Ekran ve İşleme",
        footerDisplayRendering = "Yenileme hızını düşürmek veya Cam Performans Modu GPU kullanımını azaltıp pil ömrünü artırır.",
        refreshRateHigh = "120 Hz Yüksek Yenileme Hızı",
        refreshRateDesc = "Akıcı yüksek yenileme hızlı kaydırma ve animasyonlar",
        hdrStorageCache = "Depolama ve Önbellek",
        downloadedSongs = "İndirilen Şarkılar",
        clearAudioCache = "Ses ve Söz Önbelleğini Temizle",
        clearAudioCacheDesc = "Önbelleğe alınmış ses parçalarını ve şarkı sözlerini temizle",
        clearImageCache = "Görsel Önbelleğini Temizle",
        clearImageCacheDesc = "Bellekteki ve diskteki albüm kapaklarını temizle",
        hdrBackupRestore = "Yedekle ve Geri Yükle",
        footerBackupRestore = "Ayar yedekleri düz JSON dosyasındadır; oturum çerezlerini veya hesap bilgilerini içermez.",
        exportBackup = "Ayar Yedeğini Dışa Aktar",
        exportBackupDesc = "Tercihleri ve parça söz kaydırmalarını JSON olarak kaydet",
        importBackup = "Ayar Yedeğini İçe Aktar",
        importBackupDesc = "Önceden dışa aktarılan JSON dosyasından ayarları geri yükle",
        hdrPlayerSystem = "Oynatıcı ve Sistem",
        hideStatusBarPlayer = "Oynatıcıda Durum Çubuğunu Gizle",
        hideStatusBarPlayerDesc = "Şimdi Çalınıyor açıkken tam ekran düzeni kullan",
        hideQualityBadge = "Kalite Rozetini Gizle",
        hideQualityBadgeDesc = "İlerleme çubuğu altındaki ses formatı rozetini gizle",
        stopMusicOnClose = "Kapatılınca Müziği Durdur",
        stopMusicOnCloseDesc = "Uygulama kaydırılarak kapatıldığında oynatmayı durdur",
        showNerdStats = "Teknik İstatistikleri Göster",
        showNerdStatsDesc = "Oynatıcıda canlı kodek, bit hızı, örnekleme ve önbellek kaynağını göster",
        hdrYtmAccount = "YouTube Music Hesabı",
        connectedStatus = "Bağlı",
        ytmSessionActive = "YouTube Music oturumu etkin",
        signInYtmSettingsSubtitle = "Kişisel Ana Sayfa, çalma listeleri ve beğeniler için giriş yap",
        signOutDesc = "Yerel YouTube Music oturum çerezlerini temizle",
        signInYtmTitle = "YouTube Music ile Giriş Yap",
        signInYtmDesc = "Güvenli uygulama içi tarayıcıyla oturum aç",
        hdrAboutUpdates = "Hakkında ve Güncellemeler",
        footerAboutUpdates = "Sonata Android %100 açık kaynaklıdır; izleyici veya analitik SDK içermez.",
        sonataVersion = "Sonata Sürümü",
        downloadUpdateVersion = { ver -> "Güncellemeyi İndir ($ver)" },
        openReleaseApk = "Sürüm APK indirmesini aç",
        checkForUpdates = "Güncellemeleri Denetle",
        checkingForUpdates = "Güncellemeler Denetleniyor...",
        compareGithubReleases = "Kurulu derlemeyi GitHub Releases ile karşılaştır",
        footerStreamingQuality = "Yüksek akış düzeyleri uygunsa Opus 251 / AAC 320 kbps'yi önceliklendirir.",
        hdrOrderedHighestToSmallest = "En Yüksekten En Küçüğe Sıralı",
        footerDownloadQuality = "Ortalama boyut ~3,5 dakikalık standart parçayı baz alır. Gerçek boyut süre ve kodeğe göre değişir.",
        qualityLossless = "Kayıpsız",
        qualityHigh = "Yüksek",
        qualityMedium = "Orta",
        qualityLow = "Düşük",
        qualityLosslessDetail = "Kayıpsız / Azami",
        qualityHighDetail = "Yüksek (256–320 kbps)",
        qualityMediumDetail = "Orta (128 kbps)",
        qualityLowDetail = "Veri Tasarrufu (64 kbps)",
        qualityLosslessDesc = "Kullanılabilir en yüksek bit hızlı akış",
        qualityHighDesc = "Kalite ve hızlı başlangıç arasında en iyi denge",
        qualityMediumDesc = "Standart yayın bit hızı",
        qualityLowDesc = "En düşük mobil veri tüketimi",
        dlHighest = "En Yüksek",
        dlHigh = "Yüksek",
        dlStandard = "Standart",
        dlLow = "Veri Tasarrufu",
        themeVioletDesc = "Derin ametist cam görünümü",
        themeMoonflowerDesc = "Botanik ipek ve çiçeksi aura",
        themeBasicDesc = "Monokrom editoryal tasarım",
        backdropMesh = "Kapak Ağı (Mesh)",
        backdropWaves = "Ortam Dalgaları",
        backdropBlurred = "Bulanık Kapak",
        backdropMeshDesc = "Albüm kapağından üretilen zengin renk ağı",
        backdropWavesDesc = "Yumuşak hareketli ortam paleti geçişi",
        backdropBlurredDesc = "Tam ekran yumuşak bulanıklaştırılmış kapak",
        loudnessNormalization = "Ses Seviyesi Eşitleme",
        loudnessNormalizationDesc = "Parçalar arasında algılanan ses düzeyini dengede tut",
        equalizerTitle = "5 Bantlı Ekolayzer",
        eqPresetFlat = "Düz",
        eqPresetBassBoost = "Bas Güçlendirme",
        eqPresetVocal = "Vokal",
        eqPresetAcoustic = "Akustik",
        eqPresetElectronic = "Elektronik",
        eqPresetLateNight = "Gece Modu",
        eqPresetCustom = "Özel",
        storageFreeOf = { free, total -> "$total içinde $free boş" },
    ),
    discord = DiscordStrings(
        catDiscord = "Discord Rich Presence",
        discordSummary = "Mobil Discord RPC, durum, uygulama kimliği",
        hdrDiscordAccount = "Discord Hesabı",
        hdrDiscordPresence = "Görünüm Seçenekleri",
        hdrDiscordAppId = "Discord Uygulama Kimliği",
        footerDiscordAppId = "Sonata'nın varsayılan Uygulama Kimliğini kullanın veya kendi Discord Developer Portal Uygulama Kimliğinizi yapıştırın.",
        discordSignInTitle = "Discord ile Giriş Yap",
        discordSignInSubtitle = "Dinliyor durumunu paylaşmak için Discord'da oturum açın",
        discordSignInCompleting = "Giriş tamamlanıyor...",
        discordSignInAction = "Discord ile Giriş Yap",
        discordSignInActionDesc = "Mobil Rich Presence için Discord hesabınızı bağlayın",
        discordSignOutAction = "Discord Çıkışı Yap",
        discordSignOutDesc = "Yerel Discord oturumunu temizle ve Rich Presence'ı durdur",
        discordConsentTitle = "Discord Rich Presence Bilgilendirmesi",
        discordConsentBody = "Sonata, bilgisayar gerekmeden mobilde Dinliyor durumunuzu gösterebilmek için Discord oturumunuzla doğrudan Discord Gateway'e bağlanır.\n\nDiscord oturum bilginiz yalnızca bu cihazda yerel olarak saklanır, ayar yedeklerine dahil edilmez ve Discord dışında hiçbir yere gönderilmez veya günlüğe yazılmaz.",
        discordConsentContinue = "Anladım, Devam Et",
        discordEnableRpc = "Discord Rich Presence'ı Etkinleştir",
        discordEnableRpcDesc = "Şu anda çalan parçaları Discord profilinizde yayınlayın",
        discordActivityType = "Etkinlik Türü",
        discordActivityListening = "Dinliyor",
        discordActivityPlaying = "Oynuyor",
        discordActivityWatching = "İzliyor",
        discordActivityCompeting = "Yarışıyor",
        discordShowTimestamps = "İlerleme Zaman Damgalarını Göster",
        discordShowTimestampsDesc = "Oynatma hızına göre ayarlanmış şarkı ilerlemesini göster",
        discordShowArtwork = "Parça Kapağını Göster",
        discordShowArtworkDesc = "Albüm kapaklarını Discord harici varlıkları üzerinden ilet",
        discordShowButtons = "Dinle Düğmesini Göster",
        discordShowButtonsDesc = "Etkinlik kartında YouTube Music bağlantı düğmesi göster",
        discordAppIdLabel = "Uygulama Kimliği",
        discordAppIdPlaceholder = "Discord Uygulama Kimliği girin",
        discordAppIdApply = "Kaydet",
        discordAppIdReset = "Varsayılana Sıfırla",
        discordAppIdInvalid = "Geçerli bir sayısal Discord Uygulama Kimliği girin (15–22 basamak)",
        discordStatusLabel = "Bağlantı Durumu",
        discordStatusConnected = "Bağlı",
        discordStatusConnectedAs = { user -> "$user olarak bağlı" },
        discordStatusConnecting = "Discord Gateway'e bağlanılıyor...",
        discordStatusReconnecting = "Discord'a yeniden bağlanılıyor...",
        discordStatusDisconnected = "Bağlı değil",
        discordStatusAuthFailed = "Oturum süresi doldu veya reddedildi — tekrar giriş yapın",
        discordStatusError = { err -> "Bağlantı hatası: $err" },
        discordReconnect = "Gateway'e Yeniden Bağlan",
    ),
    servers = TurkishServerStrings,
)

val RussianSonataStrings = SonataStrings(
    common = CommonStrings(
        tabHome = "Главная",
        tabExplore = "Обзор",
        tabRadio = "Радио",
        tabLibrary = "Медиатека",
        tabSearch = "Поиск",
        play = "Слушать",
        pause = "Пауза",
        shuffle = "Перемешать",
        radio = "Радио",
        save = "Сохранить",
        saved = "Сохранено",
        download = "Скачать",
        downloaded = "Скачано",
        share = "Поделиться",
        close = "Закрыть",
        cancel = "Отмена",
        back = "Назад",
        clear = "Очистить",
        done = "Готово",
        retry = "Повторить",
        refresh = "Обновить",
        seeAll = "Показать все",
        signIn = "Войти",
        signOut = "Выйти",
        account = "Аккаунт",
        remove = "Удалить",
        create = "Создать",
        ok = "ОК",
        reset = "Сброс",
        off = "Выкл",
        homeHeading = "Главная",
        exploreHeading = "Обзор",
        radioHeading = "Радио",
        libraryHeading = "Медиатека",
        searchPlaceholder = "Поиск песен, исполнителей, альбомов...",
        yourLibrary = "Ваша медиатека",
        signedInStatus = "Выполнен вход",
        notSignedInStatus = "Вход не выполнен",
        signInToSyncLibrary = "Войдите в YouTube Music для синхронизации",
        noHistoryYet = "История прослушиваний пуста",
        playSongsToBuildHistory = "Прослушанные треки появятся здесь",
        searchEmptyTitle = "Поиск музыки",
        searchEmptySubtitle = "Ищите треки, альбомы, артистов и плейлисты",
        noResultsFound = "Ничего не найдено",
        allTab = "Все",
        noDownloadsYet = "Нет скачанных треков",
        noDownloadsHint = "Скачивайте треки или плейлисты для офлайн-прослушивания",
        playAll = "Слушать все",
        downloadAll = "Скачать все",
        removeDownload = "Удалить загрузку",
        removeAllDownloads = "Удалить все загрузки",
        topSongs = "Популярные треки",
        releases = "Релизы",
        fansMightLike = "Похожие исполнители",
        aboutArtist = "Об исполнителе",
        subscribe = "Подписаться",
        subscribed = "Вы подписаны",
        addedArtistToast = { name -> "Подписка на $name" },
        removedArtistToast = { name -> "Удалено: $name" },
        emptyCollection = "Треки не найдены",
        localFolderEmpty = "Локальная папка пуста",
        newPlaylist = "Новый плейлист",
        createPlaylist = "Создать плейлист",
        createNewPlaylistHint = "Создать новый плейлист YouTube Music",
        noPlaylistsYet = "Плейлистов пока нет",
        addingTracksToPlaylist = { n -> "Добавление $n треков в плейлист" },
        addedToPlaylistStatus = { name -> "Добавлено в «$name»" },
        editPlaylist = "Изменить плейлист",
        playlistName = "Название плейлиста",
        playlistDescription = "Описание (необязательно)",
        playlistPrivacy = "Приватность",
        privacyKeep = "Не менять",
        privacyPrivate = "Личный",
        privacyUnlisted = "По ссылке",
        privacyPublic = "Открытый",
        changeCover = "Сменить обложку",
        coverSelected = "Новая обложка выбрана",
        saving = "Сохранение...",
        editPlaylistHint = "Изменения синхронизируются с вашим аккаунтом YouTube Music.",
        updateAvailable = "Доступно обновление",
        downloadNow = "Скачать сейчас",
        installNow = "Установить",
        later = "Позже",
        downloadingApk = "Загрузка APK...",
        offlineLibrary = "Офлайн-медиатека",
        offlinePlaylist = "Офлайн-плейлист",
        artistDefault = "Исполнитель",
        nothingToUpdate = "Нечего обновлять",
        signInToEditPlaylists = "Войдите в YouTube Music, чтобы редактировать плейлисты",
        selectAtLeastOnePlaylist = "Выберите хотя бы один плейлист",
        playlistNameEmpty = "Название плейлиста не может быть пустым",
        noYtmMatchFound = "Совпадающий трек в YouTube Music не найден",
        failedUpdatePlaylist = "Не удалось обновить плейлист",
        failedCreatePlaylist = "Не удалось создать плейлист",
        failedUploadCover = "Не удалось загрузить обложку плейлиста",
        updatedPlaylistAndCover = "Плейлист и обложка обновлены",
        updatedPlaylistCover = "Обложка плейлиста обновлена",
        updatedPlaylist = "Плейлист обновлён",
        addingToPlaylistStatus = "Добавление в плейлист...",
        creatingPlaylistStatus = { name -> "Создание «$name»..." },
        createdPlaylistStatus = { name -> "Создан плейлист «$name»" },
        createdOfflinePlaylistStatus = { name -> "Создан офлайн-плейлист «$name»" },
        addedSingleTrackToPlaylists = { title, count -> "Трек «$title» добавлен в $count плейлист(а/ов)" },
        addedTracksToPlaylistsCount = { tracks, playlists -> "Добавлено треков: $tracks, плейлистов: $playlists" },
        addedToSomePlaylists = { ok, total -> "Добавлено в $ok из $total плейлистов" },
        couldNotAddToPlaylist = "Не удалось добавить в плейлист",
        noTracksInCollection = { title -> "В «$title» треки не найдены" },
        loadingNamedStatus = { title -> "Загрузка: $title..." },
        openingNamedStatus = { title -> "Открытие: $title..." },
        tracksInCollectionStatus = { count, title -> "$count треков в «$title»" },
        offlineSongsCount = { count -> "$count офлайн-треков" },
        offlinePlaylistSongsCount = { count -> "$count треков офлайн-плейлиста" },
        downloadedSongsCount = { count -> "$count скачанных треков" },
        pinned = "Закреплённые",
        pinToLibrary = "Закрепить",
        unpinFromLibrary = "Открепить",
        addToLibrary = "Добавить в медиатеку",
        removeFromLibrary = "Удалить из медиатеки",
        savedInLibrary = "В медиатеке",
        pinnedStatus = { title -> "Закреплено: «$title»" },
        unpinnedStatus = { title -> "Откреплено: «$title»" },
        addedToLibraryStatus = { title -> "Добавлено в медиатеку: «$title»" },
        removedFromLibraryStatus = { title -> "Удалено из медиатеки: «$title»" },
    ),
    feed = FeedStrings(
        topPicksForYou = "Рекомендации для вас",
        lastPlayed = "Недавно прослушанные",
        quickPicks = "Быстрый выбор",
        recentSearches = "Недавние запросы",
        listeningHistory = "История",
        downloads = "Загрузки",
        playlists = "Плейлисты",
        likedSongs = "Понравившиеся",
        albums = "Альбомы",
        artists = "Исполнители",
        songs = "Песни",
        samples = "Фрагменты",
        videos = "Видео",
        signInBannerTitle = "Войдите в YouTube Music",
        signInBannerSubtitle = "Синхронизируйте плейлисты, лайки и Extreme-качество звука",
        recentSearchesSubtitle = "Недавние запросы",
        quickControls = "Быстрые настройки",
        refreshHomeMixes = "Обновить Главную и миксы",
        curatedForYou = "ПОДОБРАНО ДЛЯ ВАС",
        discoveryStation = "Станция открытий",
        myStation = "Моя станция",
        userStationSuffix = { name -> "Станция: $name" },
        artistMixDefaultTitle = "Микс избранного",
        artistMixFeaturing = { artist -> "С участием $artist" },
        artistMixDefaultSubtitle = "Персональный микс артистов",
        newReleaseEyebrow = "НОВЫЙ РЕЛИЗ",
        featuredPlaylistEyebrow = "ПЛЕЙЛИСТ ДНЯ",
        featuredChartEyebrow = "ХИТ-ПАРАД",
        updatedPlaylistEyebrow = "ОБНОВЛЁННЫЙ ПЛЕЙЛИСТ",
        findYourMood = "Настроение",
        yourPlaylists = "Ваши плейлисты",
        playlistLabel = "Плейлист",
        albumLabel = "Альбом",
        singleLabel = "Сингл",
        artistLabel = "Исполнитель",
        trackLabel = "Трек",
        radioMixEyebrow = "РАДИО-МИКС",
        featuredMixes = "Избранные миксы",
        featuredStation = "Рекомендуемая станция",
        artistAndTrackStations = "Станции по артистам и трекам",
        moodAndGenreStations = "Станции по настроению и жанрам",
        artistRadioTitle = { artist -> "Радио $artist" },
        basedOnTrack = { title -> "На основе: $title" },
        trendingViralHits = "В тренде и хиты",
        playlistsAndAlbums = "Плейлисты и альбомы",
        exploreMore = "Больше разделов",
        recentlyPlayedFallback = "Недавние треки",
        allMoodsAndGenres = "Настроения и жанры",
        forgottenFavorites = "Забытое любимое",
        recommendedAlbums = "Рекомендуемые альбомы",
        newReleases = "Новые релизы",
        trending = "В тренде",
        charts = "Чарты",
        similarArtists = "Похожие исполнители",
        fromYourLibrary = "Из вашей медиатеки",
        mixedForYou = "Микс для вас",
        musicVideos = "Музыкальные клипы",
        moodRelax = "Отдых",
        moodWorkout = "Тренировка",
        moodEnergize = "Энергия",
        moodFocus = "Фокус",
        moodCommute = "В дороге",
        moodParty = "Вечеринка",
        moodSleep = "Сон",
        moodFeelGood = "Хорошее настроение",
        moodRomance = "Романтика",
        moodSad = "Грусть",
        moodChill = "Спокойствие",
        moodPodcasts = "Подкасты",
        featuredPlaylistsForYou = "Рекомендованные плейлисты",
        fromTheCommunity = "От сообщества",
        singlesAndEps = "Синглы и EP",
        singles = "Синглы",
        newReleasesForYou = "Новые релизы для вас",
        coversAndRemixes = "Каверы и ремиксы",
        livePerformances = "Живые выступления",
        featuredOn = "Встречается в",
        latestRelease = "Последний релиз",
        albumsAndSingles = "Альбомы и синглы",
        similarToPrefix = { x -> "Похожее на $x" },
        moreFromPrefix = { x -> "Ещё от $x" },
        becauseYouListenedToPrefix = { x -> "Потому что вы слушали $x" },
        recentlyPlayedSubtitle = "Недавно прослушанное",
        personalizedShelvesCount = { n -> "$n персональных полок" },
        searchingQueryStatus = { q -> "Поиск «$q»..." },
        searchResultsHeader = { q -> "Поиск: $q" },
        songsFoundCount = { n -> "Найдено треков: $n" },
        songsFoundYtmJioCount = { n -> "Найдено треков: $n (YTM + JioSaavn)" },
        songsFoundJioCount = { n -> "Найдено треков: $n (JioSaavn)" },
        searchErrorStatus = { msg -> "Ошибка поиска: $msg" },
        homeErrorStatus = { msg -> "Ошибка главной: $msg" },
        artistErrorStatus = { msg -> "Ошибка страницы артиста: $msg" },
        collectionErrorStatus = { msg -> "Ошибка коллекции: $msg" },
    ),
    player = PlayerStrings(
        audioPipeline = "Аудиотракт",
        audioPipelineSubtitle = "Кодек и битность",
        autoplay = "Бесконечный поток",
        autoplayOn = "Бесконечное воспроизведение включено",
        autoplayOff = "Бесконечное воспроизведение выключено",
        shuffleOn = "Перемешивание включено",
        shuffleOff = "Перемешивание выключено",
        repeatOff = "Повтор выключен",
        repeatAll = "Повторять всё",
        repeatOne = "Повторять трек",
        loadingYoutubeMusic = "Загрузка YouTube Music...",
        startingRadio = "Запуск бесконечного радио...",
        radioUnavailable = "Не удалось запустить радио",
        startingPersonalMix = "Запуск личного микса...",
        startingDiscoveryMix = "Запуск микса открытий...",
        sleepTimerPaused = "Таймер сна поставил паузу",
        resolving = "Поиск потока...",
        offlineLabel = "Офлайн",
        liveStream = "Прямой эфир",
        noTrackPlaying = "Ничего не играет",
        playingTrackStatus = { title -> "Воспроизведение: $title" },
        playingTrackOfflineStatus = { title -> "Офлайн: $title" },
        resolvingTrackStatus = { title -> "Загрузка: $title..." },
        addedToPlayNextStatus = { n -> "Добавлено $n треков далее" },
        addedToQueueStatus = { n -> "Добавлено $n треков в очередь" },
        playingNextStatus = { title -> "Следующий: $title" },
        addedTrackToQueueStatus = { title -> "В очереди: $title" },
        mediaCacheClearedStatus = "Кэш аудио и текстов очищен",
        imageCacheClearedStatus = "Кэш обложек очищен",
        updateCheckFailed = "Ошибка проверки обновлений",
        offlinePlaybackError = { msg -> "Ошибка офлайн-воспроизведения: $msg" },
        lyrics = "Текст песни",
        queue = "Очередь",
        upNext = "Далее",
        loadingLyrics = "Загрузка текста...",
        noLyricsAvailable = "Текст песни недоступен",
        lyricsTranslation = "Перевод текста",
        favorite = "Избранное",
        options = "Опции",
        previous = "Предыдущий",
        next = "Следующий",
        tracksCount = { n -> "$n треков" },
        savedCount = { n -> "$n сохранено" },
        wordSynced = "По словам",
        syncedLabel = "Синхронизированный",
        unavailableLabel = "Недоступно",
        lyricsDisplay = "Отображение текста",
        lyricsTiming = "Сдвиг синхронизации",
        syncOffset = "Сдвиг текста",
        translateLyrics = "Перевод текста",
        romanizeLyrics = "Транслитерация",
        sourceLabel = { src -> "Источник: $src" },
        playNext = "Слушать далее",
        addToQueue = "Добавить в очередь",
        addToPlaylist = "В плейлист",
        startRadio = "Включить радио",
        goToAlbum = "К альбому",
        goToArtist = "К исполнителю",
        songInfo = "О треке",
        like = "Нравится",
        unlike = "Убрать лайк",
        dislike = "Не нравится",
        removeFromCache = "Удалить из загрузок",
        playCollection = "Слушать",
        shufflePlay = "Перемешать",
        openAlbum = "Открыть альбом",
        openPlaylist = "Открыть плейлист",
        sleepTimer = "Таймер сна",
        sleepOff = "Выкл",
        sleepEndOfTrack = "После трека",
        sleepMinutesRemaining = { m -> "Осталось $m мин" },
        audioControls = "Управление звуком",
        playbackSpeed = "Скорость",
        skipSilence = "Пропуск тишины",
        shareCardTitle = "Поделиться",
        copyLink = "Копировать ссылку",
        shareMore = "Ещё",
        linkCopied = "Ссылка скопирована",
        audioPipelineCodec = "Кодек",
        audioPipelineBitrate = "Битрейт",
        audioPipelineSampleRate = "Частота",
        audioPipelineOutput = "Вывод",
        audioPipelineSource = "Источник",
        audioPipelineMode = "Режим звука",
        audioPipelineBitTransparent = "Прямой вывод (без ресемплинга и DSP)",
        openSignInPrompt = "Войти в YouTube Music",
        wifiOnlyDownloadBlocked = "Для загрузки требуется Wi-Fi (включён режим только по Wi-Fi)",
        alreadyDownloadedNamed = { title -> "Уже скачано: $title" },
        alreadyDownloadedShort = "Уже скачано",
        downloadingNamed = { title -> "Загрузка: $title..." },
        downloadFailedStreamUnavailable = "Ошибка загрузки: поток недоступен",
        downloadFailedShort = "Ошибка загрузки",
        downloadFailedNamed = { msg -> "Ошибка загрузки: $msg" },
        downloadedNamed = { title -> "Скачано: $title" },
        allTracksAlreadyDownloaded = { count -> "Все $count треков уже скачаны" },
        allTracksAlreadyDownloadedShort = "Все треки уже скачаны",
        downloadingTracksCount = { count -> "Загрузка $count треков..." },
        downloadedCountOfTotal = { saved, total -> "Скачано $saved из $total треков" },
        downloadsClearedStatus = "Загрузки очищены",
        removedFromCacheStatus = "Удалено из кэша",
        signingInYtmStatus = "Вход в YouTube Music...",
        signedInAsStatus = { name -> "Вход выполнен: $name" },
        signedInYtmStatus = "Выполнен вход в YouTube Music",
        signInFailedStatus = { msg -> "Ошибка входа: $msg" },
        signedOutStatus = "Выход выполнен",
        engineInitErrorStatus = { msg -> "Ошибка запуска движка: $msg" },
        recoveringStreamStatus = "Восстановление потока...",
        playbackStoppedStatus = { msg -> "Воспроизведение остановлено: $msg" },
        playbackErrorStatus = { msg -> "Ошибка воспроизведения: $msg" },
        playFailedStatus = { msg -> "Не удалось воспроизвести: $msg" },
        streamBadgeError = "Ошибка",
        updateOpeningInstaller = "Открытие установщика...",
        updatePermissionRequired = "Разрешите установку из неизвестных источников для Sonata и нажмите «Установить».",
        updateNoApkAsset = "В этом релизе не найден APK-файл.",
        updateStartingDownload = "Начало загрузки...",
        updateInstallingVersion = { ver -> "Установка Sonata v$ver..." },
        updateApkMissingRetry = "Скачанный APK не найден; попробуйте снова.",
        musicMode = "Музыка",
        videoMode = "Видео",
        videoQualityWarningTitle = "Качество звука в режиме видео",
        videoQualityWarningBody = "В режиме видео воспроизводится аудиодорожка клипа, поэтому качество звука может быть ниже, чем в режиме музыки.",
        doNotShowAgain = "Больше не показывать",
        switchToVideo = "Перейти к видео",
        videoQuality = "Качество видео",
        videoQualityAuto = "Авто",
        fullscreen = "На весь экран",
        exitFullscreen = "Выйти из полноэкранного режима",
        loadingVideo = "Загрузка видео...",
        videoUnavailableStatus = "Видео недоступно",
    ),
    settings = SettingsStrings(
        settingsTitle = "Настройки",
        settingsSubtitle = "Звук, тема, кэш",
        searchSettingsPlaceholder = "Поиск настроек...",
        searchNoMatch = { q -> "Нет настроек по запросу «$q»" },
        sectionAccount = "АККАУНТ",
        sectionAudioQuality = "КАЧЕСТВО ЗВУКА",
        sectionTheme = "ТЕМА",
        sectionLanguage = "ЯЗЫК",
        sectionGlassMotion = "СТЕКЛО И ДВИЖЕНИЕ",
        sectionPlayerCovers = "ПЛЕЕР И ОБЛОЖКИ",
        catDownloads = "Загрузки",
        catPlayback = "Воспроизведение",
        catContent = "Настройки контента",
        catAppearance = "Внешний вид",
        catPerformance = "Производительность",
        catStorage = "Хранилище",
        catYourData = "Ваши данные",
        catMisc = "Прочее",
        catAccountUpdates = "Аккаунт и обновления",
        streamingQuality = "Качество потока",
        downloadQuality = "Качество загрузки",
        jioSaavnFallback = "Резерв JioSaavn 320 kbps",
        jioSaavnFallbackDesc = "Использовать более высокий битрейт AAC при наличии",
        playbackSummary = "Автопоток, скорость, пропуск тишины, таймер",
        contentSummary = "Полки главной, недавние, настроения, видео",
        performanceSummary = "120 Гц · Стекло и анимация",
        storageSummary = "Лимиты кэша, очистка изображений",
        yourDataSummary = "Резервное копирование и восстановление",
        miscSummary = "Строка состояния, поведение плеера, статистика",
        accountUpdatesSummary = "Сессия, проверка обновлений, версия",
        designTheme = "Тема оформления",
        colorAppearance = "Цветовой режим",
        appLanguage = "Язык",
        appLanguageDesc = "Системный язык Android или English / Türkçe / Русский",
        langSystem = "Системный",
        langEnglish = "English",
        langTurkish = "Türkçe",
        langRussian = "Русский",
        langSystemDesc = "Автоматически повторяет язык устройства Android",
        langEnglishDesc = "Использовать английский язык",
        langTurkishDesc = "Использовать турецкий язык",
        langRussianDesc = "Использовать русский язык",
        themeModeSystem = "Система",
        themeModeLight = "Светлая",
        themeModeDark = "Тёмная",
        themeModeAmoled = "AMOLED",
        liquidGlass = "Liquid Glass",
        liquidGlassDesc = "Кристально прозрачная стеклянная навигация и панели",
        liquidGlassApiHint = "Требуется Android 13+ (API 33)",
        glassPerformanceMode = "Режим производительности стекла",
        glassPerformanceDesc = "Облегчённый рендеринг стекла для плавной прокрутки",
        reduceDynamicBlur = "Уменьшить динамическое размытие",
        reduceDynamicBlurDesc = "Использовать чёткое статичное стекло вместо живого",
        reduceAnimation = "Уменьшить анимацию",
        reduceAnimationDesc = "Свести к минимуму пружинные переходы и волны",
        fullScreenCoverArt = "Полноэкранная обложка",
        fullScreenCoverArtDesc = "Обложка от края до края в портретном плеере",
        animatedArtwork = "Анимированные обложки",
        animatedArtworkDesc = "Воспроизводить живые обложки, если они доступны",
        animatedArtworkCellular = "Анимация по мобильной сети",
        animatedArtworkCellularDesc = "Разрешить загрузку живых обложек без Wi-Fi",
        artworkTintedPages = "Оттенок обложки на страницах",
        artworkTintedPagesDesc = "Добавлять мягкие оттенки альбома на экраны деталей",
        fullScreenDetailCover = "Полноширинная обложка деталей",
        fullScreenDetailCoverDesc = "Растягивать обложку альбома и плейлиста за верхнюю панель",
        showTotalDuration = "Общая длительность",
        showTotalDurationDesc = "Показывать полную длительность трека вместо остатка",
        playerBackdrop = "Фон плеера",
        hdrQualitySources = "Качество и источники",
        footerQualitySources = "Средний размер рассчитан для ~3,5 мин трека. При включении JioSaavn загрузки выбирают 320 kbps AAC, если доступно.",
        includeJioSaavnDownloads = "Включить JioSaavn",
        includeJioSaavnDownloadsDesc = "Искать 320 kbps AAC в JioSaavn для наивысшего качества",
        wifiOnlyDownloads = "Загрузка только по Wi-Fi",
        wifiOnlyDownloadsDesc = "Не расходовать мобильный трафик при скачивании",
        autoDownloadLiked = "Автозагрузка лайков",
        autoDownloadLikedDesc = "Автоматически скачивать понравившиеся треки",
        hdrVideoDownloads = "Загрузка видеоклипов",
        footerVideoDownloads = "Если включено, для треков с клипом на YouTube Music также сохраняется видео для офлайн-режима.",
        downloadVideosIfAvailable = "Скачивать видео, если доступно",
        downloadVideosIfAvailableDesc = "Сохранять клип вместе с аудио для офлайн-просмотра",
        videoDownloadQuality = "Качество загрузки видео",
        videoDl1080Title = "Full HD (1080p)",
        videoDl1080Desc = "Максимальная чёткость для больших экранов",
        videoDl1080Size = "~45 МБ / видео",
        videoDl720Title = "HD (720p)",
        videoDl720Desc = "Отличное качество при умеренном объёме",
        videoDl720Size = "~22 МБ / видео",
        videoDl480Title = "Стандарт (480p)",
        videoDl480Desc = "Лёгкий размер для мобильных экранов",
        videoDl480Size = "~12 МБ / видео",
        videoDl360Title = "Экономия (360p)",
        videoDl360Desc = "Минимальный размер файла для офлайн-видео",
        videoDl360Size = "~7 МБ / видео",
        downloadingVideoStatus = "Загрузка видеоклипа…",
        hdrStorageLocation = "Папка и место на диске",
        footerStorageLocation = "Уже скачанные треки останутся доступными. Новые загрузки сохраняются в выбранную папку.",
        downloadLocation = "Папка загрузок",
        storageLocationInternal = "Внутренняя",
        storageLocationExternal = "Внешняя",
        storageInternalDesc = "Личная папка приложения; удаляется вместе с приложением",
        storageExternalDesc = "Общая папка Music/Sonata; видна другим плеерам",
        storageInternalLabel = "Внутренняя · Хранилище приложения",
        storageExternalLabel = "Внешняя · Music/Sonata",
        downloadedStorage = "Занято загрузками",
        removeAllDownloadsConfirmTitle = "Удалить все загрузки?",
        removeAllDownloadsConfirmBody = { count, bytes -> "Будет удалено $count загруженных треков ($bytes) из офлайн-памяти." },
        deleteAllOfflineSongs = { count -> "Удалить все $count офлайн-треков" },
        hdrQueueTransitions = "Очередь и воспроизведение",
        autoplayDesc = "Продолжать поток похожими треками после завершения очереди",
        crossfade = "Кроссфейд",
        crossfadeSecondsOverlap = { sec -> "${sec} с наложение" },
        durationLabel = "Длительность",
        skipSilenceDesc = "Сокращать тихие паузы во время воспроизведения",
        sleepTimerFadeOut = "Затухание таймера сна",
        sleepTimerFadeOutDesc = "Плавно снижать громкость перед остановкой по таймеру",
        hdrSpeedAndTiming = "Скорость воспроизведения",
        hdrHomeFeed = "Лента Главной",
        footerHomeFeed = "Настройте отображение полок и обрезку видеокарточек на Главной.",
        hideRecents = "Скрыть недавние",
        hideRecentsDesc = "Скрыть полки недавних треков с Главной",
        hideTopPicks = "Скрыть рекомендации",
        hideTopPicksDesc = "Скрыть полку «Рекомендации для вас» на Главной",
        hideHomeMoods = "Скрыть «Настроение»",
        hideHomeMoodsDesc = "Скрыть карусель настроений с Главной",
        hideHomeVideos = "Скрыть видео на Главной",
        hideHomeVideosDesc = "Исключить полки с видеоклипами с Главной",
        squareVideoThumbnails = "Квадратные обложки видео",
        squareVideoThumbnailsDesc = "Обрезать видео 16:9 до квадрата 1:1",
        hideLyricsPreview = "Скрыть превью текста",
        hideLyricsPreviewDesc = "Убрать строку текста над полосой перемотки",
        blurUnfocusedLyrics = "Размытие неактивных строк",
        blurUnfocusedLyricsDesc = "Мягко размывать прошлые и следующие строки текста",
        wordByWordSync = "Синхронизация по словам",
        wordByWordSyncDesc = "Предпочитать караоке с пословной синхронизацией",
        translateLyricsDesc = "Показывать перевод под оригинальной строкой",
        translationLanguage = "Язык перевода",
        romanizeLyricsDesc = "Транслитерировать японский, корейский и кириллицу",
        globalLyricOffset = "Общий сдвиг текста",
        footerAppearanceTheme = "Basic включает минималистичный монохромный стиль для Главной, Обзора, Альбома и Артиста. Violet и Moonflower сохраняют свои темы.",
        hdrDisplayRendering = "Экран и рендеринг",
        footerDisplayRendering = "Снижение частоты кадров или облегчённый режим стекла уменьшают нагрузку на GPU и экономят заряд.",
        refreshRateHigh = "Повышенная частота 120 Гц",
        refreshRateDesc = "Плавная прокрутка и анимация с высокой частотой",
        hdrStorageCache = "Хранилище и кэш",
        downloadedSongs = "Скачанные треки",
        clearAudioCache = "Очистить кэш аудио и текстов",
        clearAudioCacheDesc = "Удалить кэшированные аудиосегменты и тексты песен",
        clearImageCache = "Очистить кэш обложек",
        clearImageCacheDesc = "Удалить кэшированные обложки из памяти и с диска",
        hdrBackupRestore = "Резервная копия",
        footerBackupRestore = "Резервная копия настроек — это JSON без cookie-файлов и учётных данных.",
        exportBackup = "Экспортировать настройки",
        exportBackupDesc = "Сохранить настройки и сдвиги текстов в JSON",
        importBackup = "Импортировать настройки",
        importBackupDesc = "Восстановить настройки из ранее сохранённого JSON",
        hdrPlayerSystem = "Плеер и система",
        hideStatusBarPlayer = "Скрывать строку состояния",
        hideStatusBarPlayerDesc = "Полноэкранный режим в окне «Сейчас играет»",
        hideQualityBadge = "Скрыть значок качества",
        hideQualityBadgeDesc = "Убрать плашку формата звука под полосой прогресса",
        stopMusicOnClose = "Останавливать при закрытии",
        stopMusicOnCloseDesc = "Останавливать воспроизведение при смахивании приложения",
        showNerdStats = "Техническая статистика",
        showNerdStatsDesc = "Показывать кодек, битрейт, частоту и источник кэша в плеере",
        hdrYtmAccount = "Аккаунт YouTube Music",
        connectedStatus = "Подключено",
        ytmSessionActive = "Сессия YouTube Music активна",
        signInYtmSettingsSubtitle = "Войдите для персональных подборок, плейлистов и лайков",
        signOutDesc = "Очистить локальные cookie сессии YouTube Music",
        signInYtmTitle = "Войти в YouTube Music",
        signInYtmDesc = "Открыть встроенный браузер для входа",
        hdrAboutUpdates = "О приложении и обновления",
        footerAboutUpdates = "Sonata Android — полностью открытый клиент без трекеров и рекламных SDK.",
        sonataVersion = "Версия Sonata",
        downloadUpdateVersion = { ver -> "Скачать обновление ($ver)" },
        openReleaseApk = "Открыть загрузку APK",
        checkForUpdates = "Проверить обновления",
        checkingForUpdates = "Проверка обновлений...",
        compareGithubReleases = "Сравнить текущую сборку с GitHub Releases",
        footerStreamingQuality = "Более высокие уровни выбирают Opus 251 / AAC 320 kbps при наличии.",
        hdrOrderedHighestToSmallest = "От максимального к экономичному",
        footerDownloadQuality = "Средний размер рассчитан для ~3,5 мин трека. Реальный размер зависит от длины и кодека.",
        qualityLossless = "Максимальное",
        qualityHigh = "Высокое",
        qualityMedium = "Среднее",
        qualityLow = "Экономия трафика",
        qualityLosslessDetail = "Максимальное / Lossless",
        qualityHighDetail = "Высокое (256–320 kbps)",
        qualityMediumDetail = "Среднее (128 kbps)",
        qualityLowDetail = "Экономия данных (64 kbps)",
        qualityLosslessDesc = "Поток с наивысшим доступным битрейтом",
        qualityHighDesc = "Лучший баланс качества и скорости запуска",
        qualityMediumDesc = "Стандартный битрейт потока",
        qualityLowDesc = "Минимальный расход мобильного трафика",
        dlHighest = "Наивысшее",
        dlHigh = "Высокое",
        dlStandard = "Стандартное",
        dlLow = "Экономия",
        themeVioletDesc = "Глубокое аметистовое стекло",
        themeMoonflowerDesc = "Ботанический шёлк и цветочная аура",
        themeBasicDesc = "Монохромный редакционный дизайн",
        backdropMesh = "Сетка обложки",
        backdropWaves = "Волны палитры",
        backdropBlurred = "Размытая обложка",
        backdropMeshDesc = "Цветовая сетка, построенная из обложки альбома",
        backdropWavesDesc = "Плавный анимированный градиент палитры",
        backdropBlurredDesc = "Мягко размытая обложка во весь экран",
        loudnessNormalization = "Выравнивание громкости",
        loudnessNormalizationDesc = "Поддерживать ровную воспринимаемую громкость между треками",
        equalizerTitle = "5-полосный эквалайзер",
        eqPresetFlat = "Ровный",
        eqPresetBassBoost = "Усиление баса",
        eqPresetVocal = "Вокал",
        eqPresetAcoustic = "Акустика",
        eqPresetElectronic = "Электроника",
        eqPresetLateNight = "Ночной режим",
        eqPresetCustom = "Свой",
        storageFreeOf = { free, total -> "Свободно $free из $total" },
    ),
    discord = DiscordStrings(
        catDiscord = "Discord Rich Presence",
        discordSummary = "Мобильный Discord RPC, статус, App ID",
        hdrDiscordAccount = "Аккаунт Discord",
        hdrDiscordPresence = "Отображение статуса",
        hdrDiscordAppId = "ID приложения Discord",
        footerDiscordAppId = "Используйте стандартный ID Sonata или вставьте свой Application ID из Discord Developer Portal.",
        discordSignInTitle = "Войти в Discord",
        discordSignInSubtitle = "Авторизуйтесь в Discord для показа статуса прослушивания",
        discordSignInCompleting = "Завершение входа...",
        discordSignInAction = "Войти в Discord",
        discordSignInActionDesc = "Подключите аккаунт Discord для мобильного Rich Presence",
        discordSignOutAction = "Выйти из Discord",
        discordSignOutDesc = "Очистить локальную сессию Discord и остановить RPC",
        discordConsentTitle = "Уведомление о Discord Rich Presence",
        discordConsentBody = "Sonata подключается напрямую к Discord Gateway с использованием вашей сессии Discord, чтобы статус прослушивания отображался с телефона без ПК.\n\nУчётные данные сессии хранятся только локально на устройстве, исключены из резервных копий настроек и не логируются.",
        discordConsentContinue = "Понятно, продолжить",
        discordEnableRpc = "Включить Discord Rich Presence",
        discordEnableRpcDesc = "Показывать текущий трек в вашем профиле Discord",
        discordActivityType = "Тип активности",
        discordActivityListening = "Слушает",
        discordActivityPlaying = "Играет",
        discordActivityWatching = "Смотрит",
        discordActivityCompeting = "Соревнуется",
        discordShowTimestamps = "Показывать таймер трека",
        discordShowTimestampsDesc = "Отображать прогресс с учётом скорости воспроизведения",
        discordShowArtwork = "Показывать обложку трека",
        discordShowArtworkDesc = "Проксировать обложки через внешние ресурсы Discord",
        discordShowButtons = "Показывать кнопку «Слушать»",
        discordShowButtonsDesc = "Добавить ссылку на YouTube Music в карточку активности",
        discordAppIdLabel = "Application ID",
        discordAppIdPlaceholder = "Введите Discord Application ID",
        discordAppIdApply = "Сохранить",
        discordAppIdReset = "Сбросить",
        discordAppIdInvalid = "Введите корректный числовой ID приложения Discord (15–22 цифры)",
        discordStatusLabel = "Статус подключения",
        discordStatusConnected = "Подключено",
        discordStatusConnectedAs = { user -> "Подключено: $user" },
        discordStatusConnecting = "Подключение к Discord Gateway...",
        discordStatusReconnecting = "Переподключение к Discord...",
        discordStatusDisconnected = "Отключено",
        discordStatusAuthFailed = "Сессия истекла или отклонена — войдите снова",
        discordStatusError = { err -> "Ошибка подключения: $err" },
        discordReconnect = "Переподключить Gateway",
    ),
    servers = RussianServerStrings,
)

/**
 * Localizes well-known YouTube Music feed shelf titles while preserving unknown titles verbatim.
 */
fun localizeShelfTitle(raw: String, strings: SonataStrings): String {
    val trimmed = raw.trim()
    val lower = trimmed.lowercase(Locale.ROOT)
    if (lower.startsWith("similar to ") && trimmed.length > "similar to ".length) {
        return strings.similarToPrefix(trimmed.substring("similar to ".length).trim())
    }
    if (lower.startsWith("more from ") && trimmed.length > "more from ".length) {
        return strings.moreFromPrefix(trimmed.substring("more from ".length).trim())
    }
    if (lower.startsWith("because you listened to ") && trimmed.length > "because you listened to ".length) {
        return strings.becauseYouListenedToPrefix(trimmed.substring("because you listened to ".length).trim())
    }
    return when (lower) {
        "quick picks" -> strings.quickPicks
        "listen again" -> strings.lastPlayed
        "recently played" -> strings.recentlyPlayedFallback
        "forgotten favorites" -> strings.forgottenFavorites
        "recommended albums" -> strings.recommendedAlbums
        "new releases" -> strings.newReleases
        "new releases for you" -> strings.newReleasesForYou
        "trending", "trending songs" -> strings.trending
        "charts" -> strings.charts
        "moods & genres", "moods and genres" -> strings.allMoodsAndGenres
        "similar artists" -> strings.similarArtists
        "fans might also like" -> strings.fansMightLike
        "from your library" -> strings.fromYourLibrary
        "mixed for you", "your mixes" -> strings.mixedForYou
        "music videos" -> strings.musicVideos
        "featured playlists for you" -> strings.featuredPlaylistsForYou
        "from the community" -> strings.fromTheCommunity
        "top songs" -> strings.topSongs
        "top picks for you" -> strings.topPicksForYou
        "latest release" -> strings.latestRelease
        "albums & singles" -> strings.albumsAndSingles
        "covers & remixes", "covers and remixes" -> strings.coversAndRemixes
        "live performances" -> strings.livePerformances
        "featured on" -> strings.featuredOn
        "your playlists" -> strings.yourPlaylists
        "playlists" -> strings.playlists
        "albums" -> strings.albums
        "singles" -> strings.singles
        "singles & eps" -> strings.singlesAndEps
        "artists" -> strings.artists
        "songs" -> strings.songs
        "videos" -> strings.videos
        else -> trimmed
    }
}

/**
 * Localizes common mood & genre labels across supported app languages.
 */
fun localizeMoodTitle(raw: String, strings: SonataStrings): String {
    val trimmed = raw.trim()
    return when (trimmed.lowercase(Locale.ROOT)) {
        "relax" -> strings.moodRelax
        "workout" -> strings.moodWorkout
        "energize", "energy" -> strings.moodEnergize
        "focus" -> strings.moodFocus
        "commute" -> strings.moodCommute
        "party" -> strings.moodParty
        "sleep" -> strings.moodSleep
        "feel good" -> strings.moodFeelGood
        "romance" -> strings.moodRomance
        "sad" -> strings.moodSad
        "chill" -> strings.moodChill
        "podcasts" -> strings.moodPodcasts
        "charts" -> strings.charts
        else -> trimmed
    }
}

/**
 * Resolves active [SonataStrings] from [appLanguage] and the current device [systemLocale].
 */
fun resolveSonataStrings(
    appLanguage: AppLanguage,
    systemLocale: Locale = Locale.getDefault(),
): SonataStrings = when (appLanguage) {
    AppLanguage.ENGLISH -> EnglishSonataStrings
    AppLanguage.TURKISH -> TurkishSonataStrings
    AppLanguage.RUSSIAN -> RussianSonataStrings
    AppLanguage.SYSTEM -> {
        when (systemLocale.language.lowercase(Locale.ROOT)) {
            "tr" -> TurkishSonataStrings
            "ru" -> RussianSonataStrings
            else -> EnglishSonataStrings
        }
    }
}

val LocalSonataStrings = staticCompositionLocalOf { EnglishSonataStrings }
