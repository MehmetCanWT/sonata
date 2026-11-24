//! On-device Listening Stats view with time-window filtering, streak/KPI cards, source breakdown, and top charts.
//!
//! 1. **Time Windows**: `TabBar` switches across `Week`, `Month`, `Year`, and `AllTime` summaries from `StatsDb`.
//! 2. **Overview & Sources**: displays total plays, listening duration, consecutive day streak, unique counts, and per-source breakdown pills.
//! 3. **Top Charts**: lists top artists, top tracks (playable on click), and top albums strictly from local SQLite records.

use super::SonataDesktopView;
use crate::backend::{can_play_track_in_mode, BackendCmd};
use crate::settings::PAGE_INSET_X;
use crate::ui::components::scroll_edge_fades;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::Sizable as _;
use gpui_kit::{
    div, prelude::FluentBuilder as _, px, AnyElement, Context, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window,
};
use sonata_core::providers::{ProviderSource, Track};
use sonata_core::stats::{StatsWindow, TrackStat};
use sonata_runtime::i18n::strings;

/// Formats total listening minutes as `"Xh Ym"` when >= 60 minutes, otherwise `"Xm"`.
pub fn format_listening_minutes(total_minutes: u64) -> String {
    if total_minutes >= 60 {
        let hours = total_minutes / 60;
        let mins = total_minutes % 60;
        format!("{hours}h {mins}m")
    } else {
        format!("{total_minutes}m")
    }
}

/// Maps a [`TrackStat`] entry back into a playable [`Track`] for queue dispatch.
pub fn track_from_stat(stat: &TrackStat) -> Track {
    let source = if stat.source_tag.eq_ignore_ascii_case("SPF") {
        ProviderSource::Spotify
    } else {
        ProviderSource::YouTubeMusic
    };
    Track {
        id: stat.track_id.clone(),
        title: stat.title.clone(),
        artist: stat.artist.clone(),
        album: stat.album.clone(),
        duration_secs: None,
        thumbnail_url: None,
        source,
        audio_badge: (!stat.source_tag.is_empty()).then(|| stat.source_tag.clone()),
        set_video_id: None,
        liked: false,
        album_id: None,
        is_explicit: false,
    }
}

impl SonataDesktopView {
    pub fn render_stats_page(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = strings(self.settings.language);
        let active_window = self.snapshot.stats_window;
        let summary = self.snapshot.stats_summary.clone();
        let loading = self.snapshot.stats_loading;
        let scroll_fades = self.settings.scroll_fades;

        let win_idx = match active_window {
            StatsWindow::Week => 0,
            StatsWindow::Month => 1,
            StatsWindow::Year => 2,
            StatsWindow::AllTime => 3,
        };
        let window_tabs = TabBar::new("stats-window-tabs")
            .pill()
            .small()
            .selected_index(win_idx)
            .on_click(cx.listener(|this, idx: &usize, _, _| {
                let win = match *idx {
                    1 => StatsWindow::Month,
                    2 => StatsWindow::Year,
                    3 => StatsWindow::AllTime,
                    _ => StatsWindow::Week,
                };
                this.backend.send(BackendCmd::LoadListeningStats(win));
            }))
            .child(Tab::new().label(s.stats_week))
            .child(Tab::new().label(s.stats_month))
            .child(Tab::new().label(s.stats_year))
            .child(Tab::new().label(s.stats_all_time));

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .flex_wrap()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(26.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(self.palette.text)
                            .child(s.listening_stats),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(self.palette.secondary)
                            .child(s.listening_stats_desc),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(window_tabs)
                    .child(
                        Button::new("stats-refresh")
                            .secondary()
                            .small()
                            .icon(IconName::RefreshCw)
                            .on_click(cx.listener(move |this, _, _, _| {
                                this.backend
                                    .send(BackendCmd::LoadListeningStats(active_window));
                            })),
                    ),
            );

        let kpi_card = |id: &'static str, label: &'static str, value: String, sub: String| {
            div()
                .id(id)
                .flex_1()
                .min_w(px(150.0))
                .p(px(14.0))
                .rounded(px(self.palette.radii.control))
                .bg(self.palette.surface.opacity(0.55))
                .border_1()
                .border_color(self.palette.outline)
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(self.palette.secondary)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(self.palette.text)
                        .child(value),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(self.palette.dim)
                        .child(sub),
                )
        };

        let kpis = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(10.0))
            .child(kpi_card(
                "stats-kpi-plays",
                s.total_plays,
                summary.total_plays.to_string(),
                format!("{} {}", summary.unique_tracks, s.tracks),
            ))
            .child(kpi_card(
                "stats-kpi-time",
                s.listening_time,
                format_listening_minutes(summary.total_minutes),
                format!("{} {}", summary.unique_artists, s.artists),
            ))
            .child(kpi_card(
                "stats-kpi-streak",
                s.streak_days,
                format!("{}d", summary.streak_days),
                "UTC".to_string(),
            ));

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(18.0))
            .px(px(PAGE_INSET_X))
            .py(px(18.0))
            .max_w(px(900.0))
            .child(header)
            .child(kpis);

        if !summary.source_breakdown.is_empty() {
            let mut chips = div().flex().flex_row().flex_wrap().gap(px(8.0));
            for src in &summary.source_breakdown {
                chips = chips.child(
                    div()
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(self.palette.radii.control))
                        .bg(self.palette.surface)
                        .border_1()
                        .border_color(self.palette.outline)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(self.palette.accent)
                                .child(src.source_tag.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(self.palette.text)
                                .child(format!(
                                    "{} • {}",
                                    src.plays,
                                    format_listening_minutes(src.minutes)
                                )),
                        ),
                );
            }
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.text)
                            .child(s.sources),
                    )
                    .child(chips),
            );
        }

        if summary.total_plays == 0 && !loading {
            page = page.child(
                div()
                    .p(px(24.0))
                    .rounded(px(self.palette.radii.control))
                    .bg(self.palette.surface.opacity(0.45))
                    .border_1()
                    .border_color(self.palette.outline)
                    .text_size(px(13.0))
                    .text_color(self.palette.dim)
                    .child(s.no_stats_yet),
            );
        } else {
            if !summary.top_tracks.is_empty() {
                let personal_only = self.snapshot.config.personal_only_mode;
                let dl_ids = self.snapshot.downloaded_track_ids.clone();
                let mut rows = div().flex().flex_col().gap(px(4.0));
                for (idx, tr) in summary.top_tracks.iter().take(10).enumerate() {
                    let play_track = track_from_stat(tr);
                    let playable = can_play_track_in_mode(
                        &play_track,
                        personal_only,
                        dl_ids.contains(&play_track.id),
                    );
                    rows = rows.child(
                        div()
                            .id(SharedString::from(format!("stats-tr-{idx}")))
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(self.palette.radii.item))
                            .bg(self.palette.surface.opacity(0.4))
                            .when(playable, |d| {
                                d.cursor_pointer()
                                    .hover(|st| st.bg(self.palette.surface_hover))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.backend.send(BackendCmd::PlayTracks {
                                            tracks: vec![play_track.clone()],
                                            start_index: 0,
                                            shuffle: false,
                                        });
                                    }))
                            })
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .w(px(22.0))
                                            .text_size(px(12.0))
                                            .text_color(self.palette.dim)
                                            .child(format!("{}.", idx + 1)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_size(px(13.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(self.palette.text)
                                                    .truncate()
                                                    .child(tr.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(self.palette.secondary)
                                                    .truncate()
                                                    .child(tr.artist.clone()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .when(!tr.source_tag.is_empty(), |r| {
                                        r.child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(self.palette.radii.small))
                                                .bg(self.palette.accent.opacity(0.14))
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(self.palette.accent)
                                                .child(tr.source_tag.clone()),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(self.palette.secondary)
                                            .child(format!(
                                                "{} • {}",
                                                tr.plays,
                                                format_listening_minutes(tr.minutes)
                                            )),
                                    ),
                            ),
                    );
                }
                page = page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(self.palette.text)
                                .child(s.top_tracks),
                        )
                        .child(rows),
                );
            }

            if !summary.top_artists.is_empty() {
                let personal_only = self.snapshot.config.personal_only_mode;
                let mut rows = div().flex().flex_col().gap(px(4.0));
                for (idx, art) in summary.top_artists.iter().take(10).enumerate() {
                    let artist_name = art.artist.clone();
                    rows = rows.child(
                        div()
                            .id(SharedString::from(format!("stats-art-{idx}")))
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(self.palette.radii.item))
                            .bg(self.palette.surface.opacity(0.4))
                            .when(!personal_only, |d| {
                                d.cursor_pointer()
                                    .hover(|st| st.bg(self.palette.surface_hover))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_artist_by_name(&artist_name, cx);
                                    }))
                            })
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .w(px(22.0))
                                            .text_size(px(12.0))
                                            .text_color(self.palette.dim)
                                            .child(format!("{}.", idx + 1)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(self.palette.text)
                                            .truncate()
                                            .child(art.artist.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(self.palette.secondary)
                                    .child(format!(
                                        "{} • {}",
                                        art.plays,
                                        format_listening_minutes(art.minutes)
                                    )),
                            ),
                    );
                }
                page = page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(self.palette.text)
                                .child(s.top_artists),
                        )
                        .child(rows),
                );
            }

            if !summary.top_albums.is_empty() {
                let mut rows = div().flex().flex_col().gap(px(4.0));
                for (idx, alb) in summary.top_albums.iter().take(10).enumerate() {
                    rows = rows.child(
                        div()
                            .id(SharedString::from(format!("stats-alb-{idx}")))
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(self.palette.radii.item))
                            .bg(self.palette.surface.opacity(0.4))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .w(px(22.0))
                                            .text_size(px(12.0))
                                            .text_color(self.palette.dim)
                                            .child(format!("{}.", idx + 1)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_size(px(13.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(self.palette.text)
                                                    .truncate()
                                                    .child(alb.album.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(self.palette.secondary)
                                                    .truncate()
                                                    .child(alb.artist.clone()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(self.palette.secondary)
                                    .child(format!(
                                        "{} • {}",
                                        alb.plays,
                                        format_listening_minutes(alb.minutes)
                                    )),
                            ),
                    );
                }
                page = page.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(self.palette.text)
                                .child(s.top_albums),
                        )
                        .child(rows),
                );
            }
        }

        div()
            .size_full()
            .relative()
            .child(
                div()
                    .id("stats-page-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(page),
            )
            .child(scroll_edge_fades(
                &self.main_scroll,
                &self.palette,
                scroll_fades,
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{format_listening_minutes, track_from_stat};
    use sonata_core::providers::ProviderSource;
    use sonata_core::stats::TrackStat;

    #[test]
    fn formats_listening_minutes_and_maps_stat_tracks() {
        assert_eq!(format_listening_minutes(0), "0m");
        assert_eq!(format_listening_minutes(45), "45m");
        assert_eq!(format_listening_minutes(60), "1h 0m");
        assert_eq!(format_listening_minutes(135), "2h 15m");

        let stat = TrackStat {
            track_id: "local:/music/song.flac".to_string(),
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            source_tag: "LOC".to_string(),
            plays: 4,
            minutes: 12,
        };
        let track = track_from_stat(&stat);
        assert_eq!(track.id, "local:/music/song.flac");
        assert_eq!(track.audio_badge.as_deref(), Some("LOC"));
        assert_eq!(track.source, ProviderSource::YouTubeMusic);
    }
}
