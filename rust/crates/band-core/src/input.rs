//! Input model: raw player input events plus tunable timing windows used to
//! grade how close an input landed to the intended song-position (a note's
//! scheduled hit time). Pure data + grading only — hit-detection against a
//! chart (matching inputs to specific notes) is Task 5.2's `Judge`.

use crate::clock::SongPosUs;

/// A single discrete player input action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    LaneDown(u8),
    LaneUp(u8),
    SpaceDown,
    SpaceUp,
}

/// A timestamped input action. `song_pos_us` is already calibration-adjusted
/// upstream (by the Godot layer) — this crate never applies calibration itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub song_pos_us: SongPosUs,
    pub action: InputAction,
}

/// Tunable timing windows (µs) used to grade an input's absolute offset from
/// its target song-position. Each tier is inclusive of its own boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingWindows {
    /// Absolute-offset (µs) at/under which an input grades as `Perfect`. TUNABLE (SP1 §7).
    pub perfect_us: SongPosUs,
    /// Absolute-offset (µs) at/under which an input grades as `Good`. TUNABLE (SP1 §7).
    pub good_us: SongPosUs,
    /// Absolute-offset (µs) at/under which an input grades as `Hit`; beyond this is `Miss`. TUNABLE (SP1 §7).
    pub hit_us: SongPosUs,
}

impl Default for TimingWindows {
    fn default() -> Self {
        Self {
            perfect_us: 25_000,
            good_us: 50_000,
            hit_us: 80_000,
        }
    }
}

impl TimingWindows {
    /// Grade an ABSOLUTE offset (caller passes `.abs()` of signed error) against
    /// the configured windows. Boundaries are inclusive at each tier.
    pub fn grade(&self, abs_offset_us: SongPosUs) -> Grade {
        if abs_offset_us <= self.perfect_us {
            Grade::Perfect
        } else if abs_offset_us <= self.good_us {
            Grade::Good
        } else if abs_offset_us <= self.hit_us {
            Grade::Hit
        } else {
            Grade::Miss
        }
    }
}

/// Result of grading an input's timing offset against a `TimingWindows`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Perfect,
    Good,
    Hit,
    Miss,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_grade_by_absolute_offset() {
        let w = TimingWindows::default();
        assert_eq!(w.grade(0), Grade::Perfect);
        assert_eq!(w.grade(25_000), Grade::Perfect);
        assert_eq!(w.grade(25_001), Grade::Good);
        assert_eq!(w.grade(50_000), Grade::Good);
        assert_eq!(w.grade(80_000), Grade::Hit);
        assert_eq!(w.grade(80_001), Grade::Miss);
    }
}
