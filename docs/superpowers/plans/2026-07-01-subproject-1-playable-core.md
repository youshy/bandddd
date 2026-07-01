# Sub-project 1 — The Playable Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One player, one instrument, one device — a fully playable, *fun* rhythm game slice that proves low-latency sample-accurate audio (incl. iOS), a deterministic content-addressed chart format, and that **groove** can be scored so a tight pocket demonstrably beats both robotic-perfect and sloppy play.

**Architecture:** A Rust workspace owns the latency- and correctness-critical core (chart format + hashing, MIDI import, song-clock, audio engine, hit detection, groove scoring) as pure headless crates plus a `gdext` GDExtension bridge; Godot 4 owns rendering, UI, input, haptics, and the export pipeline. The Rust core is validated headless (unit/integration tests + CLI harnesses); the game's *fun* is validated through a minimal programmer-art playable UI whose centerpiece is the **pocket meter**. Everything times against **song-position** derived from the audio device's sample-accurate playhead.

**Tech Stack:** Rust (stable) + `gdext` (godot-rust) · Godot 4.x · `cpal` (audio output) · `rustysynth` (SF2 sampler) · `midly` (MIDI parse) · `blake3` (content hash) · `rtrb`/`crossbeam` (lock-free audio queue) · `wasm-bindgen-test` (cross-target hash determinism). iOS-first → Mac/Win/Linux → Android.

## Global Constraints

Every task's requirements implicitly include this section. Values are copied verbatim from the specs; do not re-litigate.

- **Engine/stack:** Godot 4 + Rust (GDExtension / `gdext`). Godot owns rendering/UI/input/export; Rust owns audio scheduling, MIDI parsing, chart serialization/hashing, groove scoring.
- **Platform ship order:** iOS first, then Mac/Windows/Linux, then Android (explicit 2nd-class). CI builds all supported targets "at all times."
- **Canonical serialization = deterministic canonical BINARY** — version byte, fixed field order, length-prefixed sections, varint/fixed-width integers. **NOT JSON.** A JSON debug projection may exist but is **never** hashed.
- **All note/feel-target/tempo times are INTEGER µs-grade (`i64`), never floats.** Required for deterministic hashing and deterministic transforms, and for byte-identical hashes native-vs-WASM.
- **Core/envelope split:** the hashed **canonical playable core** = notes/lanes/space/sustains/feel-targets + tempo-beat map + lyrics + instrument-set refs + baked difficulty tiers. **Only this is hashed → the content-address.** The **unhashed envelope** = title/artist/author/version/labels/tier-names.
- **Chart format must be capable of baked difficulty tiers** (parallel note-sets per instrument). SP1's minimal importer produces exactly **one tier**; the multi-tier generator is SP3 — do not build it here.
- **Hash algorithm = BLAKE3** over the canonical core bytes; content-address = lowercase hex. (This is SP1's pinned choice for SP3's "hashing algorithm" open question; it is versioned by the format version byte.) **IMPLEMENTED (Phase 2):** the content-address is `BLAKE3(FORMAT_VERSION_byte ++ encode_core(core))` — the format version byte is hashed *before* the core bytes so structurally-identical cores under different `FORMAT_VERSION`s never collide. This is byte-identical native and under WASM — **proven** by an executed `wasm-pack test --node` run against a frozen golden vector (Task 2.5), not merely asserted.
- **Unified mechanic:** 4 fixed lanes (zero musical semantics) + space, one reading model for all four instruments (guitar/bass/drums/vocal), single instrument at a time. Sustains scored on **two axes: onset timing + hold length.** Bindings reassignable per input device (keyboard/gamepad/touch/real MIDI). Default touch = 5 large zones.
- **Song-clock:** the audio device's sample-accurate playhead is master; song-position is derived from frames-played, never wall-clock/frame-time. Built as the same abstraction SP2 will anchor to a server clock.
- **Audio:** Rust-owned audio thread (`cpal` + `rustysynth` sampler); sample sets ship with the game; each instrument sounded by *that performer's actual hit events*; unmanned instruments auto-perform at chart-ideal; **miss = audible hole** (mute that part for the note; optional subtle clank — playtest). Stylized melodic vocal voice (vowel-pad "aah" lead) + on-screen lyrics; no singing synthesis.
- **Groove IS note placement:** note times are feel-targets (swing/push/lay-back baked in, not grid-quantized). Individual groove scoring = accuracy vs feel-target + windowed tightness inside a pocket-zone guardrail; emits band-ready per-note + windowed metrics. The UI never computes groove — it renders the core's outputs.
- **Timing windows (TUNABLE, centered on the feel-target, never a metronome tick):** Perfect ±25 ms · Good ±50 ms · Hit ±80 ms · Miss beyond.
- **Pocket-zone guardrail (TUNABLE):** windowed tightness bonus only applies while `|mean offset| ≤ ~60 ms`.
- **Calibration is day-zero infrastructure, not polish:** measure & store per-device audio-output / input / haptic offsets. Haptics fire **relative to audio**, never off the raw input event.
- **Inebriated-User Test:** the *play* surface must be usable by a drunk player at a party even while dev chrome around it is not.
- **Parser is total/panic-free & bounds-checked** (SP3 §9 carried forward): decode returns `Result`, never panics/indexes out of bounds; validation clamps ranges at load.

**TUNABLES (playtest numbers, not blockers):** timing-window widths; pocket-zone guardrail width; windowed-groove math (std-dev vs MAD; window length; bonus scaling); miss feedback (silent vs clank); pocket-meter lean→position & spread→sharpness mapping; ambient warm/cold intensity; audio look-ahead/approach time. Implement each as a named constant/struct field with the proposed default; never hardcode a magic number inline.

---

## File Structure

**Rust workspace (`rust/`):**
- `rust/Cargo.toml` — workspace manifest (members below).
- `rust/crates/band-core/` — **pure, headless, no Godot/audio-hardware deps; compiles to `wasm32` too.** Chart model, canonical binary codec, BLAKE3 hashing, MIDI importer, song-clock trait, input model, hit detection, groove scoring, calibration math, timing/tunable constants. This is the correctness core.
  - `src/lib.rs`, `src/chart/mod.rs` (model), `src/chart/codec.rs` (Writer/Reader + encode/decode), `src/chart/hash.rs`, `src/midi_import.rs`, `src/clock.rs` (trait), `src/input.rs`, `src/judge.rs`, `src/groove.rs`, `src/calibration.rs`, `src/tunables.rs`.
  - `tests/` — integration tests (round-trip, determinism, judge scenarios, groove worked-example).
- `rust/crates/band-audio/` — native-only audio engine (`cpal` + `rustysynth` + lock-free queue); implements the song-clock over the sample counter; per-performer sounding, auto-perform, miss holes.
  - `src/lib.rs`, `src/engine.rs`, `src/clock.rs` (`AudioSongClock`), `src/scheduler.rs`, `src/sampler.rs`.
- `rust/crates/band-cli/` — headless debug harnesses (bins): `inspect`, `import`, `judge-replay`, `hashcheck`.
- `rust/crates/band-gdext/` — the GDExtension `cdylib` Godot loads; wraps core + audio, registers Godot classes.
  - `src/lib.rs`, `src/chart_node.rs`, `src/clock_node.rs`, `src/audio_node.rs`, `src/judge_node.rs`, `src/calibration_node.rs`.

**Godot project (`godot/`):**
- `godot/project.godot`, `godot/bandddd.gdextension` (points at compiled `band-gdext` lib per platform).
- `godot/scenes/` — `Main.tscn`, `ChartPicker.tscn`, `Calibration.tscn`, `PlayHighway.tscn`, `Results.tscn`, `AudienceView.tscn`, `PocketMeter.tscn`.
- `godot/scripts/` — GDScript glue (thin; logic lives in Rust nodes).
- `godot/assets/soundsets/` — bundled SF2 sample sets (guitar/bass/drums + vocal vowel-pad) and `test_charts/*.band` + `*.mid`.

**CI:** `.github/workflows/ci.yml` — builds `band-core` (native + wasm32), runs all Rust tests, builds `band-gdext` per target, exports Godot for desktop + iOS.

---

## Phase Overview & Review Checkpoints

Sequenced to prove the hardest unknowns as early as their dependencies allow. **STOP for review at the end of each phase.**

1. **Phase 0 — Toolchain & workspace skeleton.** gdext loads in Godot on desktop + iOS device; `band-core` compiles to wasm; CI green. *De-risks: the build/export toolchain itself (iOS gdext).*
2. **Phase 1 — Audio latency & song-clock spike.** cpal plays sample-accurately scheduled clicks; `SongClock` exposes sample-accurate song-position; runs & measured on desktop **and iOS**. *De-risks #1: low-latency sample-accurate audio on iOS.*
3. **Phase 2 — Chart format + hashing.** Deterministic canonical binary, core/envelope split, integer-µs, BLAKE3 content-address; round-trip + cross-target native-vs-WASM hash equality. *De-risks #2: deterministic format.*
4. **Phase 3 — Minimal MIDI importer.** `.mid` → single-tier believable test charts, feel-targets carried as integer µs. *Produces real content.*
5. **Phase 4 — Full audio engine.** rustysynth sampler, per-performer sounding, unmanned auto-perform, miss holes, vocal voice — playing real charts.
6. **Phase 5 — Input abstraction + hit detection.** device-agnostic event stream → nearest-eligible-note matching, signed offset, grades, sustains. Headless.
7. **Phase 6 — Groove scoring.** windowed mean-lean/spread, pocket guardrail, band-ready metrics; **headless worked-example proof that pocket > sloppy and pocket ≥ robo.** *De-risks #3 (fairness).*
8. **Phase 7 — Calibration.** tap-to-beat offset math + persistence; haptic-relative-to-audio wiring point.
9. **Phase 8 — Godot minimal playable UI + haptics (§12).** picker, calibration screen, note highway, real-time feedback, results w/ groove breakdown, **pocket meter**, audience view. *De-risks #3 (fun).*
10. **Phase 9 — Integration & Definition-of-Done.** full song end-to-end on a desktop target **and iOS**; DoD (§13) checklist.

---

# Phase 0 — Toolchain & Workspace Skeleton

**Goal:** prove the exact toolchain (Rust workspace + gdext + Godot export to desktop and **iOS device**, + wasm build of `band-core`) before writing hard code. iOS gdext export is itself an unknown; surface it now.

### Task 0.1: Rust workspace + `band-core` crate skeleton

**Files:**
- Create: `rust/Cargo.toml`, `rust/crates/band-core/Cargo.toml`, `rust/crates/band-core/src/lib.rs`, `rust/rust-toolchain.toml`, `rust/.gitignore`

**Interfaces:**
- Produces: a buildable workspace with member `band-core`; `band_core::VERSION: &str`.

- [ ] **Step 1: Write the failing test**

Create `rust/crates/band-core/src/lib.rs`:
```rust
//! band-core: pure, headless, cross-target (native + wasm32) game core.
#![forbid(unsafe_code)]

/// Human-readable crate version (distinct from the on-wire chart format version byte).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_non_empty() {
        assert!(!super::VERSION.is_empty());
    }
}
```

- [ ] **Step 2: Create the manifests**

`rust/Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/band-core"]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "UNLICENSED"
```

`rust/crates/band-core/Cargo.toml`:
```toml
[package]
name = "band-core"
edition.workspace = true
version.workspace = true
license.workspace = true

[lib]
crate-type = ["rlib"]
```

`rust/rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
targets = ["wasm32-unknown-unknown"]
```

`rust/.gitignore`:
```
/target
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cd rust && cargo test -p band-core`
Expected: PASS — `version_is_non_empty ... ok`.

- [ ] **Step 4: Verify wasm target compiles**

Run: `cd rust && cargo build -p band-core --target wasm32-unknown-unknown`
Expected: builds successfully (proves `band-core` stays wasm-clean from day one).

- [ ] **Step 5: Commit**

```bash
git add rust/
git commit -m "chore: scaffold rust workspace + band-core (native + wasm)"
```

### Task 0.2: `band-gdext` extension + Godot project that loads it

**Files:**
- Create: `rust/crates/band-gdext/Cargo.toml`, `rust/crates/band-gdext/src/lib.rs`, `godot/project.godot`, `godot/bandddd.gdextension`, `godot/scenes/Main.tscn`, `godot/scripts/main.gd`
- Modify: `rust/Cargo.toml` (add member)

**Interfaces:**
- Produces: a Godot-loadable `cdylib`; a Godot autoload/scene that calls a Rust method and prints its result — proving the bridge.

- [ ] **Step 1: Add the gdext crate manifest**

`rust/crates/band-gdext/Cargo.toml`:
```toml
[package]
name = "band-gdext"
edition.workspace = true
version.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
godot = "0.2"          # gdext; pin to the version matching your Godot 4.x
band-core = { path = "../band-core" }
```

Add `"crates/band-gdext"` to `members` in `rust/Cargo.toml`.

- [ ] **Step 2: Write the extension entry point + a probe class**

`rust/crates/band-gdext/src/lib.rs`:
```rust
use godot::prelude::*;

struct BandExtension;

#[gdextension]
unsafe impl ExtensionLibrary for BandExtension {}

/// Minimal bridge probe: proves Godot ⇄ Rust ⇄ band-core linkage.
#[derive(GodotClass)]
#[class(base=RefCounted)]
struct BandProbe {
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for BandProbe {
    fn init(base: Base<RefCounted>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl BandProbe {
    #[func]
    fn core_version(&self) -> GString {
        band_core::VERSION.into()
    }
}
```

- [ ] **Step 3: Build the extension**

Run: `cd rust && cargo build -p band-gdext`
Expected: produces `rust/target/debug/libband_gdext.{dylib,so}` (or `.dll`).

- [ ] **Step 4: Create the Godot project + extension descriptor**

`godot/bandddd.gdextension`:
```ini
[configuration]
entry_symbol = "gdext_rust_init"
compatibility_minimum = 4.3

[libraries]
macos.debug     = "res://../rust/target/debug/libband_gdext.dylib"
macos.release   = "res://../rust/target/release/libband_gdext.dylib"
linux.debug     = "res://../rust/target/debug/libband_gdext.so"
windows.debug   = "res://../rust/target/debug/band_gdext.dll"
ios.debug       = "res://../rust/target/aarch64-apple-ios/debug/libband_gdext.dylib"
ios.release     = "res://../rust/target/aarch64-apple-ios/release/libband_gdext.dylib"
```

`godot/project.godot` (minimal):
```ini
config_version=5

[application]
config/name="bandddd"
run/main_scene="res://scenes/Main.tscn"

[rendering]
renderer/rendering_method="gl_compatibility"
```

`godot/scripts/main.gd`:
```gdscript
extends Node

func _ready() -> void:
	var probe := BandProbe.new()
	print("band-core version = ", probe.core_version())
```

`godot/scenes/Main.tscn`: a single `Node` root named `Main` with `main.gd` attached. (Create in the editor, or author the `.tscn` text by hand with an `[ext_resource ... path="res://scripts/main.gd"]` and a `[node name="Main" type="Node"]` referencing it.)

- [ ] **Step 5: Verify the bridge on desktop**

Run: open `godot/` in Godot 4, press Play (or `godot --path godot --headless --quit-after 2`).
Expected: console prints `band-core version = 0.1.0`. If the class isn't found, re-check the `.gdextension` library path resolves to the built lib.

- [ ] **Step 6: Commit**

```bash
git add rust/ godot/
git commit -m "chore: gdext bridge loads in Godot, calls band-core"
```

### Task 0.3: iOS build target for the extension + CI

**Files:**
- Create: `.github/workflows/ci.yml`, `rust/.cargo/config.toml` (if needed for iOS)
- Modify: `godot/bandddd.gdextension` (already has iOS entries)

**Interfaces:**
- Produces: CI that builds `band-core` (native + wasm), runs tests, builds `band-gdext` for macOS + iOS, and an iOS export smoke.

- [ ] **Step 1: Build the extension for iOS locally**

Run:
```bash
cd rust
rustup target add aarch64-apple-ios
cargo build -p band-gdext --target aarch64-apple-ios
```
Expected: `rust/target/aarch64-apple-ios/debug/libband_gdext.dylib` is produced. Resolve any linker flags here (iOS needs the Godot iOS export template + Xcode toolchain).

- [ ] **Step 2: iOS device smoke (manual, MANDATORY early de-risk)**

In Godot: install the iOS export template, add an iOS export preset, export `godot/` to an Xcode project, run on a physical iPhone.
Expected: app launches; Xcode device console shows `band-core version = 0.1.0`.
This proves gdext + Godot export works end-to-end on iOS **before** any hard code depends on it. Record any signing/template gotchas in `docs/superpowers/plans/notes-ios.md`.

- [ ] **Step 3: Write CI**

`.github/workflows/ci.yml`:
```yaml
name: ci
on: [push, pull_request]
jobs:
  rust:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown, aarch64-apple-ios
      - run: cd rust && cargo test --workspace
      - run: cd rust && cargo build -p band-core --target wasm32-unknown-unknown
      - run: cd rust && cargo build -p band-gdext --target aarch64-apple-ios
```
(Godot export jobs are added as export presets stabilize; keep the Rust matrix green from day one. Windows/Linux runners added when those targets are exercised.)

- [ ] **Step 4: Verify CI passes**

Push the branch; confirm the `rust` job is green.

- [ ] **Step 5: Commit**

```bash
git add .github/ rust/ docs/
git commit -m "ci: build core (native+wasm) + gdext (macos+ios), run tests"
```

**REVIEW CHECKPOINT 0** — Toolchain proven: gdext runs in Godot on desktop **and a physical iPhone**; `band-core` builds for wasm; CI green. Do not proceed until the iOS smoke passed.

---

# Phase 1 — Audio Latency & Song-Clock Spike

**Goal (de-risk #1 — the biggest unknown):** get low-latency, **sample-accurate** scheduled audio out of `cpal` and expose a sample-accurate **song-position** clock, on desktop **and iOS**. No chart yet — a hardcoded click schedule. This stands up the `SongClock` spine (contract #2) that everything times against.

### Task 1.1: `SongClock` trait in `band-core`

**Files:**
- Create: `rust/crates/band-core/src/clock.rs`
- Modify: `rust/crates/band-core/src/lib.rs` (add `pub mod clock;`)
- Test: inline `#[cfg(test)]` in `clock.rs`

**Interfaces:**
- Produces: `type SongPosUs = i64;`, `trait SongClock { fn song_pos_us(&self) -> SongPosUs; fn is_playing(&self) -> bool; }`, and `struct ManualClock` (test double). SP2 will implement `SongClock` over a server-synced clock; nothing above the trait changes.

- [ ] **Step 1: Write the failing test**

In `rust/crates/band-core/src/clock.rs`:
```rust
//! Song-clock abstraction. Song-position (integer µs) is the master timebase.
//! The audio device's sample-accurate playhead is the real implementation
//! (see band-audio); SP2 swaps in a server-synced clock behind this same trait.

/// Integer microsecond song-position. Never a float — see Global Constraints.
pub type SongPosUs = i64;

pub trait SongClock: Send + Sync {
    /// Current song-position in microseconds (may be negative during count-in).
    fn song_pos_us(&self) -> SongPosUs;
    fn is_playing(&self) -> bool;
}

/// Deterministic test double: song-position is whatever you set.
#[derive(Debug, Default)]
pub struct ManualClock {
    pos_us: SongPosUs,
    playing: bool,
}

impl ManualClock {
    pub fn new() -> Self { Self::default() }
    pub fn set(&mut self, pos_us: SongPosUs) { self.pos_us = pos_us; }
    pub fn play(&mut self) { self.playing = true; }
    pub fn stop(&mut self) { self.playing = false; }
}

impl SongClock for ManualClock {
    fn song_pos_us(&self) -> SongPosUs { self.pos_us }
    fn is_playing(&self) -> bool { self.playing }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_clock_reports_set_position() {
        let mut c = ManualClock::new();
        assert!(!c.is_playing());
        c.play();
        c.set(1_500_000);
        assert_eq!(c.song_pos_us(), 1_500_000);
        assert!(c.is_playing());
    }
}
```

- [ ] **Step 2: Wire the module**

Add to `rust/crates/band-core/src/lib.rs`: `pub mod clock;`

- [ ] **Step 3: Run the test**

Run: `cd rust && cargo test -p band-core clock`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/band-core
git commit -m "feat(core): SongClock trait + ManualClock test double"
```

### Task 1.2: `band-audio` crate + `AudioSongClock` over a sample counter

**Files:**
- Create: `rust/crates/band-audio/Cargo.toml`, `rust/crates/band-audio/src/lib.rs`, `rust/crates/band-audio/src/clock.rs`
- Modify: `rust/Cargo.toml` (add member)

**Interfaces:**
- Consumes: `band_core::clock::{SongClock, SongPosUs}`.
- Produces: `AudioSongClock` (holds an `Arc<AtomicU64>` frame counter + `sample_rate`); `song_pos_us = frames * 1_000_000 / sample_rate`. The audio callback advances the counter; the game thread reads position lock-free.

- [ ] **Step 1: Add the crate manifest**

`rust/crates/band-audio/Cargo.toml`:
```toml
[package]
name = "band-audio"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
band-core = { path = "../band-core" }
cpal = "0.15"
rustysynth = "1"
rtrb = "0.3"
```
Add `"crates/band-audio"` to workspace members.

- [ ] **Step 2: Write the failing test**

`rust/crates/band-audio/src/clock.rs`:
```rust
use band_core::clock::{SongClock, SongPosUs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A song-clock driven by the audio device's frame counter. The audio callback
/// calls `advance(frames)` each buffer; `song_pos_us` is derived purely from
/// frames played (sample-accurate), never wall-clock.
#[derive(Clone)]
pub struct AudioSongClock {
    frames: Arc<AtomicU64>,
    sample_rate: u32,
    playing: Arc<std::sync::atomic::AtomicBool>,
}

impl AudioSongClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            frames: Arc::new(AtomicU64::new(0)),
            sample_rate,
            playing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    /// Called from the audio callback (real-time thread) — lock-free.
    pub fn advance(&self, frames: u64) {
        self.frames.fetch_add(frames, Ordering::Relaxed);
    }
    pub fn set_playing(&self, p: bool) {
        self.playing.store(p, Ordering::Relaxed);
    }
    fn frames_played(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }
}

impl SongClock for AudioSongClock {
    fn song_pos_us(&self) -> SongPosUs {
        // Integer math only. frames * 1_000_000 / sample_rate.
        let f = self.frames_played() as u128;
        ((f * 1_000_000u128) / self.sample_rate as u128) as SongPosUs
    }
    fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn song_pos_derives_from_frames() {
        let c = AudioSongClock::new(48_000);
        assert_eq!(c.song_pos_us(), 0);
        c.advance(48_000); // exactly one second
        assert_eq!(c.song_pos_us(), 1_000_000);
        c.advance(24_000); // + half second
        assert_eq!(c.song_pos_us(), 1_500_000);
    }
}
```

`rust/crates/band-audio/src/lib.rs`:
```rust
//! Native audio engine: cpal output + rustysynth sampler + sample-accurate
//! scheduling. Not compiled to wasm (hardware/thread deps).
pub mod clock;
```

- [ ] **Step 3: Run the test**

Run: `cd rust && cargo test -p band-audio clock`
Expected: PASS — `song_pos_derives_from_frames ... ok`.

- [ ] **Step 4: Commit**

```bash
git add rust/
git commit -m "feat(audio): AudioSongClock derives song-pos from frame counter"
```

### Task 1.3: cpal click-scheduler spike (desktop)

**Files:**
- Create: `rust/crates/band-audio/src/engine.rs`, `rust/crates/band-cli/Cargo.toml`, `rust/crates/band-cli/src/bin/clickspike.rs`
- Modify: `rust/crates/band-audio/src/lib.rs`, `rust/Cargo.toml`

**Interfaces:**
- Produces: `AudioEngine::start(schedule: Vec<SongPosUs>) -> AudioEngine` that opens a cpal output stream, advances the `AudioSongClock` by the callback frame count, and emits a short click **sample-accurately** at each scheduled song-position; `AudioEngine::clock() -> AudioSongClock`.

- [ ] **Step 1: Write the engine spike**

`rust/crates/band-audio/src/engine.rs`:
```rust
use crate::clock::AudioSongClock;
use band_core::clock::SongPosUs;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioEngine {
    _stream: cpal::Stream,
    clock: AudioSongClock,
}

impl AudioEngine {
    /// Opens the default output device and schedules a click at each song-pos.
    /// The click is generated at the exact sample offset within the callback
    /// buffer that corresponds to the scheduled song-position.
    pub fn start(mut schedule: Vec<SongPosUs>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let clock = AudioSongClock::new(sample_rate);
        clock.set_playing(true);

        schedule.sort_unstable();
        let clock_cb = clock.clone();
        // Click state: remaining samples of the current click envelope, plus a
        // cursor into the (song-pos) schedule converted to absolute frame index.
        let mut sched_frames: Vec<u64> = schedule
            .iter()
            .map(|us| ((*us as i128 * sample_rate as i128) / 1_000_000i128) as u64)
            .collect();
        sched_frames.sort_unstable();
        let mut next_idx = 0usize;
        let mut frames_emitted: u64 = 0;
        let mut click_rem: u32 = 0;
        let click_len = (sample_rate / 100).max(1); // ~10ms click

        let err_fn = |e| eprintln!("audio stream error: {e}");
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _| {
                    let frames = out.len() / channels;
                    for frame_i in 0..frames {
                        let abs = frames_emitted + frame_i as u64;
                        // Fire clicks whose frame index we've reached.
                        while next_idx < sched_frames.len() && sched_frames[next_idx] <= abs {
                            click_rem = click_len;
                            next_idx += 1;
                        }
                        let sample = if click_rem > 0 {
                            click_rem -= 1;
                            0.25 // simple audible click
                        } else {
                            0.0
                        };
                        for ch in 0..channels {
                            out[frame_i * channels + ch] = sample;
                        }
                    }
                    frames_emitted += frames as u64;
                    clock_cb.advance(frames as u64);
                },
                err_fn,
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream, clock })
    }

    pub fn clock(&self) -> AudioSongClock { self.clock.clone() }
}
```
Add `pub mod engine;` to `band-audio/src/lib.rs`.

- [ ] **Step 2: Write the CLI harness**

`rust/crates/band-cli/Cargo.toml`:
```toml
[package]
name = "band-cli"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
band-core = { path = "../band-core" }
band-audio = { path = "../band-audio" }
```
Add `"crates/band-cli"` to workspace members.

`rust/crates/band-cli/src/bin/clickspike.rs`:
```rust
use band_audio::engine::AudioEngine;
use band_core::clock::SongClock;

fn main() {
    // Clicks every 500ms for 4 seconds.
    let schedule = (0..8).map(|i| i * 500_000).collect();
    let engine = AudioEngine::start(schedule).expect("audio start");
    let clock = engine.clock();
    // Print song-position for 4.5s so we can eyeball clock↔click alignment.
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < 4500 {
        println!("song_pos_us = {}", clock.song_pos_us());
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
```

- [ ] **Step 3: Run the spike on desktop**

Run: `cd rust && cargo run -p band-cli --bin clickspike`
Expected: you **hear** a click every 0.5s; printed `song_pos_us` advances ~monotonically and lands near multiples of 500000 as clicks sound. This proves sample-accurate scheduling + clock derivation.

- [ ] **Step 4: Commit**

```bash
git add rust/
git commit -m "feat(audio): cpal sample-accurate click scheduler spike + CLI"
```

### Task 1.4: iOS audio latency measurement

**Files:**
- Create: `docs/superpowers/plans/notes-ios-audio.md`
- Modify: (a temporary Godot debug scene wiring `AudioEngine` via a stub gdext method, or run the spike through the iOS Godot host)

**Interfaces:**
- Produces: a recorded **round-trip latency figure** on a physical iPhone and confirmation that scheduled clicks stay tight (no under-run/jitter) — the go/no-go for the audio approach.

- [ ] **Step 1: Expose a temporary gdext hook**

Add a temporary `#[func] fn start_click_spike(&self)` on a gdext node (in `band-gdext`) that calls `band_audio::engine::AudioEngine::start(...)` and stores it, so the click spike runs inside the iOS app host. (cpal must open an output stream on iOS via the Godot app process; if cpal cannot, fall back plan: route audio through Godot's `AudioStreamGenerator` fed by the Rust sampler — record which path works.)

- [ ] **Step 2: Measure round-trip latency on device (manual)**

On a physical iPhone: play the click spike, record the phone's speaker with a second device (or use a loopback cable + audio interface), and measure the delay between the intended schedule and audible output. Also tap-test: does the click feel aligned to an on-screen flash driven by `song_pos_us`?
Expected: a concrete latency number (e.g. "~40–90ms output latency"); no audible dropouts over 30s.

- [ ] **Step 3: Record findings + decision**

In `docs/superpowers/plans/notes-ios-audio.md`: record the measured latency, whether cpal opened a stream directly on iOS or we fell back to `AudioStreamGenerator`, buffer size used, and any Core Audio session config needed. This number feeds calibration defaults (Phase 7) and is a **tunable**, not a blocker — but a *failure to get sample-accurate scheduling at all* IS a blocker; escalate at the review checkpoint if so.

- [ ] **Step 4: Remove the temporary hook, commit notes**

```bash
git add docs/superpowers/plans/notes-ios-audio.md
git commit -m "docs: iOS audio latency measurement + approach decision"
```

**REVIEW CHECKPOINT 1** — Sample-accurate scheduled audio + a sample-accurate `SongClock` work on desktop and **iOS**, with a recorded latency figure and no dropouts. If iOS cannot deliver sample-accurate scheduling by any path, STOP and re-plan the audio approach before building on it.

---

# Phase 2 — Chart Format + Hashing

**Goal (de-risk #2):** the deterministic canonical **binary** chart format with the core/envelope split, integer-µs times, a **total/panic-free** decoder, and a BLAKE3 content-address that is **byte-identical native-vs-WASM**. This is contract #1 — everything reads it.

### Task 2.1: Chart data model

**Files:**
- Create: `rust/crates/band-core/src/chart/mod.rs`
- Modify: `rust/crates/band-core/src/lib.rs` (`pub mod chart;`)

**Interfaces:**
- Produces the model types below. Field order here is the canonical encode order (Task 2.2). `InstrumentRole` order is fixed: `Guitar=0, Bass=1, Drums=2, Vocal=3`.

- [ ] **Step 1: Write the model + a construction test**

`rust/crates/band-core/src/chart/mod.rs`:
```rust
//! Chart model. The `ChartCore` is hashed → content-address; the `Envelope`
//! is metadata that travels alongside and is NEVER hashed (SP3 §3c).

use crate::clock::SongPosUs;

pub mod codec;
pub mod hash;

/// On-wire format version. Bump on any breaking encode change; it namespaces
/// the hash so different format versions never collide.
pub const FORMAT_VERSION: u8 = 1;

pub const NUM_ROLES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentRole { Guitar = 0, Bass = 1, Drums = 2, Vocal = 3 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceAction { None = 0, Strum = 1, Kick = 2, Trigger = 3 }

impl SpaceAction {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None), 1 => Some(Self::Strum),
            2 => Some(Self::Kick), 3 => Some(Self::Trigger),
            _ => None,
        }
    }
}

/// Sample/patch + pitch + velocity to sound when this note is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundRef {
    pub set_index: u16, // index into ChartCore.instrument_set_refs
    pub pitch: u8,      // MIDI note number
    pub velocity: u8,   // 1..=127
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEvent {
    pub song_pos_us: SongPosUs, // feel-target (groove baked in)
    pub lanes: u8,              // bitmask, bits 0..=3
    pub space: SpaceAction,
    pub sustain_len_us: SongPosUs, // 0 = hit; >0 = ring/hold
    pub sound: SoundRef,
}

/// One difficulty tier's note set for one instrument. SP1 importer emits one;
/// the format supports many (SP3 bakes tiers into the hashed core).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DifficultyTier {
    pub notes: Vec<NoteEvent>, // ordered by song_pos_us ascending
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstrumentTrack {
    pub tiers: Vec<DifficultyTier>, // >=1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoEvent {
    pub song_pos_us: SongPosUs,
    pub micros_per_beat: u32,
    pub numerator: u8,   // time signature, for bar lines
    pub denominator: u8, // power-of-two note value
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricEvent {
    pub song_pos_us: SongPosUs,
    pub text: String,   // syllable/line
    pub line_break: bool,
}

/// Optional per-section intended feel direction (enables pocket-direction bonus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrooveSection {
    pub start_us: SongPosUs,
    pub feel_dir: i8, // -1 ahead / 0 straight / +1 behind (intended lean)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentSetRef {
    pub id: String, // sample-set id shipped with the game
}

/// The hashed playable core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartCore {
    pub duration_us: SongPosUs,
    pub tempo_map: Vec<TempoEvent>,
    pub instrument_set_refs: Vec<InstrumentSetRef>,
    pub tracks: [InstrumentTrack; NUM_ROLES], // Guitar,Bass,Drums,Vocal
    pub lyrics: Vec<LyricEvent>,
    pub groove_sections: Vec<GrooveSection>, // may be empty
}

/// Unhashed metadata (SP3 §3c). Renaming these never changes the content-address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Envelope {
    pub title: String,
    pub artist: String,
    pub author: String,
    pub version: String,
    pub labels: Vec<String>,
    pub tier_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chart {
    pub core: ChartCore,
    pub envelope: Envelope,
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn sample_core() -> ChartCore {
        let note = NoteEvent {
            song_pos_us: 500_000, lanes: 0b0001, space: SpaceAction::Strum,
            sustain_len_us: 0,
            sound: SoundRef { set_index: 0, pitch: 60, velocity: 100 },
        };
        ChartCore {
            duration_us: 4_000_000,
            tempo_map: vec![TempoEvent {
                song_pos_us: 0, micros_per_beat: 500_000, numerator: 4, denominator: 4,
            }],
            instrument_set_refs: vec![InstrumentSetRef { id: "guitar_clean_v1".into() }],
            tracks: [
                InstrumentTrack { tiers: vec![DifficultyTier { notes: vec![note] }] },
                InstrumentTrack::default(),
                InstrumentTrack::default(),
                InstrumentTrack::default(),
            ],
            lyrics: vec![],
            groove_sections: vec![],
        }
    }

    #[test]
    fn core_constructs() {
        let c = sample_core();
        assert_eq!(c.tracks[0].tiers[0].notes[0].song_pos_us, 500_000);
    }
}
```

- [ ] **Step 2: Wire the module**

Add `pub mod chart;` to `lib.rs`.

- [ ] **Step 3: Run the test**

Run: `cd rust && cargo test -p band-core chart::tests::core_constructs`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/band-core
git commit -m "feat(core): chart data model (hashed core + unhashed envelope)"
```

### Task 2.2: Binary codec — Writer/Reader primitives

**Files:**
- Create: `rust/crates/band-core/src/chart/codec.rs`

**Interfaces:**
- Produces: `Writer` (`uvarint`, `ivarint`, `u8`, `bytes`, `string`, `into_bytes`) and `Reader<'a>` (matching decoders, all returning `Result<_, CodecError>`, never panicking), and `enum CodecError`.

- [ ] **Step 1: Write the failing round-trip test for primitives**

Top of `rust/crates/band-core/src/chart/codec.rs`:
```rust
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
```

- [ ] **Step 2: Run to verify it passes**

Run: `cd rust && cargo test -p band-core codec::prim_tests`
Expected: PASS (3 tests). The last two prove the decoder is total/panic-free on hostile input.

- [ ] **Step 3: Commit**

```bash
git add rust/crates/band-core/src/chart/codec.rs
git commit -m "feat(core): panic-free binary codec primitives (varints, strings)"
```

### Task 2.3: Encode/decode `ChartCore` + `Chart`

**Files:**
- Modify: `rust/crates/band-core/src/chart/codec.rs` (add core encode/decode)

**Interfaces:**
- Produces: `encode_core(&ChartCore) -> Vec<u8>`, `decode_core(&[u8]) -> Result<ChartCore, CodecError>`, `encode_chart(&Chart) -> Vec<u8>` (version byte + core section + envelope section), `decode_chart(&[u8]) -> Result<Chart, CodecError>`. The `encode_core` bytes are exactly what gets hashed (Task 2.4).

- [ ] **Step 1: Write the failing round-trip test**

Append to `rust/crates/band-core/src/chart/codec.rs`:
```rust
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
```
(Make `sample_core` reachable: in `chart/mod.rs` the test module already defines `pub(crate) fn sample_core`; ensure the `tests` module is `pub(crate) mod tests` or move `sample_core` into a `#[cfg(test)] pub(crate)` helper in `mod.rs`.)

- [ ] **Step 2: Run to verify it passes**

Run: `cd rust && cargo test -p band-core core_tests`
Expected: PASS (3 tests).

- [ ] **Step 3: Commit**

```bash
git add rust/crates/band-core/src/chart
git commit -m "feat(core): canonical binary encode/decode for chart core + envelope"
```

### Task 2.4: BLAKE3 content-address over the core

**Files:**
- Create: `rust/crates/band-core/src/chart/hash.rs`
- Modify: `rust/crates/band-core/Cargo.toml` (add `blake3`)

**Interfaces:**
- Produces: `content_address(core: &ChartCore) -> String` (lowercase hex of BLAKE3 over `encode_core`), and `content_address_bytes(core) -> [u8; 32]`.

- [ ] **Step 1: Add dependency**

In `band-core/Cargo.toml`:
```toml
[dependencies]
blake3 = { version = "1", default-features = false }
```

- [ ] **Step 2: Write the failing test**

`rust/crates/band-core/src/chart/hash.rs`:
```rust
//! Content-addressing: BLAKE3 over the canonical core bytes ONLY (envelope excluded).
//! The FORMAT_VERSION byte is hashed BEFORE the core bytes so the address is
//! version-namespaced (identical cores under different formats never collide).

use super::{codec::encode_core, ChartCore, FORMAT_VERSION};

pub fn content_address_bytes(core: &ChartCore) -> [u8; 32] {
    // As-built (Phase 2): version-namespaced — hash FORMAT_VERSION then encode_core(core).
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
```

- [ ] **Step 3: Run to verify it passes**

Run: `cd rust && cargo test -p band-core hash`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit**

```bash
git add rust/crates/band-core
git commit -m "feat(core): BLAKE3 content-address over hashed core (envelope excluded)"
```

### Task 2.5: Cross-target native-vs-WASM hash determinism

**Files:**
- Create: `rust/crates/band-core/tests/hash_determinism.rs`
- Modify: `rust/crates/band-core/Cargo.toml` (dev-dep `wasm-bindgen-test`)

**Interfaces:**
- Produces: a golden-vector test asserting the content-address of a fixed chart equals a hardcoded 64-hex string, runnable natively **and** under wasm — proving byte-identical hashes across targets (SP3 §3b requirement).

- [ ] **Step 1: Derive the golden hash natively**

Add a throwaway `println!` (or run the CLI `hashcheck` from Task 3.4 once it exists) to print `content_address` of the golden chart, then paste the value into the test as `GOLDEN`. Build the golden chart deterministically (no floats, no maps).

- [ ] **Step 2: Write the test**

`rust/crates/band-core/tests/hash_determinism.rs`:
```rust
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

// The value printed by `content_address(&golden_core())` — over the
// version-namespaced hash BLAKE3(FORMAT_VERSION ++ encode_core(core)):
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
```
Add to `band-core/Cargo.toml`:
```toml
[dev-dependencies]
wasm-bindgen-test = "0.3"
```

- [ ] **Step 3: Run natively, then under wasm**

Run: `cd rust && cargo test -p band-core --test hash_determinism`
Expected: PASS.
Run: `cd rust && wasm-pack test --node crates/band-core -- --test hash_determinism` (install `wasm-pack` first).
Expected: PASS — the **same** `GOLDEN` verifies under wasm, proving cross-target byte-identical hashing.

- [ ] **Step 4: Add the wasm test to CI**

Append to `.github/workflows/ci.yml` `rust` job:
```yaml
      - run: cargo install wasm-pack --locked
      - run: cd rust && wasm-pack test --node crates/band-core -- --test hash_determinism
```

- [ ] **Step 5: Commit**

```bash
git add rust/ .github/
git commit -m "test(core): cross-target native-vs-WASM hash determinism (golden vector)"
```

**REVIEW CHECKPOINT 2** — The chart format is deterministic, panic-free to decode, hashed over the core only (envelope excluded), and produces byte-identical content-addresses native and under WASM. This contract is now frozen; downstream tasks depend on it. Any later field addition bumps `FORMAT_VERSION`.

> **✅ REACHED (2026-07-01).** All of Phase 2 (Tasks 2.1–2.5) implemented via TDD and reviewed clean on branch `sp1-playable-core` (commits `ee17dd8..2ea60a9`). The content-address is version-namespaced BLAKE3 (`BLAKE3(FORMAT_VERSION ++ encode_core(core))`); byte-identical native-vs-WASM was **proven by an executed `wasm-pack test --node` run** (not just a compile check) against the golden vector `6989f2c9c0a25d0023d12db12a9bab6dfafcbf899aa73b9296bbb131bc434a8b`. Full workspace suite green; `band-core` builds for wasm32. CI (`.github/workflows/ci.yml`) is deferred with Phase 0 (needs iOS/hardware); the wasm-determinism CI step is parked in `rust/crates/band-core/tests/README-wasm-determinism.md` until it lands.

---

# Phase 3 — Minimal MIDI Importer

**Goal:** just enough MIDI→chart translation to produce **believable single-tier test charts** — reduce a track to lanes + space + sustains, carry note onset times through as **integer-µs feel-targets**. The fair/tunable/multi-tier translator is SP3; do not build it.

### Task 3.1: MIDI parse + tick→µs tempo mapping

**Files:**
- Create: `rust/crates/band-core/src/midi_import.rs`
- Modify: `rust/crates/band-core/src/lib.rs`, `rust/crates/band-core/Cargo.toml` (add `midly`)

**Interfaces:**
- Produces: `parse_smf(bytes: &[u8]) -> Result<ParsedMidi, ImportError>` where `ParsedMidi { ppq: u16, tempo_map: Vec<TempoEvent>, tracks: Vec<MidiTrack> }`, and `MidiTrack { name: Option<String>, channel10: bool, notes: Vec<MidiNote> }`, `MidiNote { start_us: SongPosUs, len_us: SongPosUs, pitch: u8, velocity: u8 }`. Tick→µs conversion uses the tempo map (integer math).

- [ ] **Step 1: Add dependency + module**

`band-core/Cargo.toml`: add `midly = { version = "0.5", default-features = false, features = ["alloc"] }`.
Add `pub mod midi_import;` to `lib.rs`.

- [ ] **Step 2: Write the failing test**

Create a tiny fixture MIDI programmatically in the test (via `midly` writer) — one track, 480 PPQ, tempo 500000 µs/beat, two notes at ticks 0 and 480 → expect `start_us` 0 and 500000.

`rust/crates/band-core/src/midi_import.rs` (test at bottom):
```rust
//! Minimal MIDI importer: SMF → tempo map + per-track absolute-µs note lists.
//! Scope = believable single-tier TEST charts only (SP1 §3). Not the SP3 translator.

use crate::chart::TempoEvent;
use crate::clock::SongPosUs;

#[derive(Debug, PartialEq)]
pub enum ImportError { Parse(String), NoTracks, Unsupported(&'static str) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiNote { pub start_us: SongPosUs, pub len_us: SongPosUs, pub pitch: u8, pub velocity: u8 }

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MidiTrack { pub name: Option<String>, pub channel10: bool, pub notes: Vec<MidiNote> }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMidi { pub ppq: u16, pub tempo_map: Vec<TempoEvent>, pub tracks: Vec<MidiTrack> }

// (implementation: iterate midly::Smf, track absolute ticks, on Note-On/Off pair
//  compute start/len; build tempo_map from Set-Tempo + Time-Signature meta events;
//  convert ticks→µs by integrating tempo segments with integer math.)
```
(Test uses a `midly`-built in-memory SMF; assert `parsed.tracks[0].notes[0].start_us == 0` and `[1].start_us == 500_000`, `ppq == 480`.)

- [ ] **Step 3: Implement `parse_smf` + `ticks_to_us`**

Write `fn ticks_to_us(abs_ticks, ppq, &tempo_segments) -> SongPosUs` integrating `micros_per_beat` across tempo changes with pure integer arithmetic: `us += (segment_ticks as i128 * mpb as i128) / ppq as i128`. Parse each `midly::TrackEvent`, accumulate delta ticks, pair Note-On(vel>0)/Note-Off(or Note-On vel0), detect GM channel 10 (index 9) → `channel10`, capture `TrackName` meta.

- [ ] **Step 4: Run to verify it passes**

Run: `cd rust && cargo test -p band-core midi_import`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/band-core
git commit -m "feat(core): minimal SMF parser with integer tick→µs tempo mapping"
```

### Task 3.2: Melodic reduction (guitar/bass/vocal) → lanes + space + sustains

**Files:**
- Modify: `rust/crates/band-core/src/midi_import.rs`

**Interfaces:**
- Produces: `reduce_melodic(track: &MidiTrack, role: InstrumentRole, set_index: u16) -> DifficultyTier`. Pitch-bands the track's `[min,max]` range into 4 lanes; simultaneous notes (same `start_us`) merge into a lane bitmask; `space = Strum` for guitar/bass, `Trigger` for vocal; `sustain_len_us = len_us` when `len_us >= SUSTAIN_MIN_US` (tunable), else 0; `sound` carries original pitch+velocity.

- [ ] **Step 1: Write the failing test**

Two notes at same `start_us` with pitches at opposite ends of the range → one `NoteEvent` with two lane bits set, `space == Strum`; a long note → `sustain_len_us > 0`.

```rust
#[test]
fn melodic_reduction_bands_pitch_and_merges_chords() {
    let track = MidiTrack { name: None, channel10: false, notes: vec![
        MidiNote { start_us: 0, len_us: 10_000, pitch: 40, velocity: 100 },   // low → lane 0
        MidiNote { start_us: 0, len_us: 10_000, pitch: 76, velocity: 100 },   // high → lane 3
        MidiNote { start_us: 500_000, len_us: 400_000, pitch: 58, velocity: 90 }, // long → sustain
    ] };
    let tier = reduce_melodic(&track, InstrumentRole::Guitar, 0);
    assert_eq!(tier.notes.len(), 2);
    assert_eq!(tier.notes[0].lanes, 0b1001);
    assert_eq!(tier.notes[0].space, SpaceAction::Strum);
    assert!(tier.notes[1].sustain_len_us > 0);
}
```

- [ ] **Step 2: Implement `reduce_melodic`**

Compute `min`/`max` pitch; `lane = ((pitch - min) * 4 / (max - min + 1)).min(3)`. Group notes by `start_us` (already sorted); OR their lane bits; take max `len_us` for the group's sustain; velocity = max. Define `pub const SUSTAIN_MIN_US: SongPosUs = 200_000;` in `tunables.rs` (create it) with a doc-comment marking it TUNABLE.

- [ ] **Step 3: Run to verify it passes**

Run: `cd rust && cargo test -p band-core midi_import`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/band-core
git commit -m "feat(core): melodic MIDI reduction (pitch-band lanes, chord merge, sustains)"
```

### Task 3.3: Drum reduction (GM map → lanes + kick-on-space)

**Files:**
- Modify: `rust/crates/band-core/src/midi_import.rs`

**Interfaces:**
- Produces: `reduce_drums(track: &MidiTrack, set_index: u16) -> DifficultyTier`. GM drum pitch → fixed lane (hi-hat/snare/tom/cymbal) per SP1 §2 table; kick → `space = Kick` with `lanes = 0`; no sustains (`sustain_len_us = 0`); simultaneous hits merge lane bits (kick stays on space).

- [ ] **Step 1: Write the failing test**

Kick (GM 36) + snare (GM 38) at same time → one event, `space == Kick`, snare's lane bit set.
```rust
#[test]
fn drum_reduction_maps_kick_to_space() {
    let track = MidiTrack { name: None, channel10: true, notes: vec![
        MidiNote { start_us: 0, len_us: 1_000, pitch: 36, velocity: 110 }, // kick
        MidiNote { start_us: 0, len_us: 1_000, pitch: 38, velocity: 100 }, // snare
    ] };
    let tier = reduce_drums(&track, 0);
    assert_eq!(tier.notes.len(), 1);
    assert_eq!(tier.notes[0].space, SpaceAction::Kick);
    assert_ne!(tier.notes[0].lanes, 0); // snare occupies a lane
    assert_eq!(tier.notes[0].sustain_len_us, 0);
}
```

- [ ] **Step 2: Implement `reduce_drums` + GM lane map**

Map: kick(35,36)→space Kick; snare(38,40)→lane; hi-hat(42,44,46)→lane; toms(41,43,45,47,48,50)→lane; cymbals(49,51,52,55,57,59)→lane. Group by `start_us`, OR lanes, set `space=Kick` if any kick present.

- [ ] **Step 3: Run to verify it passes**

Run: `cd rust && cargo test -p band-core midi_import`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/band-core
git commit -m "feat(core): drum MIDI reduction (GM kit → lanes, kick on space)"
```

### Task 3.4: `import` + `hashcheck` + `inspect` CLI harnesses

**Files:**
- Create: `rust/crates/band-cli/src/bin/import.rs`, `rust/crates/band-cli/src/bin/hashcheck.rs`, `rust/crates/band-cli/src/bin/inspect.rs`
- Modify: `rust/crates/band-core/src/midi_import.rs` (add `import_chart` orchestrator)

**Interfaces:**
- Produces: `import_chart(smf: &[u8], mapping: RoleMapping) -> Result<Chart, ImportError>` (assembles a one-tier chart from the reductions + tempo map + a default instrument-set ref per role), and three bins: `import <in.mid> <out.band> [--guitar N --bass N --drums N --vocal N]`; `hashcheck <file.band>` (prints content-address); `inspect <file.band>` (prints a JSON *debug projection* — never hashed).

- [ ] **Step 1: Write `import_chart` + a round-trip integration test**

`rust/crates/band-core/tests/import_roundtrip.rs`: build an in-memory SMF, `import_chart`, `encode_chart`, `decode_chart`, assert equality and that at least one track has notes.

- [ ] **Step 2: Implement `import_chart`**

Map chosen track indices to roles; run `reduce_drums` for the drums role and `reduce_melodic` for the others; assemble `ChartCore` with the parsed `tempo_map`, `duration_us` = max note end, one `InstrumentSetRef` per role (ids `guitar_clean_v1`, `bass_finger_v1`, `drums_rock_v1`, `vocal_pad_v1`), empty lyrics/groove_sections; wrap in `Chart` with a basic envelope from filename.

- [ ] **Step 3: Write the three bins**

`import.rs`: read `.mid`, parse args, `import_chart`, write `encode_chart` bytes to `out.band`.
`hashcheck.rs`: read `.band`, `decode_chart`, print `content_address(&chart.core)`.
`inspect.rs`: read `.band`, `decode_chart`, print a hand-written JSON projection (counts per role, tempo, duration, first few notes) — clearly labelled "DEBUG PROJECTION — NOT HASHED".

- [ ] **Step 4: Run integration test + exercise the bins**

Run: `cd rust && cargo test -p band-core --test import_roundtrip`
Expected: PASS.
Run: `cargo run -p band-cli --bin import -- path/to/test.mid /tmp/out.band --guitar 0 --drums 1` then `cargo run -p band-cli --bin hashcheck -- /tmp/out.band`
Expected: writes `out.band`, prints a 64-hex address; re-running `import`+`hashcheck` on the same input prints the **same** address (determinism through the whole pipeline).

- [ ] **Step 5: Generate + commit real test charts**

Import a few royalty-free/self-authored `.mid` files into `godot/assets/test_charts/*.band`. These are the playtest content.
```bash
git add rust/ godot/assets/test_charts/
git commit -m "feat(cli): MIDI import + hashcheck + inspect harnesses; seed test charts"
```

**REVIEW CHECKPOINT 3** — Real `.band` test charts exist, produced deterministically from MIDI, with stable content-addresses and a debug projection for eyeballing. Groove (feel-target times) is preserved verbatim from the source MIDI onsets.

---

# Phase 4 — Full Audio Engine

**Goal:** replace the click spike with a real **rustysynth** sampler that plays a chart: the manned instrument sounded by the player's actual hit events, unmanned instruments auto-performed at chart-ideal, **misses leave audible holes**, and a stylized melodic vocal voice. Sample sets ship with the game.

> Verification here is substantially **by ear** (audio can't be unit-asserted for "sounds right"), but scheduling *logic* (which note fires at which sample) is unit-tested in `band-core` where it's pure, and the engine wiring is smoke-tested via a CLI.

### Task 4.1: Note-schedule builder (pure, headless)

**Files:**
- Create: `rust/crates/band-core/src/schedule.rs`
- Modify: `rust/crates/band-core/src/lib.rs`

**Interfaces:**
- Consumes: `ChartCore`.
- Produces: `build_auto_schedule(core, role, tier) -> Vec<SoundCmd>` and `SoundCmd { at_us: SongPosUs, set_index: u16, pitch: u8, velocity: u8, gate_us: SongPosUs }` (gate = sustain length or a short default for hits). Pure/testable: the audio thread consumes these; unmanned roles use this directly; the manned role gets `SoundCmd`s emitted live on each hit.

- [ ] **Step 1: Write the failing test**

Assert an auto-schedule for a 2-note tier yields 2 `SoundCmd`s at the notes' `song_pos_us` with matching pitch/velocity and gate = sustain (or `DEFAULT_HIT_GATE_US` when sustain is 0).

- [ ] **Step 2: Implement + tunable**

Add `pub const DEFAULT_HIT_GATE_US: SongPosUs = 120_000;` (TUNABLE) to `tunables.rs`.

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core schedule`
```bash
git add rust/crates/band-core
git commit -m "feat(core): pure note-schedule builder (auto-perform + live-hit SoundCmd)"
```

### Task 4.2: rustysynth sampler wrapper

**Files:**
- Create: `rust/crates/band-audio/src/sampler.rs`
- Modify: `rust/crates/band-audio/src/lib.rs`

**Interfaces:**
- Produces: `Sampler` wrapping `rustysynth::Synthesizer` loaded from bundled SF2 files (one per role/set-id); `Sampler::note_on(channel, pitch, velocity)`, `note_off(channel, pitch)`, `render(out: &mut [f32], channels)`. Vocal set = a vowel-pad ("aah") SF2 program (Global Constraints: stylized melodic voice).

- [ ] **Step 1: Bundle SF2 sample sets**

Place `godot/assets/soundsets/{guitar_clean_v1,bass_finger_v1,drums_rock_v1,vocal_pad_v1}.sf2` (source solid free SF2s or author minimal ones). Map set-id → file.

- [ ] **Step 2: Implement `Sampler`**

Load each SF2 into a `rustysynth::SoundFont`; create one `Synthesizer` per set (or one multi-bank synth). `render` mixes into the output buffer.

- [ ] **Step 3: Smoke test by ear via CLI**

Extend `band-cli` with `synthspike <file.band> <role>` that loads the sampler and auto-performs the chosen role's tier through `AudioEngine`.
Run: `cargo run -p band-cli --bin synthspike -- /tmp/out.band guitar`
Expected: you hear the guitar part play through, in time with the clicks/clock.

- [ ] **Step 4: Commit**

```bash
git add rust/ godot/assets/soundsets/
git commit -m "feat(audio): rustysynth sampler + bundled SF2 sets (incl vocal pad)"
```

### Task 4.3: Lock-free scheduler + engine integration (per-performer, auto-perform, miss holes)

**Files:**
- Create: `rust/crates/band-audio/src/scheduler.rs`
- Modify: `rust/crates/band-audio/src/engine.rs`

**Interfaces:**
- Produces: `AudioEngine::play_chart(core, manned_role, tier)` — starts the stream, feeds unmanned roles' `SoundCmd`s to the audio thread pre-scheduled, and exposes `AudioEngine::sound_manned(pitch, velocity, gate_us)` (called from the game on each registered hit) via an `rtrb` producer. The audio thread triggers sampler note-ons at the exact sample for scheduled cmds and immediately for live manned cmds. **Miss = hole:** the manned role emits NO auto `SoundCmd`, so a missed note simply never sounds (optional clank behind a tunable flag).

- [ ] **Step 1: Implement the ring-buffer command path**

Game thread → `rtrb::Producer<LiveCmd>`; audio thread drains the consumer each callback, plus walks the pre-sorted unmanned schedule by absolute frame (like the click spike). Manned live cmds fire on the callback in which they're received (lowest latency for own instrument — matches the platform contract "own instrument local and instant").

- [ ] **Step 2: Implement `MissPolicy` tunable**

`pub enum MissPolicy { SilentHole, Clank }` default `SilentHole` (TUNABLE, playtest).

- [ ] **Step 3: Manual verification (by ear)**

Run `synthspike` in an "unmanned" mode (whole band auto-performs) → full song plays. Then a "manned drums, no input" mode → the drums part is **silent holes** over an otherwise full mix. This demonstrates per-performer sounding + miss-as-hole before the UI exists.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/band-audio
git commit -m "feat(audio): lock-free scheduler, per-performer sounding, miss-as-hole"
```

**REVIEW CHECKPOINT 4** — A chart plays through the real sampler on desktop: unmanned parts auto-perform, the manned part only sounds on live hit commands, misses are audible holes, vocal line is a stylized pad. Re-run the Phase 1 iOS check with the sampler if feasible; otherwise defer full iOS audio to Phase 9 integration.

---

# Phase 5 — Input Abstraction + Hit Detection

**Goal:** contract #3 — one device-agnostic input event stream, and **hit detection** that matches an action to the nearest eligible note in the pressed lane within the timing window, computes signed offset, grades it, and scores sustains on onset + hold length. Fully headless/TDD.

### Task 5.1: Input model + timing windows

**Files:**
- Create: `rust/crates/band-core/src/input.rs`, `rust/crates/band-core/src/tunables.rs` (extend)
- Modify: `rust/crates/band-core/src/lib.rs`

**Interfaces:**
- Produces: `enum InputAction { LaneDown(u8), LaneUp(u8), SpaceDown, SpaceUp }`, `struct InputEvent { song_pos_us: SongPosUs, action: InputAction }` (song_pos is already calibration-adjusted by the Godot layer), `struct TimingWindows { perfect_us, good_us, hit_us }` with `Default` = 25_000/50_000/80_000, and `enum Grade { Perfect, Good, Hit, Miss }` + `TimingWindows::grade(abs_offset_us) -> Grade`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn windows_grade_by_absolute_offset() {
    let w = TimingWindows::default();
    assert_eq!(w.grade(0), Grade::Perfect);
    assert_eq!(w.grade(25_000), Grade::Perfect);
    assert_eq!(w.grade(25_001), Grade::Good);
    assert_eq!(w.grade(50_000), Grade::Good);
    assert_eq!(w.grade(80_000), Grade::Hit);
    assert_eq!(w.grade(80_001), Grade::Miss);
}
```

- [ ] **Step 2: Implement + mark tunable**

Implement the types; doc-comment each window field "TUNABLE (SP1 §7)".

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core input`
```bash
git add rust/crates/band-core
git commit -m "feat(core): input model + tunable timing windows + grading"
```

### Task 5.2: Hit detection for taps (nearest-eligible-note matching)

**Files:**
- Create: `rust/crates/band-core/src/judge.rs`
- Modify: `rust/crates/band-core/src/lib.rs`

**Interfaces:**
- Consumes: a tier's `&[NoteEvent]`, `TimingWindows`, and a stream of `InputEvent`.
- Produces: `Judge::new(notes, windows)`, `Judge::feed(event) -> Option<NoteResult>`, `struct NoteResult { song_pos: SongPosUs, target_us, actual_us, signed_offset_us, grade, sustain_len_error_us }`, and `Judge::finalize() -> Vec<NoteResult>` (unhit notes → Miss). An actionable event matches the nearest unconsumed note whose lane/space is satisfied and whose `|actual - target| <= hit_us`; signed offset = `actual - target` (early −, late +). Each note consumed once.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn on_target_tap_is_perfect_zero_offset() {
    let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
    let mut j = Judge::new(&notes, TimingWindows::default());
    let r = j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) }).unwrap();
    assert_eq!(r.grade, Grade::Perfect);
    assert_eq!(r.signed_offset_us, 0);
}
#[test]
fn late_tap_has_positive_offset_and_good_grade() {
    let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
    let mut j = Judge::new(&notes, TimingWindows::default());
    let r = j.feed(InputEvent { song_pos_us: 1_040_000, action: InputAction::LaneDown(0) }).unwrap();
    assert_eq!(r.signed_offset_us, 40_000);
    assert_eq!(r.grade, Grade::Good);
}
#[test]
fn wrong_lane_does_not_match() {
    let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
    let mut j = Judge::new(&notes, TimingWindows::default());
    assert!(j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(2) }).is_none());
}
#[test]
fn unhit_note_finalizes_as_miss() {
    let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 0)];
    let j = Judge::new(&notes, TimingWindows::default());
    let all = j.finalize();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].grade, Grade::Miss);
}
```
(`note_at` = test helper building a `NoteEvent`.)

- [ ] **Step 2: Implement `Judge`**

Maintain a per-note `consumed` flag and a cursor. On a `LaneDown(l)`/`SpaceDown`, scan notes within `[actual - hit_us, actual + hit_us]` that require that lane/space and are unconsumed, pick the one with smallest `|offset|`, mark consumed, return `NoteResult`. Guitar/bass: `LaneDown` sets fret state; the **strum** (`SpaceDown` on a Strum note) is the actionable trigger — model this in Task 5.3. Start with drums/vocal where each lane fires directly.

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core judge`
```bash
git add rust/crates/band-core
git commit -m "feat(core): tap hit-detection (nearest-note match, signed offset, grade)"
```

### Task 5.3: Strum-chord model + sustains (onset + hold-length)

**Files:**
- Modify: `rust/crates/band-core/src/judge.rs`

**Interfaces:**
- Produces: strum handling (held lane bitmask + `SpaceDown` fires the held chord: matches a Strum note whose `lanes` equals the currently-held lane mask, within window), and sustain handling: a matched note with `sustain_len_us > 0` opens a sustain; the corresponding `LaneUp`/`SpaceUp` closes it; `sustain_len_error_us = (actual_hold_us - charted_sustain_len_us).abs()`. Adds `Judge::held_lanes` state.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn strum_fires_held_chord() {
    let notes = vec![note_at(1_000_000, 0b0011, SpaceAction::Strum, 0)];
    let mut j = Judge::new(&notes, TimingWindows::default());
    j.feed(InputEvent { song_pos_us: 980_000, action: InputAction::LaneDown(0) });
    j.feed(InputEvent { song_pos_us: 980_000, action: InputAction::LaneDown(1) });
    let r = j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::SpaceDown }).unwrap();
    assert_eq!(r.grade, Grade::Perfect);
}
#[test]
fn sustain_scores_hold_length_error() {
    let notes = vec![note_at(1_000_000, 0b0001, SpaceAction::None, 300_000)];
    let mut j = Judge::new(&notes, TimingWindows::default());
    j.feed(InputEvent { song_pos_us: 1_000_000, action: InputAction::LaneDown(0) });
    let r = j.feed(InputEvent { song_pos_us: 1_320_000, action: InputAction::LaneUp(0) }).unwrap();
    assert_eq!(r.sustain_len_error_us, 20_000); // held 320ms vs charted 300ms
}
```
(For sustains, `feed` returns the finalized `NoteResult` on release; design the return so onset returns a provisional and release finalizes — or hold provisional internally and only surface on close. Pick one and encode it in the test.)

- [ ] **Step 2: Implement strum + sustain state machine**

Track `held_lanes: u8`. `SpaceDown` on a Strum note requires `note.lanes == held_lanes` (or subset policy — TUNABLE, start with equality). Open sustains keyed by note index; close on matching `LaneUp`/`SpaceUp`, compute hold error.

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core judge`
```bash
git add rust/crates/band-core
git commit -m "feat(core): strum-chord firing + sustain onset+length scoring"
```

### Task 5.4: `judge-replay` CLI harness

**Files:**
- Create: `rust/crates/band-cli/src/bin/judge-replay.rs`

**Interfaces:**
- Produces: `judge-replay <file.band> <role> <events.json>` → prints per-note `NoteResult`s and a summary (grade counts). Lets us replay recorded input against a chart headlessly to assert scoring without the UI.

- [ ] **Step 1: Implement + smoke**

Read a simple newline-delimited event file (`song_pos_us action`), run `Judge`, print results.
Run: `cargo run -p band-cli --bin judge-replay -- /tmp/out.band drums /tmp/events.txt`
Expected: prints grades; an all-on-target event file yields all Perfect.

- [ ] **Step 2: Commit**

```bash
git add rust/crates/band-cli
git commit -m "feat(cli): judge-replay harness for headless scoring runs"
```

**REVIEW CHECKPOINT 5** — Hit detection is proven headless across taps, wrong-lane rejection, misses, strum-chords, and sustains (onset + length). Signed offsets are correct in sign and magnitude — the raw material groove scoring needs.

---

# Phase 6 — Groove Scoring

**Goal (de-risk #3, fairness half):** windowed groove scoring — mean-lean + spread inside the pocket guardrail — emitting **band-ready** per-note + windowed metrics, and a **headless proof** that a tight pocket out-scores sloppy and is competitive with (or, with feel-direction, beats) robotic-perfect. This is the novel soul, provable in a unit test.

### Task 6.1: Windowed groove metrics

**Files:**
- Create: `rust/crates/band-core/src/groove.rs`
- Modify: `rust/crates/band-core/src/lib.rs`, `tunables.rs`

**Interfaces:**
- Consumes: an ordered slice of `NoteResult` (from `judge`).
- Produces: `struct WindowMetrics { mean_offset_us: i64, spread_us: i64, groove_bonus: i64, in_pocket: bool }`, `fn window_metrics(results: &[NoteResult], cfg: &GrooveConfig) -> WindowMetrics`, and `struct GrooveConfig { window_len: usize, pocket_guardrail_us: i64, max_bonus: i64, spread_kind: SpreadKind }` with `Default` = window 8, guardrail 60_000, and a `max_bonus` scaling. `spread` uses std-dev by default (`SpreadKind::StdDev`; `MeanAbsDev` selectable — TUNABLE).

- [ ] **Step 1: Write the failing tests (the worked example)**

```rust
fn offsets(v: &[i64]) -> Vec<NoteResult> { /* build NoteResults with these signed offsets */ }

#[test]
fn tight_lean_earns_bonus_scatter_does_not() {
    let cfg = GrooveConfig::default();
    // Pocket: reliably +13ms, tiny spread → in pocket, bonus > 0.
    let pocket = window_metrics(&offsets(&[14_000, 12_000, 13_000, 13_000]), &cfg);
    assert!(pocket.in_pocket);
    assert!(pocket.groove_bonus > 0);
    // Sloppy: avg ~0 but wide spread → bonus 0.
    let sloppy = window_metrics(&offsets(&[30_000, -30_000, 20_000, -20_000]), &cfg);
    assert_eq!(sloppy.groove_bonus, 0);
    assert!(pocket.groove_bonus > sloppy.groove_bonus);
}

#[test]
fn tight_but_far_off_is_guardrailed_out() {
    let cfg = GrooveConfig::default();
    // Consistently lazy +90ms: tight spread but outside the 60ms pocket → no bonus.
    let lazy = window_metrics(&offsets(&[90_000, 88_000, 91_000, 89_000]), &cfg);
    assert!(!lazy.in_pocket);
    assert_eq!(lazy.groove_bonus, 0);
}
```

- [ ] **Step 2: Implement `window_metrics`**

`mean = sum/len` (integer); `spread` = integer std-dev (`isqrt(sum((o-mean)^2)/len)`) or MAD; `in_pocket = mean.abs() <= guardrail`; `groove_bonus = if in_pocket { scale by tightness: max_bonus * (guardrail - spread).max(0) / guardrail } else { 0 }`. All integer math (determinism). Provide an integer `isqrt` helper.

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core groove`
```bash
git add rust/crates/band-core
git commit -m "feat(core): windowed groove metrics (mean-lean, spread, pocket guardrail)"
```

### Task 6.2: Total scoring + the fairness proof (pocket > sloppy, pocket ≥ robo)

**Files:**
- Modify: `rust/crates/band-core/src/groove.rs`
- Create: `rust/crates/band-core/tests/groove_fairness.rs`

**Interfaces:**
- Produces: `fn accuracy_points(grade: Grade) -> i64` (Perfect/Good/Hit weights — TUNABLE), `fn score_run(results: &[NoteResult], cfg) -> ScoreSummary` where `ScoreSummary { total: i64, per_note: Vec<NoteResult>, windows: Vec<WindowMetrics>, streak: u32, in_pocket_pct: u8 }` (streak = consecutive non-Miss; total = sum(accuracy) + sum(window groove bonuses); windows computed as a rolling window across the run). Optional pocket-direction bonus applied when `GrooveSection.feel_dir` matches the sign of `mean_offset` (ship-optional).

- [ ] **Step 1: Write the failing fairness integration test (SP1 §8 worked example)**

`rust/crates/band-core/tests/groove_fairness.rs`:
```rust
use band_core::groove::*;
use band_core::judge::results_from_offsets; // test helper exposed for reuse

// Feel-targets 0/500/1000/1500ms; three performances from SP1 §8.
fn robo()   -> Vec<i64> { vec![0, 0, 0, 0] }                       // perfect accuracy, zero lean
fn pocket() -> Vec<i64> { vec![14_000, 12_000, 13_000, 15_000] }  // +13ms, tiny spread
fn sloppy() -> Vec<i64> { vec![30_000, -20_000, 20_000, -30_000] }// avg ~0, wide spread

#[test]
fn pocket_beats_sloppy() {
    let cfg = GrooveConfig::default();
    let p = score_run(&results_from_offsets(&pocket()), &cfg).total;
    let s = score_run(&results_from_offsets(&sloppy()), &cfg).total;
    assert!(p > s, "pocket {p} must beat sloppy {s}");
}

#[test]
fn robo_is_not_penalized_but_pocket_is_competitive() {
    let cfg = GrooveConfig::default();
    let r = score_run(&results_from_offsets(&robo()), &cfg).total;
    let p = score_run(&results_from_offsets(&pocket()), &cfg).total;
    // Robo: full accuracy, zero groove bonus. Pocket: near-full accuracy + groove bonus.
    // With a pocket-hungry song (feel-direction bonus on), pocket edges robo.
    assert!(p >= r, "tight pocket {p} should at least match robotic {r}");
}

#[test]
fn sloppy_loses_to_robo_too() {
    let cfg = GrooveConfig::default();
    let r = score_run(&results_from_offsets(&robo()), &cfg).total;
    let s = score_run(&results_from_offsets(&sloppy()), &cfg).total;
    assert!(r > s);
}
```

- [ ] **Step 2: Implement `score_run` + tune weights so the ordering holds**

Choose accuracy weights and `max_bonus` such that: Robo gets max accuracy + 0 bonus; Pocket gets (slightly-less-than-max accuracy because +13ms is still Perfect within ±25ms, so actually full accuracy) + full groove bonus → `p >= r`; Sloppy gets lower accuracy (some notes fall to Good) + 0 bonus. The exact numbers are **TUNABLE**; the *ordering assertions* are the contract. Document that `p > r` vs `p >= r` depends on whether the feel-direction bonus is enabled for the section.

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core --test groove_fairness`
Expected: PASS — the fairness of groove is now demonstrably encoded.
```bash
git add rust/crates/band-core
git commit -m "feat(core): total scoring + headless proof pocket>sloppy, pocket>=robo"
```

### Task 6.3: Band-ready metric records + JudgeNode-facing API

**Files:**
- Modify: `rust/crates/band-core/src/groove.rs`

**Interfaces:**
- Produces: the exact **band-ready** shapes SP1 §8 names — per-note `{ song_pos, target_time, actual_time, signed_offset, grade, sustain_len_error }` (already `NoteResult`) and windowed `{ mean_offset, spread, groove_bonus, in_pocket }` (`WindowMetrics`) — plus `struct LiveGroove { last_note: Option<NoteResult>, window: WindowMetrics, streak: u32 }` that the gdext `JudgeNode` polls each frame to drive the pocket meter. These are emitted, never recomputed by the UI.

- [ ] **Step 1: Write the failing test**

Feed a running `Judge` + `groove` accumulator note-by-note; assert `LiveGroove.window.mean_offset_us` tracks the rolling window and `streak` resets on a Miss.

- [ ] **Step 2: Implement the incremental accumulator**

A `GrooveTracker::push(NoteResult)` updating a ring buffer of the last `window_len` results, recomputing `WindowMetrics` and `streak`.

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core groove`
```bash
git add rust/crates/band-core
git commit -m "feat(core): incremental band-ready groove tracker for live meter"
```

**REVIEW CHECKPOINT 6** — Groove scoring is fair and proven headless: tight pocket beats sloppy, ties-or-beats robotic, and consistently-lazy is guardrailed out. Metrics are emitted in the exact band-ready shapes SP2 will aggregate. The *feel* is still unproven — that's Phase 8.

---

# Phase 7 — Calibration

**Goal:** day-zero per-device calibration math (audio-out / input / haptic offsets) as pure functions, plus a persistence contract. The tap-to-beat derivation is headless-testable; the metronome UI is Phase 8. Haptics fire relative to audio, never raw input.

### Task 7.1: Tap-to-beat offset derivation

**Files:**
- Create: `rust/crates/band-core/src/calibration.rs`
- Modify: `rust/crates/band-core/src/lib.rs`

**Interfaces:**
- Produces: `struct CalibrationOffsets { audio_out_us: i64, input_us: i64, haptic_us: i64 }` (Default all 0), and `fn derive_input_offset(metronome_us: &[i64], taps_us: &[i64]) -> i64` (median of nearest-tick deltas — robust to outliers), and `fn apply(offsets, raw_song_pos_us) -> i64` giving the corrected song-position used when timestamping input.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn derive_median_offset_from_taps() {
    let ticks = vec![0, 500_000, 1_000_000, 1_500_000];
    let taps  = vec![40_000, 542_000, 1_038_000, 1_541_000]; // ~+40ms late, one jittery
    let off = derive_input_offset(&ticks, &taps);
    assert!((38_000..=42_000).contains(&off));
}
```

- [ ] **Step 2: Implement (median of per-tap nearest-tick deltas)**

For each tap, find nearest tick, delta = tap - tick; return the median (integer).

- [ ] **Step 3: Run + commit**

Run: `cd rust && cargo test -p band-core calibration`
```bash
git add rust/crates/band-core
git commit -m "feat(core): tap-to-beat calibration offset derivation (median)"
```

### Task 7.2: Offset persistence contract + haptic-relative-to-audio helper

**Files:**
- Modify: `rust/crates/band-core/src/calibration.rs`

**Interfaces:**
- Produces: `CalibrationOffsets` binary (de)serialize via the codec `Writer`/`Reader` (per-device blob Godot stores in `user://`), and `fn haptic_fire_us(offsets, note_target_us) -> i64` = `note_target_us + audio_out_us - haptic_us` (haptic aligned to *heard audio*, not raw input — Global Constraints).

- [ ] **Step 1: Write the failing test**

Round-trip `CalibrationOffsets` through encode/decode; assert `haptic_fire_us` shifts by `audio_out - haptic`.

- [ ] **Step 2: Implement + commit**

Run: `cd rust && cargo test -p band-core calibration`
```bash
git add rust/crates/band-core
git commit -m "feat(core): calibration persistence + haptic-relative-to-audio timing"
```

**REVIEW CHECKPOINT 7** — Calibration math is headless-proven and its persistence/haptic-timing contracts are defined for the Godot layer to call. iOS latency numbers from Phase 1 seed the defaults.

---

# Phase 8 — Godot Minimal Playable UI + Haptics (§12)

**Goal (de-risk #3, fun half):** the smallest programmer-art surface that lets us *feel* the game — picker, calibration, note highway, real-time feedback, results with groove breakdown, **the pocket meter**, and the light audience view — plus per-action haptics. Godot owns these scenes; the Rust core supplies scoring/timing/audio/metrics. The UI **reads** the core's outputs; it never computes groove.

> Verification is **manual playtest** against the Inebriated-User Test plus concrete on-screen checks. gdext node code is real; scene assembly is described as node trees.

### Task 8.1: gdext bridge nodes (chart, clock, audio, judge, calibration)

**Files:**
- Create: `rust/crates/band-gdext/src/{chart_node.rs,clock_node.rs,audio_node.rs,judge_node.rs,calibration_node.rs}`
- Modify: `rust/crates/band-gdext/src/lib.rs`, `rust/crates/band-gdext/Cargo.toml` (dep `band-audio`)

**Interfaces:**
- Produces Godot classes: `ChartLoader.load(path) -> ChartHandle` (exposes per-role note arrays, tempo map, lyrics for rendering); `SongClockNode.song_pos_us()`; `AudioEngineNode.play_chart(handle, manned_role, tier)` / `sound_manned(pitch,vel,gate)` / `stop()`; `JudgeNode.feed(song_pos_us, action_code)` (returns a `Dictionary` NoteResult) + `live_groove() -> Dictionary` (mean_offset, spread, groove_bonus, in_pocket, streak); `CalibrationNode.derive(ticks, taps) -> Dictionary` + `save/load`. Input actions are integer codes matching `InputAction`.

- [ ] **Step 1: Implement the nodes wrapping core + audio**

Each is a thin `#[derive(GodotClass)]` wrapper translating between Godot types (`GString`, `PackedInt64Array`, `Dictionary`) and the Rust core APIs. `JudgeNode` owns a `Judge` + `GrooveTracker`; `feed` maps `(song_pos_us, action_code)` → `InputEvent`, runs the judge, pushes to the tracker, returns the `NoteResult` dict (or empty).

- [ ] **Step 2: Build + load-smoke in Godot**

Run: `cd rust && cargo build -p band-gdext` then launch Godot; in `main.gd` temporarily `ChartLoader.new().load("res://assets/test_charts/xxx.band")` and print role note counts.
Expected: prints correct counts (matches `inspect` CLI). Remove the temp code.

- [ ] **Step 3: Commit**

```bash
git add rust/crates/band-gdext
git commit -m "feat(gdext): chart/clock/audio/judge/calibration bridge nodes"
```

### Task 8.2: Test-chart picker + calibration screens

**Files:**
- Create: `godot/scenes/ChartPicker.tscn`, `godot/scenes/Calibration.tscn`, `godot/scripts/{chart_picker.gd,calibration.gd}`
- Modify: `godot/scenes/Main.tscn` (route to picker; first-run → calibration)

**Interfaces:**
- Consumes: `ChartLoader`, `CalibrationNode`, `AudioEngineNode`.
- Produces: picker (bare `ItemList` of `res://assets/test_charts/*.band` + role option + tier option → Play) and calibration (metronome via `AudioEngineNode` click schedule; capture taps as `song_pos_us`; `CalibrationNode.derive`; show + save offsets; first-run gated).

- [ ] **Step 1: Build the picker scene**

Node tree: `Control > VBoxContainer > {ItemList charts, OptionButton role, OptionButton tier, Button Play}`. `chart_picker.gd` populates from `DirAccess`, on Play loads the chart and changes scene to `PlayHighway.tscn` passing chart path + role + tier via an autoload singleton.

- [ ] **Step 2: Build the calibration scene**

`calibration.gd`: schedule metronome clicks, read `SongClockNode.song_pos_us()` on each tap input, collect ~8 taps, call `CalibrationNode.derive(ticks, taps)`, display audio/input/haptic offsets, Save persists via `CalibrationNode.save`. First-run gate: if no saved offsets, `Main` routes here before the picker.

- [ ] **Step 3: Manual verification**

Run the game: first launch shows calibration; tap along; see a plausible offset (~matches the Phase 1 iOS/desktop measurement); Save; relaunch skips straight to the picker. Pick a chart+role+tier → Play loads the highway (blank for now).

- [ ] **Step 4: Commit**

```bash
git add godot/
git commit -m "feat(ui): test-chart picker + first-run tap-to-beat calibration"
```

### Task 8.3: Note highway + input + audio (the heart)

**Files:**
- Create: `godot/scenes/PlayHighway.tscn`, `godot/scripts/play_highway.gd`, `godot/scripts/input_map.gd`

**Interfaces:**
- Consumes: `ChartLoader`, `SongClockNode`, `AudioEngineNode`, `JudgeNode`, `CalibrationOffsets`.
- Produces: 4 scrolling lanes + a space zone; notes positioned by `(song_pos_us - clock.song_pos_us())` scaled by approach time (TUNABLE lookahead); bar lines from the tempo map; sustain tails; per-instrument space semantics (strum bar / kick zone / sustain). Input: keyboard/gamepad/touch(5 zones)/MIDI → `JudgeNode.feed(calibrated_song_pos, action_code)`; on a non-Miss result, `AudioEngineNode.sound_manned(...)`. Unmanned roles auto-perform via `play_chart`.

- [ ] **Step 1: Build the highway rendering**

`play_highway.gd _process`: read `song_pos_us`; for each visible note compute screen-Y from `(note_us - song_pos_us)`; draw notes/tails/bar lines. Touch layout = 5 large `TouchScreenButton`s; desktop = `InputMap` actions `lane0..3` + `space`; MIDI via Godot's MIDI input → `input_map.gd` normalizes all to `(action_code)`.

- [ ] **Step 2: Wire input → judge → audio**

On each input event, timestamp with `apply(offsets, song_pos_us)` and call `JudgeNode.feed`; if result grade != Miss, call `AudioEngineNode.sound_manned` with the note's sound (so the manned part is sounded by real hits; misses stay silent holes). Start `AudioEngineNode.play_chart(handle, manned_role, tier)` on scene enter (unmanned auto-perform + count-in).

- [ ] **Step 3: Manual verification (Inebriated-User Test)**

Play a chart on desktop: notes scroll in time with the audio; hitting a lane/strum sounds your part and lands within the highway hit line; missing leaves an audible hole in your part over the full auto-performed backing. It should be *playable without instruction*.

- [ ] **Step 4: Commit**

```bash
git add godot/
git commit -m "feat(ui): note highway + device-agnostic input + per-performer audio"
```

### Task 8.4: Real-time feedback + haptics

**Files:**
- Create: `godot/scripts/feedback.gd`
- Modify: `godot/scenes/PlayHighway.tscn`, `godot/scripts/play_highway.gd`

**Interfaces:**
- Consumes: `JudgeNode` result dicts, `CalibrationOffsets`.
- Produces: per-hit judgment flash (Perfect/Good/Hit/Miss) with an **early/late** indicator (sign of `signed_offset_us`); streak counter; miss = visual gap paired with the audible hole. Haptics: `Input.vibrate_handheld` (iOS Core Haptics where available) fired at `haptic_fire_us` (relative to audio) — sharp tap for hit/strum, heavier for kick, gentle buzz during sustains.

- [ ] **Step 1: Implement feedback rendering**

On each `feed` result: flash grade label colored by grade; show `EARLY`/`LATE` from offset sign; update streak from `JudgeNode.live_groove().streak`.

- [ ] **Step 2: Implement calibrated haptics**

Fire haptics scheduled at `haptic_fire_us(offsets, note_target_us)` — NOT on the raw input event — so a mistimed buzz never lies about timing. Kick = heavier pattern; sustain = gentle continuous buzz that fades if the player drifts off the note.

- [ ] **Step 3: Manual verification on device**

On iPhone: hits produce a crisp buzz aligned to the *heard* click; kicks feel heavier; sustains buzz continuously. Flash + early/late + streak read correctly at a glance.

- [ ] **Step 4: Commit**

```bash
git add godot/
git commit -m "feat(ui): real-time judgment/early-late/streak + audio-relative haptics"
```

### Task 8.5: The pocket meter (the one novel, essential element)

**Files:**
- Create: `godot/scenes/PocketMeter.tscn`, `godot/scripts/pocket_meter.gd`
- Modify: `godot/scenes/PlayHighway.tscn` (embed the meter + ambient layer)

**Interfaces:**
- Consumes: `JudgeNode.live_groove()` → `{ mean_offset, spread, groove_bonus, in_pocket, streak }`.
- Produces: (1) a horizontal **pocket lane** — a marker at x = mean-lean (ahead←→behind) inside a marked pocket zone (the ±guardrail), where marker **sharpness vs smear encodes spread** (tight = crisp dot, sloppy = wide blur); (2) an **ambient feel layer** — the highway warms/glows when tight-and-in-pocket, cools/desaturates when scattered or out of pocket. Mappings (lean→x, spread→blur, warm/cold intensity) are **TUNABLE** exports.

- [ ] **Step 1: Build the pocket-lane meter**

`pocket_meter.gd _process`: read `live_groove()`; map `mean_offset_us` → marker x within the drawn pocket zone (TUNABLE `px_per_us`); map `spread_us` → marker blur/width (TUNABLE); shade the pocket-zone band. Everything read from the core — the UI computes nothing.

- [ ] **Step 2: Build the ambient warm/cold layer**

A full-screen `CanvasModulate`/shader tint: warm+brighter as `in_pocket && spread` low; cold+desaturated as spread rises or `in_pocket` false. Intensity capped by a TUNABLE so it never distracts (playtest).

- [ ] **Step 3: Manual verification (the core fun claim)**

Play three ways and watch the meter: **robotic** (dead-center crisp dot, neutral warmth), **tight pocket** (crisp dot parked slightly off-center inside the zone, highway warm/glowing), **sloppy** (wide smeared marker, highway cold). Groove must be *glanceable and drunk-proof*. Tune the mappings in playtest until a tight pocket is unmistakably legible.

- [ ] **Step 4: Commit**

```bash
git add godot/
git commit -m "feat(ui): pocket meter (dev pocket-lane + ambient warm/cold feel layer)"
```

### Task 8.6: Results screen (groove breakdown) + light audience view

**Files:**
- Create: `godot/scenes/Results.tscn`, `godot/scenes/AudienceView.tscn`, `godot/scripts/{results.gd,audience_view.gd}`
- Modify: `godot/scripts/play_highway.gd` (transition to results on song end)

**Interfaces:**
- Consumes: `JudgeNode` finalize → `ScoreSummary` (total, per-grade counts, mean lean, spread, groove bonus, in-pocket %, streak); `ChartLoader` lyrics + `SongClockNode` for audience view.
- Produces: Results showing the **band-ready metrics made visible** — final score + groove breakdown (mean lean, tightness/spread, groove bonus, in-pocket %, streak) so a tight pocket **provably** out-scores robotic and sloppy on screen. Audience view = the same highway scene minus input/scoring, showing scrolling lyrics + clock (built here so SP2 only routes it).

- [ ] **Step 1: Build Results**

On song end, pull `ScoreSummary`; display total + a breakdown table. Include a dev toggle to replay the same chart as robo/pocket/sloppy (feeding the `judge-replay` vectors) so the score ordering is visible in-app, matching the Phase 6 headless proof.

- [ ] **Step 2: Build the light audience view**

`AudienceView.tscn` reuses the highway scene with input + scoring + pocket meter hidden; renders lyrics (from `ChartLoader`) + song clock only. Reachable from the picker as "Audience (preview)".

- [ ] **Step 3: Manual verification**

Finish a song → Results shows a coherent groove breakdown; the robo/pocket/sloppy toggle shows pocket ≥ robo > sloppy on screen. Audience view scrolls lyrics + clock with no buttons/scoring.

- [ ] **Step 4: Commit**

```bash
git add godot/
git commit -m "feat(ui): results groove breakdown + light audience view"
```

**REVIEW CHECKPOINT 8** — The full minimal playable surface exists. Run a real playtest: does the pocket meter make groove *visible and fun*? Capture tuning changes as adjustments to the TUNABLE constants/exports — none of them is a blocker for structural sign-off.

---

# Phase 9 — Integration & Definition of Done

**Goal:** prove SP1 §13 end-to-end on a desktop target **and iOS**: load a test chart → calibrate → play one instrument through a full song via the minimal UI with highway + audio + haptics → hear your part from your own hits (misses = holes) over auto-performed backing → see groove via the pocket meter → get an individual score that demonstrably rewards a tight pocket over robotic and sloppy. Headless correctness stays green.

### Task 9.1: Full-song desktop integration pass

**Files:**
- Modify: as needed (bug fixes surfaced by end-to-end play)
- Create: `docs/superpowers/plans/sp1-dod-checklist.md`

**Interfaces:**
- Produces: a signed-off DoD checklist mapping each §13 clause to an observed result on desktop.

- [ ] **Step 1: Run the whole loop on desktop**

Fresh launch → calibration → picker → play a full test chart on each of guitar/bass/drums/vocal → results. Verify: audio in time, own-hits sound the part, misses leave holes, backing auto-performs, pocket meter legible, score breakdown coherent.

- [ ] **Step 2: Fix + re-run any breakage**

For each defect use superpowers:systematic-debugging; add a headless test in `band-core` if the bug was in core logic (keep the correctness net tight).

- [ ] **Step 3: Fill the DoD checklist + commit**

Tick each §13 clause with evidence. Commit the checklist.
```bash
git add docs/ rust/ godot/
git commit -m "test: SP1 desktop end-to-end DoD pass + checklist"
```

### Task 9.2: iOS full-song integration pass

**Files:**
- Modify: as needed; `docs/superpowers/plans/notes-ios-audio.md`

**Interfaces:**
- Produces: the same DoD checklist verified on a physical iPhone (the hardest platform).

- [ ] **Step 1: Export + run on device**

Export the Godot iOS build (with the iOS `band-gdext` lib), run on a physical iPhone. Calibrate on-device (real latency), play a full song with touch (5 zones) + haptics.

- [ ] **Step 2: Verify the audio path on iOS**

Confirm sample-accurate scheduling holds with the full sampler load (no dropouts), haptics fire relative to heard audio, misses are audible holes. Record final iOS latency/buffer settings.

- [ ] **Step 3: Fix + re-verify; sign off**

Resolve iOS-specific issues; re-tick the DoD checklist for iOS.
```bash
git add docs/ rust/ godot/
git commit -m "test: SP1 iOS end-to-end DoD pass"
```

### Task 9.3: CI green across targets + harness catalogue

**Files:**
- Modify: `.github/workflows/ci.yml`, `docs/superpowers/plans/sp1-dod-checklist.md`

**Interfaces:**
- Produces: CI building all supported targets "at all times" (core native+wasm tests, gdext macOS+iOS, plus Windows/Linux gdext builds now that those are exercised), and a short catalogue of the headless harnesses (`inspect`, `import`, `hashcheck`, `judge-replay`, `synthspike`, `clickspike`) for future contributors.

- [ ] **Step 1: Extend CI to all desktop targets + full workspace tests**

Add Linux + Windows `cargo build -p band-gdext` jobs; ensure `cargo test --workspace` + the wasm determinism test are required checks.

- [ ] **Step 2: Verify CI is green; document harnesses; commit**

```bash
git add .github/ docs/
git commit -m "ci: all-target builds + headless harness catalogue"
```

**REVIEW CHECKPOINT 9 (FINAL)** — SP1 §13 satisfied on a desktop target **and iOS**, headless correctness green (round-trips, hash determinism incl. native-vs-WASM, hit-windows, groove fairness), and the playtest confirms the pocket meter makes groove fun. If the game is fun here, ~90% of the platform's technical risk is retired and SP2/SP3 can build on frozen contracts.

---

## Self-Review

**1. Spec coverage (SP1 §1–§13 + SP3 amendments):**
- §2 unified mechanic (4 lanes + space, per-instrument space semantics, sustains onset+length, reassignable bindings) → Tasks 5.1–5.3, 8.3.
- §3 chart format (tiny, content-addressed, feel-targets, tiers-capable, minimal importer) → Phase 2, Phase 3. SP3 amendments (canonical binary, integer-µs, core/envelope split, BLAKE3, tiers in core) → Tasks 2.1–2.5.
- §4 song-clock (audio-master, integer song-pos, SP2-ready abstraction) → Tasks 1.1–1.2.
- §5 audio engine (sampler, per-performer, auto-perform, miss-holes, vocal pad) → Phase 4.
- §6 calibration (per-device audio/input/haptic, tap-to-beat, haptic-relative-to-audio) → Phase 7, Task 8.2/8.4.
- §7 game loop & timing windows → Tasks 5.1–5.3, 8.3.
- §8 groove scoring (accuracy + windowed tightness, guardrail, band-ready records, worked example) → Phase 6.
- §9 roles (performer/audience) → Tasks 8.3, 8.6.
- §10 haptics → Task 8.4.
- §12 minimal playable UI (all 6 screens + pocket meter + ambient layer) → Phase 8 Tasks 8.2–8.6.
- §13 DoD → Phase 9.
- De-risk ordering (iOS audio → format → groove fairness) → Phases 0/1, 2, 6; fun → Phase 8.
- Testing posture (headless correctness via Rust tests + CLI harnesses; fun via UI) → harnesses in Tasks 1.3, 3.4, 4.2/4.3, 5.4; tests throughout; fun in Phase 8.

**2. Placeholder scan:** No "TBD"/"add error handling"/"write tests for the above". The one deliberate fill-in is `GOLDEN` in Task 2.5 (derived in Step 1 before the test is finalized — an unavoidable golden-vector bootstrap, with explicit instructions). MIDI/audio/UI tasks that can't be pure-unit-asserted have concrete manual verification steps with specific observations, not vague "make it work".

**3. Type consistency:** `SongPosUs`, `ChartCore`/`Envelope`/`Chart`, `NoteEvent`/`SpaceAction`/`SoundRef`, `encode_core`/`decode_core`/`content_address`, `TimingWindows`/`Grade`, `Judge`/`NoteResult`, `WindowMetrics`/`GrooveConfig`/`score_run`/`GrooveTracker`, `SoundCmd`, `CalibrationOffsets`, and the gdext node names are used consistently across tasks. The single `SongClock` trait is implemented by `ManualClock` (core test), `AudioSongClock` (audio), and named as SP2's future server-clock seam.

**Flagged tunables (playtest numbers, not blockers), all as named constants/exports:** timing windows (25/50/80 ms), pocket guardrail (60 ms), groove window length (8), spread kind (std-dev vs MAD), bonus scaling & accuracy weights, sustain-min (200 ms), default hit gate (120 ms), miss policy (silent vs clank), audio approach/lookahead, pocket-meter lean→x & spread→blur mappings, ambient warm/cold intensity, and iOS buffer/latency defaults.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-07-01-subproject-1-playable-core.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
