# Porting guide: build your own NGU Idle port

This is the one document you need to create a port (a new platform or a new way to run the game) on the
`nguidle-rust` engine. It is written to be followed by a person or by an AI assistant that has nothing else.
Reading `README.md` and this file is enough to reach a working stand-in port; the checklist at the end says when yours is finished.

## What you are building

The engine is the whole game: it runs the game's logic, builds the user interface and produces each frame as a list of
textured rectangles. It knows nothing about windows, GPUs, audio devices or input hardware. A **port** is the small
program around it that provides exactly that:

| the port gives the engine | the engine gives the port |
|---|---|
| real time that passes | game logic, advanced in fixed 1/30 s steps |
| pointer, touch, gamepad and keyboard input | the frame to draw: ordered quads (rectangles) with textures |
| a place to keep saves (a folder) | textures (sprites, glyph atlas), sounds to play, text fields to edit |
| the player's own prepared game data | the home screen, save slots, menus: everything |

Existing ports are examples: the desktop window (`crates/ngu-desktop`, in this repository), the browser
([nguidle-web](https://github.com/vallfrr/nguidle-web)) and the PS Vita ([nguidle-vita](https://github.com/vallfrr/nguidle-vita)).
`crates/ngu-headless` is the smallest complete one and the best to read first.

## Rules you must respect
- **Legal.** The game is free on Steam but is not ours. Your port must never contain or distribute the game's files, code,
  images, texts or saves. Everything the engine needs is *prepared by the player, on their machine, from their own
  installation* (step 3). This project is GPL-3.0; a port that links the engine must be distributable under compatible terms.
  It is not affiliated with the game's author.
- **One thread.** Create and call the engine from one thread only, one call at a time, never from inside an engine callback.
  Buffers the engine returns are borrowed until the next call: copy them before another thread uses them.
- **Personal data.** Test with a throw-away profile folder. Never test with a modified or extreme save on the real Steam game.

## 1. Create the repository (5 minutes)

You need Rust (rustup), Python 3.11+, [just](https://github.com/casey/just) 1.43 and Git, on any OS.

```text
git clone <the engine repository URL> nguidle-rust        # while the engine is unpublished, use the copy you were given
cd nguidle-rust
just new-port nguidle-switch "Nintendo Switch"            # your own name and platform
cd ../nguidle-switch
just check
```

`just new-port <name> [platform] [engine-checkout]` (`tools/new_port.py`, which also takes `--out` and `--tag`) copies
`templates/port/` next to the engine checkout, pins the engine's newest `v0.x.y` tag, runs `git init` (it does not commit: make the first commit yourself), and adds CI, the hygiene
check, `AGENTS.md` and a contract test. `just check` (hygiene, `cargo fmt --check`, Clippy as errors, the contract test) must pass
at once; run `cargo fmt` after editing. If it does not, that is a bug in the template: report it.

**Developing against a local engine checkout.** Use this form when your checkout has no remote or no tags. Pass the checkout as the third argument:
`just new-port nguidle-switch "Nintendo Switch" ../nguidle-rust`. `Cargo.toml` then points at that folder
(`clr = { path = ".../clr" }`) instead of a tag. That is for development only: before you publish, change it to
`clr = { git = "<engine URL>", tag = "<engine.tag>" }`, run `cargo update -p clr` and commit `Cargo.lock`, so that the repository
holds no local path and pins a tag and revision (checklist, section 7).

A worked first run: after step 1, prepare the data (step 3), then
`just run <prepared folder> <throw-away profile folder>` opens the real game headless, runs 120 virtual frames and prints
`120 frames; last frame: N quads; M textures uploaded; fingerprint ...`. (The game prints a few `SteamAPI_Init() failed` lines on
start-up; they are expected and harmless.) Make the stand-in click by giving `Scripted::new` events (see `tests/contract.rs`),
or use the reference host's script format: `cargo run --release -p ngu-headless -- --script file` in the engine checkout.

## 2. Understand the contract (10 minutes)

Everything is in the `platform-api` crate (re-exported as `clr::platform_api`), documented in the code:
`cargo doc --no-deps -p platform-api --open`.

* `Engine`: what you drive. `resize`, `advance(seconds)`, `set_pointer`, `press` (right and middle buttons), `action`,
  `scroll`, `key`, `wheel`, `reset_input` (focus lost), `draw`, `texture`, `atlas_changed`, `release_texture`, `take_text_request`, `submit_text`, `take_sounds`. The implementation is
  `clr::session::Session`.
* `DrawTarget`: what you provide to receive a frame (`draw_quads`). `TriangleList` converts quads to vertex and batch arrays.
* `Action`: `Up Down Left Right Activate Back Prev Next`. `Pointer`: device pixels, top-left origin.
* `testing::ScriptedEngine`: a synthetic engine with the same contract, for tests that need no game files.

A frame of your main loop, in this order (this is `run_frame` in the template and `Host::frame_at` in `ngu-headless`):

```text
for each input event:  set_pointer / press / action / scroll / key / wheel (reset_input when the host loses focus)
advance(elapsed seconds)          // logic runs in whole 1/30 s steps; long gaps are clamped
draw(&mut your DrawTarget)        // ordered quads: draw them in order, each over the last, clipped to quad.clip
for each texture id the frame uses and you have not uploaded: texture(id) -> upload -> release_texture(id)
if atlas_changed(): texture(TEXTURE_ATLAS) -> upload again
if let Some(request) = take_text_request():  show your keyboard / text box; submit_text(Some(text) | None)
for clip in take_sounds():        play it
```

C or C++ hosts use the C ABI instead (`clr/src/capi.rs`, declared by `nguidle-vita/engine_api.h`): same service, same order
(`ngu_init`, `ngu_frame`, `ngu_pad`, `ngu_scroll`, `ngu_emit` + `ngu_batches` + `ngu_vertices`, `ngu_tex`, `ngu_atlas`,
`ngu_text_request`, `ngu_text_submit`). Its threading, units and memory rules are at the top of `capi.rs` and on each function.

**Coordinates and sizes.** The game is 960 x 600 game pixels. The engine fits that into your `width` x `height`
(centred, proportions kept) and gives you quads in your device pixels. When your window is wider than 960:600 the game
reflows to the extra width. Use any size; call `resize` when it changes.

## 3. Prepare the data (the player does this)

From the engine checkout, with the player's Steam installation of NGU Idle (Windows build 1.260), read-only:

```text
cargo run --release -p ngu-prep --bin ngu-prepare -- verify "<install>"
cargo run --release -p ngu-prep --bin ngu-prepare -- export "<install>" <prepared>
cargo run --release -p ngu-prep --bin ngu-prepare -- vita "<install>" <folder>      # or: <folder>/nguidle/ngu.pak
```

`<install>` is the NGU IDLE folder (the one with `NGUIdle.exe`). `verify` checks the files against the known build.
`export` extracts sprites, fonts, sounds, texts and the scene description to `<prepared>` and copies `Assembly-CSharp.dll`
in (`just verify-data`, `just prepare-data` and `just vita-data` run the same commands). A prepared folder only works with the engine revision whose tools made it:
re-prepare after changing the engine pin. Never commit it. `Session::open` and `ngu_init` take this folder (or the
folder containing `ngu.pak`). Sounds are `<prepared>/audio/<clip>.wav`.

## 4. Replace the stand-ins, one at a time

Every `PORT:` comment in the generated `src/` marks where your platform's code goes. Do them in this order, running
`just check` after each:

1. **Window and drawing** (`display.rs`): create a surface, upload textures, draw the quads. Quads are in order; each
   has a texture id (`0` = a white texel, `1` = glyph atlas, others via `texture`), a colour multiplier, `nearest`
   filtering, and a scissor rectangle. Premultiplied alpha is not used: blend with source alpha.
2. **Time:** call `run_frame` once per display frame with the real elapsed seconds.
3. **Pointer and touch** (`input.rs`): translate to `Pointer`. A change of `down` is a press or a release.
4. **Gamepad and keyboard:** translate to `Action`; use `scroll` for a stick. Without a pointer the whole game is
   playable through `Action` alone.
5. **Text fields:** the game opens fields (save names, numbers). Show your keyboard in `take_text_request`.
6. **Audio** (`audio.rs`): play the named clip from `<prepared>/audio/`.
7. **Saves:** the engine reads and writes the save folder itself (`SessionConfig::save_dir`). Give it a folder the
   player owns and keep it between runs. Nothing else is needed for the game's own save menu.
8. **Resolution requests:** the game may ask for a size (`Session::take_resolution_request`); honouring it is optional.

## 5. Test it

* `just check`: the contract test runs your loop against `ScriptedEngine`. Add a test for each behaviour you implement.
* The reference host, with the real game: `cargo run --release -p ngu-headless -- --data <prepared> --save <throw-away folder>`
  prints frames, quads, textures and a hash of everything drawn. A script (`--script`) can click and navigate. Two runs of
  the same script give the same hash: compare your port's draw list against it.
* Run your port on the home screen, create a save in a throw-away profile, switch menus, quit, restart and check the save loads.

## 6. Pins, updates and what is public

Pin the engine by **tag and exact revision**: the tag in `Cargo.toml` and `engine.tag`, the revision in `Cargo.lock`. The public surface, the stability promise
(`0.x`: a breaking change is a new minor version with a changelog entry and a migration note) and the C ABI table are in
[ENGINE-API.md](ENGINE-API.md). Everything else is internal and may change. After changing the pin run `cargo update -p clr`
and `just check`, and re-prepare your data.

## 7. Your port is finished when
- [ ] `just check` passes on Linux and Windows CI, with no engine change and no local path in the repository.
- [ ] The home screen, a new game, a save and its reload work on the target, with a throw-away profile.
- [ ] Pointer *or* `Action` input plays the game; text fields can be edited; the glyph atlas is uploaded again when it changes.
- [ ] Textures are uploaded once and released; the draw order and scissor rectangles are respected.
- [ ] `README.md` says what the port is, how to build it from nothing, and states the legal rule (no game files).
- [ ] The engine pin is a tag and a revision, and the port documents how it was tested and what was not.

## Troubleshooting
| message or symptom | meaning and fix |
|---|---|
| `cargo fmt --check` fails in `just check` | format with `cargo fmt`: the format check is part of the port's checks |
| `Assembly-CSharp.dll not found in ...` | the prepared folder lacks the DLL: use `ngu-prepare export` or `ngu-prepare vita` (step 3) |
| `the game scene could not be loaded` | data prepared by another engine revision: prepare again with this revision's tools |
| `the save menu could not start` | the save folder is missing or read-only |
| blank or garbled text | the glyph atlas (texture id 1) was not uploaded again after `atlas_changed()` |
| everything drawn but nothing clickable | pointer coordinates must be device pixels, top-left origin, in the same space as the draw |
| menus drawn beyond their frame | the scissor rectangle (`quad.clip`) is not applied |
| freezes after a long pause | pass the real elapsed time: gaps above 0.25 s are clamped on purpose |
