//! Content-addressing: BLAKE3 over the canonical core bytes ONLY (envelope excluded).

use super::{codec::encode_core, ChartCore, FORMAT_VERSION};

pub fn content_address_bytes(core: &ChartCore) -> [u8; 32] {
    // The format version byte namespaces the address, so structurally-identical
    // cores under different FORMAT_VERSIONs never collide (pinned: BLAKE3 versioned by format byte).
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[FORMAT_VERSION]);
    hasher.update(&encode_core(core));
    *hasher.finalize().as_bytes()
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
        // Two charts, identical core, different envelope → SAME address (envelope never hashed).
        let core = sample_core();
        let c1 = Chart { core: core.clone(), envelope: Envelope { title: "A".into(), ..Default::default() } };
        let c2 = Chart { core: core.clone(), envelope: Envelope { title: "B".into(), ..Default::default() } };
        assert_eq!(content_address(&c1.core), content_address(&c2.core));
        assert_eq!(content_address(&c1.core), content_address(&core));
    }

    #[test]
    fn note_change_changes_address() {
        let mut core = sample_core();
        let before = content_address(&core);
        core.tracks[0].tiers[0].notes[0].song_pos_us += 1; // 1µs feel change
        assert_ne!(before, content_address(&core));
    }
}
