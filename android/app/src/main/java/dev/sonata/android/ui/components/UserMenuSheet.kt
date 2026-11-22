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
import dev.sonata.android.ui.theme.LunariaColors
import dev.sonata.android.ui.theme.sonataSpecularRim
import uniffi.sonata_android.AndroidAccountProfile

private val AccountMenuGlassTint = Color(0xFF14131A).copy(alpha = 0.44f)
private val AccountMenuFallbackBg = Color(0xFF14121E).copy(alpha = 0.96f)
private val AccountInnerCardBg = Color.White.copy(alpha = 0.07f)
private val AccountInnerCardBorder = Color.White.copy(alpha = 0.11f)
private val AccountIconCircleBg = Color.White.copy(alpha = 0.10f)

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
                        .shadow(24.dp, sheetShape, clip = false)
                        .liquidGlass(shape = sheetShape, surfaceTint = AccountMenuGlassTint)
                        .sonataSpecularRim(shape = sheetShape, strokeWidth = 0.75.dp)
                } else {
                    Modifier
                        .widthIn(max = 460.dp)
                        .fillMaxWidth()
                        .heightIn(max = 680.dp)
                        .shadow(24.dp, sheetShape, clip = false)
                        .clip(sheetShape)
                        .background(AccountMenuFallbackBg, sheetShape)
                        .border(0.75.dp, Color.White.copy(alpha = 0.14f), sheetShape)
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
                    // Header row
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = "Account",
                            style = MaterialTheme.typography.headlineMedium.copy(fontWeight = FontWeight.Bold),
                            color = LunariaColors.MoonGlow,
                            modifier = Modifier.weight(1f),
                        )
                        CircleGlyph(
                            icon = SonataIcons.Close,
                            contentDescription = "Close Menu",
                            onClick = onDismiss,
                        )
                    }

                    // Profile card
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(cardShape)
                            .background(AccountInnerCardBg)
                            .border(0.5.dp, AccountInnerCardBorder, cardShape)
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
                                .background(AccountIconCircleBg),
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
                                    tint = Color.White.copy(alpha = 0.90f),
                                    modifier = Modifier.size(22.dp),
                                )
                            }
                        }

                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = if (signedIn) {
                                    accountProfile?.name?.ifBlank { "YouTube Music" } ?: "YouTube Music"
                                } else {
                                    "Guest Session"
                                },
                                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                                color = LunariaColors.MoonGlow,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = if (signedIn) {
                                    accountProfile?.handle
                                        ?: accountProfile?.email
                                        ?: "Connected to YouTube Music"
                                } else {
                                    "Sign in to sync library & mixes"
                                },
                                style = MaterialTheme.typography.bodySmall,
                                color = Color.White.copy(alpha = 0.66f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }

                        Box(
                            modifier = Modifier
                                .clip(pillShape)
                                .background(
                                    if (signedIn) Color.White.copy(alpha = 0.14f) else SonataAccent,
                                )
                                .clickable {
                                    onDismiss()
                                    if (signedIn) onSignOut() else onSignIn()
                                }
                                .padding(horizontal = 14.dp, vertical = 8.dp),
                        ) {
                            Text(
                                text = if (signedIn) "Sign Out" else "Sign In",
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = if (signedIn) Color.White else LunariaColors.EclipseBase,
                            )
                        }
                    }

                    // 2x2 Monochrome Action Grid (zero rainbow colors)
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        UserMenuActionTile(
                            title = "Downloads",
                            subtitle = if (downloadedCount == 0) "Offline library" else "$downloadedCount saved",
                            icon = SonataIcons.Download,
                            shape = tileShape,
                            onClick = {
                                onDismiss()
                                onOpenDownloads()
                            },
                            modifier = Modifier.weight(1f),
                        )
                        UserMenuActionTile(
                            title = "History",
                            subtitle = "Recent searches",
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
                            title = "Audio Pipeline",
                            subtitle = "Codec & bit depth",
                            icon = SonataIcons.GraphicEq,
                            shape = tileShape,
                            onClick = {
                                onDismiss()
                                onOpenAudioPipeline()
                            },
                            modifier = Modifier.weight(1f),
                        )
                        UserMenuActionTile(
                            title = "Settings",
                            subtitle = "Audio, theme, cache",
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
                            .background(AccountInnerCardBg)
                            .border(0.5.dp, AccountInnerCardBorder, cardShape)
                            .padding(14.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Text(
                            text = "Quick Controls",
                            style = MaterialTheme.typography.labelMedium.copy(
                                fontWeight = FontWeight.SemiBold,
                                letterSpacing = 0.4.sp,
                            ),
                            color = Color.White.copy(alpha = 0.66f),
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
                                ThemeMode.SYSTEM -> "Auto"
                                ThemeMode.DARK -> "Dark"
                                ThemeMode.AMOLED -> "AMOLED"
                                ThemeMode.LIGHT -> "Light"
                            }
                            QuickTogglePill(
                                label = themeLabel,
                                active = true,
                                onClick = onCycleThemeMode,
                                modifier = Modifier.weight(1f),
                            )
                        }

                        // Refresh Home & Mixes row
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(12.dp))
                                .background(Color.White.copy(alpha = 0.08f))
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
                                tint = SonataAccent,
                                modifier = Modifier.size(18.dp),
                            )
                            Spacer(Modifier.width(10.dp))
                            Text(
                                text = "Refresh Home & Mixes",
                                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                                color = LunariaColors.MoonGlow,
                                modifier = Modifier.weight(1f),
                            )
                            Icon(
                                imageVector = SonataIcons.ChevronRight,
                                contentDescription = null,
                                tint = Color.White.copy(alpha = 0.56f),
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
    Column(
        modifier = modifier
            .clip(shape)
            .background(AccountInnerCardBg)
            .border(0.5.dp, AccountInnerCardBorder, shape)
            .clickable(onClick = onClick)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(
            modifier = Modifier
                .size(34.dp)
                .clip(CircleShape)
                .background(AccountIconCircleBg),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = icon,
                contentDescription = title,
                tint = Color.White.copy(alpha = 0.88f),
                modifier = Modifier.size(18.dp),
            )
        }
        Text(
            text = title,
            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
            color = LunariaColors.MoonGlow,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 11.5.sp),
            color = Color.White.copy(alpha = 0.62f),
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
    val pillShape = RoundedCornerShape(percent = 50)
    Box(
        modifier = modifier
            .height(34.dp)
            .clip(pillShape)
            .background(
                if (active) SonataAccent.copy(alpha = 0.22f) else Color.White.copy(alpha = 0.06f),
            )
            .border(
                width = 0.5.dp,
                color = if (active) SonataAccent.copy(alpha = 0.62f) else AccountInnerCardBorder,
                shape = pillShape,
            )
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium.copy(
                fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
            ),
            color = if (active) LunariaColors.MoonGlow else Color.White.copy(alpha = 0.62f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
