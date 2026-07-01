//! Hit-detection: matches player `InputEvent`s against a tier's charted
//! `NoteEvent`s, grading timing accuracy.
//!
//! Task 5.2 covers taps: `LaneDown(l)` fires directly against unconsumed
//! notes (with `space != Strum`) requiring lane `l`.
//!
//! Task 5.3 adds strum-chord firing and sustains:
//! - `held_lanes` tracks which lanes are currently held (set by `LaneDown`,
//!   cleared by `LaneUp`). `SpaceDown` fires the nearest unconsumed `Strum`
//!   note whose `lanes` exactly equals `held_lanes`, within `hit_us`.
//! - A matched note (tap or strum) with `sustain_len_us > 0` doesn't return
//!   its `NoteResult` at onset; it opens a sustain record instead, and the
//!   corresponding `LaneUp`/`SpaceUp` closes it, returning the finalized
//!   result (onset grade/offset + `sustain_len_error_us`).

use crate::chart::{NoteEvent, SpaceAction};
use crate::clock::SongPosUs;
use crate::input::{Grade, InputAction, InputEvent, TimingWindows};

/// How an open sustain was opened, so the matching release event can close it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SustainOpener {
    Lane(u8),
    Space,
}

/// A note that was hit at onset but whose sustain hasn't been released yet.
struct OpenSustain {
    note_idx: usize,
    target_us: SongPosUs,
    actual_onset_us: SongPosUs,
    signed_offset_us: SongPosUs,
    grade: Grade,
    opener: SustainOpener,
}

/// Outcome of matching one input event against one charted note (or, from
/// `finalize`, a note that was never matched).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteResult {
    /// The matched note's charted position.
    pub song_pos: SongPosUs,
    /// The charted onset the input was judged against (== `song_pos`).
    pub target_us: SongPosUs,
    /// The input event's song-position (0 for unhit notes surfaced by `finalize`).
    pub actual_us: SongPosUs,
    /// `actual - target`; early is negative, late is positive.
    pub signed_offset_us: SongPosUs,
    pub grade: Grade,
    /// Sustain hold-length error, µs: `(actual_hold_us - charted_sustain_len_us).abs()`.
    /// `0` for non-sustain notes (`sustain_len_us == 0`) and for `finalize`-emitted
    /// Misses.
    pub sustain_len_error_us: SongPosUs,
}

/// Matches a stream of `InputEvent`s against one tier's notes. Handles taps
/// (5.2), strum-chord firing, and sustain onset+hold-length (5.3); each note
/// is consumed at most once.
pub struct Judge<'a> {
    notes: &'a [NoteEvent],
    windows: TimingWindows,
    consumed: Vec<bool>,
    /// Bitmask of currently-held lanes (bit `l` set while lane `l` is held).
    /// Lanes `>= 8` are never tracked (guarded, never panics).
    held_lanes: u8,
    /// Sustains that were hit at onset but not yet released, keyed by the
    /// note index that opened them. At most one open sustain per note.
    open_sustains: Vec<OpenSustain>,
}

impl<'a> Judge<'a> {
    pub fn new(notes: &'a [NoteEvent], windows: TimingWindows) -> Self {
        let consumed = vec![false; notes.len()];
        Self {
            notes,
            windows,
            consumed,
            held_lanes: 0,
            open_sustains: Vec::new(),
        }
    }

    /// Feed one input event. Returns `Some(NoteResult)` if it produced a
    /// finalized result this call: a zero-length note matched at onset, or a
    /// sustain closed by a release; `None` otherwise (wrong lane/chord,
    /// out-of-window, opened-but-not-yet-closed sustain, or a release that
    /// closes nothing).
    pub fn feed(&mut self, event: InputEvent) -> Option<NoteResult> {
        match event.action {
            InputAction::LaneDown(l) => {
                if l < 8 {
                    self.held_lanes |= 1u8 << l;
                }
                self.try_match_lane_down(l, event.song_pos_us)
            }
            InputAction::LaneUp(l) => {
                if l < 8 {
                    self.held_lanes &= !(1u8 << l);
                }
                self.close_sustain(event.song_pos_us, |opener| {
                    matches!(opener, SustainOpener::Lane(ol) if *ol == l)
                })
            }
            InputAction::SpaceDown => self.try_match_space_down(event.song_pos_us),
            InputAction::SpaceUp => self.close_sustain(event.song_pos_us, |opener| {
                matches!(opener, SustainOpener::Space)
            }),
        }
    }

    /// `LaneDown(l)` tap-match: nearest unconsumed non-Strum note requiring
    /// lane `l`, within `hit_us`. `l >= 8` never matches (no lane bit set).
    fn try_match_lane_down(&mut self, lane: u8, actual: SongPosUs) -> Option<NoteResult> {
        if lane >= 8 {
            return None;
        }
        let lane_bit = 1u8 << lane;
        let idx = self.find_nearest(actual, |note| {
            note.space != SpaceAction::Strum && note.lanes & lane_bit != 0
        })?;
        self.resolve_match(idx, actual, SustainOpener::Lane(lane))
    }

    /// `SpaceDown` strum-match: nearest unconsumed Strum note whose `lanes`
    /// exactly equals the currently-held lane mask, within `hit_us`.
    fn try_match_space_down(&mut self, actual: SongPosUs) -> Option<NoteResult> {
        let held = self.held_lanes;
        let idx = self.find_nearest(actual, |note| {
            note.space == SpaceAction::Strum && note.lanes == held
        })?;
        self.resolve_match(idx, actual, SustainOpener::Space)
    }

    /// Nearest unconsumed note (by `|actual - target|`, tie -> earliest
    /// `song_pos_us`) satisfying `pred` and within `hit_us`.
    fn find_nearest(&self, actual: SongPosUs, pred: impl Fn(&NoteEvent) -> bool) -> Option<usize> {
        let mut best: Option<(usize, SongPosUs)> = None; // (index, abs_offset)
        for (i, note) in self.notes.iter().enumerate() {
            if self.consumed[i] {
                continue;
            }
            if !pred(note) {
                continue;
            }
            let abs_offset = (actual - note.song_pos_us).abs();
            if abs_offset > self.windows.hit_us {
                continue;
            }
            match best {
                None => best = Some((i, abs_offset)),
                Some((best_i, best_offset)) => {
                    // Nearest wins; tie-break on earliest song_pos_us (== lowest index,
                    // since notes are ordered ascending by song_pos_us).
                    if abs_offset < best_offset
                        || (abs_offset == best_offset
                            && note.song_pos_us < self.notes[best_i].song_pos_us)
                    {
                        best = Some((i, abs_offset));
                    }
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Consume note `idx`, compute its onset grade/offset, and either return
    /// its `NoteResult` immediately (no sustain) or open a sustain and
    /// return `None`.
    fn resolve_match(&mut self, idx: usize, actual: SongPosUs, opener: SustainOpener) -> Option<NoteResult> {
        self.consumed[idx] = true;
        let note = &self.notes[idx];
        let target = note.song_pos_us;
        let signed_offset_us = actual - target;
        let abs_offset = signed_offset_us.abs();
        let grade = self.windows.grade(abs_offset);

        if note.sustain_len_us > 0 {
            self.open_sustains.push(OpenSustain {
                note_idx: idx,
                target_us: target,
                actual_onset_us: actual,
                signed_offset_us,
                grade,
                opener,
            });
            None
        } else {
            Some(NoteResult {
                song_pos: target,
                target_us: target,
                actual_us: actual,
                signed_offset_us,
                grade,
                sustain_len_error_us: 0,
            })
        }
    }

    /// Close the open sustain (if any) whose opener matches `pred`, computing
    /// its hold-length error against the release event's song position.
    fn close_sustain(
        &mut self,
        release_us: SongPosUs,
        pred: impl Fn(&SustainOpener) -> bool,
    ) -> Option<NoteResult> {
        let pos = self.open_sustains.iter().position(|s| pred(&s.opener))?;
        let sustain = self.open_sustains.remove(pos);
        let actual_hold_us = release_us - sustain.actual_onset_us;
        let charted_sustain_us = self.notes[sustain.note_idx].sustain_len_us;
        let sustain_len_error_us = (actual_hold_us - charted_sustain_us).abs();
        Some(NoteResult {
            song_pos: sustain.target_us,
            target_us: sustain.target_us,
            actual_us: sustain.actual_onset_us,
            signed_offset_us: sustain.signed_offset_us,
            grade: sustain.grade,
            sustain_len_error_us,
        })
    }

    /// Consume `self`, returning: a `Miss` result for every note never
    /// consumed, plus a provisional onset result for every sustain still
    /// open (hit at onset but never released). Documented default: an
    /// unreleased sustain is scored as if held for zero time, i.e.
    /// `sustain_len_error_us == note.sustain_len_us`. It is NOT a Miss — the
    /// onset was hit. Results are in note order; nothing is double-emitted.
    pub fn finalize(self) -> Vec<NoteResult> {
        let mut results: Vec<NoteResult> = self
            .notes
            .iter()
            .zip(self.consumed.iter())
            .filter(|(_, &consumed)| !consumed)
            .map(|(note, _)| NoteResult {
                song_pos: note.song_pos_us,
                target_us: note.song_pos_us,
                actual_us: note.song_pos_us,
                signed_offset_us: 0,
                grade: Grade::Miss,
                sustain_len_error_us: 0,
            })
            .collect();

        for sustain in &self.open_sustains {
            let charted_sustain_us = self.notes[sustain.note_idx].sustain_len_us;
            results.push(NoteResult {
                song_pos: sustain.target_us,
                target_us: sustain.target_us,
                actual_us: sustain.actual_onset_us,
                signed_offset_us: sustain.signed_offset_us,
                grade: sustain.grade,
                sustain_len_error_us: charted_sustain_us,
            });
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{SoundRef, SpaceAction};

    fn note_at(song_pos: SongPosUs, lanes: u8, space: SpaceAction, sustain: SongPosUs) -> NoteEvent {
        NoteEvent {
            song_pos_us: song_pos,
            lanes,
            space,
            sustain_len_us: sustain,
            sound: SoundRef { set_index: 0, pitch: 60, velocity: 100 },
        }
    }

    #[test]
    fn on_target_tap_is_perfect_zero_offset() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        let r = j
            .feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) })
            .unwrap();
        assert_eq!(r.grade, Grade::Perfect);
        assert_eq!(r.signed_offset_us, 0);
    }

    #[test]
    fn late_tap_has_positive_offset_and_good_grade() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        let r = j
            .feed(InputEvent { song_pos_us: 1_040_000, action: InputAction::LaneDown(0) })
            .unwrap();
        assert_eq!(r.signed_offset_us, 40_000);
        assert_eq!(r.grade, Grade::Good);
    }

    #[test]
    fn wrong_lane_does_not_match() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        assert!(j
            .feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(2) })
            .is_none());
    }

    #[test]
    fn unhit_note_finalizes_as_miss() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
        let j = Judge::new(&notes, TimingWindows::default());
        let all = j.finalize();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].grade, Grade::Miss);
    }

    #[test]
    fn tap_outside_hit_window_does_not_match() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        // 80_001 offset is 1us beyond the default hit_us (80_000) boundary.
        assert!(j
            .feed(InputEvent { song_pos_us: 1_080_001, action: InputAction::LaneDown(0) })
            .is_none());
    }

    /// Three same-lane notes are all within `hit_us` of one tap, but at
    /// different distances, and the notes are deliberately stored
    /// out-of-order (nearest note is neither index 0 nor the last index).
    /// This rules out both a "pick the first eligible note" bug and a
    /// "pick the last eligible note" bug: either would select a note at
    /// 1_060_000 or 1_100_000 (offset 40_000 / 80_000) instead of the true
    /// nearest note at 1_000_000 (offset 20_000), which fails the
    /// `song_pos` / `signed_offset_us` assertions below.
    #[test]
    fn nearest_note_wins_among_two_in_window() {
        let notes = vec![
            note_at(1_060_000, 0b0001, SpaceAction::None, 0), // index 0: offset 40_000
            note_at(1_000_000, 0b0001, SpaceAction::None, 0), // index 1: nearest, offset 20_000
            note_at(1_100_000, 0b0001, SpaceAction::None, 0), // index 2: offset 80_000 (hit_us boundary)
        ];
        let mut j = Judge::new(&notes, TimingWindows::default());
        let r = j
            .feed(InputEvent { song_pos_us: 1_020_000, action: InputAction::LaneDown(0) })
            .unwrap();
        assert_eq!(r.song_pos, 1_000_000);
        assert_eq!(r.signed_offset_us, 20_000);
        assert_eq!(r.grade, Grade::Perfect);

        // Only the nearest note was consumed; the other two remain and
        // finalize as Misses.
        let misses = j.finalize();
        assert_eq!(misses.len(), 2);
        let miss_positions: std::collections::BTreeSet<_> =
            misses.iter().map(|m| m.song_pos).collect();
        assert_eq!(
            miss_positions,
            std::collections::BTreeSet::from([1_060_000, 1_100_000])
        );
        assert!(misses.iter().all(|m| m.grade == Grade::Miss));
    }

    /// Once a note is consumed by a matching tap, an identical follow-up tap
    /// must not re-match it (each note is consumable at most once).
    #[test]
    fn consumed_note_not_rematched() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());

        let first = j
            .feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) })
            .unwrap();
        assert_eq!(first.grade, Grade::Perfect);

        let second = j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) });
        assert!(second.is_none());

        assert!(j.finalize().is_empty());
    }

    /// Two same-lane notes are equidistant from the tap (both offset
    /// 20_000). Ties break on earliest `song_pos_us`. The notes are stored
    /// with the later note first in the slice, so a buggy implementation
    /// that keeps whichever note it saw first on a tie (rather than
    /// explicitly preferring the earliest `song_pos_us`) would wrongly
    /// select the 1_040_000 note and fail this assertion.
    #[test]
    fn tie_break_prefers_earliest_song_pos() {
        let notes = vec![
            note_at(1_040_000, 0b0001, SpaceAction::None, 0), // index 0: later note, seen first
            note_at(1_000_000, 0b0001, SpaceAction::None, 0), // index 1: earlier note
        ];
        let mut j = Judge::new(&notes, TimingWindows::default());
        let r = j
            .feed(InputEvent { song_pos_us: 1_020_000, action: InputAction::LaneDown(0) })
            .unwrap();
        assert_eq!(r.song_pos, 1_000_000);
        assert_eq!(r.signed_offset_us, 20_000);
    }

    // -- Task 5.3: strum-chord + sustain tests --------------------------

    #[test]
    fn strum_fires_held_chord() {
        let notes = vec![note_at(1_000_000, 0b0011, SpaceAction::Strum, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        j.feed(InputEvent { song_pos_us: 980_000, action: InputAction::LaneDown(0) });
        j.feed(InputEvent { song_pos_us: 980_000, action: InputAction::LaneDown(1) });
        let r = j
            .feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::SpaceDown })
            .unwrap();
        assert_eq!(r.grade, Grade::Perfect);
    }

    #[test]
    fn sustain_scores_hold_length_error() {
        let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 300_000)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) });
        let r = j
            .feed(InputEvent { song_pos_us: 1_320_000, action: InputAction::LaneUp(0) })
            .unwrap();
        assert_eq!(r.sustain_len_error_us, 20_000); // held 320ms vs charted 300ms
    }

    /// A Strum note requires the currently-held lane mask to equal the
    /// note's `lanes` exactly. Holding only lane 0 when the note needs lanes
    /// {0,1} must not fire on `SpaceDown`; the note remains unconsumed and
    /// finalizes as a Miss.
    #[test]
    fn strum_does_not_fire_on_wrong_held_chord() {
        let notes = vec![note_at(1_000_000, 0b0011, SpaceAction::Strum, 0)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        j.feed(InputEvent { song_pos_us: 980_000, action: InputAction::LaneDown(0) });
        let r = j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::SpaceDown });
        assert!(r.is_none());

        let misses = j.finalize();
        assert_eq!(misses.len(), 1);
        assert_eq!(misses[0].grade, Grade::Miss);
    }

    /// A Strum note with a sustain: `SpaceDown` opens the sustain (no result
    /// yet, note consumed), and `SpaceUp` closes it, reporting the onset
    /// grade plus the hold-length error.
    #[test]
    fn strum_sustain_opens_on_space_down_and_closes_on_space_up() {
        let notes = vec![note_at(1_000_000, 0b0011, SpaceAction::Strum, 300_000)];
        let mut j = Judge::new(&notes, TimingWindows::default());
        j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) });
        j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(1) });
        let opened = j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::SpaceDown });
        assert!(opened.is_none());

        let r = j
            .feed(InputEvent { song_pos_us: 1_310_000, action: InputAction::SpaceUp })
            .unwrap();
        assert_eq!(r.grade, Grade::Perfect);
        assert_eq!(r.sustain_len_error_us, 10_000); // held 310ms vs charted 300ms

        // Already emitted via SpaceUp; finalize must not double-emit it.
        assert!(j.finalize().is_empty());
    }
}
