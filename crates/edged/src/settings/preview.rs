//! Schematic desktop previews. Only drawing changes: no host window is touched.

use std::{any::Any, time::Duration};

use edged_core::{
    Chord, Direction, Key, PanelReveal, PreviewTrigger, ResizeCorner, RingZones, Settings, Zone,
};
use edged_macos::{Frame, Side};
use inset::ui::valo::{Point, Rect};
use inset::ui::{Canvas, FillRule, Paint, PathBuilder};
use inset::*;
use inset_animation::{AnimationBehavior, AnimationController, Curves};
use inset_rendering::CustomPainter;
use inset_winui::{Button, PathIcon, ToolTip, ToolTipService};

use super::{column, muted};

/// A demo is explicit so replay never needs to synthesize real keyboard events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Demo {
    Panel,
    Preview,
    Move,
    Resize,
    Ring(Direction),
}

/// Window states used only to explain preview availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scenario {
    Visible,
    Minimized,
    OtherSpace,
}

/// A value snapshot ensures a settings change restarts only the affected preview.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Configuration {
    pub demo: Demo,
    pub side: Side,
    pub reveal: PanelReveal,
    pub trigger: PreviewTrigger,
    pub corner: ResizeCorner,
    pub stays: bool,
    pub zone: Option<Zone>,
    pub scenario: Scenario,
    pub enabled: bool,
    pub reduced_motion: bool,
    pub shortcut: Chord,
    pub ring_zones: RingZones,
}

impl Configuration {
    pub fn new(settings: &Settings, demo: Demo, scenario: Scenario, reduced_motion: bool) -> Self {
        Self {
            demo,
            side: settings.panel_side,
            reveal: settings.panel_reveal,
            trigger: settings.preview_trigger,
            corner: settings.resize_corner,
            stays: match demo {
                Demo::Resize => settings.resize_stays_on_screen,
                _ => settings.move_stays_on_screen,
            },
            zone: match demo {
                Demo::Ring(direction) => settings.ring_zones[direction.index()],
                _ => None,
            },
            scenario,
            enabled: match demo {
                Demo::Move => settings.move_enabled && !settings.move_chord.is_empty(),
                Demo::Resize => settings.resize_enabled && !settings.resize_chord.is_empty(),
                Demo::Ring(_) => settings.ring_enabled && !settings.arrange_chord.is_empty(),
                _ => true,
            },
            reduced_motion,
            ring_zones: settings.ring_zones,
            shortcut: match demo {
                Demo::Move => settings.move_chord,
                Demo::Resize => settings.resize_chord,
                Demo::Ring(_) => settings.arrange_chord,
                Demo::Preview if settings.preview_trigger == PreviewTrigger::CommandKey => {
                    Chord::NONE.with(Key::Command)
                }
                _ => Chord::NONE,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct DesktopPreview {
    pub config: Configuration,
    pub revision: u64,
    pub secondary: Color,
    pub accent: Color,
}

pub(super) struct DesktopPreviewState {
    state: StateData<DesktopPreview>,
    ticker: SingleTickerProviderStateMixinData,
    controller: Option<Handle<AnimationController>>,
}

impl StatefulWidget for DesktopPreview {
    type State = DesktopPreviewState;

    fn create_state(&self) -> Self::State {
        DesktopPreviewState {
            state: StateData::new(),
            ticker: SingleTickerProviderStateMixinData::default(),
            controller: None,
        }
    }
}

impl SingleTickerProviderStateMixin for DesktopPreviewState {
    fn single_ticker_provider_data(
        self: Handle<Self>,
        app: &App,
    ) -> &SingleTickerProviderStateMixinData {
        &app.get(self).ticker
    }

    fn single_ticker_provider_data_mut(
        self: Handle<Self>,
        app: &mut App,
    ) -> &mut SingleTickerProviderStateMixinData {
        &mut app.get_mut(self).ticker
    }
}

impl TickerProviderObject for DesktopPreviewState {
    fn create_ticker(
        self: Handle<Self>,
        app: &mut App,
        callback: TickerCallback,
    ) -> Handle<Ticker> {
        SingleTickerProviderStateMixin::create_ticker(self, app, callback)
    }
}

impl DesktopPreviewState {
    fn replay(self: Handle<Self>, app: &mut App) {
        let config = &self.widget(app).config;
        let animate = config.enabled && !config.reduced_motion;
        let controller = app.get(self).controller.unwrap();
        controller.set_value(app, if animate { 0.0 } else { 1.0 });
        if animate {
            controller.animate_to(app, 1.0, None, Curves::linear());
        }
    }
}

impl State for DesktopPreviewState {
    type Widget = DesktopPreview;
    inset::state_accessors!();

    fn init_state(self: Handle<Self>, app: &mut App) {
        let controller = AnimationController::create(
            app,
            Some(0.0),
            Some(Duration::from_millis(2200)),
            None,
            0.0,
            1.0,
            AnimationBehavior::Normal,
            self,
        );
        ListenableObject::add_listener(
            controller,
            app,
            Listener::new(move |app| self.set_state(app, |_| {})),
        );
        app.get_mut(self).controller = Some(controller);
    }

    fn did_update_widget(self: Handle<Self>, app: &mut App, old: &DesktopPreview) {
        let widget = self.widget(app);
        if widget.config != old.config || widget.revision != old.revision {
            self.replay(app);
        }
    }

    fn dispose(self: Handle<Self>, app: &mut App) {
        app.get(self).controller.unwrap().dispose(app);
        SingleTickerProviderStateMixin::dispose(self, app);
    }

    fn build(self: Handle<Self>, app: &mut App, _: BuildContext) -> WidgetRef {
        let widget = self.widget(app).clone();
        let progress = app.get(self).controller.unwrap().value(app);
        let painter = DesktopPainter {
            config: widget.config.clone(),
            progress,
            accent: widget.accent,
        };
        let config = widget.config.clone();
        let canvas = LayoutBuilder::new(move |_, _, constraints| {
            let width = constraints.max_width.min(460.0);
            let height = width / 1.5;
            let phase = demo_phase(&config, progress);
            let mut layers = vec![
                CustomPaint::new()
                    .painter(painter.clone())
                    .size(Size::new(width, height))
                    .into_widget(),
            ];
            layers.push(
                Positioned::new(
                    Text::new("Finder   File   Edit   View").style(
                        TextStyle::new()
                            .font_size(8.0)
                            .color(Color::from_argb(255, 87, 107, 129)),
                    ),
                )
                .left(9.0)
                .top(4.0)
                .into_widget(),
            );
            let frame = window_frame(&config, phase.window_motion);
            // Text is laid out by Inset rather than approximated by painted bars.
            let mut window_labels = Vec::new();
            if phase.window_opacity > 0.0 {
                let x = frame.x * width / 100.0;
                let y = frame.y * height / 100.0;
                let w = frame.width * width / 100.0;
                let h = frame.height * height / 100.0;
                window_labels.push(
                    Positioned::new(
                        Text::new(if phase.second_window {
                            "Safari"
                        } else {
                            "Notes"
                        })
                        .style(
                            TextStyle::new()
                                .font_size(9.0)
                                .color(Color::from_argb(255, 101, 115, 133)),
                        )
                        .text_align(TextAlign::Center),
                    )
                    .left(x + 35.0)
                    .top(y + 3.0)
                    .width((w - 45.0).max(0.0))
                    .into_widget(),
                );
                if w > 125.0 && h > 90.0 {
                    window_labels.push(
                        Positioned::new(column(
                            vec![
                                Text::new(if phase.second_window {
                                    "Reading list"
                                } else {
                                    "Project notes"
                                })
                                .style(
                                    TextStyle::new()
                                        .font_size(12.0)
                                        .font_weight(FontWeight::W600)
                                        .color(Color::from_argb(255, 52, 68, 87)),
                                )
                                .into_widget(),
                                Text::new(if phase.second_window {
                                    "Saved for later"
                                } else {
                                    "Ideas for this week"
                                })
                                .style(
                                    TextStyle::new()
                                        .font_size(9.0)
                                        .color(Color::from_argb(255, 116, 133, 150)),
                                )
                                .into_widget(),
                            ],
                            6.0,
                        ))
                        .left(x + 14.0)
                        .top(y + 34.0)
                        .width(w - 28.0)
                        .into_widget(),
                    );
                }
            }
            layers.push(
                Positioned::fill(
                    Opacity::new(phase.window_opacity).child(Stack::new().children(window_labels)),
                )
                .into_widget(),
            );
            if matches!(config.demo, Demo::Panel | Demo::Preview) {
                layers.push(panel_layer(
                    &config,
                    width,
                    height,
                    phase.motion,
                    painter.accent,
                ));
            }
            layers.push(
                Positioned::fill(CustomPaint::new().painter(InteractionPainter(painter.clone())))
                    .into_widget(),
            );
            if phase.held && config.enabled && !config.shortcut.is_empty() {
                let keys = Key::ALL
                    .into_iter()
                    .filter(|key| config.shortcut.has(*key))
                    .map(|key| {
                        Container::new()
                            .padding(EdgeInsetsGeometry::symmetric(8.0, 4.0))
                            .decoration(
                                BoxDecoration::new()
                                    .color(Color::from_argb(235, 36, 42, 52))
                                    .border_radius(BorderRadius::circular(5.0)),
                            )
                            .child(
                                Text::new(key.symbol()).style(
                                    TextStyle::new()
                                        .font_size(17.0)
                                        .color(Color::from_argb(255, 255, 255, 255)),
                                ),
                            )
                            .into_widget()
                    })
                    .collect::<Vec<_>>();
                layers.push(
                    Positioned::new(
                        Center::new().child(
                            Row::new()
                                .main_axis_size(MainAxisSize::Min)
                                .spacing(5.0)
                                .children(keys),
                        ),
                    )
                    .left(0.0)
                    .right(0.0)
                    .bottom(12.0)
                    .into_widget(),
                );
            }
            let mut play = PathBuilder::new();
            play.move_to(Point::new(0.0, 0.0));
            play.line_to(Point::new(10.0, 6.0));
            play.line_to(Point::new(0.0, 12.0));
            play.close();

            layers.push(
                Positioned::new(ToolTipService::new(
                    Button::new(
                        SizedBox::new()
                            .width(12.0)
                            .height(12.0)
                            .child(PathIcon::new(play.build())),
                        Listener::new(move |app| self.replay(app)),
                    )
                    .is_enabled(config.enabled),
                    ToolTip::text("Replay preview"),
                ))
                .top(8.0)
                .right(8.0)
                .width(32.0)
                .height(32.0)
                .into_widget(),
            );
            ClipRRect::new()
                .border_radius(BorderRadius::circular(9.0).into())
                .child(Stack::new().children(layers))
                .into_widget()
        });
        let mut children = vec![canvas.into_widget()];
        if !widget.config.enabled {
            children.push(muted(
                "Enable the feature and select a shortcut to preview it.",
                widget.secondary,
            ));
        }
        column(children, 12.0)
    }
}

/// The panel surface, icons and labels share one clipped layout at every animation frame.
fn panel_layer(
    config: &Configuration,
    width: f64,
    height: f64,
    progress: f64,
    accent: Color,
) -> WidgetRef {
    let progress = if config.demo == Demo::Panel {
        progress
    } else {
        1.0
    };
    let full = width * 0.4;
    let visible = 20.0 + (full - 20.0) * progress;
    let left = config.side == Side::Left;
    let corners = if left {
        BorderRadius::horizontal(Radius::ZERO, Radius::circular(7.0))
    } else {
        BorderRadius::horizontal(Radius::circular(7.0), Radius::ZERO)
    };
    let viewport_x = if left { 0.0 } else { width - visible };
    let slide = config.reveal == PanelReveal::Slide;
    let content_width = if slide { full } else { visible };
    let offset = if slide && left { visible - full } else { 0.0 };
    let icons_right = (!slide && !left) || (slide && left);
    let mut content = Vec::new();
    for (i, title) in ["Notes", "Safari", "Finder"].into_iter().enumerate() {
        let y = height * 0.07 + i as f64 * 25.0;
        content.push(
            Positioned::new(
                Container::new().decoration(
                    BoxDecoration::new()
                        .color(accent)
                        .border_radius(BorderRadius::circular(3.0)),
                ),
            )
            .left(if icons_right {
                content_width - 17.0
            } else {
                5.0
            })
            .top(y)
            .width(12.0)
            .height(12.0)
            .into_widget(),
        );
        content.push(
            Positioned::new(
                ClipRect::new().child(
                    Text::new(title).soft_wrap(false).style(
                        TextStyle::new()
                            .font_size(9.0)
                            .color(Color::from_argb(255, 77, 95, 117)),
                    ),
                ),
            )
            .left(if icons_right { 9.0 } else { 24.0 })
            .top(y)
            .width((content_width - 35.0).max(0.0))
            .height(16.0)
            .into_widget(),
        );
    }
    let surface = Container::new()
        .decoration(
            BoxDecoration::new()
                .color(Color::from_argb(255, 249, 251, 254))
                .border(Border::all(
                    Color::from_argb(90, 112, 140, 169),
                    0.7,
                    BorderStyle::Solid,
                    -1.0,
                ))
                .border_radius(corners)
                .box_shadow(vec![BoxShadow::new(
                    Color::from_argb(55, 24, 53, 86),
                    Offset::new(0.0, 4.0),
                    12.0,
                    0.0,
                    BlurStyle::Normal,
                )]),
        )
        .child(
            ClipRRect::new()
                .border_radius(corners.into())
                .child(Stack::new().children(vec![
                        Positioned::new(Stack::new().children(content))
                            .left(offset)
                            .top(0.0)
                            .width(content_width)
                            .height(height * 0.64)
                            .into_widget(),
                    ])),
        );
    Positioned::new(surface)
        .left(viewport_x)
        .top(height * 0.25)
        .width(visible)
        .height(height * 0.64)
        .into_widget()
}

/// Normalized frames are shared by painting and geometry tests.
fn window_frame(config: &Configuration, progress: f64) -> Frame {
    let start = Frame {
        x: 21.0,
        y: 28.0,
        width: 55.0,
        height: 49.0,
    };
    let target = match config.demo {
        Demo::Ring(_) => config
            .zone
            .map(|zone| {
                zone.frame(&Frame {
                    x: 3.0,
                    y: 10.0,
                    width: 94.0,
                    height: 86.0,
                })
            })
            .unwrap_or(start),
        Demo::Move => Frame {
            x: if config.stays { 42.0 } else { 64.0 },
            y: 36.0,
            ..start
        },
        Demo::Resize => match config.corner {
            ResizeCorner::BottomRight => Frame {
                width: if config.stays { 76.0 } else { 91.0 },
                height: 65.0,
                ..start
            },
            ResizeCorner::Nearest => Frame {
                x: 6.0,
                y: 14.0,
                width: 70.0,
                height: 63.0,
            },
        },
        Demo::Preview => Frame {
            x: 7.0,
            y: 26.0,
            width: 55.0,
            height: 57.0,
        },
        Demo::Panel => start,
    };
    let t = if config.enabled { progress } else { 0.0 };
    Frame {
        x: start.x + (target.x - start.x) * t,
        y: start.y + (target.y - start.y) * t,
        width: start.width + (target.width - start.width) * t,
        height: start.height + (target.height - start.height) * t,
    }
}

#[derive(Clone)]
struct DesktopPainter {
    config: Configuration,
    progress: f64,
    accent: Color,
}

impl CustomPainter for DesktopPainter {
    fn paint(&self, _: &mut App, canvas: &mut Canvas, size: Size) {
        let w = size.width() as f32;
        let h = size.height() as f32;
        let fill = |color: Color| Paint {
            color: color.into(),
            ..Paint::default()
        };
        canvas.draw_rrect(
            Rect::new(0.0, 0.0, w, h),
            8.0,
            &fill(Color::from_argb(255, 191, 218, 245)),
        );
        // Inset clipping is visual only; none of this diagram is a platform view.
        canvas.save();
        canvas.clip_rect(Rect::new(0.0, 0.0, w, h), inset::ui::ClipOp::Intersect);
        let mut wallpaper = PathBuilder::new();
        wallpaper.move_to(Point::new(0.0, h * 0.8));
        wallpaper.line_to(Point::new(w, h * 0.3));
        wallpaper.line_to(Point::new(w, h));
        wallpaper.line_to(Point::new(0.0, h));
        wallpaper.close();
        canvas.draw_path(
            &wallpaper.build(),
            FillRule::NonZero,
            &fill(Color::from_argb(32, 90, 153, 221)),
        );
        canvas.draw_rect(
            Rect::new(0.0, 0.0, w, h * 0.075),
            &fill(Color::from_argb(255, 228, 239, 252)),
        );
        let phase = demo_phase(&self.config, self.progress);
        let frame = window_frame(&self.config, phase.window_motion);
        let rect = Rect::new(
            frame.x as f32 * w / 100.0,
            frame.y as f32 * h / 100.0,
            frame.width as f32 * w / 100.0,
            frame.height as f32 * h / 100.0,
        );
        if phase.window_opacity > 0.0 {
            let opacity = phase.window_opacity;
            let faded = |color: Color| {
                fill(color.with_values(Some(color.a * opacity), None, None, None, None))
            };
            canvas.draw_rrect(
                Rect::new(rect.x, rect.y + 3.0, rect.width, rect.height),
                5.0,
                &faded(Color::from_argb(40, 30, 73, 116)),
            );
            canvas.draw_rrect(rect, 5.0, &faded(Color::from_argb(255, 250, 252, 255)));
            for i in 0..3 {
                canvas.draw_circle(
                    inset::ui::valo::Point::new(rect.x + 8.0 + i as f32 * 7.0, rect.y + 8.0),
                    2.0,
                    &faded(Color::from_argb(255, 183, 196, 213)),
                );
            }
        }
        canvas.restore();
    }

    fn should_repaint(&self, _: &App, old: &dyn CustomPainter) -> bool {
        old.as_any().downcast_ref::<Self>().is_none_or(|old| {
            old.config != self.config || old.progress != self.progress || old.accent != self.accent
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Destination pictogram on each ring button; uses the same Zone::frame as the real ring action.
#[derive(Clone, Debug)]
pub(super) struct ZoneIcon {
    pub zone: Option<Zone>,
    pub color: Color,
}

impl CustomPainter for ZoneIcon {
    fn paint(&self, _: &mut App, canvas: &mut Canvas, size: Size) {
        let fill = |color: Color| Paint {
            color: color.into(),
            ..Paint::default()
        };
        canvas.draw_rrect(
            Rect::new(0.0, 0.0, size.width() as f32, size.height() as f32),
            3.0,
            &fill(Color::from_argb(80, 128, 150, 174)),
        );
        canvas.draw_rrect(
            Rect::new(
                1.0,
                1.0,
                size.width() as f32 - 2.0,
                size.height() as f32 - 2.0,
            ),
            2.0,
            &fill(Color::from_argb(255, 245, 248, 253)),
        );
        if let Some(zone) = self.zone {
            let frame = zone.frame(&Frame {
                x: 3.0,
                y: 3.0,
                width: size.width() - 6.0,
                height: size.height() - 6.0,
            });
            canvas.draw_rrect(
                Rect::new(
                    frame.x as f32,
                    frame.y as f32,
                    frame.width as f32,
                    frame.height as f32,
                ),
                1.0,
                &fill(self.color),
            );
        }
    }

    fn should_repaint(&self, _: &App, old: &dyn CustomPainter) -> bool {
        old.as_any()
            .downcast_ref::<Self>()
            .is_none_or(|old| old.zone != self.zone || old.color != self.color)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Named stages keep pointer movement separate from its effect on a window.
#[derive(Clone, Copy, Debug)]
struct DemoPhase {
    motion: f64,
    window_motion: f64,
    window_opacity: f64,
    second_window: bool,
    held: bool,
}

/// Interpolates one interval of the demonstration using Inset's native curve.
fn interval(time: f64, start: f64, end: f64) -> f64 {
    Curves::ease_in_out().transform(((time - start) / (end - start)).clamp(0.0, 1.0))
}

fn demo_phase(config: &Configuration, time: f64) -> DemoPhase {
    let motion = interval(time, 0.25, 0.75);
    let mut phase = DemoPhase {
        motion,
        window_motion: motion,
        window_opacity: 1.0,
        second_window: false,
        held: (0.12..0.87).contains(&time),
    };
    match config.demo {
        Demo::Ring(_) => {
            // Grab::let_go applies the destination when the shortcut is released.
            phase.window_motion = if time >= 0.87 { 1.0 } else { 0.0 };
        }
        Demo::Preview => {
            // Rest on Notes, wait for its picture, then move down to Safari.
            // At 2.2 seconds total, these pauses approximate REST (400 ms)
            // and FOLLOW (80 ms), followed by the host's 140 ms frame glide.
            phase.motion = interval(time, 0.58, 0.64);
            phase.second_window = time >= 0.68;
            phase.window_motion = interval(time, 0.68, 0.744);
            phase.window_opacity = if config.scenario == Scenario::Visible {
                // The frontmost visible window does not need a preview.
                1.0
            } else {
                interval(time, 0.38, 0.46)
            };
            phase.held = time >= 0.2;
        }
        _ => {}
    }
    phase
}

/// Foreground interaction cues stay above the sample window's text.
struct InteractionPainter(DesktopPainter);

impl CustomPainter for InteractionPainter {
    fn paint(&self, app: &mut App, canvas: &mut Canvas, size: Size) {
        let w = size.width() as f32;
        let h = size.height() as f32;
        let phase = demo_phase(&self.0.config, self.0.progress);
        let frame = window_frame(&self.0.config, phase.window_motion);
        let rect = Rect::new(
            frame.x as f32 * w / 100.0,
            frame.y as f32 * h / 100.0,
            frame.width as f32 * w / 100.0,
            frame.height as f32 * h / 100.0,
        );
        let fill = |color: Color| Paint {
            color: color.into(),
            ..Paint::default()
        };
        if let Demo::Ring(direction) = self.0.config.demo
            && phase.held
            && self.0.config.enabled
        {
            let mut zones = self.0.config.ring_zones;
            zones[direction.index()] = self.0.config.zone;
            canvas.save();
            canvas.translate(w * 0.5 - 44.0, h * 0.5 - 44.0);
            canvas.scale(0.4, 0.4);
            crate::ring::RingPainter {
                zones,
                chosen: (phase.motion > 0.15).then_some(direction),
                accent: self.0.accent,
                on_accent: Color::from_argb(255, 255, 255, 255),
            }
            .paint(
                app,
                canvas,
                Size::new(crate::ring::WINDOW_SIZE, crate::ring::WINDOW_SIZE),
            );
            canvas.restore();
        }
        if self.0.config.enabled {
            let pointer = match self.0.config.demo {
                Demo::Panel => Point::new(
                    if self.0.config.side == Side::Left {
                        8.0
                    } else {
                        w - 14.0
                    },
                    h * 0.47,
                ),
                Demo::Preview => Point::new(
                    if self.0.config.side == Side::Left {
                        w * 0.2
                    } else {
                        w * 0.8
                    },
                    h * 0.32
                        + 6.0
                        + 25.0 * phase.motion as f32
                        + h * 0.3 * (1.0 - interval(self.0.progress, 0.0, 0.2)) as f32,
                ),
                Demo::Move => Point::new(rect.x + rect.width * 0.65, rect.y + rect.height * 0.5),
                Demo::Resize if self.0.config.corner == ResizeCorner::Nearest => {
                    Point::new(rect.x, rect.y)
                }
                Demo::Resize => Point::new(rect.x + rect.width - 3.0, rect.y + rect.height - 3.0),
                Demo::Ring(direction) => {
                    let angle = -std::f32::consts::FRAC_PI_2
                        + direction.index() as f32 * std::f32::consts::FRAC_PI_4;
                    Point::new(
                        w * 0.5 + angle.cos() * 30.0 * phase.motion as f32,
                        h * 0.5 + angle.sin() * 30.0 * phase.motion as f32,
                    )
                }
            };
            let mut path = PathBuilder::new();
            path.move_to(pointer);
            for (dx, dy) in [
                (0.0, 17.0),
                (5.0, 12.0),
                (9.0, 20.0),
                (12.0, 18.0),
                (8.0, 10.0),
                (15.0, 10.0),
            ] {
                path.line_to(Point::new(pointer.x + dx, pointer.y + dy));
            }
            path.close();
            canvas.draw_path(
                &path.build(),
                FillRule::NonZero,
                &fill(Color::from_argb(255, 37, 56, 80)),
            );
        }
    }

    fn should_repaint(&self, _: &App, old: &dyn CustomPainter) -> bool {
        old.as_any().downcast_ref::<Self>().is_none_or(|old| {
            old.0.config != self.0.config
                || old.0.progress != self.0.progress
                || old.0.accent != self.0.accent
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(demo: Demo) -> Configuration {
        Configuration {
            demo,
            side: Side::Right,
            reveal: PanelReveal::Unfold,
            trigger: PreviewTrigger::Hover,
            corner: ResizeCorner::BottomRight,
            stays: true,
            zone: Some(Zone::Centred),
            scenario: Scenario::Visible,
            enabled: true,
            reduced_motion: false,
            shortcut: Chord::NONE.with(Key::Control).with(Key::Shift),
            ring_zones: Direction::CLOCKWISE.map(Zone::default_for),
        }
    }

    #[test]
    fn ring_keeps_the_window_still_until_shortcut_release() {
        let config = config(Demo::Ring(Direction::Right));
        let start = window_frame(&config, 0.0);
        for time in [0.0, 0.2, 0.5, 0.86] {
            let phase = demo_phase(&config, time);
            assert_eq!(window_frame(&config, phase.window_motion), start);
        }
        let selected = demo_phase(&config, 0.8);
        assert!(selected.held && selected.motion == 1.0);
        let released = demo_phase(&config, 0.87);
        assert!(!released.held);
        assert_eq!(
            window_frame(&config, released.window_motion),
            window_frame(&config, 1.0)
        );
    }

    #[test]
    fn preview_waits_then_switches_picture_after_pointer_reaches_next_row() {
        for scenario in [Scenario::Minimized, Scenario::OtherSpace] {
            let mut config = config(Demo::Preview);
            config.scenario = scenario;
            assert_eq!(demo_phase(&config, 0.3).window_opacity, 0.0);
            let shown = demo_phase(&config, 0.5);
            assert_eq!(shown.window_opacity, 1.0);
            assert!(!shown.second_window);
            assert_eq!(shown.window_motion, 0.0);
            let moving = demo_phase(&config, 0.62);
            assert!(moving.motion > 0.0);
            assert!(!moving.second_window);
            let switched = demo_phase(&config, 0.72);
            assert!(switched.second_window);
            assert!(switched.window_motion > 0.0 && switched.window_motion < 1.0);
            assert_eq!(switched.window_opacity, 1.0);
        }
    }

    #[test]
    fn panel_contents_are_present_during_opening() {
        use crate::test_support::Fixture;

        for side in [Side::Left, Side::Right] {
            for reveal in [PanelReveal::Slide, PanelReveal::Unfold] {
                let mut configuration = config(Demo::Panel);
                configuration.side = side;
                configuration.reveal = reveal;
                let fixture = Fixture::with_root([480, 320], move |app| {
                    let configuration = configuration.clone();
                    run_app(
                        app,
                        WidgetsApp::new(Color::from_argb(255, 0, 122, 255))
                            .debug_show_checked_mode_banner(false)
                            .builder(move |_, _, _| {
                                Container::new()
                                    .color(Color::from_argb(255, 173, 215, 245))
                                    .child(Stack::new().children(vec![panel_layer(
                                        &configuration,
                                        480.0,
                                        320.0,
                                        0.6,
                                        Color::from_argb(255, 0, 122, 255),
                                    )]))
                                    .into_widget()
                            })
                            .into_widget(),
                    );
                });
                fixture.find("Notes");
                fixture.capture(&format!("settings_panel_{side:?}_{reveal:?}"));
            }
        }
    }

    #[test]
    fn ring_preview_uses_the_real_destination_frames() {
        let mut config = config(Demo::Ring(Direction::Down));
        let usable = Frame {
            x: 3.0,
            y: 10.0,
            width: 94.0,
            height: 86.0,
        };
        for zone in Zone::ALL {
            config.zone = Some(zone);
            assert_eq!(window_frame(&config, 1.0), zone.frame(&usable));
        }
        config.zone = None;
        assert_eq!(window_frame(&config, 0.0), window_frame(&config, 1.0));
    }

    #[test]
    fn disabled_demo_stays_at_rest_and_screen_bounds_are_respected() {
        for demo in [Demo::Move, Demo::Resize] {
            let mut config = config(demo);
            let frame = window_frame(&config, 1.0);
            assert!(frame.x >= 0.0 && frame.y >= 0.0);
            assert!(frame.x + frame.width <= 100.0);
            assert!(frame.y + frame.height <= 100.0);
            config.enabled = false;
            assert_eq!(window_frame(&config, 0.0), window_frame(&config, 1.0));
        }
    }

    #[test]
    fn nearest_corner_resize_keeps_the_opposite_corner_fixed() {
        let mut config = config(Demo::Resize);
        config.corner = ResizeCorner::Nearest;
        let start = window_frame(&config, 0.0);
        let end = window_frame(&config, 1.0);
        assert_eq!(start.x + start.width, end.x + end.width);
        assert_eq!(start.y + start.height, end.y + end.height);
    }
}
