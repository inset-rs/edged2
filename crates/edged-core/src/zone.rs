//! The places a window can be put in on its screen, and the directions of the ring that
//! pick them.

use edged_macos::Frame;

/// Pointer travel before the ring counts a direction.
const RING_DEAD_ZONE: f64 = 24.0;
/// How much of the screen the centred zone takes, per side.
const CENTRED_SHARE: f64 = 0.8;

/// A place on the screen a window is put in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// The whole of the screen's usable area.
    Fill,
    /// Most of it, centred.
    Centred,
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Zone {
    /// Every zone, in the order a menu lists them.
    pub const ALL: [Zone; 10] = [
        Zone::Fill,
        Zone::Centred,
        Zone::LeftHalf,
        Zone::RightHalf,
        Zone::TopHalf,
        Zone::BottomHalf,
        Zone::TopLeft,
        Zone::TopRight,
        Zone::BottomLeft,
        Zone::BottomRight,
    ];

    /// The zone each direction picks until set otherwise: up fills, down centres, left
    /// and right take their halves, and the diagonals their quarters.
    pub fn default_for(direction: Direction) -> Option<Zone> {
        Some(match direction {
            Direction::Up => Zone::Fill,
            Direction::Down => Zone::Centred,
            Direction::Left => Zone::LeftHalf,
            Direction::Right => Zone::RightHalf,
            Direction::UpLeft => Zone::TopLeft,
            Direction::UpRight => Zone::TopRight,
            Direction::DownLeft => Zone::BottomLeft,
            Direction::DownRight => Zone::BottomRight,
        })
    }

    /// What the ring calls the zone: a word or a glyph that fits beside its direction.
    pub fn label(self) -> &'static str {
        match self {
            Zone::Fill => "Fill",
            Zone::Centred => "80%",
            Zone::LeftHalf => "Left",
            Zone::RightHalf => "Right",
            Zone::TopHalf => "Top",
            Zone::BottomHalf => "Bottom",
            Zone::TopLeft => "◰",
            Zone::TopRight => "◳",
            Zone::BottomLeft => "◱",
            Zone::BottomRight => "◲",
        }
    }

    /// What a menu calls the zone.
    pub fn name(self) -> &'static str {
        match self {
            Zone::Fill => "Fill the screen",
            Zone::Centred => "Centre at 80%",
            Zone::LeftHalf => "Left half",
            Zone::RightHalf => "Right half",
            Zone::TopHalf => "Top half",
            Zone::BottomHalf => "Bottom half",
            Zone::TopLeft => "Top-left quarter",
            Zone::TopRight => "Top-right quarter",
            Zone::BottomLeft => "Bottom-left quarter",
            Zone::BottomRight => "Bottom-right quarter",
        }
    }

    pub fn stored(self) -> &'static str {
        match self {
            Zone::Fill => "fill",
            Zone::Centred => "centred",
            Zone::LeftHalf => "left-half",
            Zone::RightHalf => "right-half",
            Zone::TopHalf => "top-half",
            Zone::BottomHalf => "bottom-half",
            Zone::TopLeft => "top-left",
            Zone::TopRight => "top-right",
            Zone::BottomLeft => "bottom-left",
            Zone::BottomRight => "bottom-right",
        }
    }

    pub fn from_stored(value: &str) -> Option<Zone> {
        Zone::ALL.into_iter().find(|zone| zone.stored() == value)
    }

    /// The frame a window in this zone has, within the screen's usable area.
    pub fn frame(self, usable: &Frame) -> Frame {
        let half_width = usable.width / 2.0;
        let half_height = usable.height / 2.0;
        let right = usable.x + half_width;
        let bottom = usable.y + half_height;
        let quarter = |x, y| Frame {
            x,
            y,
            width: half_width,
            height: half_height,
        };
        match self {
            Zone::Fill => *usable,
            Zone::Centred => {
                let width = usable.width * CENTRED_SHARE;
                let height = usable.height * CENTRED_SHARE;
                Frame {
                    x: usable.x + (usable.width - width) / 2.0,
                    y: usable.y + (usable.height - height) / 2.0,
                    width,
                    height,
                }
            }
            Zone::LeftHalf => Frame {
                width: half_width,
                ..*usable
            },
            Zone::RightHalf => Frame {
                x: right,
                width: half_width,
                ..*usable
            },
            Zone::TopHalf => Frame {
                height: half_height,
                ..*usable
            },
            Zone::BottomHalf => Frame {
                y: bottom,
                height: half_height,
                ..*usable
            },
            Zone::TopLeft => quarter(usable.x, usable.y),
            Zone::TopRight => quarter(right, usable.y),
            Zone::BottomLeft => quarter(usable.x, bottom),
            Zone::BottomRight => quarter(right, bottom),
        }
    }
}

/// Where the pointer went from where the ring opened: one of eight, as a compass has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

/// The zone each direction of the ring picks, in [`Direction::CLOCKWISE`]'s order; `None`
/// for a direction that does nothing.
pub type RingZones = [Option<Zone>; 8];

impl Direction {
    /// Clockwise from up, as the ring lays them out.
    pub const CLOCKWISE: [Direction; 8] = [
        Direction::Up,
        Direction::UpRight,
        Direction::Right,
        Direction::DownRight,
        Direction::Down,
        Direction::DownLeft,
        Direction::Left,
        Direction::UpLeft,
    ];

    /// The zones the ring opens with until set otherwise.
    pub fn default_zones() -> RingZones {
        Direction::CLOCKWISE.map(Zone::default_for)
    }

    /// This direction's place in [`Self::CLOCKWISE`], which is its place in a [`RingZones`].
    pub fn index(self) -> usize {
        Direction::CLOCKWISE
            .iter()
            .position(|direction| *direction == self)
            .expect("every direction is in the clockwise order")
    }

    pub fn name(self) -> &'static str {
        match self {
            Direction::Up => "Up",
            Direction::UpRight => "Up-right",
            Direction::Right => "Right",
            Direction::DownRight => "Down-right",
            Direction::Down => "Down",
            Direction::DownLeft => "Down-left",
            Direction::Left => "Left",
            Direction::UpLeft => "Up-left",
        }
    }

    pub fn stored(self) -> &'static str {
        match self {
            Direction::Up => "up",
            Direction::UpRight => "up-right",
            Direction::Right => "right",
            Direction::DownRight => "down-right",
            Direction::Down => "down",
            Direction::DownLeft => "down-left",
            Direction::Left => "left",
            Direction::UpLeft => "up-left",
        }
    }

    /// The direction of an offset past the ring's dead zone, in eight sectors of 45°
    /// centred on the compass points; `None` within the dead zone.
    pub fn of((dx, dy): (f64, f64)) -> Option<Direction> {
        if dx.hypot(dy) < RING_DEAD_ZONE {
            return None;
        }
        // Clockwise from up, in eighths of a turn; the screen's y grows downwards.
        let turn = (dx.atan2(-dy) / std::f64::consts::TAU).rem_euclid(1.0);
        let sector = ((turn * 8.0).round() as usize) % 8;
        Some(Direction::CLOCKWISE[sector])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_direction_needs_the_ring_dead_zone_and_falls_in_one_of_eight_sectors() {
        assert_eq!(Direction::of((10.0, -10.0)), None);
        assert_eq!(Direction::of((0.0, -30.0)), Some(Direction::Up));
        assert_eq!(Direction::of((5.0, 40.0)), Some(Direction::Down));
        assert_eq!(Direction::of((-50.0, 5.0)), Some(Direction::Left));
        assert_eq!(Direction::of((30.0, 5.0)), Some(Direction::Right));
        assert_eq!(Direction::of((30.0, -30.0)), Some(Direction::UpRight));
        assert_eq!(Direction::of((-30.0, 30.0)), Some(Direction::DownLeft));
    }

    #[test]
    fn zones_divide_the_usable_area() {
        let usable = Frame {
            x: 0.0,
            y: 25.0,
            width: 1000.0,
            height: 600.0,
        };
        assert_eq!(Zone::Fill.frame(&usable), usable);
        let centred = Zone::Centred.frame(&usable);
        assert_eq!((centred.width, centred.height), (800.0, 480.0));
        assert_eq!((centred.x, centred.y), (100.0, 85.0));
        assert_eq!(Zone::LeftHalf.frame(&usable).width, 500.0);
        assert_eq!(Zone::RightHalf.frame(&usable).x, 500.0);
        assert_eq!(Zone::BottomHalf.frame(&usable).y, 325.0);
        let quarter = Zone::BottomRight.frame(&usable);
        assert_eq!((quarter.x, quarter.y), (500.0, 325.0));
        assert_eq!((quarter.width, quarter.height), (500.0, 300.0));
    }

    #[test]
    fn every_zone_and_direction_survives_being_stored() {
        for zone in Zone::ALL {
            assert_eq!(Zone::from_stored(zone.stored()), Some(zone));
        }
        for (index, direction) in Direction::CLOCKWISE.into_iter().enumerate() {
            assert_eq!(direction.index(), index);
        }
        assert_eq!(
            Direction::default_zones()[Direction::Up.index()],
            Some(Zone::Fill)
        );
    }
}
