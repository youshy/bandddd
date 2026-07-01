//! TUNABLE constants. Values here are deliberate gameplay/feel knobs, not derived
//! facts — expect them to be revisited/balanced. Keeping them in one file makes
//! every tunable easy to find and diff independently from the algorithms that use them.

/// Minimum note length (µs) for a melodic note to become a sustain rather than a hit. TUNABLE.
pub const SUSTAIN_MIN_US: crate::clock::SongPosUs = 200_000;

/// Note-on→note-off gate (µs) applied to a hit (non-sustained) note when auto-performed. TUNABLE.
pub const DEFAULT_HIT_GATE_US: crate::clock::SongPosUs = 120_000;
