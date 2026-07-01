//! Hit-detection: matches player `InputEvent`s against a tier's charted
//! `NoteEvent`s, grading timing accuracy. This task (5.2) covers taps only —
//! `LaneDown(l)` fires directly against unconsumed notes requiring lane `l`.
//! Strum-chord firing and sustain hold-length land in Task 5.3; `Judge` is
//! structured so that work can add `held_lanes` / sustain tracking alongside
//! the existing `consumed` flags without a rewrite.

use crate::chart::NoteEvent;
use crate::clock::SongPosUs;
use crate::input::{Grade, InputAction, InputEvent, TimingWindows};

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
    /// Sustain hold-length error, µs. Always 0 in this task — sustains land in Task 5.3.
    pub sustain_len_error_us: SongPosUs,
}

/// Matches a stream of `InputEvent`s against one tier's notes. Taps only
/// (Task 5.2); each note is consumed at most once.
pub struct Judge<'a> {
    notes: &'a [NoteEvent],
    windows: TimingWindows,
    consumed: Vec<bool>,
}

impl<'a> Judge<'a> {
    pub fn new(notes: &'a [NoteEvent], windows: TimingWindows) -> Self {
        let consumed = vec![false; notes.len()];
        Self { notes, windows, consumed }
    }

    /// Feed one input event. Returns `Some(NoteResult)` if it matched an
    /// unconsumed note within `hit_us`; `None` otherwise (wrong lane,
    /// out-of-window, or an action this task doesn't handle yet).
    pub fn feed(&mut self, event: InputEvent) -> Option<NoteResult> {
        let lane = match event.action {
            InputAction::LaneDown(l) => l,
            // LaneUp / SpaceDown / SpaceUp: no tap mechanism in this task.
            _ => return None,
        };
        if lane >= 8 {
            return None;
        }
        let lane_bit = 1u8 << lane;
        let actual = event.song_pos_us;

        let mut best: Option<(usize, SongPosUs)> = None; // (index, abs_offset)
        for (i, note) in self.notes.iter().enumerate() {
            if self.consumed[i] {
                continue;
            }
            if note.lanes & lane_bit == 0 {
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

        let (idx, abs_offset) = best?;
        self.consumed[idx] = true;
        let target = self.notes[idx].song_pos_us;
        let signed_offset_us = actual - target;
        Some(NoteResult {
            song_pos: target,
            target_us: target,
            actual_us: actual,
            signed_offset_us,
            grade: self.windows.grade(abs_offset),
            sustain_len_error_us: 0,
        })
    }

    /// Consume `self`, returning a `Miss` result for every note never
    /// consumed, in note order. Consumed notes are not re-emitted here.
    pub fn finalize(self) -> Vec<NoteResult> {
        self.notes
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
            .collect()
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
}
