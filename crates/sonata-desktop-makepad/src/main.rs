//! Makepad fullscreen player prototype for Sonata.

pub use makepad_widgets;

use makepad_widgets::*;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Sonata — Makepad"
                window.inner_size: vec2(1440, 900)
                pass.clear_color: #x08090f
                body +: {
                    View{
                        width: Fill
                        height: Fill
                        flow: Overlay
                        show_bg: true
                        draw_bg.color: #x08090f

                        // Layered color fields keep this prototype independent of artwork files.
                        View{
                            width: 760
                            height: 760
                            margin: Inset{left: -180, top: -260}
                            show_bg: true
                            draw_bg.color: #x3d233f
                            draw_bg.border_radius: 380.0
                        }
                        View{
                            width: 620
                            height: 620
                            margin: Inset{right: -160, top: 180}
                            show_bg: true
                            draw_bg.color: #x173d4c
                            draw_bg.border_radius: 310.0
                        }
                        View{
                            width: Fill
                            height: Fill
                            show_bg: true
                            draw_bg.color: #x08090f99
                        }

                        View{
                            width: Fill
                            height: Fill
                            flow: Down
                            padding: Inset{left: 34, right: 34, top: 28, bottom: 24}
                            spacing: 18

                            View{
                                width: Fill
                                height: 48
                                flow: Right
                                align: Align{x: 0.0, y: 0.5}
                                spacing: 28

                                Label{
                                    text: "SONATA"
                                    draw_text.color: #xffffffff
                                    draw_text.text_style: theme.font_bold{font_size: 18}
                                }
                                Label{
                                    text: "NOW PLAYING"
                                    draw_text.color: #xffffffb0
                                    draw_text.text_style.font_size: 11
                                }
                                View{width: Fill height: Fit}
                                Label{
                                    text: "⌘  K"
                                    draw_text.color: #xffffff90
                                    draw_text.text_style.font_size: 11
                                }
                                Label{
                                    text: "⋯"
                                    draw_text.color: #xffffffff
                                    draw_text.text_style.font_size: 22
                                }
                            }

                            View{
                                width: Fill
                                height: Fill
                                flow: Right
                                spacing: 54
                                align: Align{x: 0.5, y: 0.5}

                                View{
                                    width: 440
                                    height: 440
                                    flow: Overlay
                                    show_bg: true
                                    draw_bg.color: #x855a72
                                    draw_bg.border_radius: 24.0

                                    View{
                                        width: Fill
                                        height: Fill
                                        margin: Inset{left: 74, top: 74, right: 74, bottom: 74}
                                        show_bg: true
                                        draw_bg.color: #xd28a63
                                        draw_bg.border_radius: 220.0
                                    }
                                    View{
                                        width: Fill
                                        height: Fill
                                        margin: Inset{left: 144, top: 144, right: 144, bottom: 144}
                                        show_bg: true
                                        draw_bg.color: #x3f294f
                                        draw_bg.border_radius: 160.0
                                    }
                                    Label{
                                        text: "S"
                                        draw_text.color: #xffffffe0
                                        draw_text.text_style: theme.font_bold{font_size: 92}
                                    }
                                }

                                View{
                                    width: Fill
                                    height: 500
                                    flow: Down
                                    spacing: 18
                                    align: Align{x: 0.0, y: 0.5}

                                    Label{
                                        text: "MIDNIGHT SIGNALS"
                                        draw_text.color: #xffffffff
                                        draw_text.text_style: theme.font_bold{font_size: 32}
                                    }
                                    Label{
                                        text: "Sonata Records · 2024"
                                        draw_text.color: #xffffff99
                                        draw_text.text_style.font_size: 14
                                    }
                                    View{width: Fill height: 24}
                                    Label{
                                        text: "The city is quiet, but the signal remains"
                                        draw_text.color: #xffffffff
                                        draw_text.text_style: theme.font_bold{font_size: 24}
                                    }
                                    Label{
                                        text: "A soft transmission through the blue hour"
                                        draw_text.color: #xffffffa8
                                        draw_text.text_style.font_size: 20
                                    }
                                    Label{
                                        text: "Every light becomes a memory"
                                        draw_text.color: #xffffff72
                                        draw_text.text_style.font_size: 20
                                    }
                                    Label{
                                        text: ""
                                        draw_text.text_style.font_size: 20
                                    }
                                    View{
                                        width: Fill
                                        height: 3
                                        show_bg: true
                                        draw_bg.color: #xffffff22
                                    }
                                    Label{
                                        text: "03:18   ━━━━━━━━━━━━━━━   04:42"
                                        draw_text.color: #xffffff99
                                        draw_text.text_style.font_size: 12
                                    }
                                }
                            }

                            View{
                                width: Fill
                                height: 72
                                flow: Right
                                align: Align{x: 0.5, y: 0.5}
                                spacing: 28

                                Label{text: "↶" draw_text.color: #xffffffb8 draw_text.text_style.font_size: 25}
                                Label{text: "◀" draw_text.color: #xffffffb8 draw_text.text_style.font_size: 20}
                                View{
                                    width: 54
                                    height: 54
                                    show_bg: true
                                    draw_bg.color: #xffffffff
                                    draw_bg.border_radius: 27.0
                                    Label{text: "▶" draw_text.color: #x17121d draw_text.text_style.font_size: 19}
                                }
                                Label{text: "▶" draw_text.color: #xffffffb8 draw_text.text_style.font_size: 20}
                                Label{text: "↷" draw_text.color: #xffffffb8 draw_text.text_style.font_size: 25}
                                View{width: Fill height: Fit}
                                Label{text: "☰  QUEUE" draw_text.color: #xffffffb8 draw_text.text_style.font_size: 12}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
