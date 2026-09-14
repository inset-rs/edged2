//! A chord of modifier keys, as a setting: what a hold is taken with, and what any
//! later shortcut of Edged's is set to. Keys with characters are not part of it.

use edged_macos::Modifiers;

/// One modifier key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Control,
    Option,
    Shift,
    Command,
}

impl Key {
    /// In the order the keys sit on the keyboard.
    pub const ALL: [Key; 4] = [Key::Control, Key::Option, Key::Shift, Key::Command];

    /// The key's symbol, as the menu bar spells it.
    pub fn symbol(self) -> &'static str {
        match self {
            Key::Control => "⌃",
            Key::Option => "⌥",
            Key::Shift => "⇧",
            Key::Command => "⌘",
        }
    }

    fn stored(self) -> &'static str {
        match self {
            Key::Control => "control",
            Key::Option => "option",
            Key::Shift => "shift",
            Key::Command => "command",
        }
    }

    fn from_stored(name: &str) -> Option<Key> {
        Key::ALL.into_iter().find(|key| key.stored() == name)
    }
}

/// A set of modifier keys held together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    pub control: bool,
    pub option: bool,
    pub shift: bool,
    pub command: bool,
}

impl Chord {
    pub const NONE: Chord = Chord {
        control: false,
        option: false,
        shift: false,
        command: false,
    };

    pub const fn with(self, key: Key) -> Chord {
        match key {
            Key::Control => Chord {
                control: true,
                ..self
            },
            Key::Option => Chord {
                option: true,
                ..self
            },
            Key::Shift => Chord {
                shift: true,
                ..self
            },
            Key::Command => Chord {
                command: true,
                ..self
            },
        }
    }

    pub fn has(&self, key: Key) -> bool {
        match key {
            Key::Control => self.control,
            Key::Option => self.option,
            Key::Shift => self.shift,
            Key::Command => self.command,
        }
    }

    /// This chord with `key` held or not.
    pub fn set(self, key: Key, held: bool) -> Chord {
        match key {
            Key::Control => Chord {
                control: held,
                ..self
            },
            Key::Option => Chord {
                option: held,
                ..self
            },
            Key::Shift => Chord {
                shift: held,
                ..self
            },
            Key::Command => Chord {
                command: held,
                ..self
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Chord::NONE
    }

    /// Whether the keys held are exactly this chord. An empty chord matches nothing:
    /// a shortcut with no keys is one that is off.
    pub fn matches(&self, held: Modifiers) -> bool {
        !self.is_empty()
            && self.control == held.control
            && self.option == held.option
            && self.shift == held.shift
            && self.command == held.command
    }

    /// The symbols of the keys held, in keyboard order; empty for no keys.
    pub fn label(&self) -> String {
        Key::ALL
            .into_iter()
            .filter(|key| self.has(*key))
            .map(Key::symbol)
            .collect()
    }

    /// The chord as it is kept in the defaults: the keys' names joined by `+`.
    pub fn stored(&self) -> String {
        Key::ALL
            .into_iter()
            .filter(|key| self.has(*key))
            .map(Key::stored)
            .collect::<Vec<_>>()
            .join("+")
    }

    /// A chord as `stored` wrote it; `None` for anything else, so a bad value falls
    /// back to the default rather than to no keys.
    pub fn from_stored(value: &str) -> Option<Chord> {
        if value.is_empty() {
            return Some(Chord::NONE);
        }
        value.split('+').try_fold(Chord::NONE, |chord, name| {
            Key::from_stored(name).map(|key| chord.with(key))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chord_matches_exactly_the_keys_it_names() {
        let chord = Chord::NONE.with(Key::Option).with(Key::Control);
        let held = |option, control, shift| Modifiers {
            option,
            control,
            shift,
            command: false,
        };
        assert!(chord.matches(held(true, true, false)));
        assert!(!chord.matches(held(true, false, false)));
        assert!(!chord.matches(held(true, true, true)));
        assert!(!Chord::NONE.matches(held(false, false, false)));
    }

    #[test]
    fn a_chord_survives_being_stored_and_reads_as_symbols() {
        let chord = Chord::NONE.with(Key::Option).with(Key::Shift);
        assert_eq!(chord.stored(), "option+shift");
        assert_eq!(Chord::from_stored("option+shift"), Some(chord));
        assert_eq!(Chord::from_stored(""), Some(Chord::NONE));
        assert_eq!(Chord::from_stored("option+fn"), None);
        assert_eq!(chord.label(), "⌥⇧");
        assert_eq!(chord.set(Key::Shift, false).label(), "⌥");
    }
}
