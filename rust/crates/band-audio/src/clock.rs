use band_core::clock::{SongClock, SongPosUs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A song-clock driven by the audio device's frame counter. The audio callback
/// calls `advance(frames)` each buffer; `song_pos_us` is derived purely from
/// frames played (sample-accurate), never wall-clock.
#[derive(Clone)]
pub struct AudioSongClock {
    frames: Arc<AtomicU64>,
    sample_rate: u32,
    playing: Arc<std::sync::atomic::AtomicBool>,
}

impl AudioSongClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            frames: Arc::new(AtomicU64::new(0)),
            sample_rate,
            playing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    /// Called from the audio callback (real-time thread) — lock-free.
    pub fn advance(&self, frames: u64) {
        self.frames.fetch_add(frames, Ordering::Relaxed);
    }
    pub fn set_playing(&self, p: bool) {
        self.playing.store(p, Ordering::Relaxed);
    }
    fn frames_played(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }
}

impl SongClock for AudioSongClock {
    fn song_pos_us(&self) -> SongPosUs {
        // Integer math only. frames * 1_000_000 / sample_rate.
        let f = self.frames_played() as u128;
        ((f * 1_000_000u128) / self.sample_rate as u128) as SongPosUs
    }
    fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn song_pos_derives_from_frames() {
        let c = AudioSongClock::new(48_000);
        assert_eq!(c.song_pos_us(), 0);
        c.advance(48_000); // exactly one second
        assert_eq!(c.song_pos_us(), 1_000_000);
        c.advance(24_000); // + half second
        assert_eq!(c.song_pos_us(), 1_500_000);
    }
}
