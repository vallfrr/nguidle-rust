---
name: nguidle-port
description: Create a new NGU Idle platform port on the Rust engine, from the generated template, following the porting guide. Use when asked to add a platform, a host or a "port".
---

# Create a port

Everything you need is in this repository. Read [the porting guide](../../../docs/PORTING.md) first, then [the engine API](../../../docs/ENGINE-API.md). Do not modify the engine to make a port work: report the gap.

1. **Generate.** From the engine checkout: `just new-port nguidle-<platform> "<Name>"`; add `../nguidle-rust` (your local engine checkout) as a third argument while working on both. In the new repository run `just check`; it must pass before any change.
2. **Learn the contract.** `cargo doc --no-deps -p platform-api --open`, then read `crates/ngu-headless/src/lib.rs`: its `frame_at` is the main loop in order (input, time, draw, textures, text, sound).
3. **Prepare the player's data** (guide, step 3). Never commit it. Test with a throw-away profile folder.
4. **Replace the `PORT:` stand-ins** one at a time, in the guide's order: drawing, time, pointer, gamepad and keyboard, text fields, audio, saves. Run `just check` after each; add a test in `tests/contract.rs` for each behaviour.
5. **Compare with the reference.** `cargo run --release -p ngu-headless -- --data <prepared> --save <tmp>` prints the draw statistics and a hash for a script; your port's draw list must agree.
6. **Finish with the guide's checklist** (section 7). Record the platform, engine tag and revision, the commands you ran and what you did not verify.

Rules: one thread for the engine; copy borrowed buffers before another thread uses them; the game's files, extracted assets and saves never go in Git; keep code and docs in English.
