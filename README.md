# bandddd

*Working title.* A multiplayer, MIDI-native rhythm game where **four remote players each perform one instrument** — guitar, bass, drums, or vocal — on **4 buttons + space**, and the game rewards **groove** (collective, locked-in feel) over mechanical grid-perfection.

> **Four humans, four uniquely-played instruments, one performance.** "I'm in the room with the band."

---

## The pitch

- **Party-game-first.** Five buttons means anyone can play AC/DC without knowing the instrument. Easy to learn, hard to master. The **Inebriated-User Test** governs every design decision: if a mechanic needs a tutorial, it's wrong.
- **Groove over grid.** "In time" means "in the pocket the *song* defines," not "on the metronome tick." Four players who are collectively, consistently a hair behind the beat out-score four robots nailing the grid.
- **Band > everything.** The band is the unit of success; individual play is in service of it.
- **Community-first content.** The chart format and tooling are built for user-generated content from day one. Bring any song; re-groove any song ("AC/DC with a massive swing feel").
- **We host nothing.** We render sound ourselves and never store or serve chart files — a neutral instrument. Legal shield + viral distribution.

## How it works (the clever bit)

Nobody improvises — every player performs a **known score (the MIDI)**, so the song is a **shared, deterministic clock**. We don't stream audio between players; we stream tiny **timing events** tagged to *song-position*. Each player's own instrument + backing render **local and instant**; bandmates render as a slightly-delayed **presence layer**. Because **groove is measured over a window** (a bar), scoring tolerates network latency by design. "Near-zero latency" is real for what you feel, clever for what you hear.

---

## Design docs

- [Platform Architecture & Vision](./docs/superpowers/specs/2026-07-01-band-platform-architecture-design.md) — the north star.
- [Sub-project 1 — Playable Core](./docs/superpowers/specs/2026-07-01-subproject-1-playable-core-design.md) — first buildable slice.

---

## Sub-projects (build order)

Each gets its own **brainstorm → spec → plan → implementation** cycle.

### 1. Playable Core — *spec written*
One player, one instrument, one device, fully playable. De-risks ~90% of the hard tech.
- Unified mechanic: 4 lanes + space, one reading model for all four instruments (sustains = length checks)
- Chart format (tiny, content-addressed; **groove = where the notes sit**) + minimal MIDI importer for test charts
- Song-clock model (audio-driven, sample-accurate)
- Rust audio engine: sampled instrument sets + melodic vocal voice; each part sounded from the performer's real hits; **misses leave audible holes**
- Day-zero calibration (audio / input / haptic latency, per device)
- Note-highway rendering + game loop (proposed windows: Perfect ±25 / Good ±50 / Hit ±80 ms, tunable)
- Individual **groove scoring**: accuracy vs feel-target + windowed tightness inside a pocket zone; emits **band-ready metrics**
- Role-aware views: performer vs **audience** (lyrics + clock, no scoring)
- Mobile haptics (per-action, calibrated to audio)

### 2. Band / Multiplayer — *not yet designed*
The whole point of the game.
- Session model: rooms (create, public / private, join-by-link)
- **Invite & Join:** OS share sheet (AirDrop / Messages) + QR code + short human code
- Server-synced session clock; server is session authority (host leaving doesn't kill the song)
- P2P mesh (WebRTC): content-addressed chart transfer + peer-assisted relay + timing-event fan-out
- Supabase as signaling + room metadata + presence broker (**never carries chart bytes**)
- Bandmate **presence layer** — seeing + hearing the whole band
- **Band scoring:** collective groove + coherence, band streak, band points
- Audience **joining** (read-mostly, scales cheaply)

### 3. Community Authoring & Distribution — *not yet designed*
- Full MIDI→chart **translation + authoring tool** (fair, tunable, re-groovable per instrument)
- Viral content-addressed distribution (rooms *are* discovery)
- Optional metadata index

### Cross-cutting (grown from the start, not standalone phases)
- **Backend & meta:** Supabase schema (auth, users, scores, streaks, leaderboards), anti-cheat / score validation, settings (bindings, calibration, audio, graphics)
- **Platform & build:** CI building all targets "at all times"; Steam + mobile store integration

---

## v1 scope

**In:**
- Sub-project 1 (Playable Core) in full
- Sub-project 2 (Band / Multiplayer) in full — **per-device remote** play (each player on their own device)
- Invite & Join: **share sheet + QR + short code**
- Roles: 4 performers + audience (audience = lyrics + presence)
- Instruments: guitar / bass / drums / vocal, one sampled tone-kit each; vocal = melodic voice + on-screen lyrics (the human sings in the room)
- Input: keyboard / gamepad / touch / **real MIDI controller**, fully rebindable per device
- Groove scoring at both individual and band level
- Community chart *format* + basic import/sharing (full authoring tool may trail into a 3.x)

**Out (deliberate fast-follows, not v1):**
- Shared-stage "Jackbox" party mode (phones-as-controllers, one screen renders the mix out loud)
- Crowd-energy → band-score boost (needs mic / tap)
- Local auto-discovery (mDNS / BLE)
- Vowel-sample vocal upgrade; multiple tones / kits per instrument

---

## Tech stack

- **Engine:** Godot 4 + **Rust** (GDExtension / `gdext`). Godot owns rendering / UI / input / cross-platform export; Rust owns the latency- and correctness-critical core (audio scheduling, MIDI parsing, WebRTC transport, groove scoring). Desktop runs a **Rust-owned audio thread** (`cpal` + soundfont/sampler).
- **Platforms & ship order:** **iOS first**, then **Mac / Windows / Linux**, then **Android (explicit 2nd-class citizen)**. CI builds all supported targets continuously.
- **Backend:** Supabase (auth, rooms/metadata, signaling, scores, streaks, leaderboards).
- **Networking:** WebRTC P2P mesh + STUN; Supabase for signaling/metadata only.
- **Distribution:** Steam (desktop) + iOS / Android stores.

---

## Legal posture (permanent constraint)

Neutral tool / bring-your-own model. We render sound ourselves (dodging **master-recording** rights) and host no chart bytes. The **composition** right still exists, and **inducement liability is independent of the byte path** — so **marketing never advertises copyrighted songs**; it advertises the *capability* ("bring any song — your library, your rules"). This is accepted and permanent.
