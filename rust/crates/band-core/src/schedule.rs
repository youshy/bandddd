//! Pure note-schedule builder: turns a chart's notes into a flat, sorted
//! `SoundCmd` list for the (future, Phase 4.2/4.3) audio thread to consume.
//! Stays pure/headless/wasm-clean — no audio deps here.

use crate::chart::{ChartCore, InstrumentRole};
use crate::clock::SongPosUs;
use crate::tunables::DEFAULT_HIT_GATE_US;

/// One scheduled note-on→note-off event: what to sound, when, and for how long.
/// Emitted both by [`build_auto_schedule`] (unmanned roles) and live, one at a
/// time, on each hit for the manned role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundCmd {
    pub at_us: SongPosUs,
    pub set_index: u16,
    pub pitch: u8,
    pub velocity: u8,
    /// Note-on→note-off duration (µs).
    pub gate_us: SongPosUs,
}

/// Build a flat, `at_us`-ascending list of [`SoundCmd`]s for one track's difficulty
/// tier — the auto-performed schedule the audio thread plays for unmanned roles.
///
/// Returns an empty `Vec` (no panic) if `tier` is out of range for that role's tiers.
pub fn build_auto_schedule(core: &ChartCore, role: InstrumentRole, tier: usize) -> Vec<SoundCmd> {
    let Some(tier) = core.tracks[role as usize].tiers.get(tier) else {
        return Vec::new();
    };

    tier.notes
        .iter()
        .map(|note| SoundCmd {
            at_us: note.song_pos_us,
            set_index: note.sound.set_index,
            pitch: note.sound.pitch,
            velocity: note.sound.velocity,
            gate_us: if note.sustain_len_us > 0 { note.sustain_len_us } else { DEFAULT_HIT_GATE_US },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::chart::{
        ChartCore, DifficultyTier, InstrumentRole, InstrumentTrack, NoteEvent, SoundRef,
        SpaceAction, NUM_ROLES,
    };
    use crate::tunables::DEFAULT_HIT_GATE_US;

    fn two_note_core() -> ChartCore {
        let sustained = NoteEvent {
            song_pos_us: 1_000_000,
            lanes: 0b0001,
            space: SpaceAction::Strum,
            sustain_len_us: 300_000,
            sound: SoundRef { set_index: 0, pitch: 60, velocity: 100 },
        };
        let hit = NoteEvent {
            song_pos_us: 2_000_000,
            lanes: 0b0010,
            space: SpaceAction::Strum,
            sustain_len_us: 0,
            sound: SoundRef { set_index: 1, pitch: 64, velocity: 90 },
        };
        ChartCore {
            duration_us: 4_000_000,
            tempo_map: vec![],
            instrument_set_refs: vec![],
            tracks: std::array::from_fn(|i| {
                if i == InstrumentRole::Guitar as usize {
                    InstrumentTrack {
                        tiers: vec![DifficultyTier { notes: vec![sustained.clone(), hit.clone()] }],
                    }
                } else {
                    InstrumentTrack::default()
                }
            }),
            lyrics: vec![],
            groove_sections: vec![],
        }
    }

    #[test]
    fn auto_schedule_two_notes_sustain_and_hit_gate() {
        let core = two_note_core();
        let cmds = super::build_auto_schedule(&core, InstrumentRole::Guitar, 0);

        assert_eq!(cmds.len(), 2);

        assert_eq!(cmds[0].at_us, 1_000_000);
        assert_eq!(cmds[0].set_index, 0);
        assert_eq!(cmds[0].pitch, 60);
        assert_eq!(cmds[0].velocity, 100);
        assert_eq!(cmds[0].gate_us, 300_000);

        assert_eq!(cmds[1].at_us, 2_000_000);
        assert_eq!(cmds[1].set_index, 1);
        assert_eq!(cmds[1].pitch, 64);
        assert_eq!(cmds[1].velocity, 90);
        assert_eq!(cmds[1].gate_us, DEFAULT_HIT_GATE_US);
    }

    #[test]
    fn out_of_range_tier_returns_empty_no_panic() {
        let core = two_note_core();
        let cmds = super::build_auto_schedule(&core, InstrumentRole::Guitar, 7);
        assert!(cmds.is_empty());

        // sanity: role as usize is always in-bounds for the fixed-size array.
        assert!((InstrumentRole::Vocal as usize) < NUM_ROLES);
    }
}
