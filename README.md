# bandddd

**bandddd** — *Your band. In your pocket. Right now.*

Four friends, four instruments — guitar, bass, drums, or vocals — playing one real song **together**, live, each on the **phone already in their hand**. No console, no gear, no lessons: someone taps a link and ten seconds later the whole band is shredding AC/DC from four different couches. If you can tap along to a song, you can play it.

The magic isn't hitting notes like a robot — it's **locking in as a band**. When the four of you ride just behind the beat and breathe as one, the song *comes alive* and your score climbs; drift apart and it falls apart with you. You don't win by being perfect. You win by being **tight**. And it's real music: every note you play actually sounds, so miss one and you hear the hole — it rewards *feel*, the thing real players live for.

Bring any song — your library, your rules. Start a room, drop the link in the group chat, and the whole band is playing in seconds.

> **Four humans, four uniquely-played instruments, one performance.** "I'm in the room with the band."

---

## What it is

A multiplayer, MIDI-native rhythm game where **four remote players each perform one instrument** — guitar, bass, drums, or vocal — on **4 buttons + space**, and the game rewards **groove** (collective, locked-in *feel*) over mechanical grid-perfection.

## The core idea (the clever bit)

Nobody improvises — every player performs a **known score (the MIDI)**, so the song is a **shared, deterministic clock**. We don't stream audio between players; we stream tiny **timing events** tagged to *song-position*. Each player's own instrument + backing render **locally and instantly**; bandmates render as a slightly-delayed **presence layer**. Because **groove is measured over a window** (a bar), scoring tolerates network latency *by design*. "Near-zero latency" is real for what you feel, and clever for what you hear.

## Design pillars

- **Party-game-first.** Five buttons means anyone can play AC/DC without knowing the instrument. The **Inebriated-User Test** governs every decision: *if a mechanic needs a tutorial, it's wrong.*
- **Groove over grid.** "In time" means "in the pocket the *song* defines," not "on the metronome tick." Four players consistently a hair behind the beat out-score four robots nailing the grid.
- **Band > everything.** The band is the unit of success; individual play is in service of it.
- **Community-first content.** The chart format and tooling are built for user-generated content from day one. Bring any song; re-groove any song ("AC/DC with a massive swing feel").
- **We host nothing.** We render sound ourselves and never store or serve chart files — a neutral instrument. Legal shield *and* viral distribution.

---

## 📖 Start here — documentation map

New to the project? Read in this order:

1. **[Platform Architecture & Vision](./docs/superpowers/specs/2026-07-01-band-platform-architecture-design.md)** — the north star. Product pillars, locked constraints, and how the whole thing decomposes. **Read this first.**
2. **[Sub-project 1 — Playable Core](./docs/superpowers/specs/2026-07-01-subproject-1-playable-core-design.md)** — one player, one instrument, one device, fully playable. De-risks ~90% of the hard tech.
3. **[Sub-project 2 — Band / Multiplayer](./docs/superpowers/specs/2026-07-01-subproject-2-band-multiplayer-design.md)** — four remote players in one performance. The whole point of the game.
4. **[Sub-project 3 — Community Authoring & Distribution](./docs/superpowers/specs/2026-07-01-subproject-3-community-authoring-design.md)** — MIDI→chart translation, authoring, and viral "we host nothing" distribution.

Each sub-project gets its own **brainstorm → spec → plan → implementation** cycle. The specs are the source of truth for detail; this README only orients.

## Build order & status

| # | Sub-project | In one line | Status |
|---|-------------|-------------|--------|
| 1 | [Playable Core](./docs/superpowers/specs/2026-07-01-subproject-1-playable-core-design.md) | Solo, single-device, fully playable — proves the fun and kills the hard tech (sample-accurate audio, MIDI→chart, groove scoring). | **In progress** — Phase 2 done (see below) |
| 2 | [Band / Multiplayer](./docs/superpowers/specs/2026-07-01-subproject-2-band-multiplayer-design.md) | Rooms, join-by-link, server-synced clock, presence layer, band-level groove + coherence scoring. | Spec written |
| 3 | [Community Authoring & Distribution](./docs/superpowers/specs/2026-07-01-subproject-3-community-authoring-design.md) | MIDI→chart translation + authoring tool, deterministic hashing, re-grooving, viral content-addressed distribution. | Spec written |

**Cross-cutting (grown from the start, not standalone phases):** Supabase backend (auth, scores, streaks, leaderboards), anti-cheat / score validation, settings (bindings, calibration, audio, graphics); CI building all targets continuously; Steam + mobile store integration.

### Implementation progress (SP1, branch `sp1-playable-core`)

Executed task-by-task (TDD → review gate) against the [SP1 plan](./docs/superpowers/plans/2026-07-01-subproject-1-playable-core.md).

- **Phase 0–1 (partial):** Rust workspace + `band-core` (pure/headless, wasm-clean); `SongClock` trait + `ManualClock`; `band-audio` with a sample-accurate `AudioSongClock`. Godot/gdext, iOS export, and CI are **deferred** (need hardware/GUI) and tracked as restart prompts.
- **Phase 2 — Chart format + hashing ✅ complete.** Deterministic canonical **binary** chart codec (version byte, fixed field order, LEB128/zig-zag varints), core/envelope split, integer-µs times, and a **total/panic-free** decoder. Content-address = **version-namespaced BLAKE3** — `BLAKE3(FORMAT_VERSION_byte ++ core-bytes)`, lowercase hex, over the hashed core only. **Byte-identical native-vs-WASM is *proven***, not just designed: an executed `wasm-pack test --node` run verifies the same golden content-address under WASM as native.
- **Phase 3 — Minimal MIDI importer ✅ complete.** `.mid` → single-tier **believable test charts**: SMF parse with integer tick→µs tempo mapping (`midly`), melodic reduction (pitch-band lanes + chord-merge + sustains) and drum reduction (GM kit → lanes, kick on space), assembled by `import_chart` into a content-addressed `.band`. Feel-target onset times are preserved verbatim from the source MIDI. Three CLI harnesses in `band-cli` (`import`, `hashcheck`, `inspect`); **whole-pipeline determinism proven** — the same `.mid` yields the same 64-hex address across repeated runs. A seeded chart lives at [`godot/assets/test_charts/`](./godot/assets/test_charts).
- **Next:** Phase 4 — full audio engine (needs real audio/hardware; headless-buildable parts execute here, playtest parts deferred as restart prompts).

### Dev setup

The correctness core is a Rust workspace under [`rust/`](./rust). To build & test it:

```bash
cd rust
cargo test --workspace                                   # all headless tests
cargo build -p band-core --target wasm32-unknown-unknown  # band-core stays wasm-clean
```

Requires: **Rust stable** with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`). The cross-target hash-determinism test additionally needs **Node.js** and **wasm-pack** (`cargo install wasm-pack --locked`), then:

```bash
cd rust && wasm-pack test --node crates/band-core --test hash_determinism
```

The `band-cli` crate provides the MIDI-import tooling (native-only, no extra system deps — `midly` is fetched by cargo):

```bash
cd rust
# import a .mid → content-addressed .band (role flags pick source track indices)
cargo run -p band-cli --bin import -- in.mid out.band --guitar 0 --drums 1
cargo run -p band-cli --bin hashcheck -- out.band   # prints the 64-hex content-address
cargo run -p band-cli --bin inspect   -- out.band   # prints a JSON debug projection (NOT hashed)
# regenerate the seeded test-chart MIDI fixture from source:
cargo run -p band-core --example gen_test_groove
```

Godot 4 + `gdext`, real audio, and iOS export are needed only for the not-yet-started GUI/hardware phases.

---

## v1 scope

**In:**
- Sub-projects 1, 2, and 3 in full — **per-device remote** play (each player on their own device)
- Invite & Join: **OS share sheet + QR + short human code**
- Roles: 4 performers + audience (audience = lyrics + presence, no scoring)
- Instruments: guitar / bass / drums / vocal, one sampled tone-kit each; vocal = melodic voice + on-screen lyrics (the human sings in the room)
- Input: keyboard / gamepad / touch / **real MIDI controller**, fully rebindable per device
- Groove scoring at both individual and band level
- Community chart format + authoring/sharing (full authoring tool may trail into a 3.x)

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
- **Networking:** WebRTC P2P mesh + STUN; Supabase for signaling / metadata only (**never carries chart bytes**).
- **Distribution:** Steam (desktop) + iOS / Android stores.

---

## Constraints (permanent — read before you design anything)

- **The Inebriated-User Test.** If a player-facing mechanic needs a tutorial, it's wrong. Everything assumes a drunk player at a party. (Authoring may be richer, but the light-remix surface every player touches must still pass.)
- **Legal posture — neutral tool, bring-your-own.** We render sound ourselves (dodging **master-recording** rights) and host no chart bytes. The **composition** right still exists, and **inducement liability is independent of the byte path** — so **marketing never advertises copyrighted songs**; it advertises the *capability* ("bring any song — your library, your rules"). Accepted and permanent.
