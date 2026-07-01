//! Task 3.4: end-to-end MIDI import round-trip.
//!
//! Builds a tiny in-memory SMF (one melodic track + one GM-channel-10 drum
//! track, plus a tempo event), runs `import_chart`, encodes the resulting
//! `Chart`, decodes it back, and asserts the decoded chart equals the
//! imported chart byte-for-byte (struct equality) and that at least one
//! track carries real notes.

use band_core::chart::codec::{decode_chart, encode_chart};
use band_core::midi_import::{import_chart, RoleMapping};
use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// Two tracks, 480 PPQ, tempo 500000 µs/beat (120 BPM), 4/4:
/// - Track 0 (melodic, channel 0): notes at pitch 60 (tick 0) and 64 (tick 480).
/// - Track 1 (drums, channel 9/GM ch.10): kick (36) + snare (38) at tick 0,
///   hi-hat (42) at tick 240.
fn build_fixture() -> Vec<u8> {
    let melodic: Vec<TrackEvent> = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Guitar")),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message: MidiMessage::NoteOn { key: u7::new(60), vel: u7::new(100) },
            },
        },
        TrackEvent {
            delta: u28::new(240),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message: MidiMessage::NoteOff { key: u7::new(60), vel: u7::new(0) },
            },
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message: MidiMessage::NoteOn { key: u7::new(64), vel: u7::new(100) },
            },
        },
        TrackEvent {
            delta: u28::new(240),
            kind: TrackEventKind::Midi {
                channel: u4::new(0),
                message: MidiMessage::NoteOff { key: u7::new(64), vel: u7::new(0) },
            },
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];

    let drums: Vec<TrackEvent> = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Drums")),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: u4::new(9),
                message: MidiMessage::NoteOn { key: u7::new(36), vel: u7::new(110) },
            },
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: u4::new(9),
                message: MidiMessage::NoteOn { key: u7::new(38), vel: u7::new(100) },
            },
        },
        TrackEvent {
            delta: u28::new(10),
            kind: TrackEventKind::Midi {
                channel: u4::new(9),
                message: MidiMessage::NoteOff { key: u7::new(36), vel: u7::new(0) },
            },
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: u4::new(9),
                message: MidiMessage::NoteOff { key: u7::new(38), vel: u7::new(0) },
            },
        },
        TrackEvent {
            delta: u28::new(230),
            kind: TrackEventKind::Midi {
                channel: u4::new(9),
                message: MidiMessage::NoteOn { key: u7::new(42), vel: u7::new(90) },
            },
        },
        TrackEvent {
            delta: u28::new(10),
            kind: TrackEventKind::Midi {
                channel: u4::new(9),
                message: MidiMessage::NoteOff { key: u7::new(42), vel: u7::new(0) },
            },
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];

    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(480))),
        tracks: vec![melodic, drums],
    };
    let mut buf = Vec::new();
    smf.write(&mut buf).expect("write fixture smf");
    buf
}

#[test]
fn import_encode_decode_round_trips() {
    let bytes = build_fixture();
    let mapping = RoleMapping { guitar: Some(0), bass: None, drums: Some(1), vocal: None };

    let chart = import_chart(&bytes, mapping).expect("import_chart should succeed");

    // Sanity: guitar (role 0) and drums (role 2) carry real notes; bass/vocal are empty.
    assert!(!chart.core.tracks[0].tiers[0].notes.is_empty(), "guitar track should have notes");
    assert!(!chart.core.tracks[2].tiers[0].notes.is_empty(), "drums track should have notes");
    assert!(chart.core.tracks[1].tiers[0].notes.is_empty(), "bass track should be empty (unmapped)");
    assert!(chart.core.tracks[3].tiers[0].notes.is_empty(), "vocal track should be empty (unmapped)");

    // instrument_set_refs: always 4, role-ordered, fixed ids.
    assert_eq!(chart.core.instrument_set_refs.len(), 4);
    assert_eq!(chart.core.instrument_set_refs[0].id, "guitar_clean_v1");
    assert_eq!(chart.core.instrument_set_refs[1].id, "bass_finger_v1");
    assert_eq!(chart.core.instrument_set_refs[2].id, "drums_rock_v1");
    assert_eq!(chart.core.instrument_set_refs[3].id, "vocal_pad_v1");

    let encoded = encode_chart(&chart);
    let decoded = decode_chart(&encoded).expect("decode_chart should succeed");

    assert_eq!(decoded, chart, "decoded chart must equal the imported chart");
}

#[test]
fn import_with_no_roles_mapped_is_no_tracks_error() {
    let bytes = build_fixture();
    let mapping = RoleMapping { guitar: None, bass: None, drums: None, vocal: None };
    let err = import_chart(&bytes, mapping).expect_err("no mapped roles should error");
    assert_eq!(err, band_core::midi_import::ImportError::NoTracks);
}

#[test]
fn import_with_out_of_range_index_errors_without_panic() {
    let bytes = build_fixture();
    let mapping = RoleMapping { guitar: Some(99), bass: None, drums: None, vocal: None };
    let err = import_chart(&bytes, mapping).expect_err("out-of-range track index should error");
    assert_eq!(err, band_core::midi_import::ImportError::Unsupported("track index out of range"));
}
