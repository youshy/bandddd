# CI TODO: run the wasm cross-target determinism test

`.github/workflows/ci.yml` does not exist yet (Phase 0 / Task 0.3 CI setup was
deferred — it needs iOS/hardware runners). Once that CI file lands, add the
following two steps to the `rust` job, after the existing native
`cargo test` step, so the golden-vector hash test in
`hash_determinism.rs` is actually *executed* under wasm32 (not just compiled)
on every CI run:

```yaml
      - run: cargo install wasm-pack --locked
      - run: cd rust && wasm-pack test --node crates/band-core --test hash_determinism
```

Note: do **not** insert an extra `--` before `--test` (i.e. do not write
`wasm-pack test --node crates/band-core -- --test hash_determinism`) — with
wasm-pack 0.15.0 that form is rejected by `wasm-bindgen-test-runner`
("unexpected argument '--test' found"). Pass `--test hash_determinism`
directly as one of wasm-pack's own trailing `PATH_AND_EXTRA_OPTIONS` args, as
shown above; this was verified to work locally (`node v26`, `wasm-pack
0.15.0`, `wasm-bindgen-cli 0.2.126`) and printed:

```
running 1 test
test wasm::golden_address_matches_wasm ... ok
```

This proves the BLAKE3 content-address (`band_core::chart::hash::content_address`)
is byte-identical between the native target and wasm32-unknown-unknown for a
fixed golden `ChartCore` (see `golden_core()` / `GOLDEN` in
`hash_determinism.rs`).
