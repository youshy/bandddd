//! Task 2.5: cross-target native-vs-WASM hash determinism (golden vector).
//!
//! `hash::content_address` hashes `FORMAT_VERSION` followed by
//! `codec::encode_core(core)` bytes with BLAKE3 (version-namespaced, see
//! `chart::hash`). This test fixes one `ChartCore` and asserts its
//! content-address equals a hardcoded golden hex string both natively and
//! under wasm32 (via `wasm-pack test --node`), proving the hash is
//! byte-identical across targets.

use band_core::chart::{codec, hash, *};

fn golden_core() -> ChartCore {
    ChartCore {
        duration_us: 2_000_000,
        tempo_map: vec![TempoEvent { song_pos_us: 0, micros_per_beat: 500_000, numerator: 4, denominator: 4 }],
        instrument_set_refs: vec![InstrumentSetRef { id: "guitar_clean_v1".into() }],
        tracks: [
            InstrumentTrack { tiers: vec![DifficultyTier { notes: vec![
                NoteEvent { song_pos_us: 0, lanes: 0b0001, space: SpaceAction::Strum, sustain_len_us: 0,
                    sound: SoundRef { set_index: 0, pitch: 40, velocity: 100 } },
                NoteEvent { song_pos_us: 500_013, lanes: 0b0011, space: SpaceAction::Strum, sustain_len_us: 250_000,
                    sound: SoundRef { set_index: 0, pitch: 45, velocity: 90 } },
            ] }] },
            InstrumentTrack::default(), InstrumentTrack::default(), InstrumentTrack::default(),
        ],
        lyrics: vec![LyricEvent { song_pos_us: 0, text: "la".into(), line_break: false }],
        groove_sections: vec![],
    }
}

// Paste the value printed by `content_address(&golden_core())` here:
const GOLDEN: &str = "6989f2c9c0a25d0023d12db12a9bab6dfafcbf899aa73b9296bbb131bc434a8b";

#[test]
fn golden_address_matches() {
    assert_eq!(hash::content_address(&golden_core()), GOLDEN);
}

#[test]
fn encode_is_byte_stable() {
    assert_eq!(codec::encode_core(&golden_core()), codec::encode_core(&golden_core()));
}

// --- wasm target: same assertions, same golden constant ---
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen_test::*;
    #[wasm_bindgen_test]
    fn golden_address_matches_wasm() {
        assert_eq!(hash::content_address(&golden_core()), GOLDEN);
    }
}
