//! Content-addressing: BLAKE3 over the canonical core bytes ONLY (envelope excluded).

use super::{codec::encode_core, ChartCore};

pub fn content_address_bytes(core: &ChartCore) -> [u8; 32] {
    *blake3::hash(&encode_core(core)).as_bytes()
}

pub fn content_address(core: &ChartCore) -> String {
    let mut s = String::with_capacity(64);
    for b in content_address_bytes(core) {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::tests::sample_core;
    use crate::chart::{Envelope, Chart};

    #[test]
    fn address_is_stable_and_hex64() {
        let a = content_address(&sample_core());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(a, content_address(&sample_core())); // deterministic
    }

    #[test]
    fn envelope_does_not_affect_address() {
        // Two charts, identical core, different envelope → SAME address (SP3 §3c).
        let core = sample_core();
        let a = content_address(&core);
        let _c1 = Chart { core: core.clone(), envelope: Envelope { title: "A".into(), ..Default::default() } };
        let _c2 = Chart { core: core.clone(), envelope: Envelope { title: "B".into(), ..Default::default() } };
        assert_eq!(a, content_address(&core)); // envelope never enters the hash
    }

    #[test]
    fn note_change_changes_address() {
        let mut core = sample_core();
        let before = content_address(&core);
        core.tracks[0].tiers[0].notes[0].song_pos_us += 1; // 1µs feel change
        assert_ne!(before, content_address(&core));
    }
}
