use super::{
    render_cover_art, render_icon_button, render_progress_slider, render_volume_slider,
    FluentDivExt, SonataDesktopView,
};
use crate::backend::BackendCmd;
use crate::theme_bridge::{rgb_hsla, GlassPalette};
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, img, linear_color_stop, linear_gradient, px, uniform_list, AnyElement, Context,
    FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, Window,
};
use sonata_core::lyrics::LyricsState;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub const LYRICS_LEAD_ROWS: usize = 4;
const LYRIC_ROW_CHARS: usize = 28;
const LYRIC_ROW_HEIGHT: f32 = 82.0;

struct FullscreenLyricRow {
    source_index: usize,
    time_secs: f64,
    text: String,
}

fn blurred_backdrop_path(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }

    let blurred = path.with_file_name(format!("{}.blur.png", path.file_stem()?.to_string_lossy()));
    let source_modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let blur_modified = std::fs::metadata(&blurred)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    if blur_modified.is_some_and(|modified| modified >= source_modified) {
        return Some(blurred);
    }

    let source = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&source).ok()?;
    let blurred_image = image
        .resize(160, 160, image::imageops::FilterType::Triangle)
        .blur(18.0);
    let temporary = blurred.with_extension("blur.png.tmp");
    blurred_image
        .save_with_format(&temporary, image::ImageFormat::Png)
        .ok()?;
    std::fs::rename(&temporary, &blurred).ok()?;
    Some(blurred)
}

/// Maps a timed lyric index to the visual row that starts its centered fullscreen group.
pub fn fullscreen_lyric_target(lines: &[sonata_core::lyrics::LyricLine], active: usize) -> usize {
    LYRICS_LEAD_ROWS
        + lines
            .iter()
            .take(active)
            .map(|line| split_lyric_text(&line.text).len())
            .sum::<usize>()
}

fn fullscreen_lyric_rows(lines: &[sonata_core::lyrics::LyricLine]) -> Vec<FullscreenLyricRow> {
    lines
        .iter()
        .enumerate()
        .flat_map(|(source_index, line)| {
            let text = if line.text.trim().is_empty() {
                vec!["♪".to_string()]
            } else {
                split_lyric_text(&line.text)
            };
            text.into_iter().map(move |text| FullscreenLyricRow {
                source_index,
                time_secs: line.time_secs,
                text,
            })
        })
        .collect()
}

/// GPUI's virtualized list requires a fixed row height. Split long text before rendering so every
/// language fits without clipping or making the scroll geometry incorrect.
fn split_lyric_text(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }

    let mut rows = Vec::new();
    let mut current = String::new();
    for word in words {
        if word.chars().count() > LYRIC_ROW_CHARS {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
            }
            let mut chunk = String::new();
            for character in word.chars() {
                chunk.push(character);
                if chunk.chars().count() == LYRIC_ROW_CHARS {
                    rows.push(std::mem::take(&mut chunk));
                }
            }
            current = chunk;
            continue;
        }
        let next_len =
            current.chars().count() + usize::from(!current.is_empty()) + word.chars().count();
        if !current.is_empty() && next_len > LYRIC_ROW_CHARS {
            rows.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// Render the desktop-only immersive playback workspace.
///
/// Flex wrapping keeps artwork, lyrics, and controls usable in narrow windows without imposing an
/// Android layout contract.
pub fn render_fullscreen_workspace(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let viewport = window.viewport_size();
    let width: f32 = viewport.width.into();
    let height: f32 = viewport.height.into();
    // Sonora's artwork fills roughly half the available height while lyrics keep a wide column.
    let cover_size = (height * 0.55).min(width * 0.34).clamp(280.0, 560.0);
    let lyrics_width = (width * 0.42).clamp(520.0, 680.0);
    let (r, g, b) = view.snapshot.ambient_rgb;
    let backdrop = view
        .snapshot
        .art_path
        .as_ref()
        .and_then(|path| blurred_backdrop_path(path))
        .map(|path| {
            img(path.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .opacity(0.72)
                .into_any_element()
        });

    let ambient_layer = div()
        .absolute()
        .size_full()
        .overflow_hidden()
        .rounded_2xl()
        .bg(linear_gradient(
            140.0,
            linear_color_stop(rgb_hsla((8, 3, 13), 0.96), 0.0),
            linear_color_stop(rgb_hsla((r, g, b), 0.22), 1.0),
        ));
    let ambient_layer = if let Some(art) = backdrop {
        ambient_layer.child(art)
    } else {
        ambient_layer
    };
    let ambient_layer =
        ambient_layer.child(div().absolute().size_full().bg(rgb_hsla((8, 3, 13), 0.62)));

    div()
        .relative()
        .size_full()
        .p_3()
        .on_mouse_move(cx.listener(|this, _, _, cx| this.note_fullscreen_activity(cx)))
        .bg(linear_gradient(
            145.0,
            linear_color_stop(rgb_hsla((r, g, b), 0.58), 0.0),
            linear_color_stop(p.window_bg, 0.72),
        ))
        .child(ambient_layer)
        .child(render_fullscreen_sized(
            view,
            p,
            cover_size,
            lyrics_width,
            cx,
        ))
        .into_any_element()
}

pub fn render_fullscreen(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    render_fullscreen_sized(view, p, 520.0, 580.0, cx)
}

fn render_fullscreen_sized(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    cover_size: f32,
    lyrics_width: f32,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let snap = &view.snapshot;
    let track = snap.current_track();
    let title = track
        .map(|track| track.title.clone())
        .unwrap_or_else(|| "No Active Track".to_string());
    let artist = track
        .map(|track| track.artist.clone())
        .unwrap_or_else(|| "Choose a song to begin".to_string());
    let play_icon = if snap.paused {
        IconName::Play
    } else {
        IconName::Pause
    };
    let controls_visible = view.fullscreen_controls_visible();

    div()
        .id("fullscreen-playback")
        .relative()
        .size_full()
        .p_6()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .w_full()
                .flex()
                .justify_end()
                .when(controls_visible, |el| {
                    el.child(render_icon_button(
                        "fullscreen-close",
                        IconName::Minimize2,
                        17.0,
                        36.0,
                        p.text,
                        p.pill_bg,
                        p.card_hover,
                        |this, _, cx| this.toggle_fullscreen_view(cx),
                        cx,
                    ))
                }),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(px(72.0))
                .child(
                    div()
                        .w(px(cover_size + 28.0))
                        .max_w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_5()
                        .child(render_cover_art(
                            snap.art_path.as_ref(),
                            cover_size,
                            26.0,
                            p,
                        ))
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(30.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(p.highlight_fg)
                                        .text_center()
                                        .truncate()
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_size(px(18.0))
                                        .text_color(p.muted)
                                        .text_center()
                                        .truncate()
                                        .child(artist),
                                ),
                        ),
                )
                .child(render_lyrics_panel(view, p, lyrics_width, cx)),
        )
        .child(
            div()
                .w_full()
                .max_w(px(920.0))
                .self_center()
                .flex()
                .flex_col()
                .gap_2()
                .when(controls_visible, |el| {
                    el.child(render_progress_slider(
                        view,
                        p,
                        "fullscreen-progress",
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(render_icon_button(
                                        "fullscreen-prev",
                                        IconName::SkipBack,
                                        19.0,
                                        40.0,
                                        p.text,
                                        p.pill_bg,
                                        p.card_hover,
                                        |this, _, _| this.backend.send(BackendCmd::PrevTrack),
                                        cx,
                                    ))
                                    .child(render_icon_button(
                                        "fullscreen-play",
                                        play_icon,
                                        21.0,
                                        48.0,
                                        p.accent_fg,
                                        p.accent,
                                        p.accent,
                                        |this, _, _| this.backend.send(BackendCmd::TogglePause),
                                        cx,
                                    ))
                                    .child(render_icon_button(
                                        "fullscreen-next",
                                        IconName::SkipForward,
                                        19.0,
                                        40.0,
                                        p.text,
                                        p.pill_bg,
                                        p.card_hover,
                                        |this, _, _| this.backend.send(BackendCmd::NextTrack),
                                        cx,
                                    )),
                            )
                            .child(render_volume_slider(view, p, cx)),
                    )
                }),
        )
        .into_any_element()
}

fn render_lyrics_panel(
    view: &SonataDesktopView,
    p: &GlassPalette,
    lyrics_width: f32,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    let content = match &view.snapshot.lyrics {
        LyricsState::Synced(lines) if !lines.is_empty() => {
            let count = fullscreen_lyric_rows(lines).len() + LYRICS_LEAD_ROWS * 2;
            uniform_list(
                "fullscreen-lyrics-list",
                count,
                cx.processor(|this, range: Range<usize>, _window, cx| {
                    let palette = this.palette();
                    let LyricsState::Synced(lines) = &this.snapshot.lyrics else {
                        return Vec::new();
                    };
                    let active = this
                        .snapshot
                        .active_lyric_index()
                        .or_else(|| (!lines.is_empty()).then_some(0));
                    let rows = fullscreen_lyric_rows(lines);
                    range
                        .filter_map(|index| {
                            let line_index = index.checked_sub(LYRICS_LEAD_ROWS);
                            let Some(line_index) = line_index.filter(|value| *value < rows.len())
                            else {
                                return Some(
                                    div().w_full().h(px(LYRIC_ROW_HEIGHT)).into_any_element(),
                                );
                            };
                            let line = rows.get(line_index)?;
                            let distance = active
                                .map(|value| value.abs_diff(line.source_index))
                                .unwrap_or(9);
                            let (size, opacity, color, weight) = match distance {
                                0 => (36.0, 1.0, palette.highlight_fg, FontWeight::BOLD),
                                1 => (24.0, 0.58, palette.lyrics_mid, FontWeight::SEMIBOLD),
                                2 => (20.0, 0.36, palette.lyrics_mid, FontWeight::MEDIUM),
                                _ => (18.0, 0.22, palette.lyrics_inactive, FontWeight::MEDIUM),
                            };
                            let time_secs = line.time_secs;
                            Some(
                                div()
                                    .id(SharedString::from(format!(
                                        "fullscreen-lyric-{line_index}"
                                    )))
                                    .w_full()
                                    .h(px(LYRIC_ROW_HEIGHT))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_center()
                                    .whitespace_normal()
                                    .cursor_pointer()
                                    .opacity(opacity)
                                    .hover(|style| style.opacity(1.0))
                                    .text_size(px(size))
                                    .font_weight(weight)
                                    .text_color(color)
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::SeekAbsolute(time_secs));
                                    }))
                                    .child(line.text.clone())
                                    .into_any_element(),
                            )
                        })
                        .collect()
                }),
            )
            .track_scroll(&view.lyrics_scroll)
            .flex_1()
            .w_full()
            .into_any_element()
        }
        _ => {
            let message = match &view.snapshot.lyrics {
                LyricsState::Loading => "Fetching synced lyrics...",
                LyricsState::NotFound => "No lyrics found for this track.",
                LyricsState::Plain(_) => "Synced lyrics are unavailable for this track.",
                LyricsState::Idle => "Start playback to view synced lyrics.",
                LyricsState::Synced(_) => "No lyrics found for this track.",
            };
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(p.muted)
                .child(message)
                .into_any_element()
        }
    };

    div()
        .id("fullscreen-lyrics")
        .w(px(lyrics_width))
        .max_w_full()
        .h_full()
        .min_h(px(280.0))
        .flex()
        .flex_col()
        .child(content)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{fullscreen_lyric_target, split_lyric_text, LYRIC_ROW_CHARS};
    use sonata_core::lyrics::LyricLine;

    #[test]
    fn split_lyric_text_wraps_words_and_long_tokens() {
        let rows = split_lyric_text("one two three four five six seven eight nine ten eleven");
        assert!(rows
            .iter()
            .all(|row| row.chars().count() <= LYRIC_ROW_CHARS));
        assert_eq!(
            rows.join(" "),
            "one two three four five six seven eight nine ten eleven"
        );

        let long_token = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ";
        let rows = split_lyric_text(long_token);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.concat(), long_token);
    }

    #[test]
    fn fullscreen_lyric_target_accounts_for_wrapped_previous_lines() {
        let lines = vec![
            LyricLine {
                time_secs: 0.0,
                text: "short".into(),
            },
            LyricLine {
                time_secs: 1.0,
                text: "this line contains enough words to wrap into two visual rows".into(),
            },
            LyricLine {
                time_secs: 2.0,
                text: "next".into(),
            },
        ];

        assert_eq!(fullscreen_lyric_target(&lines, 0), 4);
        assert_eq!(fullscreen_lyric_target(&lines, 1), 5);
        assert_eq!(fullscreen_lyric_target(&lines, 2), 8);
    }
}
