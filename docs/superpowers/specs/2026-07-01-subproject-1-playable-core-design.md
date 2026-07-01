# Sub-project 1 — The Playable Core (design spec)

**Date:** 2026-07-01
**Parent:** [Band — Platform Architecture & Vision](./2026-07-01-band-platform-architecture-design.md)
**Status:** Draft for review.

> **Goal:** one player, one instrument, one device — fully playable and *fun*. This slice proves the game and kills the biggest technical unknowns: low-latency sample-accurate audio, MIDI→fun-chart translation, and whether **groove** can be represented and scored so it feels good. If this isn't fun, nothing downstream matters.

**Governing constraint — the Inebriated-User Test:** if a mechanic needs a tutorial, it's wrong. Everything below assumes a drunk player at a party.

---

## 1. Scope

**In:**
- Unified instrument mechanic (4 lanes + space) for all four instruments, single instrument at a time.
- Chart format (the data contract) + a minimal chart importer to produce test charts.
- Song-clock model (local, audio-driven).
- Rust audio engine: sampled instrument sets + melodic vocal voice, low-latency output, sample-accurate scheduling, per-performer sounding + miss-degradation.
- Calibration (audio / input / haptic), day-zero.
- Note-highway rendering + game loop (reading, hit detection, sustains).
- Individual groove scoring (accuracy vs feel-target + windowed tightness), emitting **band-ready metrics**.
- Role-aware rendering: performer view vs **audience view** (lyrics + clock, no scoring).
- Mobile haptics (per-action).

**Out (later sub-projects):**
- Multiplayer, rooms, P2P mesh, shared server clock, bandmate presence, band scoring (Layer B).
- Full authoring/translation tool and community distribution (Layer C).
- Crowd-energy-as-band-boost, vowel-sample vocal upgrade, multiple tones/kits, local auto-discovery.

---

## 2. The instrument mechanic (Decision #1 — locked)

**One reading model for all four instruments:** notes scroll down **4 fixed lanes** (zero musical semantics — a lane is just a lane), and the player acts in time. Learn to read one, you can sit at any of the four. Space's role varies:

| Instrument | 4 lanes = | Space (tap) = | Space (hold) = |
|---|---|---|---|
| **Guitar** | note/chord lanes | **strum** — fires held lane(s) | **let ring** — sustain (length check) |
| **Bass** | same as guitar | **strum** | **let ring** — sustain (length check) |
| **Drums** | hi-hat / snare / tom / cymbal — each lane fires *directly* | **kick** (independent 5th voice) | *(no sustain — kick is transient)* |
| **Vocal** | pitch lanes (low→high) — each fires the charted note | trigger note | **sustain** — hold to the note's charted length (length check) |

- **Guitar/bass:** hold the lane(s), tap space to strum — "hold the chord, tap in rhythm." Re-strum a held chord without re-fretting. Easier drunk than re-tapping lanes each strum.
- **Sustains** (guitar ring, bass ring, vocal hold) are scored on **two axes: onset timing + hold length.**
- **Bindings are reassignable per input device** (keyboard / gamepad / touch / real MIDI controller). Default touch layout = 5 large on-screen zones (4 lanes + space).

---

## 3. Chart format (the data contract)

Everything reads this. Kept **tiny** (kilobytes — no audio; sample sets ship with the game) and **content-addressed** (hash of canonical serialization → the id used for dedupe/sharing later).

> **Implemented (Phase 2, 2026-07-01):** canonical **binary** codec + **version-namespaced BLAKE3** content-address — `BLAKE3(FORMAT_VERSION_byte ++ core-bytes)`, hashed core only (envelope excluded), integer-µs, panic-free decoder — **proven byte-identical native-vs-WASM** (executed `wasm-pack test --node` vs. a frozen golden vector). See §11 and the SP1 plan.

**Groove is not a separate layer — groove *is* where the notes sit.** Note times are the *feel-targets*: each note is placed exactly where it should be felt (swing/push/lay-back already baked in), **not** quantized to a grid. Re-grooving a song = a transform on note times.

Conceptual structure:
- **Header:** title, artist, chart author, version, duration, tempo/beat map (for the highway's bar lines & scroll), difficulty, referenced instrument-set ids, content hash.
- **Per instrument track** (guitar / bass / drums / vocal): ordered note events —
  - `songPos` — feel-target time (groove baked in)
  - `lanes` — which lane(s); for drums, the drum voice; `space` action flag (strum / kick / trigger / none)
  - `sustainLen` — 0 for a hit; >0 for a ring/hold (drives the length check)
  - `sound` — sample/patch + pitch + velocity to sound when this note is performed
- **Lyrics track:** timed syllables/lines (drives vocalist + audience views).
- **Optional groove metadata:** per-section intended feel direction (enables the later "pocket-direction" bonus). Optional; absence just disables that bonus.

**Minimal importer (this sub-project):** enough of a MIDI→chart translator to generate real test charts (reduce a track to lanes + space + sustains, carry note times through as feel-targets). The *full, fair, tunable* translator + authoring tool is Layer C — here we only need believable test content.

---

## 4. Song-clock model

- **The audio device's playback clock is the master.** Song-position is derived from the audio engine's sample-accurate playhead, never from wall-clock or frame time.
- All gameplay (note spawn, hit windows, scoring) times against **song-position**.
- **Calibration offsets** (Section 6) are applied so the *perceived* audio/visual/haptic events line up with the player's actions.
- This is deliberately the same abstraction multiplayer will need — Layer B swaps the local master for a server-synced session clock; nothing above the clock changes.

---

## 5. Audio engine (Decision #3 — locked)

Rust-owned audio thread (e.g. `cpal` output + a soundfont/sampler synth). **Sample sets ship with the game.**

- **Guitar / bass / drums:** sampled instrument sets (one solid tone/kit each for v1; velocity layers).
- **Vocals:** a **stylized melodic voice** (expressive vowel-pad / "aah" lead) carries the melody + pitch; **lyrics scroll on-screen**; the human sings in the room. No singing-synthesis. (Vowel-sample upgrade is later.)

**Sounding model (extends cleanly to multiplayer):**
- **Each instrument is sounded by *that performer's actual hit events*** (their real timing).
- **Unmanned instruments auto-perform at chart-ideal** — so a solo player hears the full song with their one part played by them. (In a band, all four are human; empty slots auto-fill.)
- **Miss = audible hole:** a missed note mutes/drops that part for the note (optionally a subtle clank). "The band falters when you fail." This is the coarse audio-degradation feedback that stays latency-immune.

---

## 6. Calibration (day-zero infrastructure, not polish)

Mobile is the worst latency environment, so this ships from the first build.

- Measure and store **per-device** offsets for: **audio output latency, input latency, haptic latency.**
- **Method:** standard tap-to-the-beat calibration (audio + visual metronome; player taps; derive offset), with automated loopback where the platform allows.
- Offsets are applied when aligning feel-targets to real output, and when firing haptics (haptics fire relative to *audio*, not to the raw input event — a mistimed buzz lies about timing).
- Re-calibration is reachable from settings and suggested on first run / device change.

---

## 7. Game loop & rendering

Godot owns rendering/UI/input; Rust owns audio/scoring/timing.

1. **Read:** notes scroll down 4 lanes toward a hit line; approach time (lookahead) configurable. Bar lines from the tempo map.
2. **Input:** device-agnostic events (keyboard/gamepad/touch/MIDI) → abstract lane/space stream.
3. **Hit detection:** match an action to the nearest eligible note in the pressed lane within the timing window; compute **signed offset** (early −, late +) against the note's feel-target. Sustains track hold start→release for the length check.
4. **Feedback:** visual (hit/miss/streak), **audio** (per Section 5), **haptic** (per-action; calibrated).
5. **Score update:** per Section 8.

**Proposed timing windows** (relative to the calibrated feel-target; tunable in playtest):
- Perfect ±25 ms · Good ±50 ms · Hit (pocket edge) ±80 ms · Miss beyond.
- These center on the *feel* target (groove), never a metronome tick.

---

## 8. Groove scoring (Decision #2 — locked)

Two components, anchored on the **song's pocket** (shared reference), with the reward coming from **tightness around it**. Band is the supreme unit — so every metric here is emitted as **band-ready raw material** for Layer B to aggregate into coherence.

**(a) Accuracy — per note.** Grade from `|offset|` against the feel-target (Perfect/Good/Hit/Miss windows above). Sustains additionally graded on **hold-length error**.

**(b) The groove reward — windowed** (rolling window, e.g. last bar / last ~8 notes):
- Compute the player's **mean offset** (their pocket lean) and **offset spread / std-dev** (their tightness).
- **Reward low spread** — being *locked in*. A tight, consistent lean (e.g. reliably +13 ms) scores **higher** than scatter with the same average error.
- **Pocket-zone guardrail:** the tightness bonus only applies while `|mean offset|` is within a bounded pocket (proposed ≤ ~60 ms, tunable). Tight-but-far-off is still an error, not groove — this stops "consistently lazy" from being rewarded.
- **Optional pocket-direction bonus** (needs the optional groove metadata): extra credit when your lean matches the song's intended feel direction. Ship-optional.

**Worked example** (feel-targets 0/500/1000/1500 ms):
- *Robo* 0/500/1000/1500 → perfect accuracy, no groove bonus (zero spread but zero lean — fine, not penalized).
- *Pocket* 14/512/1013/1515 → ~+13 ms, tiny spread → full groove bonus; can edge Robo if the song wants that lay-back.
- *Sloppy* 30/480/1020/1470 → avg ~0 but wide spread → no groove bonus; beaten by Pocket.

**Per-note band-ready record** emitted for every note: `{ songPos, targetTime, actualTime, signedOffset, grade, sustainLenError }`. Windowed outputs: `{ meanOffset, spread, grooveBonus, inPocket }`. Streak = consecutive non-Miss notes (individual; band streak composes later).

---

## 9. Role model (performer vs audience)

- **Performer view:** the note highway + scoring for one instrument. Vocalist additionally shows scrolling **lyrics**.
- **Audience view:** the vocalist's lyrics + song clock **minus** buttons/scoring — same rendering artifact, different privilege. Built here so multiplayer only has to *route* it to joiners.
- Audience is **non-scoring, read-mostly**; crowd-energy-as-band-boost is explicitly deferred.

---

## 10. Mobile haptics

- **Per-action:** sharp tap = strum/hit; heavier thud = kick; gentle continuous buzz during a sustain (fades if the player drifts off the note).
- iOS Core Haptics (rich); Android coarser (2nd-class).
- **Fired relative to audio via the calibration offset** — never off the raw input event.

---

## 11. Open questions to resolve during implementation

- ~~Exact canonical serialization format for the chart (binary vs JSON) and hashing scheme — must be deterministic for later content-addressing.~~ **RESOLVED (Phase 2, 2026-07-01):** deterministic canonical **binary** (version byte, fixed field order, length-prefixed sections, LEB128/zig-zag varints; never JSON), integer-µs times, panic-free/bounds-checked decoder. Content-address = **version-namespaced BLAKE3** — `BLAKE3(FORMAT_VERSION_byte ++ canonical-core-bytes)`, lowercase hex, over the hashed core only (envelope excluded). **Proven byte-identical native and under WASM** by an executed `wasm-pack test --node` run against a frozen golden vector. See the SP1 plan Phase 2 / Review Checkpoint 2.
- Sampler choice (`rustysynth`/soundfont vs a purpose-built sampler) and how the Rust audio thread cooperates with Godot's audio server on iOS specifically.
- Final timing-window and pocket-zone numbers (playtest-driven).
- Exact windowed-groove math (std-dev vs mean-absolute-deviation; window length; how bonus scales) — needs feel tuning.
- Miss feedback: silent drop vs subtle clank (playtest).
- Minimal-importer scope: which MIDI conventions it understands to produce believable test charts.
- **Pocket-meter visual tuning:** exact mapping of lean→position and spread→marker-sharpness; how strong the ambient warm/cold feel layer should be before it distracts (playtest).

---

## 12. Minimal playable UI (for playtesting — functional, not polished)

**Purpose:** the core's *correctness* is validated headless (Rust unit/integration tests + small CLI/debug harnesses over charts → assert hit-windows, scores, hashes, round-trips). But the core's *fun* cannot be unit-tested — "if this isn't fun, nothing downstream matters." So SP1 ships a **minimal, programmer-art, functionally-complete** playable surface, iterated in playtest. This is explicitly **not** the polished visual-design language (that's a post-fun pass); it is the smallest surface that lets us *feel* the game.

**Governing constraint:** the *play* surface must pass the Inebriated-User Test even while the dev chrome around it does not.

**Screens (the minimum to feel the game):**
1. **Test-chart picker** (dev-facing, bare list) — choose a local test chart + instrument (guitar/bass/drums/vocal) + difficulty tier, then play. No polish.
2. **Calibration** (§6) — tap-to-the-beat; shows measured audio/input/haptic offsets; first-run gated, re-runnable from settings.
3. **Play screen / note highway (the heart)** — 4 lanes + a space zone; notes scroll to a hit line; bar lines from the tempo map; sustains shown as tails (length check); per-instrument space semantics shown (strum bar / kick zone / sustain). Touch = 5 large zones (§2 default); desktop = keys; gamepad / real MIDI mapped via the input abstraction.
4. **Real-time feedback** — per-hit judgment flash (Perfect / Good / Hit / Miss, §7) with an early/late indicator; streak counter; miss = visual gap paired with the audible hole (§5).
5. **Results** — final score + **groove breakdown** (mean lean, tightness/spread, groove bonus, in-pocket %, streak) — the band-ready metrics (§8) made visible so we can *prove* a tight pocket beats both robotic-perfect and sloppy play.
6. **Audience view (light)** — the same highway scene minus input/scoring, lyrics + clock (§9); built here so SP2 only has to route it.

**The pocket meter (the one novel, essential element).** Every rhythm game shows note-accuracy; almost none show *groove*. If the UI only shows Perfect/Good/Miss, the groove-over-grid reward is invisible and therefore untestable. Two-part treatment:
- **Dev/playtest instrument (primary):** a horizontal **"pocket lane"** — a marker for the player's rolling **mean lean** (ahead ← → behind) inside a marked **pocket zone** (§8 guardrail), where the marker's **sharpness vs. smear encodes tightness/spread** (locked-in = crisp dot, sloppy = wide blur). Glanceable, tunable, drunk-proof.
- **Ambient feel layer:** the highway subtly **warms/glows when tight-and-in-pocket** and goes cold/desaturated when the player scatters or drifts out of the pocket — feelable with zero reading (Inebriated-User Test). Precise numbers live on the Results screen only.

**Explicitly deferred:** the full app UX / visual-design language, menus/settings chrome beyond the above, and any theming — a dedicated pass *after* the core proves fun (so we design against a mechanic we've actually felt).

**Engine split:** Godot owns these scenes/rendering/input; the Rust core supplies scoring/timing/audio and the per-note + windowed metrics the feedback and pocket meter render. The UI reads the core's outputs; it never computes groove itself.

---

## 13. Definition of done (this sub-project)

A single player, on at least one desktop target **and iOS**, can: load a test chart, calibrate their device, play one instrument through a full song **via the minimal playable UI (§12)** with the note highway + audio + haptics, hear their part sounded from their own hits (with misses leaving audible holes) over an auto-performed backing, **see their groove made visible via the pocket meter**, and receive an individual score that **demonstrably rewards a tight pocket over both robotic-perfect and sloppy play** — with all per-note/windowed metrics emitted in band-ready form. Core correctness is additionally covered by **headless Rust tests + debug harnesses** independent of the UI.
