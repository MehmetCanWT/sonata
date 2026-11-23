package dev.sonata.android.ui.components

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import coil3.compose.AsyncImage
import dev.sonata.android.ui.SonataIcons
import dev.sonata.android.ui.i18n.LocalSonataStrings
import dev.sonata.android.ui.theme.LocalLunariaColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream

/**
 * Returns `true` when [browseId] points to a user-created YouTube Music playlist
 * whose title, description, privacy, and custom cover image can be edited.
 */
fun isEditableUserPlaylist(browseId: String?, signedIn: Boolean): Boolean {
    if (!signedIn || browseId.isNullOrBlank()) return false
    val normalized = browseId.trim().removePrefix("VL")
    if (normalized.isEmpty()) return false
    if (normalized.equals("LM", ignoreCase = true) ||
        normalized.equals("SE", ignoreCase = true) ||
        normalized.equals("SS", ignoreCase = true)
    ) {
        return false
    }
    return normalized.startsWith("PL")
}

private data class EncodedCoverImage(
    val bytes: ByteArray,
    val mimeType: String,
)

private suspend fun readAndPrepareCoverImage(
    context: Context,
    uri: Uri,
): EncodedCoverImage? = withContext(Dispatchers.IO) {
    val rawBytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
        ?: return@withContext null
    if (rawBytes.isEmpty()) return@withContext null

    val detectedMime = context.contentResolver.getType(uri)?.lowercase().orEmpty()
    val isSupportedDirect = (detectedMime == "image/jpeg" || detectedMime == "image/png") &&
        rawBytes.size <= 2 * 1024 * 1024
    if (isSupportedDirect) {
        return@withContext EncodedCoverImage(rawBytes, detectedMime)
    }

    // Re-encode large or WebP/HEIF images into a crisp <=1600px JPEG so YouTube Music accepts them cleanly.
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(rawBytes, 0, rawBytes.size, bounds)
    var sampleSize = 1
    val maxDim = maxOf(bounds.outWidth, bounds.outHeight)
    while (maxDim / sampleSize > 1600) {
        sampleSize *= 2
    }
    val decodeOpts = BitmapFactory.Options().apply { inSampleSize = sampleSize }
    val decoded = BitmapFactory.decodeByteArray(rawBytes, 0, rawBytes.size, decodeOpts)
        ?: return@withContext EncodedCoverImage(rawBytes, detectedMime.ifBlank { "image/jpeg" })

    val scaled = try {
        val w = decoded.width
        val h = decoded.height
        if (maxOf(w, h) > 1600) {
            val scale = 1600f / maxOf(w, h).toFloat()
            Bitmap.createScaledBitmap(
                decoded,
                (w * scale).toInt().coerceAtLeast(1),
                (h * scale).toInt().coerceAtLeast(1),
                true,
            )
        } else {
            decoded
        }
    } catch (_: Throwable) {
        decoded
    }

    val out = ByteArrayOutputStream()
    scaled.compress(Bitmap.CompressFormat.JPEG, 92, out)
    if (scaled !== decoded) scaled.recycle()
    decoded.recycle()
    EncodedCoverImage(out.toByteArray(), "image/jpeg")
}

/**
 * Modal dialog for editing a YouTube Music playlist's title, description, privacy,
 * and custom cover art directly from the Android client.
 */
@Composable
fun EditPlaylistDialog(
    initialTitle: String,
    currentCoverUrl: String?,
    onDismiss: () -> Unit,
    onSave: (
        title: String?,
        description: String?,
        privacy: String?,
        coverBytes: ByteArray?,
        mimeType: String?,
        onComplete: (Boolean, String) -> Unit,
    ) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val strings = LocalSonataStrings.current
    val tokens = LocalLunariaColors.current

    var title by remember(initialTitle) { mutableStateOf(initialTitle) }
    var description by remember { mutableStateOf("") }
    var privacy by remember { mutableStateOf<String?>(null) }
    var selectedCoverUri by remember { mutableStateOf<Uri?>(null) }
    var saving by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    val imagePicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent(),
    ) { uri ->
        if (uri != null) {
            selectedCoverUri = uri
            errorMessage = null
        }
    }

    val dialogSurface = MaterialTheme.colorScheme.surface
    val dialogInk = MaterialTheme.colorScheme.onSurface
    val dialogMuted = MaterialTheme.colorScheme.onSurfaceVariant
    val dialogBorder = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f)
    val accent = tokens.petalLavender

    Dialog(
        onDismissRequest = { if (!saving) onDismiss() },
        properties = DialogProperties(
            dismissOnBackPress = !saving,
            dismissOnClickOutside = !saving,
        ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(24.dp))
                .background(dialogSurface)
                .border(1.dp, dialogBorder, RoundedCornerShape(24.dp))
                .verticalScroll(rememberScrollState())
                .padding(22.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(
                text = strings.editPlaylist,
                style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Bold),
                color = dialogInk,
            )

            // Cover art picker row
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(84.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(tokens.eclipseCard)
                        .border(1.dp, dialogBorder, RoundedCornerShape(14.dp))
                        .clickable(enabled = !saving) { imagePicker.launch("image/*") },
                    contentAlignment = Alignment.Center,
                ) {
                    val coverModel: Any? = selectedCoverUri ?: currentCoverUrl
                    if (coverModel != null) {
                        AsyncImage(
                            model = coverModel,
                            contentDescription = strings.changeCover,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            imageVector = SonataIcons.Image,
                            contentDescription = null,
                            tint = dialogMuted,
                            modifier = Modifier.size(30.dp),
                        )
                    }
                }

                Column(
                    modifier = Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(12.dp))
                            .background(tokens.eclipseCard)
                            .border(1.dp, dialogBorder, RoundedCornerShape(12.dp))
                            .clickable(enabled = !saving) { imagePicker.launch("image/*") }
                            .padding(horizontal = 14.dp, vertical = 10.dp),
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = SonataIcons.Image,
                                contentDescription = null,
                                tint = dialogInk,
                                modifier = Modifier.size(18.dp),
                            )
                            Text(
                                text = strings.changeCover,
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                                color = dialogInk,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                    if (selectedCoverUri != null) {
                        Text(
                            text = strings.coverSelected,
                            style = MaterialTheme.typography.bodySmall,
                            color = accent,
                        )
                    }
                }
            }

            OutlinedTextField(
                value = title,
                onValueChange = {
                    title = it
                    errorMessage = null
                },
                enabled = !saving,
                singleLine = true,
                label = { Text(strings.playlistName) },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = accent,
                    focusedLabelColor = accent,
                ),
            )

            OutlinedTextField(
                value = description,
                onValueChange = { description = it },
                enabled = !saving,
                maxLines = 3,
                label = { Text(strings.playlistDescription) },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = accent,
                    focusedLabelColor = accent,
                ),
            )

            // Privacy selector chips (optional — null keeps existing YouTube Music privacy)
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    text = strings.playlistPrivacy,
                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                    color = dialogMuted,
                )
                val options = listOf(
                    null to strings.privacyKeep,
                    "PRIVATE" to strings.privacyPrivate,
                    "UNLISTED" to strings.privacyUnlisted,
                    "PUBLIC" to strings.privacyPublic,
                )
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    options.forEach { (value, label) ->
                        val selected = privacy == value
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(10.dp))
                                .background(
                                    if (selected) accent.copy(alpha = 0.22f) else tokens.eclipseCard,
                                )
                                .border(
                                    1.dp,
                                    if (selected) accent else dialogBorder,
                                    RoundedCornerShape(10.dp),
                                )
                                .clickable(enabled = !saving) { privacy = value }
                                .padding(horizontal = 12.dp, vertical = 7.dp),
                        ) {
                            Text(
                                text = label,
                                style = MaterialTheme.typography.labelMedium.copy(
                                    fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                                ),
                                color = if (selected) accent else dialogInk,
                            )
                        }
                    }
                }
            }

            Text(
                text = strings.editPlaylistHint,
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                color = dialogMuted,
            )

            if (!errorMessage.isNullOrBlank()) {
                Text(
                    text = errorMessage!!,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFFFF6B6B),
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(46.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(tokens.eclipseCard)
                        .clickable(enabled = !saving, onClick = onDismiss),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = strings.cancel,
                        style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
                        color = dialogInk,
                    )
                }

                val trimmedTitle = title.trim()
                val titleChanged = trimmedTitle.isNotEmpty() && trimmedTitle != initialTitle.trim()
                val descChanged = description.trim().isNotEmpty()
                val canSave = !saving && (titleChanged || descChanged || privacy != null || selectedCoverUri != null)
                val onAccentInk = if (accent.luminance() > 0.55f) Color.Black else Color.White

                Box(
                    modifier = Modifier
                        .weight(1.3f)
                        .height(46.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(if (canSave) accent else accent.copy(alpha = 0.25f))
                        .clickable(enabled = canSave) {
                            saving = true
                            errorMessage = null
                            scope.launch {
                                val cover = selectedCoverUri?.let { uri ->
                                    readAndPrepareCoverImage(context, uri)
                                }
                                if (selectedCoverUri != null && cover == null) {
                                    saving = false
                                    errorMessage = "Could not read selected image"
                                    return@launch
                                }
                                onSave(
                                    trimmedTitle.takeIf { titleChanged },
                                    description.trim().takeIf { descChanged },
                                    privacy,
                                    cover?.bytes,
                                    cover?.mimeType,
                                ) { ok, msg ->
                                    saving = false
                                    if (ok) {
                                        onDismiss()
                                    } else {
                                        errorMessage = msg
                                    }
                                }
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    if (saving) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            CircularProgressIndicator(
                                color = onAccentInk,
                                strokeWidth = 2.dp,
                                modifier = Modifier.size(16.dp),
                            )
                            Text(
                                text = strings.saving,
                                style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Bold),
                                color = onAccentInk,
                            )
                        }
                    } else {
                        Text(
                            text = strings.save,
                            style = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.Bold),
                            color = if (canSave) onAccentInk else dialogInk.copy(alpha = 0.5f),
                        )
                    }
                }
            }
        }
    }
}
