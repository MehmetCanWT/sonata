package dev.sonata.android.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.sonata.android.data.AudioPipelineInfo
import dev.sonata.android.ui.theme.LocalLunariaColors
import java.util.Locale
import kotlin.math.roundToInt

private data class EqPresetSpec(
    val name: String,
    val bandsDb: List<Float>,
)

private val SONATA_EQ_PRESETS = listOf(
    EqPresetSpec("Flat", listOf(0f, 0f, 0f, 0f, 0f)),
    EqPresetSpec("Bass Boost", listOf(4.5f, 2.5f, 0f, 0.5f, 1.0f)),
    EqPresetSpec("Vocal", listOf(-1.5f, 1.0f, 3.5f, 2.5f, 0.5f)),
    EqPresetSpec("Acoustic", listOf(2.0f, 1.0f, 1.5f, 2.0f, 2.5f)),
    EqPresetSpec("Electronic", listOf(4.0f, 2.0f, -0.5f, 2.0f, 3.5f)),
    EqPresetSpec("Late Night", listOf(2.5f, 1.0f, 0f, -1.0f, -2.5f)),
)

private val EQ_BAND_LABELS = listOf("60 Hz", "230 Hz", "910 Hz", "3.6 kHz", "14 kHz")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AudioPipelineModal(
    info: AudioPipelineInfo,
    onDismiss: () -> Unit,
) {
    val themeTokens = LocalLunariaColors.current
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val sheetBg = themeTokens.eclipseSurface
    val activeAccent = themeTokens.petalLavender
    val mistAccent = themeTokens.lunarMist
    val glowAccent = themeTokens.moonGlow

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = sheetBg,
        contentColor = themeTokens.onSurfacePrimary,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        dragHandle = null,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(sheetBg)
                .navigationBarsPadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                    Text(
                        text = "Audio Signal Chain",
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontSize = 22.sp,
                            fontWeight = FontWeight.Bold,
                        ),
                        color = themeTokens.onSurfacePrimary,
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = "Live 4-stage playback telemetry",
                        style = MaterialTheme.typography.bodySmall,
                        color = themeTokens.onSurfaceSecondary,
                    )
                }
                StatusBadge(
                    label = if (info.isBitTransparentDsp) "Bypassed" else "DSP Active",
                    highlighted = !info.isBitTransparentDsp,
                    accent = if (info.isBitTransparentDsp) mistAccent else activeAccent,
                )
            }

            Spacer(Modifier.height(2.dp))

            // Stage 1: Source
            val deliveryTag = when {
                info.fromOfflineDownload -> "Offline Download"
                info.fromCache -> "Disk Cache"
                else -> "Network Stream"
            }
            PipelineStageCard(
                step = "1",
                title = "Source",
                primary = info.sourceLabel.ifBlank { "YouTube Music Stream" },
                secondary = deliveryTag,
                badgeLabel = if (info.fromOfflineDownload || info.fromCache) "Cached" else "Live",
                badgeActive = info.fromOfflineDownload || info.fromCache,
                accent = mistAccent,
            )

            // Stage 2: Decoder
            val bitrateText = if (info.bitrateKbps > 0) "${info.bitrateKbps} kbps" else "Variable bitrate"
            PipelineStageCard(
                step = "2",
                title = "Decoder",
                primary = "${info.codec.ifBlank { "Auto" }} · $bitrateText",
                secondary = "${info.sampleRateLabel} · ${info.channelLabel}",
                badgeLabel = "Active",
                badgeActive = true,
                accent = activeAccent,
            )

            // Stage 3: Playback Processing
            val activeStages = buildList {
                if (info.skipSilenceActive) add("Skip Silence")
                if (info.playbackSpeed != 1.0f) {
                    add(String.format(Locale.US, "%.2fx Speed", info.playbackSpeed))
                }
            }
            PipelineStageCard(
                step = "3",
                title = "Playback Processing",
                primary = if (activeStages.isEmpty()) "Bit-Transparent Passthrough" else activeStages.joinToString(" · "),
                secondary = if (activeStages.isEmpty()) {
                    "No DSP or time-stretching applied"
                } else {
                    "Processed inside Media3 playback chain"
                },
                badgeLabel = if (activeStages.isEmpty()) "Bypass" else "Active",
                badgeActive = activeStages.isNotEmpty(),
                accent = glowAccent,
            )

            // Stage 4: Output
            PipelineStageCard(
                step = "4",
                title = "Output",
                primary = info.outputPath.ifBlank { "Media3 AudioTrack" },
                secondary = "${info.sampleRateLabel} · ${info.channelLabel}",
                badgeLabel = "Active",
                badgeActive = true,
                accent = activeAccent,
            )

            Spacer(modifier = Modifier.height(6.dp))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun SleepTimerSheet(
    remainingMinutes: Int?,
    onSelectMinutes: (Int?) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val accent = MaterialTheme.colorScheme.primary

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh.copy(alpha = 0.96f),
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .padding(horizontal = 20.dp, vertical = 10.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text(
                        text = "Sleep Timer",
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = when {
                            remainingMinutes == null -> "Playback will continue normally"
                            remainingMinutes == 0 -> "Stopping at end of current track"
                            else -> "Pausing in $remainingMinutes min"
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                if (remainingMinutes != null) {
                    StatusBadge(
                        label = if (remainingMinutes == 0) "End of Track" else "${remainingMinutes}m left",
                        highlighted = true,
                        accent = accent,
                    )
                }
            }

            val options: List<Pair<String, Int?>> = listOf(
                "Off" to null,
                "15m" to 15,
                "30m" to 30,
                "45m" to 45,
                "60m" to 60,
                "End of Track" to 0,
            )

            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                options.forEach { (label, value) ->
                    val selected = remainingMinutes == value
                    SelectableGlassChip(
                        label = label,
                        selected = selected,
                        accent = accent,
                        onClick = {
                            onSelectMinutes(value)
                            onDismiss()
                        },
                    )
                }
            }

            Spacer(modifier = Modifier.height(10.dp))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AudioControlsSheet(
    speed: Float,
    skipSilence: Boolean,
    onSpeedChange: (Float) -> Unit,
    onSkipSilenceToggle: (Boolean) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val accent = MaterialTheme.colorScheme.primary

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh.copy(alpha = 0.96f),
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .navigationBarsPadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = "Audio Controls",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
            )

            // Playback Speed
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(18.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainer.copy(alpha = 0.72f))
                    .border(
                        width = 1.dp,
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.35f),
                        shape = RoundedCornerShape(18.dp),
                    )
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = "Playback Speed",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        text = String.format(Locale.US, "%.2fx", speed),
                        style = MaterialTheme.typography.labelLarge,
                        color = accent,
                        fontWeight = FontWeight.Bold,
                    )
                }

                Slider(
                    value = speed.coerceIn(0.5f, 2.0f),
                    onValueChange = { raw ->
                        val snapped = ((raw * 20f).roundToInt() / 20f).coerceIn(0.5f, 2.0f)
                        onSpeedChange(snapped)
                    },
                    valueRange = 0.5f..2.0f,
                    colors = SliderDefaults.colors(
                        thumbColor = accent,
                        activeTrackColor = accent,
                    ),
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    listOf(0.75f, 1.0f, 1.25f, 1.5f).forEach { presetSpeed ->
                        SelectableGlassChip(
                            label = if (presetSpeed == 1.0f) "1.0x" else "${presetSpeed}x",
                            selected = kotlin.math.abs(speed - presetSpeed) < 0.03f,
                            accent = accent,
                            onClick = { onSpeedChange(presetSpeed) },
                        )
                    }
                }
            }

            // Processing Toggles
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(18.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainer.copy(alpha = 0.72f))
                    .border(
                        width = 1.dp,
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.35f),
                        shape = RoundedCornerShape(18.dp),
                    )
                    .padding(horizontal = 14.dp, vertical = 6.dp),
            ) {
                ToggleRow(
                    title = "Skip Silence",
                    subtitle = "Trim long silent intros and gaps",
                    checked = skipSilence,
                    onCheckedChange = onSkipSilenceToggle,
                )
            }

            Spacer(modifier = Modifier.height(8.dp))
        }
    }
}

@Composable
private fun PipelineStageCard(
    step: String,
    title: String,
    primary: String,
    secondary: String,
    badgeLabel: String,
    badgeActive: Boolean,
    accent: Color,
) {
    val themeTokens = LocalLunariaColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(if (themeTokens.isViolet) 14.dp else 18.dp))
            .background(themeTokens.eclipseCard)
            .border(
                width = 0.5.dp,
                color = themeTokens.cardStroke,
                shape = RoundedCornerShape(if (themeTokens.isViolet) 14.dp else 18.dp),
            )
            .padding(horizontal = 14.dp, vertical = 13.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            modifier = Modifier
                .size(30.dp)
                .clip(CircleShape)
                .background(accent.copy(alpha = 0.16f))
                .border(0.75.dp, accent.copy(alpha = 0.45f), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = step,
                color = accent,
                fontSize = 12.sp,
                fontWeight = FontWeight.Bold,
            )
        }
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                text = title.uppercase(Locale.ROOT),
                style = MaterialTheme.typography.labelSmall.copy(
                    fontSize = 10.5.sp,
                    fontWeight = FontWeight.Bold,
                ),
                color = themeTokens.onSurfaceSecondary,
                letterSpacing = 0.8.sp,
            )
            Text(
                text = primary,
                style = MaterialTheme.typography.bodyLarge.copy(
                    fontSize = 15.sp,
                    fontWeight = FontWeight.SemiBold,
                ),
                color = themeTokens.onSurfacePrimary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = secondary,
                style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.5.sp),
                color = themeTokens.onSurfaceSecondary,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        StatusBadge(
            label = badgeLabel,
            highlighted = badgeActive,
            accent = if (badgeActive) accent else themeTokens.onSurfaceSecondary,
        )
    }
}

@Composable
private fun StatusBadge(
    label: String,
    highlighted: Boolean,
    accent: Color,
) {
    val bg = if (highlighted) accent.copy(alpha = 0.18f) else MaterialTheme.colorScheme.surfaceVariant
    val fg = if (highlighted) accent else MaterialTheme.colorScheme.onSurfaceVariant
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(999.dp))
            .background(bg)
            .border(1.dp, fg.copy(alpha = 0.35f), RoundedCornerShape(999.dp))
            .padding(horizontal = 10.dp, vertical = 5.dp),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = fg,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

@Composable
private fun SelectableGlassChip(
    label: String,
    selected: Boolean,
    accent: Color,
    onClick: () -> Unit,
) {
    val bg = if (selected) accent.copy(alpha = 0.22f) else MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.65f)
    val border = if (selected) accent.copy(alpha = 0.65f) else MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.35f)
    val fg = if (selected) accent else MaterialTheme.colorScheme.onSurface
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(999.dp))
            .background(bg)
            .border(1.dp, border, RoundedCornerShape(999.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 8.dp),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelLarge,
            color = fg,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
        )
    }
}

@Composable
private fun ToggleRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(vertical = 10.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
