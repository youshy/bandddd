//! THROWAWAY generator (Task 3.4 step 5): programmatically author a small,
//! believable 4-bar riff+bass+drums MIDI fixture so we have real test-chart
//! content without needing a royalty-free .mid file on hand. Not part of the
//! importer pipeline — run once to produce
//! `godot/assets/test_charts/src/test_groove.mid`, then delete this file.
//!
//! Run: `cargo run -p band-core --example gen_test_groove`

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

const PPQ: u16 = 480;
const EIGHTH: u32 = PPQ as u32 / 2; // 240 ticks
const QUARTER: u32 = PPQ as u32; // 480 ticks
const BARS: u32 = 4;

fn note_on(delta: u32, channel: u8, key: u8, vel: u8) -> TrackEvent<'static> {
    TrackEvent {
        delta: u28::new(delta),
        kind: TrackEventKind::Midi {
            channel: u4::new(channel),
            message: MidiMessage::NoteOn { key: u7::new(key), vel: u7::new(vel) },
        },
    }
}

fn note_off(delta: u32, channel: u8, key: u8) -> TrackEvent<'static> {
    TrackEvent {
        delta: u28::new(delta),
        kind: TrackEventKind::Midi {
            channel: u4::new(channel),
            message: MidiMessage::NoteOn { key: u7::new(key), vel: u7::new(0) },
        },
    }
}

fn guitar_track() -> Vec<TrackEvent<'static>> {
    // 120 BPM, 4/4, named "Guitar". Tempo + time-sig live here (track 0).
    let mut events = vec![
        TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))) },
        TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)) },
        TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Guitar")) },
    ];
    // A simple 8-note riff (E minor pentatonic-ish), repeated each bar, eighth notes,
    // gated slightly short (EIGHTH - 20 ticks) so consecutive notes don't tie.
    let riff = [52u8, 55, 57, 59, 60, 59, 57, 55];
    let gate = EIGHTH - 20;
    for _bar in 0..BARS {
        for (i, &pitch) in riff.iter().enumerate() {
            let delta_before = if i == 0 { 0 } else { EIGHTH - gate };
            events.push(note_on(delta_before, 0, pitch, 95));
            events.push(note_off(gate, 0, pitch));
        }
    }
    events.push(TrackEvent { delta: u28::new(EIGHTH - gate), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    events
}

fn bass_track() -> Vec<TrackEvent<'static>> {
    let mut events = vec![
        TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Bass")) },
    ];
    // Root notes on the beat, one octave down from the riff's tonic, held most of the beat.
    let roots = [40u8, 43, 45, 43]; // E1-ish walking pattern per bar
    let gate = QUARTER - 40;
    for _bar in 0..BARS {
        for (i, &pitch) in roots.iter().enumerate() {
            let delta_before = if i == 0 { 0 } else { QUARTER - gate };
            events.push(note_on(delta_before, 1, pitch, 90));
            events.push(note_off(gate, 1, pitch));
        }
    }
    events.push(TrackEvent { delta: u28::new(QUARTER - gate), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    events
}

fn drums_track() -> Vec<TrackEvent<'static>> {
    let mut events = vec![
        TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Drums")) },
    ];
    // Classic rock beat per bar: kick on 1 & 3, snare on 2 & 4, closed hi-hat on every eighth.
    // GM pitches: kick=36, snare=38, closed hi-hat=42.
    const KICK: u8 = 36;
    const SNARE: u8 = 38;
    const HIHAT: u8 = 42;
    let gate = 20u32; // short percussive hits

    for _bar in 0..BARS {
        for eighth_idx in 0..8u32 {
            let beat_in_bar = eighth_idx / 2; // 0,1,2,3
            let is_on_beat = eighth_idx % 2 == 0;
            let delta_before = if eighth_idx == 0 { 0 } else { EIGHTH - gate };

            // Hi-hat on every eighth.
            events.push(note_on(delta_before, 9, HIHAT, 70));
            // Kick on beats 0 & 2 (1 & 3), snare on beats 1 & 3 (2 & 4) — only on the beat, not the "and".
            if is_on_beat && beat_in_bar == 0 {
                events.push(note_on(0, 9, KICK, 110));
            } else if is_on_beat && beat_in_bar == 2 {
                events.push(note_on(0, 9, KICK, 105));
            } else if is_on_beat && (beat_in_bar == 1 || beat_in_bar == 3) {
                events.push(note_on(0, 9, SNARE, 100));
            }
            events.push(note_off(gate, 9, HIHAT));
            if is_on_beat && beat_in_bar == 0 {
                events.push(note_off(0, 9, KICK));
            } else if is_on_beat && beat_in_bar == 2 {
                events.push(note_off(0, 9, KICK));
            } else if is_on_beat && (beat_in_bar == 1 || beat_in_bar == 3) {
                events.push(note_off(0, 9, SNARE));
            }
        }
    }
    events.push(TrackEvent { delta: u28::new(EIGHTH - gate), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    events
}

fn main() {
    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(PPQ))),
        tracks: vec![guitar_track(), bass_track(), drums_track()],
    };
    let mut buf = Vec::new();
    smf.write(&mut buf).expect("write test_groove.mid");

    // CARGO_MANIFEST_DIR = .../rust/crates/band-core ; walk up to the repo root.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_path = manifest_dir.join("../../../godot/assets/test_charts/src/test_groove.mid");
    std::fs::write(&out_path, &buf).expect("write output file");
    println!("wrote {} bytes to {}", buf.len(), out_path.display());
}
