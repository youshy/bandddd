//! Minimal MIDI importer: SMF → tempo map + per-track absolute-µs note lists.
//! Scope = believable single-tier TEST charts only (SP1 §3). Not the SP3 translator.

use std::collections::HashMap;

use crate::chart::TempoEvent;
use crate::clock::SongPosUs;

#[derive(Debug, PartialEq)]
pub enum ImportError {
    Parse(String),
    NoTracks,
    Unsupported(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiNote {
    pub start_us: SongPosUs,
    pub len_us: SongPosUs,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MidiTrack {
    pub name: Option<String>,
    pub channel10: bool,
    pub notes: Vec<MidiNote>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMidi {
    pub ppq: u16,
    pub tempo_map: Vec<TempoEvent>,
    pub tracks: Vec<MidiTrack>,
}

const DEFAULT_MICROS_PER_BEAT: u32 = 500_000;
const DEFAULT_NUMERATOR: u8 = 4;
const DEFAULT_DENOMINATOR: u8 = 4;

/// GM channel 10 (drums), zero-based channel index.
const GM_CHANNEL_10: u8 = 9;

/// A tempo/time-signature state change keyed by absolute MIDI tick. Sorted ascending;
/// the first entry is always at tick 0 (synthesized from the MIDI default if the file
/// doesn't set one explicitly before the first note).
struct TempoSegment {
    tick: u64,
    micros_per_beat: u32,
    numerator: u8,
    denominator: u8,
}

/// Convert an absolute tick position to integer microseconds by integrating across
/// tempo segments. Pure integer arithmetic per Global Constraints (no floats): each
/// segment's contribution is `(segment_ticks as i128 * mpb as i128) / ppq as i128`.
fn ticks_to_us(abs_ticks: u64, ppq: u16, segments: &[TempoSegment]) -> SongPosUs {
    let mut us: i128 = 0;
    for (i, seg) in segments.iter().enumerate() {
        if seg.tick >= abs_ticks {
            break;
        }
        let next_tick = segments
            .get(i + 1)
            .map(|s| s.tick)
            .unwrap_or(abs_ticks)
            .min(abs_ticks);
        let segment_ticks = next_tick - seg.tick;
        us += (segment_ticks as i128 * seg.micros_per_beat as i128) / ppq as i128;
        if next_tick >= abs_ticks {
            break;
        }
    }
    us as SongPosUs
}

/// Raw tempo-relevant meta event, keyed by absolute tick, before folding into segments.
enum RawMeta {
    Tempo(u32),
    TimeSig(u8, u8),
}

/// Parse a Standard MIDI File into a tempo map and per-track absolute-µs note lists.
pub fn parse_smf(bytes: &[u8]) -> Result<ParsedMidi, ImportError> {
    let smf = midly::Smf::parse(bytes).map_err(|e| ImportError::Parse(e.to_string()))?;

    if smf.tracks.is_empty() {
        return Err(ImportError::NoTracks);
    }

    let ppq = match smf.header.timing {
        midly::Timing::Metrical(ppq) => ppq.as_int(),
        midly::Timing::Timecode(..) => return Err(ImportError::Unsupported("SMPTE timing")),
    };

    // Pass 1: collect every tempo/time-signature meta event across all tracks, keyed by
    // absolute tick. SMF tempo events typically live in track 0 but can appear anywhere.
    let mut raw_meta: Vec<(u64, RawMeta)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        for event in track {
            tick += event.delta.as_int() as u64;
            if let midly::TrackEventKind::Meta(meta) = event.kind {
                match meta {
                    midly::MetaMessage::Tempo(mpb) => {
                        raw_meta.push((tick, RawMeta::Tempo(mpb.as_int())));
                    }
                    midly::MetaMessage::TimeSignature(numerator, denom_pow, ..) => {
                        raw_meta.push((tick, RawMeta::TimeSig(numerator, 1u8 << denom_pow)));
                    }
                    _ => {}
                }
            }
        }
    }
    raw_meta.sort_by_key(|(tick, _)| *tick);

    // Pass 2: fold the raw events into tempo segments, carrying the running state forward.
    // Default tempo if none seen before the first note: 500000 µs/beat, 4/4.
    let mut segments: Vec<TempoSegment> = Vec::new();
    let mut cur_mpb = DEFAULT_MICROS_PER_BEAT;
    let mut cur_num = DEFAULT_NUMERATOR;
    let mut cur_denom = DEFAULT_DENOMINATOR;
    let mut idx = 0;
    while idx < raw_meta.len() {
        let tick = raw_meta[idx].0;
        while idx < raw_meta.len() && raw_meta[idx].0 == tick {
            match raw_meta[idx].1 {
                RawMeta::Tempo(mpb) => cur_mpb = mpb,
                RawMeta::TimeSig(n, d) => {
                    cur_num = n;
                    cur_denom = d;
                }
            }
            idx += 1;
        }
        segments.push(TempoSegment {
            tick,
            micros_per_beat: cur_mpb,
            numerator: cur_num,
            denominator: cur_denom,
        });
    }
    if segments.first().map(|s| s.tick) != Some(0) {
        segments.insert(
            0,
            TempoSegment {
                tick: 0,
                micros_per_beat: DEFAULT_MICROS_PER_BEAT,
                numerator: DEFAULT_NUMERATOR,
                denominator: DEFAULT_DENOMINATOR,
            },
        );
    }

    let tempo_map: Vec<TempoEvent> = segments
        .iter()
        .map(|seg| TempoEvent {
            song_pos_us: ticks_to_us(seg.tick, ppq, &segments),
            micros_per_beat: seg.micros_per_beat,
            numerator: seg.numerator,
            denominator: seg.denominator,
        })
        .collect();

    // Pass 3: per-track notes, pairing Note-On(vel>0) with the matching later
    // Note-Off/Note-On(vel==0) on the same channel+key.
    let mut tracks = Vec::with_capacity(smf.tracks.len());
    for track in &smf.tracks {
        let mut midi_track = MidiTrack::default();
        let mut tick: u64 = 0;
        let mut open: HashMap<(u8, u8), (u64, u8)> = HashMap::new(); // (channel, key) -> (start_tick, velocity)
        for event in track {
            tick += event.delta.as_int() as u64;
            match event.kind {
                midly::TrackEventKind::Meta(midly::MetaMessage::TrackName(name)) => {
                    midi_track.name = Some(String::from_utf8_lossy(name).into_owned());
                }
                midly::TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    if channel == GM_CHANNEL_10 {
                        midi_track.channel10 = true;
                    }
                    match message {
                        midly::MidiMessage::NoteOn { key, vel } => {
                            let key = key.as_int();
                            let vel = vel.as_int();
                            if vel > 0 {
                                open.insert((channel, key), (tick, vel));
                            } else if let Some((start_tick, velocity)) =
                                open.remove(&(channel, key))
                            {
                                push_note(&mut midi_track, start_tick, tick, key, velocity, ppq, &segments);
                            }
                        }
                        midly::MidiMessage::NoteOff { key, .. } => {
                            let key = key.as_int();
                            if let Some((start_tick, velocity)) = open.remove(&(channel, key)) {
                                push_note(&mut midi_track, start_tick, tick, key, velocity, ppq, &segments);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        midi_track.notes.sort_by_key(|n| n.start_us);
        tracks.push(midi_track);
    }

    Ok(ParsedMidi {
        ppq,
        tempo_map,
        tracks,
    })
}

#[allow(clippy::too_many_arguments)]
fn push_note(
    track: &mut MidiTrack,
    start_tick: u64,
    end_tick: u64,
    pitch: u8,
    velocity: u8,
    ppq: u16,
    segments: &[TempoSegment],
) {
    let start_us = ticks_to_us(start_tick, ppq, segments);
    let end_us = ticks_to_us(end_tick, ppq, segments);
    track.notes.push(MidiNote {
        start_us,
        len_us: end_us - start_us,
        pitch,
        velocity,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::{u15, u24, u28, u4, u7};
    use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

    /// One track, 480 PPQ, tempo 500000 µs/beat, 4/4. Two notes: pitch 60 at tick 0
    /// (240 ticks long) and pitch 62 at tick 480 (240 ticks long), on channel 0.
    fn build_fixture() -> Vec<u8> {
        let events: Vec<TrackEvent> = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
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
                    message: MidiMessage::NoteOn { key: u7::new(60), vel: u7::new(0) },
                },
            },
            TrackEvent {
                delta: u28::new(240),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn { key: u7::new(62), vel: u7::new(100) },
                },
            },
            TrackEvent {
                delta: u28::new(240),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOff { key: u7::new(62), vel: u7::new(64) },
                },
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(u15::new(480))),
            tracks: vec![events],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).expect("write fixture smf");
        buf
    }

    #[test]
    fn parses_two_notes_with_tempo_derived_start_us() {
        let bytes = build_fixture();
        let parsed = parse_smf(&bytes).expect("parse fixture");

        assert_eq!(parsed.ppq, 480);
        assert_eq!(parsed.tracks.len(), 1);
        assert_eq!(parsed.tracks[0].name.as_deref(), Some("Guitar"));
        assert!(!parsed.tracks[0].channel10);
        assert_eq!(parsed.tracks[0].notes.len(), 2);

        assert_eq!(parsed.tracks[0].notes[0].start_us, 0);
        assert_eq!(parsed.tracks[0].notes[0].len_us, 250_000);
        assert_eq!(parsed.tracks[0].notes[0].pitch, 60);
        assert_eq!(parsed.tracks[0].notes[0].velocity, 100);

        assert_eq!(parsed.tracks[0].notes[1].start_us, 500_000);
        assert_eq!(parsed.tracks[0].notes[1].len_us, 250_000);
        assert_eq!(parsed.tracks[0].notes[1].pitch, 62);
        assert_eq!(parsed.tracks[0].notes[1].velocity, 100);

        assert_eq!(parsed.tempo_map.len(), 1);
        assert_eq!(parsed.tempo_map[0].song_pos_us, 0);
        assert_eq!(parsed.tempo_map[0].micros_per_beat, 500_000);
        assert_eq!(parsed.tempo_map[0].numerator, 4);
        assert_eq!(parsed.tempo_map[0].denominator, 4);
    }

    #[test]
    fn timecode_timing_is_unsupported() {
        let smf = Smf {
            header: Header::new(
                Format::SingleTrack,
                Timing::Timecode(midly::Fps::Fps25, 40),
            ),
            tracks: vec![vec![TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            }]],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).expect("write fixture smf");

        let err = parse_smf(&buf).expect_err("smpte timing should be rejected");
        assert_eq!(err, ImportError::Unsupported("SMPTE timing"));
    }

    #[test]
    fn detects_gm_channel_10_drums() {
        let events: Vec<TrackEvent> = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(9),
                    message: MidiMessage::NoteOn { key: u7::new(36), vel: u7::new(100) },
                },
            },
            TrackEvent {
                delta: u28::new(10),
                kind: TrackEventKind::Midi {
                    channel: u4::new(9),
                    message: MidiMessage::NoteOff { key: u7::new(36), vel: u7::new(0) },
                },
            },
        ];
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(u15::new(480))),
            tracks: vec![events],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).expect("write fixture smf");

        let parsed = parse_smf(&buf).expect("parse fixture");
        assert!(parsed.tracks[0].channel10);
    }
}
