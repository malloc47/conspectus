//! Writing an age as compact text in the largest unit it has reached.

use super::Age;

/// One step of a [`Ladder`]: how many seconds the unit is and the suffix
/// that follows its count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unit {
    /// The unit's length in seconds; at least 1.
    pub seconds: u64,
    /// What follows the count: `s`, `m`, `h`, `d`, `w`.
    pub suffix: &'static str,
}

impl Unit {
    /// A second: `s`.
    pub const SECOND: Self = Self::new(1, "s");
    /// A minute: `m`.
    pub const MINUTE: Self = Self::new(60, "m");
    /// An hour: `h`.
    pub const HOUR: Self = Self::new(3_600, "h");
    /// A day: `d`.
    pub const DAY: Self = Self::new(86_400, "d");
    /// A week: `w`.
    pub const WEEK: Self = Self::new(7 * 86_400, "w");

    /// A unit `seconds` long, written with `suffix`.
    pub const fn new(seconds: u64, suffix: &'static str) -> Self {
        Self { seconds, suffix }
    }
}

/// The units an age is written in, smallest first.
///
/// An age is written as a whole count of the largest unit it has
/// reached, rounded down: on [`Ladder::DAYS`], 59 seconds is `59s`, 60
/// is `1m`, 3,599 is `59m`, and two days and a half is `2d`. The units
/// are the thresholds, so a host that wants another top unit, or no
/// seconds, passes its own to [`Ladder::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ladder {
    units: &'static [Unit],
}

impl Ladder {
    /// Seconds, minutes, hours, and days, with no larger unit: a year is
    /// `365d`. The default ladder, and the one whose labels fit a
    /// [`CELL_WIDTH`](super::CELL_WIDTH) cell for under a thousand days.
    pub const DAYS: Self = Self::new(&[Unit::SECOND, Unit::MINUTE, Unit::HOUR, Unit::DAY]);

    /// [`Ladder::DAYS`] going on to weeks: twenty days is `2w`.
    pub const WEEKS: Self = Self::new(&[
        Unit::SECOND,
        Unit::MINUTE,
        Unit::HOUR,
        Unit::DAY,
        Unit::WEEK,
    ]);

    /// A ladder of `units`, smallest first.
    ///
    /// # Panics
    ///
    /// When `units` is empty, a unit is shorter than a second, or the
    /// units are not in increasing order of length. A ladder written as a
    /// constant fails to compile instead.
    pub const fn new(units: &'static [Unit]) -> Self {
        assert!(!units.is_empty(), "a ladder needs at least one unit");
        let mut index = 0;
        while index < units.len() {
            assert!(units[index].seconds > 0, "a unit is at least a second");
            if index > 0 {
                assert!(
                    units[index].seconds > units[index - 1].seconds,
                    "a ladder's units grow from the smallest"
                );
            }
            index += 1;
        }
        Self { units }
    }

    /// The units, smallest first.
    pub const fn units(&self) -> &'static [Unit] {
        self.units
    }

    /// `age` in the largest unit it has reached, rounded down: `45s`,
    /// `17m`, `3h`, `12d`. An age shorter than the first unit is a count
    /// of 0 in that unit.
    pub fn label(&self, age: Age) -> String {
        let secs = age.secs();
        let unit = self
            .units
            .iter()
            .rev()
            .find(|unit| unit.seconds <= secs)
            .unwrap_or(&self.units[0]);
        format!("{}{}", secs / unit.seconds, unit.suffix)
    }

    /// [`Ladder::label`] as prose, the one suffix every surface shares:
    /// `5m ago`.
    pub fn ago(&self, age: Age) -> String {
        format!("{} ago", self.label(age))
    }
}

impl Default for Ladder {
    /// [`Ladder::DAYS`].
    fn default() -> Self {
        Self::DAYS
    }
}
