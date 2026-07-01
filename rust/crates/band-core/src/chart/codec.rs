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
