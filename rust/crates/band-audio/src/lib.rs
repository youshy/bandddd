//! Native audio engine: cpal output + rustysynth sampler + sample-accurate
//! scheduling. Not compiled to wasm (hardware/thread deps).
pub mod clock;
