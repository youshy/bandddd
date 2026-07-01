//! Song-clock abstraction. Song-position (integer µs) is the master timebase.
//! The audio device's sample-accurate playhead is the real implementation
//! (see band-audio); SP2 swaps in a server-synced clock behind this same trait.

/// Integer microsecond song-position. Never a float — see Global Constraints.
pub type SongPosUs = i64;

pub trait SongClock: Send + Sync {
    /// Current song-position in microseconds (may be negative during count-in).
    fn song_pos_us(&self) -> SongPosUs;
    fn is_playing(&self) -> bool;
}

/// Deterministic test double: song-position is whatever you set.
#[derive(Debug, Default)]
pub struct ManualClock {
    pos_us: SongPosUs,
    playing: bool,
}

impl ManualClock {
    pub fn new() -> Self { Self::default() }
    pub fn set(&mut self, pos_us: SongPosUs) { self.pos_us = pos_us; }
    pub fn play(&mut self) { self.playing = true; }
    pub fn stop(&mut self) { self.playing = false; }
}

impl SongClock for ManualClock {
    fn song_pos_us(&self) -> SongPosUs { self.pos_us }
    fn is_playing(&self) -> bool { self.playing }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_clock_reports_set_position() {
        let mut c = ManualClock::new();
        assert!(!c.is_playing());
        c.play();
        c.set(1_500_000);
        assert_eq!(c.song_pos_us(), 1_500_000);
        assert!(c.is_playing());
    }
}
