/*
 * Floating Liquid Glass Account Menu for Sonata Android.
 *
 * Uses an in-window sibling overlay with ZERO dark dimming scrim so underlying
 * page content refracts live through [Modifier.liquidGlass]. Uses clean monochrome
 * surfaces with subtle Lunaria Lavender highlights and zero rainbow color accents.
 */
package dev.sonata.android.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import dev.sonata.android.LocalAppBackdrop
import dev.sonata.android.LocalLiquidGlassEnabled
import dev.sonata.android.LocalReduceDynamicBlur
import dev.sonata.android.backdrop.Backdrop
import dev.sonata.android.data.SonataSettingsState
import dev.sonata.android.data.ThemeMode
import dev.sonata.android.isGlassSupported
import dev.sonata.android.liquidGlass
import dev.sonata.android.ui.CircleGlyph
import dev.sonata.android.ui.SonataAccent
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.rememberArtworkRequest
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.theme.LocalLunariaColors
import dev.sonata.android.ui.theme.sonataSpecularRim
import uniffi.sonata_android.AndroidAccountProfile

/**
 * Floating Liquid Glass Account Menu.
 * Must be placed as an in-window sibling OUTSIDE [backdrop] so live backdrop sampling
 * never introduces a RenderNode cycle.
 */
@Composable
fun UserMenuSheet(
    visible: Boolean = true,
    signedIn: Boolean,
    accountProfile: AndroidAccountProfile?,
    settings: SonataSettingsState,
    downloadedCount: Int,
    backdrop: Backdrop? = null,
    onDismiss: () -> Unit,
    onSignIn: () -> Unit,
    onSignOut: () -> Unit,
    onOpenDownloads: () -> Unit,
    onOpenHistory: () -> Unit,
    onOpenAudioPipeline: () -> Unit,
    onOpenSettings: () -> Unit,
    onToggleLiquidGlass: () -> Unit,
    onToggleAnimatedArt: () -> Unit,
    onCycleThemeMode: () -> Unit,
    onRefreshLibrary: () -> Unit,
    modifier: Modifier = Modifier,
) {
    AnimatedVisibility(
        visible = visible,
        enter = fadeIn() + slideInVertically { it / 8 } + scaleIn(
            initialScale = 0.95f,
            transformOrigin = TransformOrigin(0.5f, 1f),
        ),
        exit = fadeOut() + slideOutVertically { it / 8 } + scaleOut(
            targetScale = 0.95f,
            transformOrigin = TransformOrigin(0.5f, 1f),
        ),
        modifier = modifier.fillMaxSize(),
    ) {
        val targetBackdrop = backdrop ?: LocalAppBackdrop.current
        val tokens = LocalLunariaColors.current
        val innerCardBg = tokens.onSurfacePrimary.copy(alpha = if (tokens.isDark) 0.07f else 0.05f)
        val innerCardBorder = tokens.cardStroke
        val iconCircleBg = tokens.onSurfacePrimary.copy(alpha = if (tokens.isDark) 0.10f else 0.08f)
        val outsideInteraction = remember { MutableInteractionSource() }
        val sheetShape = remember { RoundedCornerShape(28.dp) }
        val tileShape = remember { RoundedCornerShape(18.dp) }
        val cardShape = remember { RoundedCornerShape(20.dp) }
        val pillShape = remember { RoundedCornerShape(percent = 50) }

        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Transparent)
                .clickable(
                    interactionSource = outsideInteraction,
                    indication = null,
                    onClick = onDismiss,
                )
                .statusBarsPadding()
                .navigationBarsPadding()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            contentAlignment = Alignment.BottomCenter,
        ) {
            CompositionLocalProvider(LocalAppBackdrop provides targetBackdrop) {
                val glassOn = LocalLiquidGlassEnabled.current &&
                    isGlassSupported() &&
                    !LocalReduceDynamicBlur.current
                val containerModifier = if (glassOn) {
                    Modifier
                        .widthIn(max = 460.dp)
                        .fillMaxWidth()
                        .heightIn(max = 680.dp)
                        .shadow(24.dp, sheetShape, clip = false, ambientColor = tokens.glassShadowColor, spotColor = tokens.glassShadowColor)
                        .liquidGlass(shape = sheetShape, surfaceTint = tokens.glassTint)
                        .sonataSpecularRim(shape = sheetShape, strokeWidth = 0.75.dp)
                } else {
                    Modifier
                        .widthIn(max = 460.dp)
                        .fillMaxWidth()
                        .heightIn(max = 680.dp)
                        .shadow(24.dp, sheetShape, clip = false, ambientColor = tokens.glassShadowColor, spotColor = tokens.glassShadowColor)
                        .clip(sheetShape)
                        .background(tokens.eclipseSurface.copy(alpha = 0.96f), sheetShape)
                        .border(0.75.dp, tokens.cardStroke, sheetShape)
                }

                val cardInteraction = remember { MutableInteractionSource() }
                Column(
                    modifier = containerModifier
                        .clickable(
                            interactionSource = cardInteraction,
                            indication = null,
                            onClick = {},
                        )
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 18.dp, vertical = 18.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    val strings = LocalSonataStrings.current
                    // Header row
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = strings.account,
                            style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
                            color = tokens.onSurfacePrimary,
                            modifier = Modifier.weight(1f),
                        )
                        CircleGlyph(
                            icon = SonataIcons.Close,
                            contentDescription = strings.close,
                            onClick = onDismiss,
                        )
                    }

                    // Profile card
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(cardShape)
                            .background(innerCardBg)
                            .border(0.5.dp, innerCardBorder, cardShape)
                            .padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        val avatarRequest = rememberArtworkRequest(
                            url = if (signedIn) accountProfile?.avatarUrl else null,
                            widthPx = 144,
                            heightPx = 144,
                        )
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .clip(CircleShape)
                                .background(iconCircleBg),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (avatarRequest != null) {
                                AsyncImage(
                                    model = avatarRequest,
                                    contentDescription = null,
                                    contentScale = ContentScale.Crop,
                                    modifier = Modifier.fillMaxSize(),
                                )
                            } else {
                                Icon(
                                    imageVector = SonataIcons.Person,
                                    contentDescription = null,
                                    tint = tokens.onSurfacePrimary,
                                    modifier = Modifier.size(22.dp),
                                )
                            }
                        }

                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = if (signedIn) {
                                    accountProfile?.name?.ifBlank { "YouTube Music" } ?: "YouTube Music"
                                } else {
                                    strings.notSignedInStatus
                                },
                                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                                color = tokens.onSurfacePrimary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = if (signedIn) {
                                    accountProfile?.handle
                                        ?: accountProfile?.email
                                        ?: strings.ytmSessionActive
                                } else {
                                    strings.signInYtmSettingsSubtitle
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = tokens.onSurfaceSecondary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }

                        val signInBg = tokens.petalLavender
                        val signInFg = if (signInBg.luminance() > 0.5f) Color.Black else Color.White
                        Box(
                            modifier = Modifier
                                .clip(pillShape)
                                .background(
                                    if (signedIn) tokens.onSurfacePrimary.copy(alpha = 0.12f) else signInBg,
                                )
                                .clickable {
                                    onDismiss()
                                    if (signedIn) onSignOut() else onSignIn()
                                }
                                .padding(horizontal = 14.dp, vertical = 8.dp),
                        ) {
                            Text(
                                text = if (signedIn) strings.signOut else strings.signIn,
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = if (signedIn) tokens.onSurfacePrimary else signInFg,
                            )
                        }
                    }

                    // 2x2 Action Grid
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        UserMenuActionTile(
                            title = strings.downloads,
                            subtitle = if (downloadedCount == 0) strings.downloads else strings.savedCount(downloadedCount),
                            icon = SonataIcons.Download,
                            shape = tileShape,
                            onClick = {
                                onDismiss()
                                onOpenDownloads()
                            },
                            modifier = Modifier.weight(1f),
                        )
                        UserMenuActionTile(
                            title = strings.listeningHistory,
                            subtitle = strings.recentSearchesSubtitle,
                            icon = SonataIcons.History,
                            shape = tileShape,
                            onClick = {
                                onDismiss()
                                onOpenHistory()
                            },
                            modifier = Modifier.weight(1f),
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        UserMenuActionTile(
                            title = strings.audioPipeline,
                            subtitle = strings.audioPipelineSubtitle,
                            icon = SonataIcons.GraphicEq,
                            shape = tileShape,
                            onClick = {
                                onDismiss()
                                onOpenAudioPipeline()
                            },
                            modifier = Modifier.weight(1f),
                        )
                        UserMenuActionTile(
                            title = strings.settingsTitle,
                            subtitle = strings.settingsSubtitle,
                            icon = SonataIcons.Settings,
                            shape = tileShape,
                            onClick = {
                                onDismiss()
                                onOpenSettings()
                            },
                            modifier = Modifier.weight(1f),
                        )
                    }

                    // Quick Preferences Strip
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(cardShape)
                            .background(innerCardBg)
                            .border(0.5.dp, innerCardBorder, cardShape)
                            .padding(14.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Text(
                            text = strings.quickControls,
                            style = MaterialTheme.typography.labelMedium.copy(
                                fontWeight = FontWeight.SemiBold,
                                letterSpacing = 0.4.sp,
                            ),
                            color = tokens.onSurfaceSecondary,
                        )

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            QuickTogglePill(
                                label = "Glass",
                                active = settings.liquidGlassEnabled,
                                onClick = onToggleLiquidGlass,
                                modifier = Modifier.weight(1f),
                            )
                            QuickTogglePill(
                                label = "Motion Art",
                                active = settings.animatedArtwork,
                                onClick = onToggleAnimatedArt,
                                modifier = Modifier.weight(1f),
                            )
                            val themeLabel = when (settings.themeMode) {
                                ThemeMode.VIOLET_LIGHT -> "Violet Light"
                                ThemeMode.VIOLET_DARK -> "Violet Dark"
                                ThemeMode.MOONFLOWER_LIGHT -> "Moonflower Light"
                                ThemeMode.MOONFLOWER_DARK -> "Moonflower Dark"
                                ThemeMode.BASIC_LIGHT -> "Basic Light"
                                ThemeMode.BASIC_DARK -> "Basic Dark"
                                ThemeMode.SYSTEM -> "Auto"
                                ThemeMode.AMOLED -> "AMOLED"
                            }
                            QuickTogglePill(
                                label = themeLabel,
                                active = true,
                                onClick = onCycleThemeMode,
                                modifier = Modifier.weight(1.25f),
                            )
                        }

                        // Refresh Home & Mixes row
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(12.dp))
                                .background(tokens.onSurfacePrimary.copy(alpha = 0.08f))
                                .clickable {
                                    onDismiss()
                                    onRefreshLibrary()
                                }
                                .padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                imageVector = SonataIcons.Sparkles,
                                contentDescription = null,
                                tint = tokens.petalLavender,
                                modifier = Modifier.size(18.dp),
                            )
                            Spacer(Modifier.width(10.dp))
                            Text(
                                text = strings.refreshHomeMixes,
                                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                                color = tokens.onSurfacePrimary,
                                modifier = Modifier.weight(1f),
                            )
                            Icon(
                                imageVector = SonataIcons.ChevronRight,
                                contentDescription = null,
                                tint = tokens.onSurfaceSecondary,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun UserMenuActionTile(
    title: String,
    subtitle: String,
    icon: ImageVector,
    shape: RoundedCornerShape,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    Column(
        modifier = modifier
            .clip(shape)
            .background(tokens.onSurfacePrimary.copy(alpha = if (tokens.isDark) 0.07f else 0.05f))
            .border(0.5.dp, tokens.cardStroke, shape)
            .clickable(onClick = onClick)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(
            modifier = Modifier
                .size(34.dp)
                .clip(CircleShape)
                .background(tokens.onSurfacePrimary.copy(alpha = if (tokens.isDark) 0.10f else 0.08f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = title,
                tint = tokens.onSurfacePrimary,
                modifier = Modifier.size(18.dp),
            )
        }
        Text(
            text = title,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
            color = tokens.onSurfacePrimary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 11.5.sp),
            color = tokens.onSurfaceSecondary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun QuickTogglePill(
    label: String,
    active: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tokens = LocalLunariaColors.current
    val pillShape = RoundedCornerShape(percent = 50)
    Box(
        modifier = modifier
            .height(34.dp)
            .clip(pillShape)
            .background(
                if (active) tokens.petalLavender.copy(alpha = 0.22f) else tokens.onSurfacePrimary.copy(alpha = 0.06f),
            )
            .border(
                width = 0.5.dp,
                color = if (active) tokens.petalLavender.copy(alpha = 0.62f) else tokens.cardStroke,
                shape = pillShape,
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium.copy(
                fontSize = 11.5.sp,
                fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
            ),
            color = if (active) tokens.onSurfacePrimary else tokens.onSurfaceSecondary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
