//! band-core: pure, headless, cross-target (native + wasm32) game core.
#![forbid(unsafe_code)]

pub mod clock;

/// Human-readable crate version (distinct from the on-wire chart format version byte).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_non_empty() {
        assert!(!super::VERSION.is_empty());
    }
}
