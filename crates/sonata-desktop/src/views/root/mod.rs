//! Layout of the full (non-mini) desktop workspace.
//!
//! 1. **Workspace Shell (`mod.rs`)**: `render_full_workspace` composes the skin-native shell (`Sonata` borderless glass, `Spotify` black-frame rounded panels, or `YouTubeMusic` hairline-divided canvas) and delegates fullscreen mode untouched to `fullscreen`.
//! 2. **Header (`header`)**: skin-adapted search bar, view toggles, sign-in/account trigger, and account popover.
//! 3. **Sidebar (`sidebar`)**: primary navigation, brand header (in Sonata), and user playlists.
//! 4. **Center Stage & Queue (`stage`)**: active page, synced lyrics view, and collapsible queue drawer.
//! 5. **Player Bar (`player_bar`)**: floating glass bar (Sonata), flush black bar (Spotify), or top-scrubber bar (YouTube Music).

mod header;
mod player_bar;
mod sidebar;
mod stage;

pub use header::render_account_popover;

use super::{fullscreen, CenterMode, FluentDivExt, SonataDesktopView};
use crate::theme_bridge::{DesktopSkin, GlassPalette};
use gpui_kit::{
    div, linear_color_stop, linear_gradient, AnyElement, Context, IntoElement, ParentElement,
    Styled, Window,
};

pub fn render_full_workspace(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    if view.center_mode == CenterMode::Fullscreen {
        return fullscreen::render_fullscreen_workspace(view, p, window, cx);
    }

    match p.skin {
        DesktopSkin::Sonata => render_sonata_shell(view, p, window, cx),
        DesktopSkin::Spotify => render_spotify_shell(view, p, window, cx),
        DesktopSkin::YouTubeMusic => render_ytm_shell(view, p, window, cx),
    }
}

/// Apple Music / Sonora-inspired unified borderless glass shell:
/// full-height left sidebar + integrated top header inside the content column + floating frosted player bar.
fn render_sonata_shell(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .size_full()
        .font_family("Inter")
        .bg(p.window_bg)
        .child(
            div()
                .size_full()
                .bg(linear_gradient(
                    155.0,
                    linear_color_stop(p.ambient_top, 0.0),
                    linear_color_stop(p.window_bg, 0.62),
                ))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(sidebar::render_sidebar(view, p, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .child(header::render_top_header(view, p, cx))
                                .child(stage::render_center_stage(view, p, window, cx)),
                        )
                        .when(view.show_queue, |el| {
                            el.child(stage::render_queue_drawer(view, p, cx))
                        }),
                )
                .child(player_bar::render_player_bar(view, p, cx)),
        )
        .into_any_element()
}

/// Authentic Spotify Desktop shell:
/// pure black outer frame, top centered search header, `#121212` rounded panels, flush black bottom player bar.
fn render_spotify_shell(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .size_full()
        .font_family("Inter")
        .bg(p.shell_bg)
        .flex()
        .flex_col()
        .child(header::render_top_header(view, p, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .px_2()
                .flex()
                .gap_2()
                .child(sidebar::render_sidebar(view, p, cx))
                .child(stage::render_center_stage(view, p, window, cx))
                .when(view.show_queue, |el| {
                    el.child(stage::render_queue_drawer(view, p, cx))
                }),
        )
        .child(player_bar::render_player_bar(view, p, cx))
        .into_any_element()
}

/// Authentic YouTube Music Desktop shell:
/// deep `#030303` canvas with crisp hairline dividers, top header bar, edge-to-edge sidebar/stage, and flush `#212121` top-scrubber player bar.
fn render_ytm_shell(
    view: &mut SonataDesktopView,
    p: &GlassPalette,
    window: &mut Window,
    cx: &mut Context<SonataDesktopView>,
) -> AnyElement {
    div()
        .size_full()
        .font_family("Inter")
        .bg(p.shell_bg)
        .flex()
        .flex_col()
        .child(header::render_top_header(view, p, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(sidebar::render_sidebar(view, p, cx))
                .child(stage::render_center_stage(view, p, window, cx))
                .when(view.show_queue, |el| {
                    el.child(stage::render_queue_drawer(view, p, cx))
                }),
        )
        .child(player_bar::render_player_bar(view, p, cx))
        .into_any_element()
}
