//! What the user set about Edged itself, kept across launches.

use edged_macos::Side;
use inset_foundation::{Context, EventEmitter};

use crate::shortcut::{Chord, Key};
use crate::zone::{Direction, RingZones, Zone};

/// When a window's preview shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewTrigger {
    /// Whenever the pointer rests on a window's row.
    Hover,
    /// Only while the Command key is held as the pointer rests on one.
    CommandKey,
}

impl PreviewTrigger {
    const KEY: &'static str = "preview_trigger";

    fn from_stored(value: &str) -> Option<PreviewTrigger> {
        match value {
            "hover" => Some(PreviewTrigger::Hover),
            "command" => Some(PreviewTrigger::CommandKey),
            _ => None,
        }
    }

    fn stored(self) -> &'static str {
        match self {
            PreviewTrigger::Hover => "hover",
            PreviewTrigger::CommandKey => "command",
        }
    }
}

/// Which corner a held window is resized from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeCorner {
    /// The bottom-right corner, wherever the pointer took hold: the window grows and
    /// shrinks the way a drag of its size box would.
    BottomRight,
    /// The corner nearest the pointer when the hold began.
    Nearest,
}

impl ResizeCorner {
    const KEY: &'static str = "grab_resize_corner";

    fn from_stored(value: &str) -> Option<ResizeCorner> {
        match value {
            "bottom-right" => Some(ResizeCorner::BottomRight),
            "nearest" => Some(ResizeCorner::Nearest),
            _ => None,
        }
    }

    fn stored(self) -> &'static str {
        match self {
            ResizeCorner::BottomRight => "bottom-right",
            ResizeCorner::Nearest => "nearest",
        }
    }
}

/// How the panel comes out when the pointer reaches its strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelReveal {
    /// The icons stay at the screen's edge and the titles unfold beside them.
    Unfold,
    /// The whole panel slides out, icons first, the titles following them.
    Slide,
}

impl PanelReveal {
    const KEY: &'static str = "panel_reveal";

    fn from_stored(value: &str) -> Option<PanelReveal> {
        match value {
            "unfold" => Some(PanelReveal::Unfold),
            "slide" => Some(PanelReveal::Slide),
            _ => None,
        }
    }

    fn stored(self) -> &'static str {
        match self {
            PanelReveal::Unfold => "unfold",
            PanelReveal::Slide => "slide",
        }
    }
}

const PANEL_SIDE_KEY: &str = "panel_side";

fn side_from_stored(value: &str) -> Option<Side> {
    match value {
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        _ => None,
    }
}

fn side_stored(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
    }
}

/// The user asked for the settings window.
#[derive(Clone, Copy, Debug)]
pub struct SettingsRequested;

/// The stored keys of the hold: whether it is on, and the chords that move, resize and
/// arrange.
const GRAB_KEY: &str = "grab";
const MOVE_CHORD_KEY: &str = "grab_move";
const RESIZE_CHORD_KEY: &str = "grab_resize";
const ARRANGE_CHORD_KEY: &str = "grab_arrange";
const STAYS_ON_SCREEN_KEY: &str = "grab_stay_on_screen";
const RING_KEY: &str = "ring";
/// The stored key of a ring direction's zone.
fn ring_zone_key(direction: Direction) -> String {
    format!("ring_{}", direction.stored())
}
/// The chords a fresh install holds windows with.
pub const DEFAULT_MOVE_CHORD: Chord = Chord::NONE.with(Key::Control).with(Key::Shift);
pub const DEFAULT_RESIZE_CHORD: Chord = Chord::NONE.with(Key::Option).with(Key::Shift);
pub const DEFAULT_ARRANGE_CHORD: Chord = Chord::NONE.with(Key::Control).with(Key::Command);

pub struct Settings {
    /// Whether macOS starts Edged at login.
    pub launches_at_login: bool,
    /// The screen edge the panel sits at.
    pub panel_side: Side,
    /// How the panel comes out.
    pub panel_reveal: PanelReveal,
    pub preview_trigger: PreviewTrigger,
    /// Whether holding the chords below moves and resizes the window under the pointer.
    pub grab_enabled: bool,
    /// The keys that move the window under the pointer while held.
    pub move_chord: Chord,
    /// The keys that resize it while held.
    pub resize_chord: Chord,
    /// The corner the resize drags.
    pub resize_corner: ResizeCorner,
    /// Whether holding the arrange chord shows the ring.
    pub ring_enabled: bool,
    /// The keys that show the ring of zones the window under the pointer can be put in.
    pub arrange_chord: Chord,
    /// Whether a moved or resized window stops at the screen's edge.
    pub stays_on_screen: bool,
    /// The zone each direction of the ring picks.
    pub ring_zones: RingZones,
}

/// A stored chord, or the default where none was stored or it does not read.
fn chord_default(key: &str, default: Chord) -> Chord {
    edged_macos::read_default(key)
        .as_deref()
        .and_then(Chord::from_stored)
        .unwrap_or(default)
}

impl EventEmitter<SettingsRequested> for Settings {}

#[cfg(test)]
impl Settings {
    /// The defaults, read from nothing.
    pub(crate) fn for_test() -> Settings {
        Settings {
            launches_at_login: false,
            panel_side: Side::Right,
            panel_reveal: PanelReveal::Unfold,
            preview_trigger: PreviewTrigger::Hover,
            grab_enabled: true,
            move_chord: DEFAULT_MOVE_CHORD,
            resize_chord: DEFAULT_RESIZE_CHORD,
            resize_corner: ResizeCorner::BottomRight,
            ring_enabled: true,
            arrange_chord: DEFAULT_ARRANGE_CHORD,
            stays_on_screen: true,
            ring_zones: Direction::default_zones(),
        }
    }
}

impl Settings {
    pub fn read() -> Settings {
        Settings {
            launches_at_login: edged_macos::launches_at_login(),
            panel_side: edged_macos::read_default(PANEL_SIDE_KEY)
                .as_deref()
                .and_then(side_from_stored)
                .unwrap_or(Side::Right),
            panel_reveal: edged_macos::read_default(PanelReveal::KEY)
                .as_deref()
                .and_then(PanelReveal::from_stored)
                .unwrap_or(PanelReveal::Unfold),
            preview_trigger: edged_macos::read_default(PreviewTrigger::KEY)
                .as_deref()
                .and_then(PreviewTrigger::from_stored)
                .unwrap_or(PreviewTrigger::CommandKey),
            grab_enabled: edged_macos::read_default(GRAB_KEY).as_deref() != Some("off"),
            move_chord: chord_default(MOVE_CHORD_KEY, DEFAULT_MOVE_CHORD),
            resize_chord: chord_default(RESIZE_CHORD_KEY, DEFAULT_RESIZE_CHORD),
            resize_corner: edged_macos::read_default(ResizeCorner::KEY)
                .as_deref()
                .and_then(ResizeCorner::from_stored)
                .unwrap_or(ResizeCorner::BottomRight),
            ring_enabled: edged_macos::read_default(RING_KEY).as_deref() != Some("off"),
            arrange_chord: chord_default(ARRANGE_CHORD_KEY, DEFAULT_ARRANGE_CHORD),
            stays_on_screen: edged_macos::read_default(STAYS_ON_SCREEN_KEY).as_deref()
                != Some("off"),
            ring_zones: Direction::CLOCKWISE.map(|direction| {
                match edged_macos::read_default(&ring_zone_key(direction)) {
                    Some(stored) if stored == "none" => None,
                    Some(stored) => Zone::from_stored(&stored).or(Zone::default_for(direction)),
                    None => Zone::default_for(direction),
                }
            }),
        }
    }

    pub fn set_stays_on_screen(&mut self, cx: &mut Context<Settings>, stays: bool) {
        if self.stays_on_screen == stays {
            return;
        }
        self.stays_on_screen = stays;
        edged_macos::write_default(STAYS_ON_SCREEN_KEY, if stays { "on" } else { "off" });
        cx.notify();
    }

    /// Sets what a direction of the ring does; `None` for nothing.
    pub fn set_ring_zone(
        &mut self,
        cx: &mut Context<Settings>,
        direction: Direction,
        zone: Option<Zone>,
    ) {
        let slot = &mut self.ring_zones[direction.index()];
        if *slot == zone {
            return;
        }
        *slot = zone;
        edged_macos::write_default(&ring_zone_key(direction), zone.map_or("none", Zone::stored));
        cx.notify();
    }

    pub fn set_arrange_chord(&mut self, cx: &mut Context<Settings>, chord: Chord) {
        if self.arrange_chord == chord {
            return;
        }
        self.arrange_chord = chord;
        edged_macos::write_default(ARRANGE_CHORD_KEY, &chord.stored());
        cx.notify();
    }

    pub fn set_resize_corner(&mut self, cx: &mut Context<Settings>, corner: ResizeCorner) {
        if self.resize_corner == corner {
            return;
        }
        self.resize_corner = corner;
        edged_macos::write_default(ResizeCorner::KEY, corner.stored());
        cx.notify();
    }

    pub fn set_move_chord(&mut self, cx: &mut Context<Settings>, chord: Chord) {
        if self.move_chord == chord {
            return;
        }
        self.move_chord = chord;
        edged_macos::write_default(MOVE_CHORD_KEY, &chord.stored());
        cx.notify();
    }

    pub fn set_resize_chord(&mut self, cx: &mut Context<Settings>, chord: Chord) {
        if self.resize_chord == chord {
            return;
        }
        self.resize_chord = chord;
        edged_macos::write_default(RESIZE_CHORD_KEY, &chord.stored());
        cx.notify();
    }

    pub fn set_panel_side(&mut self, cx: &mut Context<Settings>, side: Side) {
        if self.panel_side == side {
            return;
        }
        self.panel_side = side;
        edged_macos::write_default(PANEL_SIDE_KEY, side_stored(side));
        cx.notify();
    }

    pub fn set_panel_reveal(&mut self, cx: &mut Context<Settings>, reveal: PanelReveal) {
        if self.panel_reveal == reveal {
            return;
        }
        self.panel_reveal = reveal;
        edged_macos::write_default(PanelReveal::KEY, reveal.stored());
        cx.notify();
    }

    pub fn set_ring_enabled(&mut self, cx: &mut Context<Settings>, enabled: bool) {
        if self.ring_enabled == enabled {
            return;
        }
        self.ring_enabled = enabled;
        edged_macos::write_default(RING_KEY, if enabled { "on" } else { "off" });
        cx.notify();
    }

    pub fn set_grab_enabled(&mut self, cx: &mut Context<Settings>, enabled: bool) {
        if self.grab_enabled == enabled {
            return;
        }
        self.grab_enabled = enabled;
        edged_macos::write_default(GRAB_KEY, if enabled { "on" } else { "off" });
        cx.notify();
    }

    /// Registers or unregisters Edged as a login item.
    pub fn set_launches_at_login(
        &mut self,
        cx: &mut Context<Settings>,
        launches: bool,
    ) -> Result<(), String> {
        edged_macos::set_launches_at_login(launches)?;
        self.launches_at_login = launches;
        cx.notify();
        Ok(())
    }

    /// Asks for the settings window; the interface opens it.
    pub fn request_window(&mut self, cx: &mut Context<Settings>) {
        cx.emit(SettingsRequested);
    }

    pub fn set_preview_trigger(&mut self, cx: &mut Context<Settings>, trigger: PreviewTrigger) {
        if self.preview_trigger == trigger {
            return;
        }
        self.preview_trigger = trigger;
        edged_macos::write_default(PreviewTrigger::KEY, trigger.stored());
        cx.notify();
    }
}
