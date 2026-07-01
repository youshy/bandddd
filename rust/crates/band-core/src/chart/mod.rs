//! Chart model. The `ChartCore` is hashed → content-address; the `Envelope`
//! is metadata that travels alongside and is NEVER hashed (SP3 §3c).

use crate::clock::SongPosUs;

pub mod codec;
pub mod hash;

/// On-wire format version. Bump on any breaking encode change; it namespaces
/// the hash so different format versions never collide.
pub const FORMAT_VERSION: u8 = 1;

pub const NUM_ROLES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentRole { Guitar = 0, Bass = 1, Drums = 2, Vocal = 3 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceAction { None = 0, Strum = 1, Kick = 2, Trigger = 3 }

impl SpaceAction {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None), 1 => Some(Self::Strum),
            2 => Some(Self::Kick), 3 => Some(Self::Trigger),
            _ => None,
        }
    }
}

/// Sample/patch + pitch + velocity to sound when this note is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundRef {
    pub set_index: u16, // index into ChartCore.instrument_set_refs
    pub pitch: u8,      // MIDI note number
    pub velocity: u8,   // 1..=127
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEvent {
    pub song_pos_us: SongPosUs, // feel-target (groove baked in)
    pub lanes: u8,              // bitmask, bits 0..=3
    pub space: SpaceAction,
    pub sustain_len_us: SongPosUs, // 0 = hit; >0 = ring/hold
    pub sound: SoundRef,
}

/// One difficulty tier's note set for one instrument. SP1 importer emits one;
/// the format supports many (SP3 bakes tiers into the hashed core).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DifficultyTier {
    pub notes: Vec<NoteEvent>, // ordered by song_pos_us ascending
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstrumentTrack {
    pub tiers: Vec<DifficultyTier>, // >=1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoEvent {
    pub song_pos_us: SongPosUs,
    pub micros_per_beat: u32,
    pub numerator: u8,   // time signature, for bar lines
    pub denominator: u8, // power-of-two note value
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricEvent {
    pub song_pos_us: SongPosUs,
    pub text: String,   // syllable/line
    pub line_break: bool,
}

/// Optional per-section intended feel direction (enables pocket-direction bonus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrooveSection {
    pub start_us: SongPosUs,
    pub feel_dir: i8, // -1 ahead / 0 straight / +1 behind (intended lean)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentSetRef {
    pub id: String, // sample-set id shipped with the game
}

/// The hashed playable core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartCore {
    pub duration_us: SongPosUs,
    pub tempo_map: Vec<TempoEvent>,
    pub instrument_set_refs: Vec<InstrumentSetRef>,
    pub tracks: [InstrumentTrack; NUM_ROLES], // Guitar,Bass,Drums,Vocal
    pub lyrics: Vec<LyricEvent>,
    pub groove_sections: Vec<GrooveSection>, // may be empty
}

/// Unhashed metadata (SP3 §3c). Renaming these never changes the content-address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Envelope {
    pub title: String,
    pub artist: String,
    pub author: String,
    pub version: String,
    pub labels: Vec<String>,
    pub tier_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chart {
    pub core: ChartCore,
    pub envelope: Envelope,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn sample_core() -> ChartCore {
        let note = NoteEvent {
            song_pos_us: 500_000, lanes: 0b0001, space: SpaceAction::Strum,
            sustain_len_us: 0,
            sound: SoundRef { set_index: 0, pitch: 60, velocity: 100 },
        };
        ChartCore {
            duration_us: 4_000_000,
            tempo_map: vec![TempoEvent {
                song_pos_us: 0, micros_per_beat: 500_000, numerator: 4, denominator: 4,
            }],
            instrument_set_refs: vec![InstrumentSetRef { id: "guitar_clean_v1".into() }],
            tracks: [
                InstrumentTrack { tiers: vec![DifficultyTier { notes: vec![note] }] },
                InstrumentTrack::default(),
                InstrumentTrack::default(),
                InstrumentTrack::default(),
            ],
            lyrics: vec![],
            groove_sections: vec![],
        }
    }

    #[test]
    fn core_constructs() {
        let c = sample_core();
        assert_eq!(c.tracks[0].tiers[0].notes[0].song_pos_us, 500_000);
    }
}
