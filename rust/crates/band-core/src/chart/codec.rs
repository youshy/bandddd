//! Deterministic canonical binary codec. Rules: version byte first, fixed field
//! order, length-prefixed vectors (uvarint count), unsigned LEB128 varints,
//! zig-zag varints for signed ints, UTF-8 strings as (len, bytes). Panic-free.

#[derive(Debug, PartialEq, Eq)]
pub enum CodecError {
    UnexpectedEof,
    BadVarint,
    BadUtf8,
    BadVersion(u8),
    BadEnum(&'static str),
    TrailingBytes,
    OutOfRange(&'static str),
}

#[derive(Default)]
pub struct Writer { buf: Vec<u8> }

impl Writer {
    pub fn new() -> Self { Self::default() }
    pub fn into_bytes(self) -> Vec<u8> { self.buf }
    pub fn u8(&mut self, v: u8) { self.buf.push(v); }
    pub fn uvarint(&mut self, mut v: u64) {
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 { byte |= 0x80; }
            self.buf.push(byte);
            if v == 0 { break; }
        }
    }
    pub fn ivarint(&mut self, v: i64) {
        self.uvarint(((v << 1) ^ (v >> 63)) as u64); // zig-zag
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.uvarint(b.len() as u64);
        self.buf.extend_from_slice(b);
    }
    pub fn string(&mut self, s: &str) { self.bytes(s.as_bytes()); }
}

pub struct Reader<'a> { buf: &'a [u8], pos: usize }

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self { Self { buf, pos: 0 } }
    pub fn u8(&mut self) -> Result<u8, CodecError> {
        let b = *self.buf.get(self.pos).ok_or(CodecError::UnexpectedEof)?;
        self.pos += 1;
        Ok(b)
    }
    pub fn uvarint(&mut self) -> Result<u64, CodecError> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        loop {
            if shift >= 64 { return Err(CodecError::BadVarint); }
            let byte = self.u8()?;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 { break; }
            shift += 7;
        }
        Ok(result)
    }
    pub fn ivarint(&mut self) -> Result<i64, CodecError> {
        let zz = self.uvarint()?;
        Ok(((zz >> 1) as i64) ^ -((zz & 1) as i64))
    }
    pub fn bytes(&mut self) -> Result<&'a [u8], CodecError> {
        let len = self.uvarint()? as usize;
        let end = self.pos.checked_add(len).ok_or(CodecError::UnexpectedEof)?;
        let slice = self.buf.get(self.pos..end).ok_or(CodecError::UnexpectedEof)?;
        self.pos = end;
        Ok(slice)
    }
    pub fn string(&mut self) -> Result<String, CodecError> {
        let b = self.bytes()?;
        core::str::from_utf8(b).map(|s| s.to_owned()).map_err(|_| CodecError::BadUtf8)
    }
    /// Assert no trailing bytes remain (canonical form is exact-length).
    pub fn finish(self) -> Result<(), CodecError> {
        if self.pos == self.buf.len() { Ok(()) } else { Err(CodecError::TrailingBytes) }
    }
}

#[cfg(test)]
mod prim_tests {
    use super::*;
    #[test]
    fn varints_round_trip() {
        let mut w = Writer::new();
        w.uvarint(0); w.uvarint(300); w.uvarint(u64::MAX);
        w.ivarint(0); w.ivarint(-1); w.ivarint(1_500_000); w.ivarint(i64::MIN);
        w.string("héllo"); w.u8(7);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.uvarint().unwrap(), 0);
        assert_eq!(r.uvarint().unwrap(), 300);
        assert_eq!(r.uvarint().unwrap(), u64::MAX);
        assert_eq!(r.ivarint().unwrap(), 0);
        assert_eq!(r.ivarint().unwrap(), -1);
        assert_eq!(r.ivarint().unwrap(), 1_500_000);
        assert_eq!(r.ivarint().unwrap(), i64::MIN);
        assert_eq!(r.string().unwrap(), "héllo");
        assert_eq!(r.u8().unwrap(), 7);
        r.finish().unwrap();
    }
    #[test]
    fn truncated_input_errors_not_panics() {
        let mut r = Reader::new(&[0x80]); // varint continuation with no next byte
        assert_eq!(r.uvarint(), Err(CodecError::UnexpectedEof));
    }
    #[test]
    fn oversized_length_prefix_errors() {
        // claims 100-byte string, buffer has 0 → must error, never panic/allocate-crash
        let mut r = Reader::new(&[100]);
        assert_eq!(r.bytes(), Err(CodecError::UnexpectedEof));
    }
}

use super::{
    Chart, ChartCore, DifficultyTier, Envelope, GrooveSection, InstrumentSetRef,
    InstrumentTrack, LyricEvent, NoteEvent, SoundRef, SpaceAction, TempoEvent,
    FORMAT_VERSION, NUM_ROLES,
};

fn enc_note(w: &mut Writer, n: &NoteEvent) {
    w.ivarint(n.song_pos_us);
    w.u8(n.lanes);
    w.u8(n.space as u8);
    w.ivarint(n.sustain_len_us);
    w.uvarint(n.sound.set_index as u64);
    w.u8(n.sound.pitch);
    w.u8(n.sound.velocity);
}
fn dec_note(r: &mut Reader) -> Result<NoteEvent, CodecError> {
    let song_pos_us = r.ivarint()?;
    let lanes = r.u8()?;
    let space = SpaceAction::from_u8(r.u8()?).ok_or(CodecError::BadEnum("SpaceAction"))?;
    let sustain_len_us = r.ivarint()?;
    let set_index = u16::try_from(r.uvarint()?).map_err(|_| CodecError::OutOfRange("set_index"))?;
    let pitch = r.u8()?;
    let velocity = r.u8()?;
    Ok(NoteEvent { song_pos_us, lanes, space, sustain_len_us,
        sound: SoundRef { set_index, pitch, velocity } })
}

/// Canonical encoding of the hashed core. Field order here is THE contract.
pub fn encode_core(c: &ChartCore) -> Vec<u8> {
    let mut w = Writer::new();
    w.ivarint(c.duration_us);
    w.uvarint(c.tempo_map.len() as u64);
    for t in &c.tempo_map {
        w.ivarint(t.song_pos_us);
        w.uvarint(t.micros_per_beat as u64);
        w.u8(t.numerator);
        w.u8(t.denominator);
    }
    w.uvarint(c.instrument_set_refs.len() as u64);
    for s in &c.instrument_set_refs { w.string(&s.id); }
    // Exactly NUM_ROLES tracks, fixed order.
    for track in &c.tracks {
        w.uvarint(track.tiers.len() as u64);
        for tier in &track.tiers {
            w.uvarint(tier.notes.len() as u64);
            for n in &tier.notes { enc_note(&mut w, n); }
        }
    }
    w.uvarint(c.lyrics.len() as u64);
    for l in &c.lyrics {
        w.ivarint(l.song_pos_us);
        w.string(&l.text);
        w.u8(l.line_break as u8);
    }
    w.uvarint(c.groove_sections.len() as u64);
    for g in &c.groove_sections {
        w.ivarint(g.start_us);
        w.u8(g.feel_dir as u8);
    }
    w.into_bytes()
}

pub fn decode_core(buf: &[u8]) -> Result<ChartCore, CodecError> {
    let mut r = Reader::new(buf);
    let core = decode_core_from(&mut r)?;
    r.finish()?;
    Ok(core)
}

fn decode_core_from(r: &mut Reader) -> Result<ChartCore, CodecError> {
    let duration_us = r.ivarint()?;
    let tempo_len = r.uvarint()? as usize;
    let mut tempo_map = Vec::with_capacity(tempo_len.min(4096));
    for _ in 0..tempo_len {
        tempo_map.push(TempoEvent {
            song_pos_us: r.ivarint()?,
            micros_per_beat: u32::try_from(r.uvarint()?).map_err(|_| CodecError::OutOfRange("mpb"))?,
            numerator: r.u8()?,
            denominator: r.u8()?,
        });
    }
    let set_len = r.uvarint()? as usize;
    let mut instrument_set_refs = Vec::with_capacity(set_len.min(4096));
    for _ in 0..set_len { instrument_set_refs.push(InstrumentSetRef { id: r.string()? }); }
    let mut tracks: [InstrumentTrack; NUM_ROLES] = Default::default();
    for track in tracks.iter_mut() {
        let ntiers = r.uvarint()? as usize;
        let mut tiers = Vec::with_capacity(ntiers.min(64));
        for _ in 0..ntiers {
            let nnotes = r.uvarint()? as usize;
            let mut notes = Vec::with_capacity(nnotes.min(1 << 20));
            for _ in 0..nnotes { notes.push(dec_note(r)?); }
            tiers.push(DifficultyTier { notes });
        }
        track.tiers = tiers;
    }
    let nlyr = r.uvarint()? as usize;
    let mut lyrics = Vec::with_capacity(nlyr.min(1 << 20));
    for _ in 0..nlyr {
        lyrics.push(LyricEvent {
            song_pos_us: r.ivarint()?,
            text: r.string()?,
            line_break: r.u8()? != 0,
        });
    }
    let ngs = r.uvarint()? as usize;
    let mut groove_sections = Vec::with_capacity(ngs.min(4096));
    for _ in 0..ngs {
        groove_sections.push(GrooveSection { start_us: r.ivarint()?, feel_dir: r.u8()? as i8 });
    }
    Ok(ChartCore { duration_us, tempo_map, instrument_set_refs, tracks, lyrics, groove_sections })
}

pub fn encode_chart(chart: &Chart) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(FORMAT_VERSION);
    let core_bytes = encode_core(&chart.core);
    w.bytes(&core_bytes); // length-prefixed core section
    // Envelope section (never hashed).
    let e = &chart.envelope;
    w.string(&e.title); w.string(&e.artist); w.string(&e.author); w.string(&e.version);
    w.uvarint(e.labels.len() as u64);
    for l in &e.labels { w.string(l); }
    w.uvarint(e.tier_names.len() as u64);
    for n in &e.tier_names { w.string(n); }
    w.into_bytes()
}

pub fn decode_chart(buf: &[u8]) -> Result<Chart, CodecError> {
    let mut r = Reader::new(buf);
    let ver = r.u8()?;
    if ver != FORMAT_VERSION { return Err(CodecError::BadVersion(ver)); }
    let core_bytes = r.bytes()?;
    let core = decode_core(core_bytes)?;
    let title = r.string()?; let artist = r.string()?;
    let author = r.string()?; let version = r.string()?;
    let nlabels = r.uvarint()? as usize;
    let mut labels = Vec::with_capacity(nlabels.min(4096));
    for _ in 0..nlabels { labels.push(r.string()?); }
    let ntn = r.uvarint()? as usize;
    let mut tier_names = Vec::with_capacity(ntn.min(64));
    for _ in 0..ntn { tier_names.push(r.string()?); }
    r.finish()?;
    Ok(Chart { core, envelope: Envelope { title, artist, author, version, labels, tier_names } })
}

#[cfg(test)]
mod core_tests {
    use super::*;
    use crate::chart::tests::sample_core;

    #[test]
    fn core_round_trips() {
        let core = sample_core();
        let bytes = encode_core(&core);
        let back = decode_core(&bytes).unwrap();
        assert_eq!(core, back);
    }

    #[test]
    fn chart_round_trips_with_envelope() {
        let chart = Chart {
            core: sample_core(),
            envelope: Envelope {
                title: "Test".into(), artist: "Me".into(), author: "Me".into(),
                version: "v1".into(), labels: vec!["rock".into()],
                tier_names: vec!["Party".into()],
            },
        };
        let bytes = encode_chart(&chart);
        assert_eq!(decode_chart(&bytes).unwrap(), chart);
    }

    #[test]
    fn wrong_version_rejected() {
        let mut bytes = encode_chart(&Chart {
            core: sample_core(), envelope: Envelope::default(),
        });
        bytes[0] = 99;
        assert_eq!(decode_chart(&bytes), Err(CodecError::BadVersion(99)));
    }
}
