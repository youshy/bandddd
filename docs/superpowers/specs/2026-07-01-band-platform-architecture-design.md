# Band Game — Platform Architecture & Vision

**Date:** 2026-07-01
**Status:** Approved (platform-level). Decomposed into sub-projects; each gets its own spec → plan → build cycle.

> Working title: **Band** (repo `bandddd`). A multiplayer, MIDI-native rhythm game where four remote players each perform one instrument (guitar / bass / drums / vocal) on 4 buttons + space, and the game rewards *groove* — collective, locked-in feel — over mechanical grid-perfection.

---

## 1. Product pillars (the soul)

1. **Four humans, four uniquely-played instruments, one performance.** Each player performs a real part; the band's sound is the four of them reassembled. "I'm in the room with the band."
2. **Groove over grid.** "In time" means "in the pocket the *song* defines," not "on the metronome tick." Four players who are collectively, consistently a hair behind the beat should out-score four robots nailing the grid. Reward internal groove and collective coherence.
3. **Party-game-first accessibility.** 5 buttons means anyone can play AC/DC without knowing the instrument. Easy to learn (party floor), hard to master (groove ceiling). Instant onboarding, trivial room-join, social "we nailed it" moment.
4. **Community-first content.** The chart format and tooling are designed for UGC from day one. Bring any song; re-groove any song ("AC/DC with a massive swing feel"). We build the first charts; the community carries it.
5. **We host nothing.** Neutral instrument: we render sound ourselves, we never store or serve chart bytes. Legal shield + viral distribution.

---

## 2. The core technical insight (why "clever" works)

Nobody improvises — every player performs a **known score (the MIDI)**. The song is a **shared, deterministic clock**. Therefore:

- We **do not stream audio** between players. We stream tiny **timing events** ("guitar hit note 47 at song-position 30.412s, +38ms"), tagged to **song-position, not wall-clock**. A late packet can still be placed correctly.
- **Physical law we accept:** you cannot simultaneously have (a) zero-latency perception of your *own* instrument and (b) sample-accurate live timing of *bandmates'* instruments in the same mix. We choose **hybrid**: own instrument + backing render **local and instant**; bandmates render as a slightly-delayed **presence layer**.
- **Groove *strengthens* the netcode instead of fighting it:** groove is measured over a **window** (a bar/phrase), not an instant, so scoring tolerates latent event streams. The shared groove reference lives in the **song data** (groove-annotated chart), so "locked in with the band" = everyone independently locking into the same annotated score, and (measurably, after the fact) correlating with each other.

---

## 3. Multiplayer & distribution model

- **Roles = 4 performers (guitar / bass / drums / vocal) + N audience.** Audience members receive the song clock + scrolling lyrics + the vocalist's button cues; they sing along and do **not** score. Audience is read-mostly (no timing events pushed), so it scales cheaply. The **vocalist is a button-player, not a singer** — singing is a social act the whole room does off the on-screen lyrics; nobody is ever *required* to sing. "Audience input → crowd-energy → band-score boost" is a flagged fast-follow (needs mic/tap; v1 audience = lyrics + presence + vibes). The audience *view* is a first-class part of the core loop (it's the vocalist's lyrics view minus scoring); audience *joining* is a Layer-B mechanic using the same Invite & Join.
- **All players remote at all times.** Co-located parties are handled as "everyone on their own device" (each device renders its own track/groove/feel). Shared-stage "Jackbox" mode (phones-as-controllers, one screen renders the mix out loud) is a **deliberate fast-follow**, not v1 — but the input/session layer is designed so it can be added without re-architecting.
- **Session authority is the server, not a player.** The room creator is only the **content source + room owner**. The **shared clock, event fan-out, and score aggregation live on the server (Supabase)**. Host rage-quits → the song keeps playing (everyone already has the chart + clock); also gives server-side anti-cheat later.
- **Chart bytes travel true peer-to-peer (WebRTC data channel); company infrastructure never carries a chart byte.**
  - Charts are **content-addressed** (hashed); a joiner who already owns that hash skips transfer. Charts spread **virally** — play in a room, now you own the chart, now you can host it.
  - **STUN** (address discovery, zero content) + **Supabase** (signaling + room metadata + presence) are the only company infra in the path — neither ever carries a chart.
  - NAT-stuck peers are served by **peer-assisted relay** (a well-connected bandmate forwards the kilobyte-sized chart) — **no company TURN relay**, preserving the clean "we never touch bytes" line.
- **Discovery = rooms, not a file catalog.** We host room metadata (song hash, public/private), never a searchable library of copyrighted files.

### Invite & Join (Layer B component)
The room is a **Universal/deep link** (opens the app into the room, or the store if not installed — doubles as install funnel). v1 ships:
- **A) OS share sheet** (AirDrop / Messages / …) — iOS-native "in the room" magic, zero custom code.
- **B) QR code** on the host screen — co-located hero, ecosystem-agnostic (iPhone ↔ Android).
- **C) Short human code** — universal verbal fallback.
- **D) Automatic local discovery** (mDNS/BLE) — **deferred to v1.5** (permission/complexity tax not worth it before shipping).

---

## 4. Legal posture (existential — do not drift from this)

- **Neutral tool / BYO model (Clone-Hero-style).** We sell the *game + instruments*; charts and any source material are community-supplied and never hosted by us.
- **Two rights exist; synthesis only dodges one.** Rendering sound ourselves avoids the **master-recording** right, but a chart of a copyrighted song still encodes the **composition**, and audibly performing it is a derivative work. Byte-purity handles distribution/contributory exposure; it does **not** handle **inducement** (Grokster).
- **Marketing discipline is part of the architecture.** Never advertise copyrighted songs ("Play AC/DC!"). Market the *capability* ("bring any song — your library, your rules"). This constraint is accepted and permanent.

---

## 5. Technology stack

- **Engine: Godot 4 + Rust (GDExtension / `gdext`).**
  - **Godot** owns the tedious-on-5-platforms parts: note-highway rendering, UI, input abstraction, and the **export pipeline** (best non-Unity "builds everywhere" story; free/open; no Unity-style fee risk).
  - **Rust** owns the latency- and correctness-critical core: audio scheduling, MIDI parsing, WebRTC transport, groove scoring. On desktop, a **Rust-owned audio thread** (e.g. `cpal` + a soundfont synth like `rustysynth`) keeps the crown-jewel audio path off the engine's mercy.
- **Platforms & ship order:** **iOS first** (existing infra), then **Mac / Windows / Linux**, then **Android (explicit 2nd-class citizen)**. CI builds all supported targets "at all times."
- **Mobile realities baked in:**
  - Mobile is the **worst latency environment** → **per-device A/V + input calibration is core infrastructure from day zero**, not polish. (Our latency-tolerant netcode already accommodates this.)
  - No keyboard → **input is fully abstracted**: keyboard / gamepad / **touch** (5 on-screen buttons, single-instrument UI) / **real MIDI controllers** (signature feature — plug in a drum pad or keyboard and actually play). "Reassignable keys" generalizes to **reassignable bindings per input device**.
- **Backend:** Supabase (auth, users, rooms/metadata, signaling, scores, streaks, leaderboards).
- **Distribution:** Steam (desktop) + iOS/Android stores.

---

## 6. System map

**Layer A — Playable core (single instrument, single device)**
Input abstraction · Chart format + MIDI translation (→ 4-button+space + **groove template**) · Rust audio engine (synth, low-latency out, sample-accurate scheduling) · **Calibration** (A/V + input, day zero) · Note-highway rendering + game loop (timing windows, hit detection) · Individual scoring (accuracy-vs-groove, streak).

**Layer B — Band / multiplayer**
Session model (rooms: create, public/private, join-by-link) · Shared session clock · P2P mesh (chart transfer + timing-event fan-out) · Supabase signaling/metadata/presence · Bandmate presence (see + hear the band) · Band scoring (collective groove, band streak/points) · **Invite & Join** (share sheet + QR + short code).

**Layer C — Content & community**
Chart authoring tool (internal → community) · Viral P2P distribution (rooms are discovery) · Optional metadata index.

**Layer D — Backend & meta**
Supabase schema (auth, users, scores, streaks, leaderboards) · Anti-cheat / score validation · Settings (bindings, calibration, audio, graphics).

**Layer E — Platform / build**
CI building all targets always (iOS → desktop → Android) · Steam + mobile store integration.

### Three cross-cutting contracts to fix early (thin, stable)
1. **The chart format** — the data contract everything reads.
2. **The song-time clock model** — the spine of both audio and netcode (everything times against song-position).
3. **The input abstraction** — one event stream, many devices.

---

## 7. Build order & rationale

The hardest *unproven* risks all live in **Layer A**: low-latency sample-accurate audio (esp. iOS); **whether "groove" can be represented and scored so it's fun and fair** (the novel soul); whether MIDI auto-translates into fun charts. Layers B–E are comparatively conventional engineering on top.

1. **Sub-project 1 — Playable Core** (single instrument, single device): chart format + clock + audio engine + calibration + one game loop + groove scoring. *If this isn't fun, nothing downstream matters.* De-risks ~90% of the technical unknowns.
2. **Sub-project 2 — Band / multiplayer** (Layer B).
3. **Sub-project 3 — Community authoring & distribution** (Layer C).
4. Backend/meta (D) and platform/CI (E) grow alongside from the start.

Each sub-project gets its own brainstorm → spec → plan → implementation cycle.

---

## 8. Open questions carried into sub-project specs

- **Chart/groove format:** how is "groove" represented (per-note micro-timing offsets? swing/feel templates? push-pull curves?) and how does MIDI reduction (chords/solos → 4 buttons) stay fun and fair per instrument?
- **Audio synthesis:** soundfont vs sampled instrument sets; the **vocal problem** (button-triggered vocals sound robotic with samples — synth voice? stylized instrument-representation of the vocal line? accept it?).
- **Clock/sync:** server time-sync method; jitter-buffer / look-ahead sizing for the bandmate presence layer.
- **Event transport:** do timing events ride the P2P mesh, Supabase Realtime, or both (mesh for speed, Supabase for persistence/anti-cheat)?
- **Scoring math:** individual accuracy-vs-groove; internal-groove reward; collective-coherence reward; individual + band streaks and points.
- **Anti-cheat:** client-computed vs server-validated scores for leaderboards.
