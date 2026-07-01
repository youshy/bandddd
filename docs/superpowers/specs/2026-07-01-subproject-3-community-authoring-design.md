# Sub-project 3 — Community Authoring & Distribution (design spec)

**Date:** 2026-07-01
**Parent:** [Band — Platform Architecture & Vision](./2026-07-01-band-platform-architecture-design.md)
**Builds on:** [Sub-project 1 — The Playable Core](./2026-07-01-subproject-1-playable-core-design.md) · [Sub-project 2 — Band / Multiplayer](./2026-07-01-subproject-2-band-multiplayer-design.md)
**Status:** Draft for review.

> **Goal:** turn "bring any song, re-groove any song" from a promise into a pipeline. Give authors a way to translate arbitrary MIDI into fair, fun, re-groovable 4-lanes-+-space charts; give every player a way to remix, own, share, and virally spread charts — while company infrastructure still never stores or serves a chart byte, and marketing/discovery never surfaces a copyrighted title. This is Layer C.

**Governing constraint — the Inebriated-User Test:** if a *player-facing* mechanic needs a tutorial, it's wrong. (Authoring is a producer activity and may be richer — but the light-remix surface every player touches must pass the test.)

---

## 0. Locked constraints carried in (not re-litigated)

- **We host NOTHING.** Charts and source material are community-supplied; company infra never stores or serves a chart byte or a source byte. Neutral BYO/Clone-Hero model.
- **Two rights, one dodge.** Synthesis dodges only the master-recording right; the **composition right** and **inducement liability** remain. Marketing/discovery must **never** advertise or surface copyrighted songs by name. Existential and permanent.
- **Charts are content-addressed;** the id is the hash of the canonical serialization. Discovery is by **room**, not a searchable file catalog.
- **Groove IS the note placement** (feel-targets). Re-grooving = a transform on note times. Re-groovable per instrument is a first-class goal.
- **Stack:** Godot 4 + Rust (`gdext`). Rust owns MIDI parsing + chart serialization/hashing/transforms. iOS-first → desktop → Android.
- **The canonical serialization + hashing scheme** was flagged as an SP1 open question; SP3 pins it down deterministically (§3).

---

## 1. Scope

**In:**
- Full **MIDI→chart translation** model (arbitrary polyphony/chords/solos → 4 lanes + space per instrument), fair and tunable per instrument (§2).
- **Canonical serialization + deterministic hashing** contract; core/envelope split (§3).
- **Authoring surfaces:** in-game editor (main) + open-access WASM web app (add-on) + in-game light-remix; one shared Rust core; MIDI import + web→game handoff (§4).
- **Re-grooving** as a first-class deterministic transform (§5).
- **Difficulty tiers** generated from one source chart (§6).
- **Distribution graph** beyond a single room: local library, viral re-hosting, publish-as-social-gesture (§7).
- **Opt-in metadata index** — what it may and may not contain (§8).
- **Anti-abuse / trust:** malformed/malicious charts, versioning, attribution, self-sovereign identity, moderation posture (§9).
- **Identity, accounts, analytics:** no-login model, PostHog anonymous analytics, Supabase touchpoints (§10, §11).
- **On-device library management + share/import flow** (§12).

**Out (later / fast-follows):**
- Non-MIDI import formats (MusicXML, audio-to-MIDI). MIDI only for v1.
- Humanize/random-jitter re-groove primitive (fights tight-pocket scoring; deferred).
- WebRTC-paired web→game transfer (deferred escape hatch; §4).
- Nick/label/index content moderation *policy* (hooks present; policy is a dedicated future discussion; §9).
- Cloud sync / backup of the local library beyond OS device backup.
- Runtime difficulty reduction (rejected in favor of baked tiers; §6).
- Global peer-seeding overlay / DHT (rejected; §7).

---

## 2. MIDI→chart translation (Decision — locked: envelope + per-instrument salience, effort-weighted)

The creative heart: squash arbitrary MIDI into 4 lanes + space per instrument so it stays **fun** (Inebriated-User Test) and **fair** (no instrument unplayably dense or boringly sparse at a given difficulty). Rejected alternatives: *fidelity-first* (solos become key-mashing walls, sparse parts bore — fails fun and fairness).

**(a) Playability envelope (fairness).** Each difficulty tier defines a **per-instrument-role envelope**: a target/max **effort rate**, max simultaneous lanes, and minimum gap between actionable events. Fairness is measured in **effort, not raw note count** — simultaneous lanes, strums, and sustains are weighted as more costly than sequential taps, because 4 lanes at once is far harder than 4 taps in a row. Envelopes are role-tuned (drums may run denser than vocal).

**(b) Per-instrument salience/mapping (keeps each part feeling like that instrument).** The envelope caps *how much*; a per-instrument model decides *what to keep and how to lane it*:
- **Guitar / bass:** collapse chord voicings to a small set of stable lane-shapes (chord = held lane-combo; single notes = pitch-banded to a lane); space = strum. Bass biases toward root/rhythm.
- **Drums:** kit-piece → fixed lane (SP1 table: hat/snare/tom/cymbal + kick on space); reduction merges flams/ghost-notes.
- **Vocal:** pitch **contour** banded into 4 lanes relative to a moving window (low→high), **not absolute pitch** — melodies stay singable/readable regardless of key or range.

**(c) Reduction.** To fit content into the envelope, keep the most musically-salient events (downbeats, accents, phrase starts, contour peaks) and drop/merge the rest.

**(d) Author override.** Every translation output is a **starting point**. A per-instrument **"fit" knob** (faithful ↔ simplified) moves the envelope; the author can then hand-override anything in the editor (§4).

The translator is an **authoring-time tool** — it emits a resolved chart (§1-C philosophy, §3). Its knobs live in the editor, never in the distributed format.

---

## 3. Canonical serialization + deterministic hashing (Decision — locked; closes SP1's open question)

**(a) Canonical binary encoding — not JSON.** JSON invites nondeterminism (key ordering, whitespace, unicode normalization, float printing). The canonical form is a **binary encoding** with a version byte, fixed field order, length-prefixed sections, and varint/fixed-width integers. Deterministic by construction; smaller (helps the kilobyte P2P/QR transfer). A human-readable JSON *debug projection* may exist in the editor but is **never** what gets hashed.

**(b) Integer times, never floats.** All note times (feel-targets) are quantized to **integer units** (µs-grade — sub-ms, inaudible), as are the tempo/beat map subdivisions. This makes both the **hash** and every **transform** (re-groove, difficulty) reproducible across platforms and compilers — and, critically, **byte-identical between the native game and the WASM web app** (§4). Floats would have made a WASM authoring tool unsafe.

**(c) Core / envelope split (the "stable across edits vs new address" answer).**
- **Canonical playable core** = notes/lanes/space/sustains/feel-targets + tempo-beat map + lyrics + instrument-set refs + baked difficulty tiers (§6). **This — and only this — is hashed → the content-address.**
- **Metadata envelope** = human title/label, author nick + optional signature, version label, difficulty *names*, credits. Travels alongside; **not** in the hash.

**Consequences (intended):**
- Editing notes/feel/lanes/tiers → new core bytes → **new content-address.** Re-groove and difficulty transforms therefore automatically mint new addresses — no special-casing.
- Editing title/label/nick/version → **same content-address.** Attribution/versioning live in the envelope + index (§8), so renaming/re-crediting never forks the playable bytes or defeats viral dedup.
- Matches SP2 exactly: `chart_ownership` and dedup-on-join key on the *playable* hash — "I already have these notes, skip transfer" stays true across differing labels.
- **Amends SP1's Header sketch:** title/artist/author/version move *out* of the hashed structure into the envelope.

The accepted tension: two authors who chart a song identically to the microsecond collide to one content-address (correct for transfer/dedup); their differing attributions live in envelopes, not the address. Cosmetic label fixes do not create a new content "version."

---

## 4. Authoring surfaces & ingest (Decision — locked)

One **shared Rust core** (translate / serialize / hash / transform) is the single source of truth. Three surfaces call it; only the editing surface differs.

**(a) In-game editor (main authoring surface).** Full authoring, native Rust core. Editable: per-instrument lane assignment, space-action type, sustain lengths, **feel-targets** (per-note groove nudge), the per-instrument fit/envelope knob (§2), section/structure markers, lyrics timing, and the metadata envelope. Desktop-primary for precision; available in-game.

**(b) Open-access web app (add-on).** Same authoring via the Rust core compiled to **WASM**. Zero install, runs in any desktop browser (and iPad Safari), Web MIDI where supported. Ownership of the game **not required** — authoring is open; playing requires the game (funnel: make a chart → want to hear it → buy the game). **Byte-purity is absolute and load-bearing for the legal posture:**
- **MIDI never uploaded** — parsed/translated 100% in-browser (WASM). Source material never touches company infra.
- **Output is a downloaded file**, never a stored chart. We serve the *app* (HTML/JS/WASM) — our neutral instrument — never a chart byte or source byte.
- **No gallery, no upload, no search.** The web app is transform-only ("your MIDI in locally → your chart file out"). The instant it hosts/lists charts it becomes a searchable catalog of composition derivatives and the shield collapses.

**(c) In-game light-remix (every player, all platforms).** Only the deterministic **transforms** (re-groove §5, difficulty §6) exposed as sliders — touch-friendly, passes the Inebriated-User Test. Applying one **mints a new resolved chart with a new content-address** the player now owns and can host → viral remix falls straight out of content-addressing. "AC/DC with a massive swing" becomes a consumer gesture, not an author-only one.

**(d) MIDI import — the editor's front door.** Both editor surfaces accept `.mid`/`.midi` via file picker / drag-drop / (web) file input, parsed by SP1's Rust MIDI parser (native / WASM). MIDI is read **client-side, never uploaded**.
- **Track/channel → role mapping step:** the importer **auto-guesses** role assignment (GM channel 10 → drums, track-name heuristics, pitch-range hints) and the author confirms/remaps tracks/channels → the 4 roles. Unmapped tracks are dropped; a role may pull from one or more tracks. Then §2 translation runs per mapped role → §6 tiers → author tunes → export.

**(e) Editor "project" vs. exported `.band` (non-destructive authoring).** The editor holds a **local project** (imported MIDI + role mapping + translation/envelope params + manual edits) as working state, so translation can be **re-run with different settings without re-importing**. The **exported `.band` is the baked, resolved, hashed, MIDI-free artifact** that ships and is content-addressed. Project files are **local-only**, may embed source MIDI, and **never leave the device / are never published.**

**(f) Web→game handoff (almost-zero friction, hosting nothing).** Charts are kilobytes and content-addressed, so bytes move off-infra the same way SP2 does — screen-to-camera or link-fragment. Size-keyed hybrid:
- **Same device → fragment deep-link.** `band://import#<payload>` (or universal link). Payload rides the **URL `#` fragment, which is never sent to any server**, so we resolve the link without receiving chart bytes. (Chart in the *path* would be logged server-side — **forbidden**; fragment only.) One tap.
- **Cross-device → QR.** The web app renders the resolved chart as a QR (static if it fits ~2–3 KB, **animated/chunked** if larger — "hold your phone at the screen ~2s"). Screen → camera; no server.
- **Fallback → file export/import** via OS share sheet / Files (universal, most taps).
- **Deferred escape hatch → SP2-style WebRTC pairing** (Supabase signaling only, no bytes) *if* charts ever outgrow QR/link capacity. At kilobytes, unnecessary.

---

## 5. Re-grooving (Decision — locked: grid-relative feel functions)

A deterministic function on the resolved chart's integer-µs feel-targets → a **new resolved chart (new content-address)**. Because notes are already feel-targets (feel baked in), re-groove operates **against the grid**, not the raw times, so it doesn't double-count existing feel.

**Model.** Using the tempo/beat map, each note is classified to its nearest **metric position** (beat + subdivision), giving a **grid anchor** and a **residual** (`feel_target − anchor`). A re-groove is a **feel function `f(metric_position) → offset-from-grid`**, applied as `new_feel_target = anchor + f(position)`, optionally blending the residual. This makes swing mean "swing" regardless of prior feel, and makes transforms predictable and near-idempotent.

**Shipped primitives (small on purpose):**
- **Swing(amount, subdivision)** — pushes off-subdivision notes later; `amount` 0 = straight → ~0.33 = hard triplet; subdivision selectable (8th/16th).
- **Lean(±)** — uniform push (ahead, −) / lay-back (behind, +) across targeted notes; this *is* the pocket lean SP1/SP2 score against.
- **Humanize — deferred** (seeded jitter fights SP1's tight-pocket reward and adds RNG-determinism burden).

**Targeting — per instrument (locked goal).** The transform takes a **subset of instrument tracks** → "swing the drums, keep the vocal straight" is native. In-game: a few presets (whole band / rhythm section / per-instrument). Editor: full per-track control.

**Residual blend.** Default = **replace** the note's existing micro-feel (predictable, idempotent, "this is now a swing chart"). The **editor exposes a blend knob** (0 = replace … 1 = preserve-and-layer) for authors who want to keep hand-feel *and* add swing. In-game sliders use replace only.

**Sustains.** Re-groove shifts note **onsets** only; **hold lengths stay as charted** (SP1 scores sustains on onset + length). Editor may later expose length-scaling; v1 leaves lengths fixed.

All integer-µs math on integer inputs against an integer beat map → fully deterministic → identical bytes in WASM (web) and native (game). Structure (lanes/space/sustain-lengths) untouched; only onset feel-targets move; scoring "just works" because it always scored against feel-targets. Re-grooving multi-tier charts transforms **all tiers'** feel-targets (§6).

---

## 6. Difficulty tiers (Decision — locked: baked within-chart tiers)

Difficulty *generation* = the §2 effort-weighted envelope run at different budgets (party-floor = low budget/heavy reduction; groove-ceiling = faithful). The decision is **where tiers live**, and it turns on SP2's "a room = one `song_hash`."

**Key semantic distinction:**
- **Re-groove is shared** — it moves the *pocket* the whole band locks to. Two bandmates on different swings are in different pockets → coherence breaks → re-groove = separate chart per room (§5).
- **Difficulty is per-player** — different tiers of the same part share the *same feel-targets/pocket*, just fewer/more notes. Two bandmates on different tiers are still in the same pocket; coherence still works (it clusters *leans* on the notes you actually play; SP2 already handles partial participation as "fewer contributing windows").

Therefore **difficulty tiers are baked *within one resolved chart*, not separate content-addresses:**
- The one resolved chart carries, per instrument, **parallel note sets for a small set of tiers** (default **3** — e.g. Party / Player / Groove; author may adjust). The reducer generates them; the author can **hand-tune any tier**; all tiers are **baked** and the **content-address hashes all of them**.
- **Per-player, per-instrument tier selection at play time.** Mixed-skill band = **one room, one hash**, each player on their own tier. **No SP2 change** — the room still references one `song_hash`.
- **Timing windows stay constant across tiers** (SP1 owns windows; difficulty = density/complexity only, never "hit tighter"). "Hard to master" = play the full dense part *and* nail the pocket.

**Why baked beats runtime reduction:** runtime reduction would make the *reducer algorithm* a cross-version playback contract — two players on different app versions would generate different "Medium" sets and desync. Baking fixes the notes in the content-address (version-proof) and lets authors hand-tune tiers. Cost: ~3 note sets per instrument — still kilobytes, fine for QR/P2P.

---

## 7. Distribution graph (Decision — locked: room-scoped viral + local library + publish-as-social-gesture)

SP2 already gives viral-by-play, a public-room surface, peer-assisted in-room relay, and `chart_ownership` routing. SP3 decides reach *beyond* the room — dominated by the legal posture.

**Rejected: persistent peer-seeding overlay / DHT.** It would build (a) always-on seeding and (b) a **global index of who-has-which-composition-derivative** — exactly the searchable-catalog/inducement shape the posture forbids.

**Locked model — bytes stay strictly room-scoped; reach beyond the room is social:**

1. **Local chart library (new in SP3).** Every owned chart persists in a per-user, on-device library (§12) — the durable substrate SP2 assumed but didn't specify. You can **re-host any owned chart by starting a room** → *that's* the re-seed mechanism (viral by re-hosting, not by silent seeding).

2. **"Publish" = three non-hosting gestures.** A brand-new chart enters the graph when its author:
   - **Creates a public room** for it → appears on SP2's public-room surface (title-safe, §8) → joiners now own it → viral; or
   - **Shares the chart file directly** via the §4 handoff (AirDrop / QR / link / file); or
   - **Opts into the metadata index** (§8) for discoverability.
   
   Publishing announces existence and provides a path to fetch *from people* — it never uploads a byte to us.

3. **Cross-room fetch resolves through people, not a service.** You obtain a chart by joining a room that has an owner in it, or by someone sharing the file. The index may point to *live public rooms currently on a chart* (SP2 room metadata) — not a hosted byte store.

**Accepted tradeoff (a feature, not a bug):** a chart is reachable only where a player who owns it is present or willing to share. There is deliberately **no** "fetch any chart from the cloud." That friction is the virality/"make it human" engine (a USA player wanting an India chart must find a human who has it) **and** the legal shield — we never become the place charts live.

---

## 8. Metadata index (Decision — locked: opt-in, title-free, never label-searchable)

Lives on Supabase (metadata only — never a chart/source byte). The existential rule: **discovery is never by song-identity.** A global "search charts by name" box *is* a searchable catalog of composition derivatives (the Chorus model) — refused.

**Discovery axes (deliberately not song-identity):**
- **By author** — follow a self-sovereign author identity (§9); see what *they* chose to publish, labeled however *they* chose (author bears labeling responsibility; we don't amplify).
- **By hash** — you already have a hash (friend/QR/file) → resolve metadata, version lineage, and *live public rooms currently on it* (SP2).
- **By title-free structural filters** — instruments present, difficulty range, feel (swing/straight), tempo, duration, coarse mood/genre tags.

**MAY contain:** content-address hash; opt-in author pubkey + nick + optional signature; author-chosen display label; structural metadata (instruments, tier set, duration, BPM, feel tags); **version/lineage** (`parent_hash` + transform record + version label); coarse genre/mood tags; live-public-room pointers.

**MAY NOT contain / MAY NOT do:** no chart bytes or source MIDI (absolute); **no global full-text search over labels/titles**; no lyrics-as-searchable-text; our own UI/marketing never promotes copyrighted titles/artists as browse axes; no recommending-by-song-identity.

**The mechanism that keeps us off the inducement hook:** author display labels *are stored and shown* — but only in **author-scoped contexts** (an author's own published list, shown to people who chose to follow them or opened that author). They are **not globally full-text searchable.** We host the label as attribution; we do not provide the search surface that would make it a title catalog. **Enforced at the DB layer** (§11): no full-text index on `display_label`; the only query paths are by hash, by author pubkey, and by structural filter.

---

## 9. Anti-abuse & trust (Decision — locked: layered, local-first, no central authority)

**1) Content integrity — hash-verify on every receipt (free from content-addressing).** On every import path (P2P, QR, fragment-link, file) the Rust core **recomputes the hash and rejects any mismatch.** Defeats corruption and in-transit tampering for free.

**2) Hardened parser + load-time validation (safety regardless of origin).** Anyone can produce a chart (open-access web app), so the parser is **total/panic-free, bounds-checked, and fuzzed**, with a validation layer that **clamps ranges at load** — velocity/pitch bounds, note-rate and simultaneity caps, sustain-length and total-count limits — so no chart can crash the game or grief the player (ear-rape velocities, seizure-density walls). Lyrics/label text sanitized before display. This is the primary defense — it protects the player *whatever* the source. (Parser panics surface as anonymous PostHog exception events, §10.)

**3) Self-sovereign identity + optional signatures (provenance, not exclusivity).** Identity is a **locally-generated keypair + a self-chosen nick**, stored on-device — **no login, no account, no server-side user record.** An author may **sign the content-hash**; the signature rides in the envelope + index so verifiers confirm "this hash was signed by pubkey X" (defeats impersonation). Anyone may strip/re-sign as themselves — a *different* attribution, not forgery. "Following an author" = following a **public key."
- **Signatures optional; anonymous authoring is first-class** (matches the open-access web app — a nick without a key is fine; a key without a login is fine). *Publishing to the index with attribution* requires an identity + signature.

**4) Versioning & lineage.** Content change = new hash (automatic, §3). On top: a human **version label** (envelope) + **`parent_hash` lineage** in the index (§8) — "v2 of H" / "re-groove of H." Updating publishes a *new* hash with a lineage pointer; **old hashes stay immutable and playable** (rooms reference a specific hash — no forced updates, no rug-pulls). Signed lineage makes "derived from H" verifiable where H was signed.

**5) Moderation posture — v1 hands-off, hooks present.** Nicks and labels are free-form, self-asserted, and **unmoderated in v1** (per product call). The *mechanism* exists (the index is ours: an entry can be de-listed, a pubkey de-listed from the index — never a byte takedown, since we hold no bytes), but **v1 policy is hands-off**; nick/label/index moderation is a **dedicated future discussion.** The user-facing safety valve is **local blocklists** (block a pubkey → hide their entries, never auto-fetch their charts).

**Trust = content-integrity + local validation + optional-signature provenance + author-following + (dormant) index de-listing.** No central gatekeeper; the player is protected locally regardless of source.

---

## 10. Identity, accounts & analytics (Decision — locked)

- **No login, ever. One purchase, no accounts, no subscription** (Steam + app stores). No required server-side per-user state. Identity is the §9 local keypair + nick; leaderboards (SP2) are pseudonymous off that pubkey.
- **No user tracking.** 
- **Analytics = PostHog (`posthog-rs`) in the Rust core, anonymous mode.** Use **`Event::new_anon`** — anonymous events, **no person profiles, no `distinct_id`, no login.** Capture is **non-blocking** (background batching worker; `flush()`/`shutdown()` on exit) so it never touches the audio hot path. Events are PII-free and hash-keyed (`chart played`, `room created`, `chart imported`, `chart shared`, with a `content_hash` property); PostHog derives coarse geo (IP-level), time, funnels, and the **spread/network graph** (who-plays-with-whom, how wide, how fast — a title-free product/sales asset).
- **Bonus PostHog capabilities we adopt:** **feature flags** (A/B the §2 envelope curves, §6 difficulty budgets, and scoring knobs without shipping builds) and **panic/exception capture** (a malformed chart that slips a §9 range-check surfaces as an anonymous crash event → free hardening telemetry).
- **Supabase's role narrows** to rooms / signaling / metadata / index / leaderboards. Analytics does **not** run through Supabase (no derived spread view, no `rooms.coarse_geo` column — PostHog owns it).

---

## 11. Supabase schema touchpoints (metadata only; bytes never touch company infra)

SP2 owns `rooms`, `room_members`, `chart_ownership`, `play_windows`, `band_scores`, the signaling channel, and per-note audit. SP3 adds a small, metadata-only, **login-free** surface:

- **`chart_index`** (opt-in; one row per published hash) — `content_hash` (PK), `author_pubkey`, `author_nick`, `display_label`, structural fields (`instruments`, `tier_set`, `duration_ms`, `bpm`, `feel_tags`), coarse `genre_mood_tags`, `version_label`, `parent_hash` (lineage), `transform_record` (nullable), `signature`, `created_at`.
  - **RLS:** anonymous self-publish, payload signed by `author_pubkey` (no login).
  - **Constrained query surface (enforces §8's anti-catalog line):** lookups by `content_hash`, by `author_pubkey`, and by structural filters only — **no full-text index on `display_label`.**
- **Lineage** — folded into `chart_index.parent_hash` (+ `transform_record`); no separate table.
- **Author-following → local-only, no server table.** The device keeps the followed-pubkey list; discovering their charts is a `chart_index` query by `author_pubkey`.
- **`chart_ownership` (SP2) refined:** `user_id` = the pseudonymous **local pubkey**; opt-in advertisement for transfer-request routing only.
- **Analytics → PostHog, not Supabase** (§10). No spread view/column here.

**Never in the schema / never on company infra:** chart bytes, source MIDI, per-user tracking profiles, globally-searchable titles.

---

## 12. On-device library management + share/import flow

The **only** "hosting" is the user's own device. SP3 mostly *composes* SP2's Invite&Join and §4's handoff into the library UX.

**Local library.**
- Canonical `.band` binaries stored in a platform-appropriate, **backup-included** location (iOS app Documents → rides iCloud *device* backup if enabled; desktop app-data dir, user-copyable). **No custom cloud sync** (that would mean hosting bytes) — lose the device, re-fetch from rooms/friends. The **local keypair** (author identity, §9) lives in secure storage and should be included in that backup so identity survives a device change.
- A **local metadata cache** (SQLite) parsed from envelopes for instant browse: `content_hash` (key), nick, label, instruments, tiers, duration, bpm, feel, `parent_hash`/lineage, **provenance** (authored / got-in-a-room / shared-by / re-grooved-from), date-added, last-played.

**Manage.** Browse/sort/filter on the same title-free axes as the index; chart detail; edit label/nick (envelope only — no hash change, §3); delete (your copy only); dedup-by-hash on import (SP2 viral no-op); **version grouping** via lineage ("v1 / v2 / swing remix" clustered); re-groove/difficulty → in-game remix (§5/§6) → new `.band` added with lineage recorded.

**Share — one "Share" action per chart, three intents, all reusing existing primitives:**
1. **Play together → host a room** (SP2 Invite&Join): mint a room for this hash → link / QR / short-code → chart transfers P2P in-room.
2. **Send the chart → §4 handoff** (OS share sheet `.band` / QR-of-bytes / fragment-link / file export). Off-infra.
3. **Publish → opt-in signed index entry** (§8/§9).

**Import (the reverse):** receive via any §4 path → **hash-verify (§9)** → dedup → add to library with provenance. Playing in a room also auto-adds the chart you now own (SP2).

So: **share = compose(SP2 Invite&Join, §4 handoff, §8 index); import = §4 receive + §9 verify + dedup.**

---

## 13. Cross-cutting contract amendments (for SP1/SP2 implementers)

- **SP1 chart Header** → title/artist/author/version move **out of the hashed core into the unhashed envelope** (§3c). The hashed core gains **baked difficulty tiers** (§6).
- **SP1 canonical serialization/hashing open question** → **closed:** canonical binary, integer-µs times, core/envelope split (§3).
- **SP2 `chart_ownership.user_id`** → a pseudonymous **local pubkey** (§9, §11); no login.
- **Identity model platform-wide** → self-sovereign local keypair + nick; no accounts (§9, §10). SP2 leaderboards are pseudonymous off the pubkey.

---

## 14. Open questions to resolve during implementation

- Exact **envelope/effort weights** per instrument role and per tier, and the salience-ranking heuristics — playtest/feel-tuned; A/B'd via PostHog feature flags.
- Exact **binary layout** (section order, varint scheme, time quantum) and the **hashing algorithm** choice — must be pinned once and versioned.
- **Track/channel auto-guess** heuristics quality (how often authors must remap).
- **QR capacity ceiling** and the animated/chunked-QR encoding (framing, error correction, dedup of frames).
- **Signature scheme** (curve, key storage per platform, keychain/secure-enclave integration) and keypair backup UX.
- **Difficulty tier count/names** default and cap; whether authors commonly need >3.
- **Re-groove primitive math** (swing curve shape, subdivision detection robustness on syncopated input) — feel-tuned.
- **Nick/label/index moderation policy** — deliberately deferred to a dedicated discussion; only local blocklists ship in v1.
- **PostHog event taxonomy** (which anonymous events, which properties) and self-host vs. cloud.

---

## 15. Definition of done (this sub-project)

An author (owning the game *or* using the open-access web app) can: **import a MIDI file** entirely client-side, **map its tracks to the four instrument roles**, get a **fair, fun, effort-balanced** 4-lanes-+-space translation per instrument with a per-instrument **fit knob** and full manual override, generate **baked difficulty tiers** and **re-groove** per instrument, and **export a resolved `.band` chart** whose **canonical binary serialization hashes deterministically and identically in the native game and the WASM web app**. That chart moves **web→game with near-zero friction** (fragment-link / QR / file) **without a byte touching company infra**. Any player can **re-groove/re-difficulty in-game** to mint a new owned content-address, **manage a persistent local library**, and **share** a chart three ways (host a room / send the file / opt-in publish), with **hash-verified, deduped import**. Charts spread **virally and socially** (room-scoped P2P + local re-hosting) with **no seeding overlay and no searchable title catalog**; an **opt-in, title-free metadata index** provides author/hash/structural discovery; **anonymous PostHog analytics** capture the title-free spread graph; and the whole system requires **no login, no accounts, and no user tracking**, protecting players locally via a **hardened parser, hash integrity, optional signatures, and local blocklists.**
