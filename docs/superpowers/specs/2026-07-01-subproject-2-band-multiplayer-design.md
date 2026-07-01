# Sub-project 2 — Band / Multiplayer (design spec)

**Date:** 2026-07-01
**Parent:** [Band — Platform Architecture & Vision](./2026-07-01-band-platform-architecture-design.md)
**Builds on:** [Sub-project 1 — The Playable Core](./2026-07-01-subproject-1-playable-core-design.md)
**Status:** Draft for review.

> **Goal:** turn the single-player Playable Core into a **band**. Four remote players each perform one instrument on their own device; the server is the session authority; bandmates arrive as a slightly-delayed presence layer; the band's collective groove + coherence is the supreme score. This is the whole point of the game.

**Governing constraint — the Inebriated-User Test:** if a mechanic needs a tutorial, it's wrong. Joining a band, grabbing an instrument, and jamming must be trivial for a drunk player at a party.

---

## 0. Locked constraints carried from the platform spec (not re-litigated)

- **All players remote at all times; per-device audio (v1).** Shared-stage "Jackbox" mode is a fast-follow, not v1.
- **Sync = tiny timing EVENTS tagged to song-position, not audio.** Own instrument + backing render local/instant; bandmates render as a slightly-delayed presence layer. Groove is measured over a window, so scoring tolerates latency by design.
- **Server (Supabase) is session authority** — clock, event-record fan-out, score aggregation. The **room creator is only the content source + room-metadata owner.** Host leaving must **not** kill the song.
- **Chart bytes travel true P2P (WebRTC) + peer-assisted relay.** Company infra **never** carries a chart byte. Only STUN (address discovery) + Supabase (signaling / metadata / presence) sit in the path.
- **Charts are content-addressed;** a joiner who owns the hash skips transfer. Discovery is by **room**, not a file catalog.
- **Invite & Join v1 = OS share sheet + QR + short code.** Local auto-discovery (mDNS/BLE) deferred to v1.5.
- **Band > everything.** Band streak/points outrank individual. Audience = read-mostly, non-scoring (crowd-energy boost deferred).
- **Stack:** Godot 4 + Rust (`gdext`). iOS-first → Mac/Win/Linux → Android (2nd-class).

---

## 1. Scope

**In:**
- Session/room lifecycle: create, public/private, join-by-link, 4 instrument slots + audience, fill/empty slots, players joining/leaving/swapping mid-song, server-authoritative "host migration."
- Invite & Join component: OS share sheet + QR + short human code, universal/deep links, install funnel.
- Server-synced session clock replacing SP1's local audio-master clock (anchor-only; local playhead still free-runs).
- P2P mesh + transport: content-addressed chart transfer (+ peer-assisted relay), timing-event fan-out. **Hybrid transport** (mesh for feel, Supabase for truth).
- Bandmate presence layer: seeing + hearing the band (jitter buffer / look-ahead).
- Band scoring: collective groove + **coherence** (four pockets clustering), band streak, band points — aggregated from SP1's band-ready metrics.
- Audience joining/routing (clock + lyrics + aggregated state; read-mostly; scales cheaply).
- Anti-cheat posture for scores that matter.
- Supabase schema touchpoints for all of the above.

**Out (later sub-projects / fast-follows):**
- Shared-stage "Jackbox" mode; crowd-energy → band-score boost; local auto-discovery.
- Full authoring/translation tool + community distribution (Layer C).
- Full deterministic server-side score replay / ranked-competitive integrity (hooks present, see §9).
- Chart-ownership presence table + "grab a slot before the owner leaves" warnings (hooks present, see §4).

---

## 2. Session / room lifecycle (Decision — locked)

A **room** is a server-side (Supabase) record; it is the unit of a band session and the unit of discovery.

**Room record (conceptual):** `id`, `owner_id`, `song_hash`, `visibility` (public / private), `state` (lobby / playing / ended), `song_epoch`, `short_code`, 4 slots, N audience.

**Roles:**
- **4 instrument slots** — guitar / bass / drums / vocal.
- **Audience** — unbounded, read-mostly.
- An **empty slot auto-performs at chart-ideal** (SP1's unmanned-instrument behavior). A one-person "band" is three auto-performed slots — exactly SP1's solo experience, now inside a room.

**Slot dynamics (mid-song, Decision — locked; see §7 of SP1 for the audio consequences):**
- **Leave mid-song** (rage-quit / drop) → the slot **instantly reverts to auto-perform** (chart-ideal). The band keeps playing; the departed part is robotically covered rather than a silent hole. The player's banked band-score contribution is frozen at what they earned.
- **Join mid-song** → land in the **audience view first**; claim an empty slot, and **control transfers at the next musical boundary** (bar or section line), never on a random subdivision. Between claim and boundary the slot stays auto-performed; at the boundary control transfers to the human.
- **Swap slots mid-song** → treated as **leave-then-claim**: drop current slot (reverts to auto-perform), claim target at the next boundary. No special case; reuses the two rules above.
- **Mid-song joiners score normally from their boundary onward** (Decision Q3-A). Band score is windowed anyway, so partial participation simply contributes fewer windows. Chosen for "jump in and jam" over strict leaderboard symmetry.

**Authority & host migration:**
- The **server** owns the clock, the fan-out record, and score aggregation. The **owner** owns only the chart-content source and the room metadata.
- **Owner leaving does not end the song or the room.** The 4 active performers already hold chart + clock in RAM; the room row persists server-side. "Host migration" is a **metadata row update** (reassign `owner_id` to a remaining member), **not** a clock handoff — the clock was never a player's to hold.

**Create / visibility:**
- **Create:** owner picks a chart (by content hash) → server mints a `rooms` row + `short_code` + universal link.
- **Public** rooms are listable/joinable via the platform's room surface; **private** rooms are reachable only by link / QR / short code.

---

## 3. Session clock (Decision Q2 — locked: NTP-style offset, free-running local playhead, slew-only mid-song)

SP1 made the audio device's playback clock the master. SP2 needs a **shared** song-position without letting a network round-trip drive the crown-jewel local audio path.

**Method:**
1. On join, the client runs a few **RTT probes** against Supabase (Cristian's algorithm: send `t0`, server stamps, measure RTT, estimate clock **offset**).
2. The server publishes **`song_epoch`** — the authoritative wall-clock instant at which song-position 0 occurs.
3. Each client computes `local_song_pos = (now + offset) − song_epoch`, **anchors its SP1 sample-accurate playhead** there, and then lets that playhead **free-run**.
4. Offset is **re-synced periodically**; disagreement is corrected by **slew only during a song** (slow resample / micro-adjust — never an audible/visible jump). A **hard set** is allowed only at song boundaries.

**Consequence:** nothing above the clock (from SP1) changes. Local audio stays instant; the shared reference only sets *where* the free-running clock is anchored. This is exactly the "swap the master's anchor, nothing above the clock changes" hook SP1 left open.

---

## 4. Transport & mesh (Decision Q1-C — locked: hybrid, mesh for feel + Supabase for truth)

Two paths carry **different-granularity** data, so the hot path is not doubled.

**Path 1 — Mesh (WebRTC data channels):**
- **Topology:** only the **4 performer slots** form a WebRTC **full mesh** (6 connections — trivial). Audience never joins the mesh.
- **Carries:** per-hit **timing events** — `{songPos, instrument, lane/space action, signedOffset}` — fanned out peer-to-peer.
- **Drives:** the **bandmate presence layer** (see + hear others, §5) and the **live approximate band meter** (§6). Lowest latency for what you feel.

**Path 2 — Supabase (Realtime + tables):**
- **Carries:** per client, a compact **windowed summary** — `{window_idx, meanOffset, spread, grooveBonus, inPocket, missCount}` — plus the per-note **band-ready records** SP1 already emits, retained per session for audit.
- **Drives:** **authoritative** band-score aggregation, persistence, and anti-cheat. Server truth for the leaderboard.

**Signaling & NAT:** **Supabase Realtime** exchanges SDP/ICE (ephemeral per-room channel); **STUN** provides address discovery. Neither ever carries a chart or an audio byte.

**Chart transfer (content-addressed + peer-assisted relay):**
- On join, the client **hash-checks**: already owns the content-address → **skip transfer entirely** (the viral win).
- Otherwise it requests the chart from a **connected peer that owns it**. Peers **advertise which chart hashes they own during signaling**, so requests route to an owner (owner-first, any-owner fallback).
- **NAT-stuck peers** are served by **peer-assisted relay** — a well-connected bandmate forwards the ~kilobyte chart. **No company TURN; no company byte-carrying.**
- **Failure mode — nobody reachable can serve the chart** (e.g., owner left before transfer completed, no other peer owns it): the joiner **falls back gracefully to the audience view** (clock + lyrics + aggregated presence) and can claim a performer slot once they own the chart. Never a hard error; the room and the four active performers are unaffected (they hold the chart in RAM).
- **Deferred (Q4-B):** a full **chart-ownership presence table** + "grab a slot before the owner leaves" UI warnings. v1 keeps only the lightweight owned-hash advertisement needed to route transfer requests.

---

## 5. Bandmate presence layer

The locked hybrid: **own instrument + backing = local / instant; bandmates = slightly-delayed presence.**

- **See:** each peer's incoming timing events render their hits as a lightweight **presence overlay** (avatars / mini-highways / hit flashes) — you watch the band play in near-real-time.
- **Hear:** incoming events are placed in a **jitter buffer / look-ahead** and sounded against the shared song-clock, so a late packet still lands at its correct song-position. **Target ~1 bar of look-ahead** (sized to observed mesh jitter). Because **groove is measured over a window**, this delay is inaudible to *feel* — you feel your own part instantly and hear the band cohere a hair later.
- Auto-performed slots are sounded at chart-ideal (no network dependency).

**Open (implementation):** final jitter-buffer sizing and adaptive-vs-fixed look-ahead — playtest-driven, bounded by "≤ ~1 bar."

---

## 6. Band scoring (Decisions Q5 + Q6 — locked)

Built entirely from SP1's **band-ready metrics** (per note: `{songPos, targetTime, actualTime, signedOffset, grade, sustainLenError}`; per window: `{meanOffset, spread, grooveBonus, inPocket}`). No new per-note data is required.

**(a) Coherence — the novel soul (Q5-A: inter-player lean-cluster tightness, pocket-guarded).**
- Per rolling window, take the four players' `meanOffset` (each player's pocket **lean**) and measure how tightly **they cluster** (std-dev across the four leans). **Low inter-player spread = high coherence** — "all four are sitting in the *same spot* in the pocket."
- **Pocket guardrail** (reusing SP1's pocket-zone): coherence counts only while contributing players are `inPocket`. Four players tightly clustered but collectively lazy/rushing does **not** win.
- **Auto-performed slots** sit at chart-ideal and act as a **stable cluster anchor**, so a solo-with-3-bots player still gets a sensible band meter.
- This realizes the pillar directly: four humans all leaning +13 ms cluster tightly (high coherence **and** groove lean); four robots at 0 cluster tightly but earn no groove-lean bonus — so human groove can edge robotic grid, collectively, exactly as SP1's solo example does.

**(b) Band points.**
- `band_points` = collective groove + coherence bonus, **streak-multiplied**. This is the number that **outranks individual score** (band > everything).

**(c) Band streak (Q6-A + knob).**
- Breaks when **`coherence < threshold`** **OR** **`missCount_in_window ≥ N`** — both **playtest-tunable** knobs.
- Default posture is **forgiving / collective-collapse**, not weakest-link: a *single* player's isolated miss does **not** break the band streak (the other three + auto-perform keep the cluster coherent). It costs *that player's* individual score and leaves SP1's audible hole — personal stakes without collective punishment.
- The `missCount ≥ N` knob generalizes toward a stricter "genuine collective stumble" break for higher-skill lobbies without re-architecting.

**(d) Live vs. authoritative split (Q6, follows transport-C).**
- **Live meter:** each client computes an **approximate** band meter from the **mesh** timing-events for real-time on-screen feedback (the "we're cohering!" needle moving *now*).
- **Authoritative score:** **Supabase** computes the band/individual score from the **windowed summaries** for the leaderboard + persistence.
- The two differ slightly (raw events vs. windowed summaries); expected and fine. Live = feel; server = truth.

**Open (implementation):** coherence std-dev vs. MAD; window length alignment with SP1's window; exact bonus/multiplier curves; default `threshold` and `N` — all feel-tuned in playtest.

---

## 7. Invite & Join

- **Room = Universal / App Link** — `https://<domain>/r/<roomId>`. Installed → opens **into the room**; not installed → the App Store (the **install funnel**), and the link **survives install** → auto-join on first launch.
- **v1 surfaces:**
  - **A) OS share sheet** (AirDrop / Messages / …) — iOS-native "in the room" magic, near-zero custom code.
  - **B) QR code** on the host screen — co-located hero, ecosystem-agnostic (iPhone ↔ Android).
  - **C) Short human code** — universal verbal fallback; a server-issued, collision-free token (e.g. 6 chars) that the server **resolves code → room**, with a TTL tied to the room's lifetime.
- **Deferred:** automatic local discovery (mDNS / BLE) → v1.5.

---

## 8. Audience routing

- Audience members are **Supabase Realtime subscribers only** — never on the WebRTC mesh.
- They receive: the **session clock** (`song_epoch` + their own offset), scrolling **lyrics**, and the **aggregated** (server-side, coarse) band presence + score state — **not** per-hit mesh events.
- Read-mostly → **scales cheaply** (no timing events pushed to them; fan-out is a single Realtime channel).
- Reuses **SP1's audience-view artifact** (the vocalist's lyrics + clock minus buttons/scoring). SP2 only has to **route** it. Non-scoring; crowd-energy → band-boost is deferred.

---

## 9. Anti-cheat (Decision Q7-A — locked: pragmatic server-aggregate + plausibility; full replay deferred)

- The **server is score authority.** Clients submit **play data** (windowed summaries), **not** an asserted score — the server does the math.
- **Plausibility checks:** physical-offset bounds, input-rate caps (no impossible hit rates), and flagging of whole-song runs of impossibly-perfect zero-spread play.
- **Audit trail:** the per-note band-ready records (already emitted by SP1) are **retained per session**, so a flagged score can be recomputed/audited after the fact.
- **Deferred (Q7-B), hooks present:** full **server-side deterministic replay** for a ranked/competitive mode. SP1's scoring is Rust and portable to a server function; we already retain the per-note stream — so hardening later does not re-architect anything. v1 keeps company infra carrying only compact metadata, never a per-hit firehose.

---

## 10. Supabase schema touchpoints

Conceptual — final columns/indexes settled during implementation.

- **`rooms`** — `id`, `owner_id`, `song_hash`, `visibility` (public/private), `state` (lobby/playing/ended), `song_epoch`, `short_code`, `created_at`.
- **`room_members`** — `room_id`, `user_id`, `role` (slot | audience), `instrument` (nullable), `joined_at`, `connection_meta`. Backs presence + slot occupancy.
- **`chart_ownership`** — `user_id`, `song_hash`. Lightweight, for **transfer-request routing** only (full ownership-presence table + owner-leaving warnings deferred, §4).
- **`play_windows`** — `room_id`, `user_id`, `window_idx`, `mean_offset`, `spread`, `groove_bonus`, `in_pocket`, `miss_count`. The **authoritative aggregation input** (§6, §9).
- **`band_scores`** — `room_id`, `song_hash`, `band_points`, `coherence_avg`, `band_streak_max`, per-player breakdown, `finalized_at`. The leaderboard record.
- **Signaling** — Supabase **Realtime** channel per room for ephemeral SDP/ICE exchange (§4). Not a durable table.
- **Per-note audit records** — retained per session for anti-cheat (§9); an ephemeral store, not necessarily a durable table.

**Never in the schema / never on company infra:** chart bytes, audio bytes.

---

## 11. Open questions to resolve during implementation

- Final **jitter-buffer / look-ahead** sizing (adaptive vs. fixed; bounded ≤ ~1 bar) — playtest.
- **Coherence math** details (std-dev vs. MAD; window alignment with SP1's window; bonus/multiplier curves) — feel-tuned.
- Default **band-streak knobs** (`coherence threshold`, `miss_count N`) — playtest.
- **Time-sync** probe count / re-sync cadence / slew rate — measured against real mobile jitter.
- **Realtime vs. table** split for windowed summaries (stream via Realtime and persist, vs. periodic table writes) — measured against Supabase quotas/latency.
- Short-code **length / alphabet / TTL / collision** policy.
- Public-room **surfacing** (how public rooms are listed) — minimal for v1; not a searchable file catalog.

---

## 12. Definition of done (this sub-project)

Four remote players, each on their own device (at least one desktop target **and** iOS), can: receive a room link (share sheet / QR / short code), open **into the room** (installing via the funnel if needed), obtain the content-addressed chart **peer-to-peer** (skipping transfer if already owned, relayed if NAT-stuck, gracefully degrading to audience if unobtainable), **sync to a shared server clock** while their local audio stays instant, each **perform one instrument** with bandmates appearing as a slightly-delayed **presence layer** (seen + heard via a jitter buffer), and earn a **band score** whose **collective coherence + streak demonstrably outrank individual play** — with the score **authoritatively aggregated server-side**, a **live band meter** driven by the mesh, **audience members** able to join and follow read-only, and a **pragmatic anti-cheat** posture in place. The owner can leave mid-song without ending it, and players can **join / leave / swap slots mid-song** at musical boundaries with empty slots auto-performing throughout.
