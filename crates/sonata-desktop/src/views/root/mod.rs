//! Layout of the full (non-mini) desktop workspace.
//!
//! 1. **Workspace Shell (`mod.rs`)**: `render_full_workspace` composes the ambient gradient, top header, sidebar, center stage, optional queue drawer, and player bar, or delegates to `fullscreen`.
//! 2. **Header (`header`)**: search bar, view toggles, sign-in/account button, and account popover.
//! 3. **Sidebar (`sidebar`)**: primary navigation buttons and user playlists.
//! 4. **Center Stage & Queue (`stage`)**: active page, synced lyrics view, and collapsible queue drawer.
//! 5. **Player Bar (`player_bar`)**: transport controls, artwork toggle, progress scrubber, and volume slider.

mod header;
mod player_bar;
mod sidebar;
mod stage;

pub use header::render_account_popover;

use super::{fullscreen, CenterMode, FluentDivExt, SonataDesktopView};
use crate::theme_bridge::GlassPalette;
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
                    linear_color_stop(p.window_bg, 0.72),
                ))
                .flex()
                .flex_col()
                .child(header::render_top_header(view, p, cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px_3()
                        .pb_3()
                        .flex()
                        .gap_3()
                        .child(sidebar::render_sidebar(view, p, cx))
                        .child(stage::render_center_stage(view, p, window, cx))
                        .when(view.show_queue, |el| {
                            el.child(stage::render_queue_drawer(view, p, cx))
                        }),
                )
                .child(player_bar::render_player_bar(view, p, cx)),
        )
        .into_any_element()
}
